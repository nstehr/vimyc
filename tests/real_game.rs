//! vimyc against a recorded game.
//!
//! `testdata/real_differential.json` is built by `TestBuildRealDifferential` in
//! `vimy-core/rules` from a live export plus the doctrines archived for that
//! game. Unlike the synthetic corpus, these states carry accumulated intel,
//! formed squads and real threat fields — the things a generator cannot produce.
//!
//! Each case carries the fingerprint of the rule set that ran, so pairing is
//! exact. Inferring it does not work: a doctrine takes effect after it is
//! archived, and two doctrines can differ only in a threshold inside a
//! comparison, which no amount of matching on names or recorded state can see.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RealCorpus {
    /// One vimyc source file per distinct rule set the game ran.
    rule_sets: Vec<String>,
    /// The expr conditions each was translated from, per rule name.
    #[serde(default)]
    conditions: Vec<std::collections::HashMap<String, String>>,
    states: Vec<vimyc::state::State>,
    cases: Vec<RealCase>,
}

#[derive(Debug, Deserialize)]
struct RealCase {
    tick: i64,
    rule: String,
    state: usize,
    rule_set: usize,
    fired: bool,
    skipped: bool,
}

fn corpus() -> Option<RealCorpus> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/testdata/real_differential.json"
    );
    let text = std::fs::read_to_string(path).ok()?;
    Some(serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}")))
}

fn parse(src: &str, which: usize) -> vimyc::ast::Ast {
    let (tokens, ld) = vimyc::lexer::lex(src);
    assert!(ld.is_empty(), "rule set {which} does not lex: {ld:?}");
    let (ast, pd) = vimyc::parser::parse(&tokens);
    assert!(pd.is_empty(), "rule set {which} does not parse: {pd:?}");
    ast
}

/// Every rule set a real game ran parses and type checks.
///
/// Priority collisions are excluded: they are real findings about Vimy rather
/// than about the translation, and are tracked as vimy-axv. Everything else
/// must be clean.
#[test]
fn real_rule_sets_check() {
    let Some(c) = corpus() else {
        eprintln!("no recorded game; skipping");
        return;
    };
    for (i, src) in c.rule_sets.iter().enumerate() {
        let ast = parse(src, i);
        let diags: Vec<_> = vimyc::check::check(&ast)
            .into_iter()
            .filter(|d| !d.message.contains("share priority"))
            .collect();
        assert!(
            diags.is_empty(),
            "rule set {i} ({} rules) does not type check:\n{}",
            ast.rules.len(),
            diags
                .iter()
                .take(5)
                .map(|d| format!("  {}", d.message))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    eprintln!(
        "{} rule sets from a real game, all check",
        c.rule_sets.len()
    );
}

/// vimyc agrees with expr on every rule evaluation the game actually made.
///
/// Two harness bugs surfaced here before the evaluator was ever in question. The
/// exporter projected once per evaluation, so a rule running after a firing saw
/// a stale `unassigned-idle-ground` — 36 disagreements in 19,925. And pairing
/// was inferred rather than recorded, which could not separate doctrines
/// differing only in a threshold — 1 in 12,442. Both are fixed upstream.
#[test]
fn agrees_with_a_real_game() {
    let Some(c) = corpus() else {
        eprintln!("no recorded game; skipping");
        return;
    };
    let asts: Vec<_> = c
        .rule_sets
        .iter()
        .enumerate()
        .map(|(i, s)| parse(s, i))
        .collect();

    let mut checked = 0usize;
    let mut mismatches = Vec::new();

    for case in &c.cases {
        // Go never evaluated these, so there is nothing to agree about.
        if case.skipped {
            continue;
        }
        let Some(rule) = asts[case.rule_set]
            .rules
            .iter()
            .find(|r| r.name.text == case.rule)
        else {
            panic!("rule set {} has no `{}`", case.rule_set, case.rule);
        };

        let got = vimyc::eval::rule_fires(rule, &c.states[case.state]);
        checked += 1;
        if got != case.fired {
            mismatches.push(format!(
                "tick {}: `{}` — expr said {}, vimyc said {got}",
                case.tick, case.rule, case.fired
            ));
        }
    }

    assert!(checked > 1000, "corpus looks truncated: {checked}");
    assert!(
        mismatches.is_empty(),
        "{} of {checked} disagreed:\n{}",
        mismatches.len(),
        mismatches
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    eprintln!("{checked} evaluations from a real game, no disagreements");
}

/// A full round trip: Go's expr, translated to vimyc, lowered, emitted, and back
/// to the condition it started as.
///
/// Stronger than comparing evaluations, because it reaches the rules the
/// recorded states never make true — most of a rule set, in a real game.
#[test]
fn emitted_expr_round_trips_on_a_real_game() {
    let Some(c) = corpus() else {
        eprintln!("no recorded game; skipping");
        return;
    };
    assert!(
        !c.conditions.is_empty(),
        "corpus predates the conditions field; regenerate it"
    );

    let squeeze = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    // Go writes the parentheses its templates happen to contain; vimyc writes
    // only the ones precedence needs. Comparing without them keeps this test on
    // the question it exists to answer — do the names and the shape survive —
    // and leaves precedence to `agrees_with_a_real_game`, which evaluates.
    // Go writes what its templates contain; vimyc writes a canonical form. The
    // differences that remain are parenthesisation, spacing and how a float is
    // spelled — `0.40` against `0.4`. None of them reach the engine, and
    // `agrees_with_a_real_game` covers precedence by evaluating.
    let bare = |s: &str| trim_zeros(&squeeze(s).replace(['(', ')', ' '], ""));

    let mut differ = Vec::new();
    let mut checked = 0usize;

    for (i, src) in c.rule_sets.iter().enumerate() {
        let ir = vimyc::lower::lower(&parse(src, i));
        let vimyc::emit::Artifact::Expr(emitted) =
            vimyc::emit::emit(&ir, vimyc::emit::Target::Expr);

        for rule in &emitted {
            let Some(want) = c.conditions[i].get(&rule.name) else {
                panic!(
                    "rule set {i} emitted `{}`, which Go did not compile",
                    rule.name
                );
            };
            checked += 1;
            if bare(&rule.condition) != bare(want) {
                differ.push(format!(
                    "  {}\n    go:    {want}\n    vimyc: {}",
                    rule.name, rule.condition
                ));
            }
        }
    }

    assert!(checked > 1000, "corpus looks truncated: {checked}");
    assert!(
        differ.is_empty(),
        "{} of {checked} conditions did not round trip:\n{}",
        differ.len(),
        differ
            .iter()
            .take(6)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    eprintln!("{checked} conditions round trip to Go's expr");
}

/// `0.40` and `0.4` are the same number; Go keeps the doctrine's spelling and
/// vimyc reprints the parsed value.
fn trim_zeros(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        // Only inside a fraction, so the `0` in `flame_tower` survives.
        if b[i] == '.' && i > 0 && b[i - 1].is_ascii_digit() {
            let mut end = i + 1;
            while end < b.len() && b[end].is_ascii_digit() {
                end += 1;
            }
            let frac = b[i + 1..end].iter().collect::<String>();
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
