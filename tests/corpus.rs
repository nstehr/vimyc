//! The corpus harness.
//!
//! `testdata/` is generated from Vimy's Go rule compiler, so these are the real
//! conditions the language has to handle — not invented fixtures.
//!
//! The milestone tests below are `#[ignore]`d. Remove the attribute as each
//! stage starts working; that list is the project's progress bar.
//!
//! Scope is the 13 seed rules, end to end. The wider corpus that
//! `CompileDoctrine` emits comes later, once those work.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Rule {
    pub name: String,
    pub priority: i64,
    pub category: String,
    pub exclusive: bool,
    pub condition: String,
}

fn load(file: &str) -> Vec<Rule> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/");
    let text = std::fs::read_to_string(format!("{path}{file}"))
        .unwrap_or_else(|e| panic!("read {file}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {file}: {e}"))
}

pub fn seed() -> Vec<Rule> {
    load("seed_rules.json")
}

#[test]
fn corpus_loads() {
    let seed = seed();
    assert_eq!(seed.len(), 13, "seed corpus is DefaultRules()");
    assert!(seed.iter().all(|r| !r.condition.is_empty()));
}

/// Milestone 1 — every seed condition lexes and parses.
#[test]
#[ignore = "milestone 1"]
fn seed_parses() {
    // for r in seed() {
    //     vimyc::parser::parse(&r.condition).unwrap();
    // }
    unimplemented!("wire up the parser");
}

/// Milestone 2 — every seed condition type checks as a bool.
#[test]
#[ignore = "milestone 2"]
fn seed_type_checks() {
    unimplemented!("wire up the type checker");
}

/// Milestone 3 — the interpreter agrees with expr on every seed condition.
///
/// Needs expected values from the Go side: evaluate each condition against each
/// mocked state with expr, dump `(state, condition, expected)`, compare here.
#[test]
#[ignore = "milestone 3"]
fn seed_agrees_with_expr() {
    unimplemented!("wire up the evaluator + expected values");
}
