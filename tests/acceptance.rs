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

/// Rules whose priority was deliberately changed after the port, and the reason.
///
/// Every one of these is a tie broken on purpose: Go gave two rules in one
/// exclusive category the same priority, so which of them got the queue was
/// decided by an unstable sort. Only the priority is exempted — the condition,
/// category, action and exclusivity are still held against the corpus, so this
/// cannot quietly hide a rule that drifted in some other way.
const RETUNED: &[(&str, &str)] = &[
    (
        "build-aa-defense",
        "below base defense when the doctrine weights air and ground equally",
    ),
    (
        "build-extra-refinery",
        "above the tech centre when economy and tech are weighted equally",
    ),
    (
        "build-gap-generator",
        "below both other defenses; it is the least urgent of the three",
    ),
    (
        "build-naval-yard",
        "below the airfield when air and naval are weighted equally",
    ),
    (
        "build-second-refinery",
        "above the tech centre when economy and tech are weighted equally",
    ),
    (
        "build-service-depot",
        "below the radar, which is the tech gate",
    ),
    (
        "build-tesla-coil-for-shock-trooper",
        "below the flame tower, the cheaper unlock",
    ),
    (
        "defend-base",
        "above the scramble it duplicates, being the more specific rule",
    ),
    (
        "produce-apc",
        "above the flak truck; an engineer is already built and waiting",
    ),
    (
        "produce-assault-apc",
        "above the vehicle rules, being capped and doctrine-opted-in",
    ),
    (
        "produce-attack-dog",
        "below specialist infantry when the doctrine puts specialists first",
    ),
    (
        "produce-bridge-infantry",
        "below the rocket soldier; rifle top-ups are the more disposable",
    ),
    (
        "produce-spy",
        "below capture-defense rifles, which are cheaper and defensive",
    ),
    (
        "recall-overextended-naval-attack",
        "below its ground mirror",
    ),
    (
        "rebuild-naval-yard",
        "below the airfield, which is useful on every map",
    ),
    ("squad-disengage-naval-attack", "below its ground mirror"),
    (
        "squad-focus-fire",
        "capped below the retreat band, so retreating outranks it",
    ),
];

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

/// The files of Vimy's rule set, in the order vimyc would read the directory.
const BLOCKS: &[&str] = &[
    "buildings.vy",
    "combat.vy",
    "core.vy",
    "economy.vy",
    "micro.vy",
    "production.vy",
];

/// Vimy's rule set, parsed as the one unit a game compiles.
///
/// No block stands alone — `core.vy` holds the defs the others call — so there
/// is nothing to read but the whole set.
fn rule_set(dir: &std::path::Path) -> vimyc::unit::Unit {
    let paths: Vec<_> = BLOCKS.iter().map(|f| dir.join(f)).collect();
    let unit = vimyc::unit::Unit::read(&paths).unwrap_or_else(|e| panic!("{e}"));
    assert!(unit.diags.is_empty(), "{:?}", unit.diags);
    unit
}

/// The rules one block declares, by name.
///
/// Parsed rather than checked: a block on its own does not type-check now that
/// the defs live in `core.vy`, and the names are all this needs.
fn rules_in(dir: &std::path::Path, file: &str) -> HashSet<String> {
    let path = dir.join(file);
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let (tokens, ld) = vimyc::lexer::lex(&src);
    assert!(ld.is_empty(), "{file}: {ld:?}");
    let (ast, pd) = vimyc::parser::parse(&tokens);
    assert!(pd.is_empty(), "{file}: {pd:?}");
    ast.rules.into_iter().map(|r| r.name.text).collect()
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
    block_matches_go(Some("economy.vy"));
}

#[test]
fn the_buildings_block_matches_go() {
    block_matches_go(Some("buildings.vy"));
}

#[test]
fn the_production_block_matches_go() {
    block_matches_go(Some("production.vy"));
}

#[test]
fn the_combat_block_matches_go() {
    block_matches_go(Some("combat.vy"));
}

#[test]
fn the_micro_block_matches_go() {
    block_matches_go(Some("micro.vy"));
}

#[test]
fn the_core_block_matches_go() {
    block_matches_go(Some("core.vy"));
}

/// Compares one block's rules against Go, or the whole set when `file` is
/// `None`.
///
/// The unit is always the whole rule set — a block does not compile alone — and
/// `file` only narrows which rules are compared.
fn block_matches_go(file: Option<&str>) {
    let Some(cases) = corpus() else {
        eprintln!("no acceptance corpus; run TestDumpAcceptanceCorpus");
        return;
    };

    let Some(dir) = vy_dir() else {
        eprintln!("Vimy's rules are not beside this checkout; skipping");
        return;
    };
    let label = file.unwrap_or("the rule set");
    let ast = rule_set(&dir).ast;

    // The names being compared. Anything outside them belongs to a block that
    // has not been ported.
    let ported: HashSet<String> = {
        let ir = vimyc::check::check(&ast)
            .unwrap_or_else(|d| panic!("{label} does not check: {d:?}"))
            .ir;
        let own = file.map(|f| rules_in(&dir, f));
        ir.rules
            .iter()
            .map(|r| r.name.clone())
            .filter(|n| own.as_ref().is_none_or(|o| o.contains(n)))
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
            // The unit emits every block's rules; `ported` is the subset under
            // comparison, and already has the post-port additions removed.
            if !ported.contains(&r.name) {
                continue;
            }
            let Some(want) = theirs.get(r.name.as_str()) else {
                differ.push(format!("doctrine {i}: emitted `{}`, Go did not", r.name));
                continue;
            };
            compared += 1;
            let retuned = RETUNED.iter().any(|(n, _)| *n == r.name);
            let mismatch = if r.priority != want.priority && !retuned {
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
        "{compared} rules from {label} across {} doctrines match Go",
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
    block_matches_go(None);
}

/// Every `RETUNED` entry names a real rule that really does differ.
///
/// Without this the list only ever grows: an entry for a rule that was deleted,
/// or one whose priority was later put back, would sit there silently widening
/// the hole in the comparison.
#[test]
fn every_retuned_rule_exists_and_still_differs() {
    let Some(cases) = corpus() else {
        eprintln!("no acceptance corpus; run TestDumpAcceptanceCorpus");
        return;
    };
    let Some(dir) = vy_dir() else {
        eprintln!("Vimy's rules are not beside this checkout; skipping");
        return;
    };
    let ast = rule_set(&dir).ast;

    let names: HashSet<&str> = ast.rules.iter().map(|r| r.name.text.as_str()).collect();
    for (n, _) in RETUNED {
        assert!(names.contains(n), "`{n}` is retuned but no longer exists");
    }

    // A rule is only exempt if some doctrine actually gives it a different
    // priority from the one Go recorded.
    let mut differs: HashSet<&str> = HashSet::new();
    for case in cases.iter().step_by(7) {
        let mut ir = vimyc::check::check(&ast).expect("checks").ir;
        let params = vimyc::ir::ParamValues::bind(&ir, &case.params).expect("binds");
        vimyc::specialise::specialise(&mut ir, &params);
        let vimyc::emit::Artifact::Expr(mine) =
            vimyc::emit::emit(&ir, &params, vimyc::emit::Target::Expr)
        else {
            unreachable!()
        };
        for r in &mine {
            if let Some(want) = case.rules.iter().find(|g| g.name == r.name)
                && r.priority != want.priority
                && let Some((n, _)) = RETUNED.iter().find(|(n, _)| *n == r.name)
            {
                differs.insert(n);
            }
        }
    }
    let stale: Vec<&str> = RETUNED
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| !differs.contains(n))
        .collect();
    assert!(
        stale.is_empty(),
        "retuned but matching Go on every doctrine, so the entry is dead: {stale:?}"
    );
}

#[test]
fn the_blocks_cover_every_rule_go_emits() {
    let Some(cases) = corpus() else {
        eprintln!("no acceptance corpus; run TestDumpAcceptanceCorpus");
        return;
    };
    let Some(dir) = vy_dir() else {
        eprintln!("Vimy's rules are not beside this checkout; skipping");
        return;
    };

    // Per block rather than from the unit, so this still catches a rule that
    // two blocks both define — which the unit would happily accept as two
    // rules with the same name.
    let mut ported: HashSet<String> = HashSet::new();
    for file in BLOCKS {
        for name in rules_in(&dir, file) {
            assert!(
                ported.insert(name.clone()),
                "`{name}` is defined by two blocks"
            );
        }
    }
    // Every block belongs to the set that actually compiles.
    let compiled: HashSet<String> = rule_set(&dir)
        .ast
        .rules
        .iter()
        .map(|r| r.name.text.clone())
        .collect();
    assert_eq!(ported, compiled, "a block is missing from BLOCKS");

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
    let ast = rule_set(&dir).ast;
    for case in &cases {
        let mut ir = vimyc::check::check(&ast).expect("checks").ir;
        let params = vimyc::ir::ParamValues::bind(&ir, &case.params).expect("binds");
        vimyc::specialise::specialise(&mut ir, &params);
        emitted.extend(ir.rules.iter().map(|r| r.name.clone()));
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
    let ast = rule_set(&dir).ast;

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
