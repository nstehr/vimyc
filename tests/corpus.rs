//! The corpus harness.
//!
//! `testdata/` is generated from Vimy's Go rule compiler, so these are the
//! conditions the language actually has to handle rather than fixtures I made
//! up.
//!
//! The milestone tests below are `#[ignore]`d; dropping the attribute as each
//! stage lands is the project's progress bar.
//!
//! Scope is the 13 seed rules, end to end. The wider corpus `CompileDoctrine`
//! emits comes later.

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

/// Reads `rules/seed.vy` — the hand translation of `DefaultRules()`.
fn seed_source() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/rules/seed.vy");
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read seed.vy: {e}"))
}

fn parse_seed() -> vimyc::ast::Ast {
    let src = seed_source();
    let (tokens, lex_diags) = vimyc::lexer::lex(&src);
    assert!(lex_diags.is_empty(), "seed.vy does not lex: {lex_diags:?}");
    let (ast, parse_diags) = vimyc::parser::parse(&tokens);
    assert!(
        parse_diags.is_empty(),
        "seed.vy does not parse: {parse_diags:?}"
    );
    ast
}

/// Splits an expr condition on top-level `&&`, ignoring those inside parens.
/// One conjunct in the Go source should be one `require` in the translation.
fn conjuncts(cond: &str) -> usize {
    let (mut depth, mut n) = (0i32, 1usize);
    let b = cond.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'&' if depth == 0 && b.get(i + 1) == Some(&b'&') => {
                n += 1;
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    n
}

/// Milestone 1 — the seed rules lex and parse.
#[test]
fn seed_parses() {
    let ast = parse_seed();
    assert_eq!(ast.rules.len(), 13);
}

/// The translation still matches the Go source it came from.
///
/// `seed.vy` is hand-written, so nothing else stops it drifting from
/// `DefaultRules()`. The conditions can't be compared directly since the syntax
/// differs, but one top-level `&&` conjunct must equal one `require` line.
#[test]
fn seed_matches_the_go_corpus() {
    let ast = parse_seed();
    let expected = seed();

    assert_eq!(ast.rules.len(), expected.len(), "rule count");

    for want in &expected {
        let got = ast
            .rules
            .iter()
            .find(|r| r.name.text == want.name)
            .unwrap_or_else(|| panic!("`{}` missing from seed.vy", want.name));

        assert_eq!(got.priority, want.priority, "{}: priority", want.name);
        assert_eq!(got.category.text, want.category, "{}: category", want.name);
        assert_eq!(got.exclusive, want.exclusive, "{}: exclusive", want.name);
        assert_eq!(
            got.requires.len(),
            conjuncts(&want.condition),
            "{}: one `require` per top-level conjunct of `{}`",
            want.name,
            want.condition
        );
    }
}

/// Rules appear in the same order as the Go source, which is priority order.
#[test]
fn seed_preserves_rule_order() {
    let ast = parse_seed();
    let got: Vec<&str> = ast.rules.iter().map(|r| r.name.text.as_str()).collect();
    let want: Vec<String> = seed().into_iter().map(|r| r.name).collect();
    assert_eq!(got, want);
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
