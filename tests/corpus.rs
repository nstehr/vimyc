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
fn seed_type_checks() {
    let ast = parse_seed();
    let diags = vimyc::check::check(&ast);
    assert!(diags.is_empty(), "seed.vy does not type check: {diags:?}");
}

/// The checks that make the language worth having. Each of these is accepted by
/// `expr` today and silently produces a rule that never fires.
#[test]
fn typos_are_caught() {
    fn errors(src: &str) -> Vec<String> {
        let (tokens, lex_diags) = vimyc::lexer::lex(src);
        assert!(lex_diags.is_empty(), "{lex_diags:?}");
        let (ast, parse_diags) = vimyc::parser::parse(&tokens);
        assert!(parse_diags.is_empty(), "{parse_diags:?}");
        vimyc::check::check(&ast)
            .into_iter()
            .map(|d| d.message)
            .collect()
    }

    fn rule_with(require: &str) -> String {
        format!(
            "rule r {{\n  priority 1\n  category economy\n  do scout\n  require {require}\n}}\n"
        )
    }

    // The motivating example: a misspelled role.
    let e = errors(&rule_with("has-role(war-facotry)"));
    assert_eq!(e.len(), 1, "{e:?}");
    assert!(e[0].contains("unknown role"), "{e:?}");
    assert!(e[0].contains("did you mean `war-factory`"), "{e:?}");

    // A real name, but from the wrong domain.
    let e = errors(&rule_with("has-building(e1)"));
    assert_eq!(e.len(), 1, "{e:?}");
    assert!(e[0].contains("unknown building"), "{e:?}");

    // An optional used as a bool — `NearestEnemy() != nil` in expr.
    let e = errors(&rule_with("nearest-enemy"));
    assert_eq!(e.len(), 1, "{e:?}");
    assert!(e[0].contains("optional"), "{e:?}");

    // One mistake, one message: `Type::Error` must not cascade.
    let e = errors(&rule_with("cash >= 300 and has-role(war-facotry)"));
    assert_eq!(e.len(), 1, "a single typo should report once: {e:?}");
}

/// The differential corpus discriminates on every rule.
///
/// A rule that is always true, or always false, across the whole corpus is not
/// being tested by `seed_agrees_with_expr` — the comparison passes for it no
/// matter what the evaluator does. Six of the thirteen were silently in that
/// state until this was checked, so it is worth asserting rather than assuming.
#[test]
fn the_differential_corpus_exercises_every_rule() {
    let corpus = differential_corpus();
    let cases = &corpus.cases;
    let mut counts: std::collections::HashMap<&str, (usize, usize)> =
        std::collections::HashMap::new();

    for case in cases {
        if case.skipped {
            continue;
        }
        let e = counts.entry(case.rule.as_str()).or_default();
        if case.fired { e.0 += 1 } else { e.1 += 1 }
    }

    let mut dead: Vec<String> = counts
        .iter()
        .filter(|(_, (t, f))| *t == 0 || *f == 0)
        .map(|(rule, (t, f))| format!("  {rule}: {t} true, {f} false"))
        .collect();
    dead.sort();

    assert_eq!(counts.len(), 13, "expected all seed rules in the corpus");
    assert!(
        dead.is_empty(),
        "these rules never change value, so the differential test cannot fail on them:\n{}",
        dead.join("\n")
    );
}

/// Every individual `require` varies across the corpus.
///
/// One level below `the_differential_corpus_exercises_every_rule`. A rule can
/// change value while one of its conjuncts never does — and because `rule_fires`
/// short-circuits, a conjunct that is always false hides every conjunct after
/// it. `build-refinery` fires in 11 of 400 states, so it discriminates, but that
/// says nothing about its other three conditions.
#[test]
fn the_differential_corpus_exercises_every_conjunct() {
    let corpus = differential_corpus();
    let cases = &corpus.cases;
    let src = seed_source();
    let ast = parse_seed();

    let mut counts: Vec<Vec<(usize, usize)>> = ast
        .rules
        .iter()
        .map(|r| vec![(0usize, 0usize); r.requires.len()])
        .collect();

    for case in cases {
        if case.skipped {
            continue;
        }
        let Some(ri) = ast.rules.iter().position(|r| r.name.text == case.rule) else {
            continue;
        };
        for (ci, held) in vimyc::eval::conjuncts(&ast.rules[ri], &corpus.states[case.state])
            .iter()
            .enumerate()
        {
            let e = &mut counts[ri][ci];
            if *held { e.0 += 1 } else { e.1 += 1 }
        }
    }

    // At least this share of cases each way. "Varies at all" is too weak — a
    // conjunct true in 1 of 400 states discriminates on paper while leaving a
    // bug in it almost certain to survive.
    const MIN_SHARE: f64 = 0.05;
    let per_rule = cases.len() / ast.rules.len();
    let floor = (per_rule as f64 * MIN_SHARE) as usize;

    let mut thin = Vec::new();
    for (ri, rule) in ast.rules.iter().enumerate() {
        for (ci, (t, f)) in counts[ri].iter().enumerate() {
            if *t <= floor || *f <= floor {
                let span = rule.requires[ci].span;
                let text = &src[span.start as usize..span.end as usize];
                thin.push(format!(
                    "  {}: `{text}` — {t} true, {f} false",
                    rule.name.text
                ));
            }
        }
    }

    let total: usize = counts.iter().map(|r| r.len()).sum();
    assert_eq!(total, 37, "expected the 37 seed conjuncts");
    assert!(
        thin.is_empty(),
        "{} of {total} conjuncts are exercised in under {}% of cases each way, so a bug \
         in them would likely survive:\n{}",
        thin.len(),
        MIN_SHARE * 100.0,
        thin.join("\n")
    );
}

/// One rule's evaluation, with the state as it stood at that moment.
///
/// Per rule rather than per tick: Go mutates `Memory` as its loop runs, so a
/// single per-tick snapshot would make every squad rule disagree for a reason
/// that is not a bug. See `docs/design.md`, "Evaluation semantics".
/// States are stored once and referenced by index — only 400 are distinct, and
/// inlining each into all thirteen of its cases made the file 21MB.
#[derive(Debug, Deserialize)]
struct DifferentialCorpus {
    states: Vec<vimyc::state::State>,
    cases: Vec<DifferentialCase>,
}

#[derive(Debug, Deserialize)]
struct DifferentialCase {
    rule: String,
    state: usize,
    fired: bool,
    /// Blocked by an exclusive rule in the same category, so Go never evaluated
    /// it. Recorded anyway — see `docs/design.md`.
    skipped: bool,
}

fn differential_corpus() -> DifferentialCorpus {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/differential.json");
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{path}: {e}\nregenerate with TestDumpDifferential"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Milestone 3 — the interpreter agrees with expr on every seed condition.
///
/// `testdata/differential.jsonl` is generated by `TestDumpDifferential` in
/// `vimy-core/rules`: Go builds a real `GameState`, evaluates each seed
/// condition through expr, and projects the state down to the flat view this
/// crate understands. Go is the source of truth on both sides of the comparison.
#[test]
fn seed_agrees_with_expr() {
    let corpus = differential_corpus();
    let cases = &corpus.cases;
    let ast = parse_seed();

    let mut checked = 0usize;
    let mut mismatches = Vec::new();

    for (i, case) in cases.iter().enumerate() {
        // Go did not evaluate these, so there is nothing to agree about.
        if case.skipped {
            continue;
        }
        let rule = ast
            .rules
            .iter()
            .find(|r| r.name.text == case.rule)
            .unwrap_or_else(|| panic!("line {}: unknown rule `{}`", i + 1, case.rule));

        let got = vimyc::eval::rule_fires(rule, &corpus.states[case.state]);
        checked += 1;
        if got != case.fired {
            mismatches.push(format!(
                "line {}: `{}` — expr said {}, vimyc said {got}",
                i + 1,
                case.rule,
                case.fired
            ));
        }
    }

    assert!(
        checked > 1000,
        "corpus looks truncated: {checked} comparisons"
    );
    assert!(
        mismatches.is_empty(),
        "{} of {checked} disagreed:\n{}",
        mismatches.len(),
        mismatches
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    eprintln!("{checked} comparisons, no disagreements");
}

/// Every production in `docs/grammar.md` in one file.
///
/// The grammar is hand-written and nothing enforces that it matches the parser,
/// so this at least keeps the documented shapes exercised: if a construct stops
/// parsing, or stops type checking, one of the two has drifted.
#[test]
fn the_documented_grammar_parses_and_checks() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/rules/grammar.vy");
    let src = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));

    let (tokens, ld) = vimyc::lexer::lex(&src);
    assert!(ld.is_empty(), "{ld:?}");
    let (ast, pd) = vimyc::parser::parse(&tokens);
    assert!(pd.is_empty(), "{pd:?}");
    let diags = vimyc::check::check(&ast);
    assert!(diags.is_empty(), "{diags:?}");

    // The shapes the file is there to cover.
    let r = &ast.rules[0];
    assert!(r.exclusive, "category modifier");
    assert!(r.because.is_some(), "because");
    assert_eq!(r.lets.len(), 2, "let bindings");
    assert_eq!(r.action.args.len(), 4, "action arguments");
    assert_eq!(r.requires.len(), 9, "requires");

    let m = &ast.rules[1];
    assert!(!m.exclusive);
    assert!(m.because.is_none());
    assert!(m.lets.is_empty());
    assert!(m.action.args.is_empty(), "action without arguments");
}
