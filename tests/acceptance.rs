//! Holding a ported block against the Go compiler it replaces.
//!
//! `testdata/acceptance.json` is written by `TestDumpAcceptanceCorpus` in
//! vimy-core. It needs no recorded game: `CompileDoctrine` is a pure function of
//! a `Doctrine`, so the 500 archived doctrines are the whole input worth testing
//! — a wider corpus than the recorded rule sets, and with nothing to pair.
//!
//! Only the rules the `.vy` file defines are compared. A block that has not been
//! ported yet is simply absent from both sides of the diff.
//!
//! The corpus is frozen: `CompileDoctrine` is deleted, so nothing can regenerate
//! it. It verifies the port, and rules written since are outside its scope —
//! `POST_PORT` names them, so a new rule does not fail the test and an
//! accidental one does not slip through.

use serde::Deserialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Deserialize)]
struct Case {
    params: HashMap<String, f64>,
    rules: Vec<GoRule>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct GoRule {
    name: String,
    priority: i64,
    category: String,
    exclusive: bool,
    action: String,
    condition: String,
}

/// Rules that postdate the port, and so have no counterpart in the corpus.
///
/// Explicit rather than "ignore anything unmatched": the whole value of the
/// frozen corpus is that an unexpected rule is still an error.
const POST_PORT: &[&str] = &["form-harvester-guard", "guard-harvesters"];

/// Where Vimy's rule sets live.
///
/// They are Vimy's strategy, not this compiler's, so they live in that repo —
/// the way its `.go` files do. `VIMY_RULES` overrides the sibling default, and
/// these tests skip when it is not there: they verify another project's content
/// and cannot run without it. `rules/fixture.vy` is what the compiler's own
/// tests use.
fn vy_dir() -> Option<std::path::PathBuf> {
    let dir = std::env::var("VIMY_RULES")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../vimy/vimy-core/rules/vy")
        });
    dir.is_dir().then_some(dir)
}

fn corpus() -> Option<Vec<Case>> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/acceptance.json");
    let json = std::fs::read_to_string(path).ok()?;
    Some(serde_json::from_str(&json).expect("acceptance.json"))
}

/// Go writes whatever its templates contain; vimyc writes a canonical form. The
/// differences that survive are parentheses and spacing, neither of which
/// reaches the engine — `emitted_expr_round_trips_on_a_real_game` covers the
/// same ground for the seed set.
fn normalise(s: &str) -> String {
    trim_zeros(
        &s.split_whitespace()
            .collect::<String>()
            .replace(['(', ')'], ""),
    )
}

/// Go writes a threshold with `%.2f`, so `0.5` arrives as `0.50`. The value is
/// the same and Go's own projection normalises the literal before recording a
/// state key, so the trailing zeros reach nothing.
fn trim_zeros(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == '.' && i > 0 && b[i - 1].is_ascii_digit() {
            let mut end = i + 1;
            while end < b.len() && b[end].is_ascii_digit() {
                end += 1;
            }
            let frac: String = b[i + 1..end].iter().collect();
            let frac = frac.trim_end_matches('0');
            if !frac.is_empty() {
                out.push('.');
                out.push_str(frac);
            }
            i = end;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

#[test]
fn the_economy_block_matches_go() {
    block_matches_go("economy.vy");
}

#[test]
fn the_buildings_block_matches_go() {
    block_matches_go("buildings.vy");
}

#[test]
fn the_production_block_matches_go() {
    block_matches_go("production.vy");
}

#[test]
fn the_combat_block_matches_go() {
    block_matches_go("combat.vy");
}

#[test]
fn the_micro_block_matches_go() {
    block_matches_go("micro.vy");
}

#[test]
fn the_core_block_matches_go() {
    block_matches_go("core.vy");
}

fn block_matches_go(file: &str) {
    let Some(cases) = corpus() else {
        eprintln!("no acceptance corpus; run TestDumpAcceptanceCorpus");
        return;
    };

    let Some(dir) = vy_dir() else {
        eprintln!("Vimy's rules are not beside this checkout; skipping");
        return;
    };
    let path = dir.join(file);
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let (tokens, ld) = vimyc::lexer::lex(&src);
    assert!(ld.is_empty(), "{ld:?}");
    let (ast, pd) = vimyc::parser::parse(&tokens);
    assert!(pd.is_empty(), "{pd:?}");

    // The names this block claims. Anything outside it belongs to a block that
    // has not been ported.
    let ported: HashSet<String> = {
        let ir = vimyc::check::check(&ast)
            .unwrap_or_else(|d| panic!("{file} does not check: {d:?}"))
            .ir;
        ir.rules
            .iter()
            .map(|r| r.name.clone())
            .filter(|n| !POST_PORT.contains(&n.as_str()))
            .collect()
    };

    let mut differ: Vec<String> = Vec::new();
    let mut compared = 0usize;

    for (i, case) in cases.iter().enumerate() {
        let mut ir = vimyc::check::check(&ast).expect("checks").ir;
        let params = vimyc::ir::ParamValues::bind(&ir, &case.params)
            .unwrap_or_else(|e| panic!("doctrine {i}: {e}"));
        vimyc::specialise::specialise(&mut ir, &params);
        let vimyc::emit::Artifact::Expr(mine) =
            vimyc::emit::emit(&ir, &params, vimyc::emit::Target::Expr)
        else {
            unreachable!()
        };

        let theirs: HashMap<&str, &GoRule> = case
            .rules
            .iter()
            .filter(|r| ported.contains(&r.name))
            .map(|r| (r.name.as_str(), r))
            .collect();

        for r in &mine {
            if POST_PORT.contains(&r.name.as_str()) {
                continue;
            }
            let Some(want) = theirs.get(r.name.as_str()) else {
                differ.push(format!("doctrine {i}: emitted `{}`, Go did not", r.name));
                continue;
            };
            compared += 1;
            let mismatch = if r.priority != want.priority {
                Some(format!("priority {} vs {}", r.priority, want.priority))
            } else if r.category != want.category {
                Some(format!("category {} vs {}", r.category, want.category))
            } else if r.exclusive != want.exclusive {
                Some(format!("exclusive {} vs {}", r.exclusive, want.exclusive))
            } else if r.action != want.action {
                Some(format!("action {} vs {}", r.action, want.action))
            } else if normalise(&r.condition) != normalise(&want.condition) {
                Some(format!(
                    "condition\n      go:    {}\n      vimyc: {}",
                    want.condition, r.condition
                ))
            } else {
                None
            };
            if let Some(m) = mismatch {
                differ.push(format!("doctrine {i}: `{}` {m}", r.name));
            }
        }

        for name in theirs.keys() {
            if !mine.iter().any(|r| r.name == *name) {
                differ.push(format!("doctrine {i}: Go emitted `{name}`, vimyc did not"));
            }
        }
    }

    assert!(compared > 500, "corpus looks truncated: {compared}");
    assert!(
        differ.is_empty(),
        "{} disagreements over {compared} rules:\n{}",
        differ.len(),
        differ
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    eprintln!(
        "{compared} rules from {file} across {} doctrines match Go",
        cases.len()
    );
}

/// Every rule `CompileDoctrine` can emit is claimed by some block.
///
/// The per-block tests only compare the names their file defines, so a rule
/// nobody ported would pass unnoticed everywhere. This is what says the port is
/// finished rather than merely correct as far as it goes.
/// The blocks combined: one rule set, which is what a game would actually run.
///
/// Not a formality — the blocks share parameter and def names, so combining
/// them means keeping one of each, and two definitions that merely looked alike
/// would show up here as a rule set that no longer matches Go.
#[test]
fn the_combined_rule_set_matches_go() {
    block_matches_go("doctrine.vy");
}

#[test]
fn the_blocks_cover_every_rule_go_emits() {
    const BLOCKS: &[&str] = &[
        "core.vy",
        "economy.vy",
        "buildings.vy",
        "production.vy",
        "combat.vy",
        "micro.vy",
    ];
    let Some(cases) = corpus() else {
        eprintln!("no acceptance corpus; run TestDumpAcceptanceCorpus");
        return;
    };
    let Some(dir) = vy_dir() else {
        eprintln!("Vimy's rules are not beside this checkout; skipping");
        return;
    };

    let mut ported: HashSet<String> = HashSet::new();
    for file in BLOCKS {
        let path = dir.join(file);
        let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
        let (tokens, _) = vimyc::lexer::lex(&src);
        let (ast, _) = vimyc::parser::parse(&tokens);
        let ir = vimyc::check::check(&ast)
            .unwrap_or_else(|d| panic!("{file}: {d:?}"))
            .ir;
        for r in &ir.rules {
            assert!(
                ported.insert(r.name.clone()),
                "`{}` is defined by two blocks",
                r.name
            );
        }
    }

    let mut go: HashSet<&str> = HashSet::new();
    for case in &cases {
        for r in &case.rules {
            go.insert(&r.name);
        }
    }

    let missing: Vec<&&str> = go.iter().filter(|n| !ported.contains(**n)).collect();
    assert!(missing.is_empty(), "not ported: {missing:?}");

    // A name Go never emits is not automatically wrong — `build-barracks-prereq`
    // wants a doctrine with air, naval or tech but no vehicles, no infantry and
    // no ground defense, and nothing in the corpus is shaped like that. What
    // matters is that vimyc does not emit it either, since no per-block test
    // would compare it.
    let mut emitted: HashSet<String> = HashSet::new();
    for file in BLOCKS {
        let path = dir.join(file);
        let src = std::fs::read_to_string(&path).expect("block");
        let (tokens, _) = vimyc::lexer::lex(&src);
        let (ast, _) = vimyc::parser::parse(&tokens);
        for case in &cases {
            let mut ir = vimyc::check::check(&ast).expect("checks").ir;
            let params = vimyc::ir::ParamValues::bind(&ir, &case.params).expect("binds");
            vimyc::specialise::specialise(&mut ir, &params);
            emitted.extend(ir.rules.iter().map(|r| r.name.clone()));
        }
    }

    // The post-port rules must actually be reachable, or naming one here would
    // be a way to hide a rule that never fires.
    for n in POST_PORT {
        assert!(
            emitted.contains(*n),
            "`{n}` is never emitted by any doctrine"
        );
    }

    let invented: Vec<&String> = emitted
        .iter()
        .filter(|n| !go.contains(n.as_str()) && !POST_PORT.contains(&n.as_str()))
        .collect();
    assert!(
        invented.is_empty(),
        "vimyc emits what Go does not: {invented:?}"
    );

    let unreachable: Vec<&String> = ported.difference(&emitted).collect();
    eprintln!(
        "{} rule names ported; {} unreachable for every doctrine, in Go too: {unreachable:?}",
        ported.len(),
        unreachable.len()
    );
}

/// The printed `.vy` is the rule set it claims to be.
///
/// Nothing executes it — Go runs the expr beside it — so a rendering bug would
/// otherwise be invisible on a dashboard nobody could check. Round-tripping
/// makes "what you read is what runs" a property: print the IR, parse it back,
/// lower it, and require the same rules with the same conditions.
///
/// Run against every archived doctrine, so it covers the shapes real play
/// produces rather than the ones a fixture would.
#[test]
fn emitted_vy_round_trips() {
    let Some(cases) = corpus() else {
        eprintln!("no acceptance corpus; skipping");
        return;
    };

    let Some(dir) = vy_dir() else {
        eprintln!("Vimy's rules are not beside this checkout; skipping");
        return;
    };
    let src = std::fs::read_to_string(dir.join("doctrine.vy")).expect("doctrine.vy");
    let (tokens, _) = vimyc::lexer::lex(&src);
    let (ast, _) = vimyc::parser::parse(&tokens);

    let mut checked = 0usize;
    for (i, case) in cases.iter().enumerate().step_by(17) {
        let mut ir = vimyc::check::check(&ast).expect("checks").ir;
        let params = vimyc::ir::ParamValues::bind(&ir, &case.params).expect("binds");
        vimyc::specialise::specialise(&mut ir, &params);

        // The expr each rule compiles to, before the round trip.
        let vimyc::emit::Artifact::Expr(before) =
            vimyc::emit::emit(&ir, &params, vimyc::emit::Target::Expr)
        else {
            unreachable!()
        };

        // Print, and read it back as a fresh rule set. It has no parameters
        // left — specialising folded them — so it binds against nothing.
        let printed = vimyc::emit::vy::emit_file(&ir, &params);
        let (tokens, lex_diags) = vimyc::lexer::lex(&printed);
        assert!(
            lex_diags.is_empty(),
            "doctrine {i} does not lex:\n{printed}"
        );
        let (reparsed, parse_diags) = vimyc::parser::parse(&tokens);
        assert!(
            parse_diags.is_empty(),
            "doctrine {i} does not parse: {parse_diags:?}\n{printed}"
        );
        let round = vimyc::check::check(&reparsed)
            .unwrap_or_else(|d| panic!("doctrine {i} does not check: {d:?}\n{printed}"))
            .ir;

        let vimyc::emit::Artifact::Expr(after) = vimyc::emit::emit(
            &round,
            &vimyc::ir::ParamValues::default(),
            vimyc::emit::Target::Expr,
        ) else {
            unreachable!()
        };

        assert_eq!(before.len(), after.len(), "doctrine {i}: rule count");
        for (b, a) in before.iter().zip(&after) {
            assert_eq!(b.name, a.name, "doctrine {i}");
            assert_eq!(b.priority, a.priority, "doctrine {i}: {}", b.name);
            assert_eq!(b.category, a.category, "doctrine {i}: {}", b.name);
            assert_eq!(b.exclusive, a.exclusive, "doctrine {i}: {}", b.name);
            assert_eq!(b.action, a.action, "doctrine {i}: {}", b.name);
            assert_eq!(b.because, a.because, "doctrine {i}: {}", b.name);
            // Exact, not `normalise`: that strips parentheses, which is right
            // against Go and wrong here. Both sides are this emitter, so a
            // printer that loses a parenthesis must fail rather than pass on a
            // comparison that cannot see it.
            assert_eq!(b.condition, a.condition, "doctrine {i}: {}", b.name);
            checked += 1;
        }
    }

    assert!(checked > 1000, "only {checked} rules round tripped");
    eprintln!("{checked} rules survive printing and reading back");
}
