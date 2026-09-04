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
use vimyc::ir::ParamValues;

/// No rule set carries parameters yet, so every call site passes an empty set.
const NO_PARAMS: ParamValues = ParamValues { values: Vec::new() };

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

/// seed.vy sets every priority to a plain number; a doctrine-derived one would
/// not be comparable here at all.
fn literal_priority(e: &vimyc::ast::Expr) -> i64 {
    match e.kind {
        vimyc::ast::ExprKind::Int(n) => n,
        _ => panic!("seed.vy has a priority that is not a literal"),
    }
}

/// Checks and lowers, which is the only route to an `Ir`. Panics on an error
/// and ignores warnings — the seed set has none, and `seed_type_checks` is
/// where that is asserted.
fn lower_checked(ast: &vimyc::ast::Ast) -> vimyc::ir::Ir {
    match vimyc::check::check(ast) {
        Ok(c) => c.ir,
        Err(diags) => panic!("does not check: {diags:?}"),
    }
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

        assert_eq!(
            literal_priority(&got.priority),
            want.priority,
            "{}: priority",
            want.name
        );
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
    let checked = vimyc::check::check(&ast).expect("seed.vy does not type check");
    assert!(
        checked.warnings.is_empty(),
        "seed.vy warns: {:?}",
        checked.warnings
    );
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
        match vimyc::check::check(&ast) {
            Ok(c) => c.warnings.into_iter().map(|d| d.message).collect(),
            Err(diags) => diags.into_iter().map(|d| d.message).collect(),
        }
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
    let ir = lower_checked(&parse_seed());

    let mut counts: Vec<Vec<(usize, usize)>> = ir
        .rules
        .iter()
        .map(|r| vec![(0usize, 0usize); r.requires.len()])
        .collect();

    for case in cases {
        if case.skipped {
            continue;
        }
        let Some(ri) = ir.rules.iter().position(|r| r.name == case.rule) else {
            continue;
        };
        for (ci, held) in
            vimyc::eval::conjuncts(&ir.rules[ri], &NO_PARAMS, &corpus.states[case.state])
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
    let per_rule = cases.len() / ir.rules.len();
    let floor = (per_rule as f64 * MIN_SHARE) as usize;

    let mut thin = Vec::new();
    for (ri, rule) in ir.rules.iter().enumerate() {
        for (ci, (t, f)) in counts[ri].iter().enumerate() {
            if *t <= floor || *f <= floor {
                let span = rule.requires[ci].span;
                let text = &src[span.start as usize..span.end as usize];
                thin.push(format!("  {}: `{text}` — {t} true, {f} false", rule.name));
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
    let ir = lower_checked(&parse_seed());

    let mut checked = 0usize;
    let mut mismatches = Vec::new();

    for (i, case) in cases.iter().enumerate() {
        // Go did not evaluate these, so there is nothing to agree about.
        if case.skipped {
            continue;
        }
        let rule = ir
            .rules
            .iter()
            .find(|r| r.name == case.rule)
            .unwrap_or_else(|| panic!("line {}: unknown rule `{}`", i + 1, case.rule));

        let got = vimyc::eval::rule_fires(rule, &NO_PARAMS, &corpus.states[case.state]);
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
    let checked = vimyc::check::check(&ast).expect("grammar.vy does not check");
    assert!(checked.warnings.is_empty(), "{:?}", checked.warnings);

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

/// Lowering resolves every name in the seed rules.
///
/// It panics rather than reporting — an unresolvable name means `check` accepted
/// something it should not have — so simply running it is the assertion.
#[test]
fn seed_lowers() {
    let ir = lower_checked(&parse_seed());
    assert_eq!(ir.rules.len(), 13);

    // `count(powr)` and `count(idle-ground-units)` are different predicates
    // after lowering; nothing downstream should see a `count` node.
    let names: Vec<&str> = ir.rules.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"build-power"));
}

/// Every rule set a real game ran lowers.
#[test]
fn real_rule_sets_lower() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/testdata/real_differential.json"
    );
    let Ok(text) = std::fs::read_to_string(path) else {
        eprintln!("no recorded game; skipping");
        return;
    };
    #[derive(serde::Deserialize)]
    struct Corpus {
        rule_sets: Vec<String>,
    }
    let c: Corpus = serde_json::from_str(&text).expect("corpus");

    for (i, src) in c.rule_sets.iter().enumerate() {
        let (tokens, ld) = vimyc::lexer::lex(src);
        assert!(ld.is_empty(), "rule set {i}: {ld:?}");
        let (ast, pd) = vimyc::parser::parse(&tokens);
        assert!(pd.is_empty(), "rule set {i}: {pd:?}");
        let ir = lower_checked(&ast);
        assert_eq!(ir.rules.len(), ast.rules.len(), "rule set {i}");
    }
    eprintln!("{} real rule sets lower", c.rule_sets.len());
}

/// expr emitted from `seed.vy` matches what Go compiled it from.
///
/// The strongest check available without leaving the crate: `rules/seed.vy` was
/// hand-translated from `DefaultRules()`, so emitting it should land back on the
/// original conditions. Anything else means the translation or the emitter lost
/// something.
///
/// Compared with whitespace normalised, since the two were written by different
/// hands and spacing is not meaning.
#[test]
fn emitted_expr_round_trips_to_go() {
    let ir = lower_checked(&parse_seed());
    let vimyc::emit::Artifact::Expr(emitted) =
        vimyc::emit::emit(&ir, &NO_PARAMS, vimyc::emit::Target::Expr);

    let expected = seed();
    let squeeze = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");

    let mut differ = Vec::new();
    for want in &expected {
        let got = emitted
            .iter()
            .find(|r| r.name == want.name)
            .unwrap_or_else(|| panic!("`{}` was not emitted", want.name));

        if squeeze(&got.condition) != squeeze(&want.condition) {
            differ.push(format!(
                "  {}\n    go:    {}\n    vimyc: {}",
                want.name, want.condition, got.condition
            ));
        }
        assert_eq!(got.priority, want.priority, "{}: priority", want.name);
        assert_eq!(got.category, want.category, "{}: category", want.name);
        assert_eq!(got.exclusive, want.exclusive, "{}: exclusive", want.name);
    }

    assert!(
        differ.is_empty(),
        "{} of {} conditions did not round trip:\n{}",
        differ.len(),
        expected.len(),
        differ.join("\n")
    );
    eprintln!(
        "{} conditions round trip to Go's expr exactly",
        expected.len()
    );
}

/// Emits one condition, so parenthesisation can be asserted exactly.
fn emit_one(requires: &str) -> String {
    let src = format!(
        "rule probe {{\n  priority 1\n  category production\n  require {requires}\n  do air-defend-base()\n}}\n"
    );
    let (tokens, lex_diags) = vimyc::lexer::lex(&src);
    assert!(lex_diags.is_empty(), "{requires}: {lex_diags:?}");
    let (ast, parse_diags) = vimyc::parser::parse(&tokens);
    assert!(parse_diags.is_empty(), "{requires}: {parse_diags:?}");
    let ir = match vimyc::check::check(&ast) {
        Ok(c) => c.ir,
        Err(diags) => panic!("{requires}: {diags:?}"),
    };
    let vimyc::emit::Artifact::Expr(rules) =
        vimyc::emit::emit(&ir, &NO_PARAMS, vimyc::emit::Target::Expr);
    rules[0].condition.clone()
}

/// The round trip against a real game normalises parentheses away, since Go
/// emits whatever its templates contain. That leaves precedence unchecked for
/// any rule the recorded states never make true — which is most of them. These
/// pin it directly: each case is one where dropping a paren changes the meaning.
#[test]
fn emits_the_parentheses_precedence_needs() {
    let cases = [
        // `||` under `&&` keeps its group; `&&` under `||` does not gain one.
        (
            "has-role(barracks) and (has-role(radar) or is-rushed())",
            r#"HasRole("barracks") && (HasRole("radar") || IsRushed())"#,
        ),
        // A require is an operand of the `&&` that joins the others, so a
        // top-level `||` is grouped even when it is the only require.
        (
            "has-role(barracks) or has-role(radar) and is-rushed()",
            r#"(HasRole("barracks") || HasRole("radar") && IsRushed())"#,
        ),
        // Arithmetic below a comparison never needs one; the reverse always does.
        (
            "count(powr) + count(proc) < 7",
            r#"BuildingCount("powr") + BuildingCount("proc") < 7"#,
        ),
        (
            "cash() * (count(powr) + 1) > 100",
            r#"Cash() * (BuildingCount("powr") + 1) > 100"#,
        ),
        // Subtraction is not associative, so a right operand keeps its group.
        (
            "cash() - (count(powr) - 1) > 0",
            r#"Cash() - (BuildingCount("powr") - 1) > 0"#,
        ),
        // `not` binds tighter than everything below it.
        (
            "not (has-role(barracks) and has-role(radar))",
            r#"!(HasRole("barracks") && HasRole("radar"))"#,
        ),
    ];
    for (src, want) in cases {
        assert_eq!(emit_one(src), want, "emitting `{src}`");
    }
}

/// A doctrine's numbers, through the whole pipeline.
///
/// `lerp` has to agree with Go's `doctrine.go` exactly — it rounds rather than
/// truncating, and a priority off by one reorders a category.
#[test]
fn parameters_fold_through_to_expr() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/rules/params.vy"))
        .expect("params.vy");
    let (tokens, ld) = vimyc::lexer::lex(&src);
    assert!(ld.is_empty(), "{ld:?}");
    let (ast, pd) = vimyc::parser::parse(&tokens);
    assert!(pd.is_empty(), "{pd:?}");
    let ir = vimyc::check::check(&ast).expect("params.vy checks").ir;

    let doctrine = std::collections::HashMap::from([
        ("aggression".to_string(), 0.7),
        ("naval-weight".to_string(), 0.4),
        ("ground-attack-group-size".to_string(), 8.0),
    ]);
    let mut ir = ir;
    let params = vimyc::ir::ParamValues::bind(&ir, &doctrine).expect("binds");
    vimyc::specialise::specialise(&mut ir, &params);
    let vimyc::emit::Artifact::Expr(rules) =
        vimyc::emit::emit(&ir, &params, vimyc::emit::Target::Expr);

    // 200 + round((400 - 200) * 0.7) = 340, matching Go's lerp.
    let form = rules.iter().find(|r| r.name == "form-naval-squad").unwrap();
    assert_eq!(form.priority, 340);
    assert_eq!(
        rules
            .iter()
            .find(|r| r.name == "naval-attack-move")
            .unwrap()
            .priority,
        330
    );

    // An int parameter reaches an action argument as an integer, not `8.0`.
    assert_eq!(form.action, "form-squad(naval-attack, Naval, 8, Attack)");
    assert!(
        form.condition.contains("len(UnassignedIdleNaval()) >= 8"),
        "{}",
        form.condition
    );

    // The gate held, so it is gone rather than folded to `0.4 >= 0.3`.
    assert!(
        form.condition.starts_with("MapHasWater()"),
        "{}",
        form.condition
    );

    // And with a doctrine that ignores the sea, neither rule is there at all —
    // which is what CompileDoctrine does in Go.
    let mut ir = vimyc::check::check(&ast).expect("checks").ir;
    let land = std::collections::HashMap::from([
        ("aggression".to_string(), 0.7),
        ("naval-weight".to_string(), 0.1),
        ("ground-attack-group-size".to_string(), 8.0),
    ]);
    let params = vimyc::ir::ParamValues::bind(&ir, &land).expect("binds");
    vimyc::specialise::specialise(&mut ir, &params);
    let vimyc::emit::Artifact::Expr(rules) =
        vimyc::emit::emit(&ir, &params, vimyc::emit::Target::Expr);
    assert!(rules.is_empty(), "{rules:?}");
}

/// Rounding is where a hand-written `lerp` and Go's diverge, so it is pinned
/// rather than left to the one value the sample doctrine happens to use.
#[test]
fn lerp_rounds_the_way_go_does() {
    for (t, want) in [
        (0.0, 200),
        (1.0, 400),
        (0.5, 300),
        (0.333, 267),
        (0.334, 267),
    ] {
        let src = "param t: float\nrule r {\n priority lerp(200, 400, t)\n \
                   category economy\n do scout\n require cash >= 1\n}\n";
        let (tokens, _) = vimyc::lexer::lex(src);
        let (ast, _) = vimyc::parser::parse(&tokens);
        let ir = vimyc::check::check(&ast).expect("checks").ir;
        let params = vimyc::ir::ParamValues::bind(
            &ir,
            &std::collections::HashMap::from([("t".to_string(), t)]),
        )
        .expect("binds");
        let vimyc::emit::Artifact::Expr(rules) =
            vimyc::emit::emit(&ir, &params, vimyc::emit::Target::Expr);
        assert_eq!(rules[0].priority, want, "lerp(200, 400, {t})");
    }
}

/// The builtins the savings stack needs, pinned against Go.
///
/// `trunc` is the one that matters: Go's `int(x)` truncates while `lerp`
/// rounds, and 800 * 0.29 is 231.999… — so rounding would give 232 where Go
/// gives 231, an off-by-one buried in a cash threshold.
#[test]
fn the_arithmetic_builtins_match_go() {
    let emit = |src: &str, doctrine: &[(&str, f64)]| -> String {
        let (tokens, ld) = vimyc::lexer::lex(src);
        assert!(ld.is_empty(), "{ld:?}");
        let (ast, pd) = vimyc::parser::parse(&tokens);
        assert!(pd.is_empty(), "{pd:?}");
        let mut ir = vimyc::check::check(&ast).expect("checks").ir;
        let map: std::collections::HashMap<String, f64> = doctrine
            .iter()
            .map(|(k, v)| ((*k).to_string(), *v))
            .collect();
        let params = vimyc::ir::ParamValues::bind(&ir, &map).expect("binds");
        vimyc::specialise::specialise(&mut ir, &params);
        let vimyc::emit::Artifact::Expr(rules) =
            vimyc::emit::emit(&ir, &params, vimyc::emit::Target::Expr);
        rules[0].condition.clone()
    };

    let trunc = "param s: float\nrule r {\n priority 1\n category economy\n do scout\n \
                 require cash >= trunc(800.0 * s)\n}\n";
    // int(800.0 * s) in Go, for the same five values.
    for (s, want) in [(0.3, 240), (0.7, 560), (0.29, 231), (0.1, 80), (0.55, 440)] {
        assert_eq!(
            emit(trunc, &[("s", s)]),
            format!("Cash() >= {want}"),
            "s = {s}"
        );
    }

    // max clamps a negative difference to zero, which is what Go's
    // `if reserveScale < 0 { reserveScale = 0 }` does.
    let clamp = "param a: float\nparam b: float\n\
                 rule r {\n priority 1\n category economy\n do scout\n \
                 require cash >= 800 + trunc(800.0 * max(0.0, a - b))\n}\n";
    assert_eq!(emit(clamp, &[("a", 0.6), ("b", 0.3)]), "Cash() >= 1040");
    assert_eq!(emit(clamp, &[("a", 0.2), ("b", 0.5)]), "Cash() >= 800");

    // select is a step, which no lerp or clamp can express: at a = 0.2 the
    // answer is 1.0, not 0.8.
    let step = "param a: float\n\
                rule r {\n priority 1\n category economy\n do scout\n \
                require cash >= trunc(select(a >= 0.4, 1.0 - a, 1.0) * 100.0)\n}\n";
    assert_eq!(emit(step, &[("a", 0.6)]), "Cash() >= 40");
    assert_eq!(emit(step, &[("a", 0.2)]), "Cash() >= 100");

    let m = "param a: float\nparam b: float\n\
             rule r {\n priority 1\n category economy\n do scout\n \
             require cash >= trunc(min(a, b) * 1000.0)\n}\n";
    assert_eq!(emit(m, &[("a", 0.6), ("b", 0.3)]), "Cash() >= 300");
}
