//! Holding a ported block against the Go compiler it replaces.
//!
//! `testdata/acceptance.json` is written by `TestDumpAcceptanceCorpus` in
//! vimy-core. It needs no recorded game: `CompileDoctrine` is a pure function of
//! a `Doctrine`, so the 500 archived doctrines are the whole input worth testing
//! — a wider corpus than the recorded rule sets, and with nothing to pair.
//!
//! Only the rules the `.vy` file defines are compared. A block that has not been
//! ported yet is simply absent from both sides of the diff.

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

fn block_matches_go(file: &str) {
    let Some(cases) = corpus() else {
        eprintln!("no acceptance corpus; run TestDumpAcceptanceCorpus");
        return;
    };

    let path = format!("{}/rules/{file}", env!("CARGO_MANIFEST_DIR"));
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
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
        ir.rules.iter().map(|r| r.name.clone()).collect()
    };

    let mut differ: Vec<String> = Vec::new();
    let mut compared = 0usize;

    for (i, case) in cases.iter().enumerate() {
        let mut ir = vimyc::check::check(&ast).expect("checks").ir;
        let params = vimyc::ir::ParamValues::bind(&ir, &case.params)
            .unwrap_or_else(|e| panic!("doctrine {i}: {e}"));
        vimyc::specialise::specialise(&mut ir, &params);
        let vimyc::emit::Artifact::Expr(mine) =
            vimyc::emit::emit(&ir, &params, vimyc::emit::Target::Expr);

        let theirs: HashMap<&str, &GoRule> = case
            .rules
            .iter()
            .filter(|r| ported.contains(&r.name))
            .map(|r| (r.name.as_str(), r))
            .collect();

        for r in &mine {
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
