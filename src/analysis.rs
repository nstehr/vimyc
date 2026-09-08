//! Why a rule did not fire, counted over a recorded game.
//!
//! The compiler is the only place this can be answered. By the time a rule
//! reaches the engine its condition is one flattened conjunction, but `IrRule`
//! still holds `requires` as separate expressions, each with the span of the
//! line an author wrote. So a count of "this rule never fired" can be turned
//! into "and it was this `require`, on this line, that stopped it".
//!
//! Three counts per rule, because they answer different questions:
//!
//! - `held` — the condition was satisfiable. What the rule managed.
//! - `preempted` — an exclusive rule already claimed the category this tick, so
//!   the rule was never evaluated. Not the rule's fault; a contest it lost.
//! - per-clause `blocked` / `sole` — which requirement was false, and how often
//!   it was the *only* false one.
//!
//! `sole` is the actionable number. A clause that is merely one of several
//! failing is not what to fix; a clause that stands alone between a rule and
//! firing is exactly what to fix.

use crate::diag::Span;
use crate::eval::{conjuncts, priority, rule_fires};
use crate::ir::{Ir, IrExpr, IrExprKind, IrRule, ParamValues};
use crate::state::State;
use std::cmp::Reverse;
use std::collections::HashSet;

/// One `require`, and how often it stood in the way.
#[derive(Debug, Clone)]
pub struct ClauseBlame {
    /// Position in the rule's `requires`, which is the order they were written.
    pub index: usize,
    /// States where this requirement was false.
    pub blocked: u64,
    /// States where it was the *only* false requirement — satisfy it and the
    /// rule fires.
    pub sole: u64,
    pub span: Span,
}

/// One rule's record over a run of states.
#[derive(Debug, Clone)]
pub struct RuleBlame {
    pub rule: String,
    /// The rule's category. A `rebuild` or `aircraft-maintenance` rule that
    /// never fires is usually good news — nothing was lost, nothing was stuck —
    /// and reading it as a starved production rule inverts the finding.
    pub category: String,
    /// What the rule does. The reliable link from a clause that reports a thing
    /// missing to whichever rule was supposed to make it.
    pub action: String,
    /// States the rule was considered in — every state in the run.
    pub seen: u64,
    /// States where every requirement held.
    pub held: u64,
    /// States where an exclusive rule had already claimed the category, so this
    /// rule was never evaluated.
    pub preempted: u64,
    pub clauses: Vec<ClauseBlame>,
}

impl RuleBlame {
    /// States where the rule was evaluated and something stopped it.
    pub fn blocked(&self) -> u64 {
        self.seen - self.held - self.preempted
    }

    /// The clause worth acting on: the one most often solely responsible.
    ///
    /// `None` when nothing was ever solely responsible, which means the rule is
    /// blocked by several requirements at once and no single fix frees it.
    pub fn culprit(&self) -> Option<&ClauseBlame> {
        self.clauses
            .iter()
            .filter(|c| c.sole > 0)
            .max_by_key(|c| c.sole)
    }
}

/// Replays a rule set over recorded states and attributes every non-firing.
///
/// Category exclusivity is applied exactly as `eval::evaluate` applies it, so
/// `preempted` means what it means at run time rather than what a simpler pass
/// would guess.
pub fn blame(ir: &Ir, params: &ParamValues, states: &[State]) -> Vec<RuleBlame> {
    let mut out: Vec<RuleBlame> = ir
        .rules
        .iter()
        .map(|r| RuleBlame {
            rule: r.name.clone(),
            category: crate::env::category_name(r.category.0).to_string(),
            action: crate::env::action_name(r.action.id.0).to_string(),
            seen: 0,
            held: 0,
            preempted: 0,
            clauses: r
                .requires
                .iter()
                .enumerate()
                .map(|(index, e)| ClauseBlame {
                    index,
                    blocked: 0,
                    sole: 0,
                    span: e.span,
                })
                .collect(),
        })
        .collect();

    // Priority is fixed within a doctrine window, so the order is computed once.
    let mut order: Vec<(i64, usize)> = ir
        .rules
        .iter()
        .enumerate()
        .map(|(i, r)| (priority(r, params), i))
        .collect();
    order.sort_by_key(|(p, _)| Reverse(*p));

    for state in states {
        let mut claimed: HashSet<u32> = HashSet::new();
        for &(_, i) in &order {
            let rule: &IrRule = &ir.rules[i];
            let rec = &mut out[i];
            rec.seen += 1;

            if claimed.contains(&rule.category.0) {
                rec.preempted += 1;
                continue;
            }

            let held = conjuncts(rule, params, state);
            let failing = held.iter().filter(|ok| !**ok).count();
            if failing == 0 {
                rec.held += 1;
                if rule.exclusive {
                    claimed.insert(rule.category.0);
                }
                continue;
            }
            for (c, ok) in rec.clauses.iter_mut().zip(&held) {
                if !ok {
                    c.blocked += 1;
                    if failing == 1 {
                        c.sole += 1;
                    }
                }
            }
        }
    }
    out
}

/// A cheap check that `blame` agrees with the evaluator it models.
///
/// Kept public because it is the property worth asserting anywhere this is
/// wired up: a rule whose clauses all hold is a rule that fires.
pub fn agrees_with_eval(ir: &Ir, params: &ParamValues, state: &State) -> bool {
    ir.rules.iter().all(|r| {
        let all_hold = conjuncts(r, params, state).into_iter().all(|ok| ok);
        all_hold == rule_fires(r, params, state)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::check;
    use crate::unit::Unit;

    fn ir_of(src: &str) -> Ir {
        let u = Unit::parse([("t.vy".to_string(), src.to_string())]);
        assert!(u.diags.is_empty(), "{:?}", u.diags);
        check(&u.ast).expect("checks").ir
    }

    fn state(json: &str) -> State {
        serde_json::from_str(json).expect("state")
    }

    const NO_PARAMS: ParamValues = ParamValues { values: Vec::new() };

    /// Two requirements, one false: that one is solely responsible.
    #[test]
    fn a_single_failing_clause_is_sole() {
        let ir = ir_of(
            "rule r {\n priority 1\n category economy\n do scout\n \
             require cash >= 500\n require power-excess >= 0\n}\n",
        );
        let s = state(r#"{"scalars":{"cash":100,"power-excess":50}}"#);
        let b = &blame(&ir, &NO_PARAMS, &[s])[0];
        assert_eq!(b.held, 0);
        assert_eq!(b.clauses[0].blocked, 1);
        assert_eq!(b.clauses[0].sole, 1);
        assert_eq!(b.clauses[1].blocked, 0);
        assert_eq!(b.culprit().map(|c| c.index), Some(0));
    }

    /// Two failing at once: neither is solely responsible, and there is no
    /// single clause to fix.
    #[test]
    fn two_failing_clauses_leave_no_culprit() {
        let ir = ir_of(
            "rule r {\n priority 1\n category economy\n do scout\n \
             require cash >= 500\n require power-excess >= 0\n}\n",
        );
        let s = state(r#"{"scalars":{"cash":100,"power-excess":-20}}"#);
        let b = &blame(&ir, &NO_PARAMS, &[s])[0];
        assert_eq!(b.clauses[0].blocked, 1);
        assert_eq!(b.clauses[1].blocked, 1);
        assert_eq!(b.clauses[0].sole, 0);
        assert_eq!(b.clauses[1].sole, 0);
        assert!(b.culprit().is_none(), "no single fix frees this rule");
    }

    #[test]
    fn a_satisfied_rule_is_held_and_blames_nothing() {
        let ir =
            ir_of("rule r {\n priority 1\n category economy\n do scout\n require cash >= 50\n}\n");
        let s = state(r#"{"scalars":{"cash":100}}"#);
        let b = &blame(&ir, &NO_PARAMS, &[s])[0];
        assert_eq!(b.held, 1);
        assert_eq!(b.blocked(), 0);
        assert_eq!(b.clauses[0].blocked, 0);
    }

    /// An exclusive rule that fires takes the category, and the rule below it is
    /// recorded as preempted rather than blamed for a condition never evaluated.
    #[test]
    fn a_preempted_rule_is_not_blamed() {
        let ir = ir_of(
            "rule winner {\n priority 900\n category economy exclusive\n do scout\n \
             require cash >= 50\n}\n\
             rule loser {\n priority 100\n category economy exclusive\n do scout\n \
             require cash >= 999999\n}\n",
        );
        let s = state(r#"{"scalars":{"cash":100}}"#);
        let out = blame(&ir, &NO_PARAMS, &[s]);
        let loser = out.iter().find(|b| b.rule == "loser").expect("loser");
        assert_eq!(loser.preempted, 1);
        assert_eq!(loser.held, 0);
        assert_eq!(loser.blocked(), 0, "preempted is not blocked");
        assert_eq!(
            loser.clauses[0].blocked, 0,
            "a condition that was never evaluated cannot be the culprit"
        );
    }

    #[test]
    fn counts_accumulate_across_states() {
        let ir =
            ir_of("rule r {\n priority 1\n category economy\n do scout\n require cash >= 500\n}\n");
        let states: Vec<State> = [100, 100, 900, 100]
            .iter()
            .map(|c| state(&format!(r#"{{"scalars":{{"cash":{c}}}}}"#)))
            .collect();
        let b = &blame(&ir, &NO_PARAMS, &states)[0];
        assert_eq!(b.seen, 4);
        assert_eq!(b.held, 1);
        assert_eq!(b.clauses[0].sole, 3);
    }
}

/// A numeric gate, and what it would have cost at other values.
///
/// `sole` says removing a clause would let a rule fire. It does not say whether
/// *moving* it would, and for a threshold that is the question actually worth
/// asking: a gate at 600 against a treasury that never exceeds 50 is not a
/// threshold problem, and lowering it achieves nothing. The curve shows which
/// case you are in.
#[derive(Debug, Clone)]
pub struct Threshold {
    pub rule: String,
    /// Which `require` this comparison sits in.
    pub clause: usize,
    pub span: Span,
    /// The comparison as it stands, e.g. `>=` and 600.
    pub op: crate::ast::BinOp,
    pub at: f64,
    /// States where the comparison was false at the value it has.
    pub blocked: u64,
    /// Candidate values and what each would have blocked, ascending.
    pub curve: Vec<(f64, u64)>,
    /// The largest value the measured side ever reached. A gate above this is
    /// unreachable however it is tuned.
    pub reached: f64,
    /// The smallest. A gate at or below it excludes nothing, so "relief" found
    /// there is the comparison going vacuous rather than a dial being turned.
    pub floor: f64,
}

/// Threshold curves for every numeric gate the rule set compares against a
/// constant.
///
/// Only constant-sided comparisons: `cash >= 600` has a curve, `role-count(a) <
/// role-count(b)` does not, because moving it is not a number someone can
/// change. Parameters are already folded by `specialise`, so a gate written as
/// `lerp(1500, 300, ground-defense-priority)` is a constant by the time it gets
/// here.
pub fn thresholds(ir: &Ir, params: &ParamValues, states: &[State]) -> Vec<Threshold> {
    let mut out = Vec::new();
    for rule in &ir.rules {
        for (clause, require) in rule.requires.iter().enumerate() {
            let mut gates = Vec::new();
            collect_gates(require, &mut gates);
            for (span, op, measured, at) in gates {
                let mut values: Vec<f64> = Vec::with_capacity(states.len());
                for state in states {
                    match numeric(rule, params, state, measured) {
                        Some(v) => values.push(v),
                        // A side that is not a number in some state is not a
                        // gate anyone can tune; drop the whole curve rather
                        // than report one built from a subset.
                        None => {
                            values.clear();
                            break;
                        }
                    }
                }
                if values.is_empty() {
                    continue;
                }
                out.push(build_curve(rule.name.clone(), clause, span, op, at, values));
            }
        }
    }
    out
}

/// Comparisons against a constant, found under the `and`s of one `require`.
///
/// Descends `and` only. A gate inside an `or` is not the reason a require was
/// false — the other side could have carried it — so reporting a curve for it
/// would invite a change that fixes nothing.
type Gate<'a> = (Span, crate::ast::BinOp, &'a IrExpr, f64);

fn collect_gates<'a>(e: &'a IrExpr, out: &mut Vec<Gate<'a>>) {
    use crate::ast::BinOp::*;
    if let IrExprKind::Binary(op, l, r) = &e.kind {
        match op {
            And => {
                collect_gates(l, out);
                collect_gates(r, out);
            }
            Lt | LtEq | Gt | GtEq => {
                if let Some(k) = constant(r) {
                    out.push((e.span, *op, l, k));
                } else if let Some(k) = constant(l) {
                    // Mirrored so the measured side is always on the left.
                    let flipped = match op {
                        Lt => Gt,
                        LtEq => GtEq,
                        Gt => Lt,
                        _ => LtEq,
                    };
                    out.push((e.span, flipped, r, k));
                }
            }
            _ => {}
        }
    }
}

fn constant(e: &IrExpr) -> Option<f64> {
    match e.kind {
        IrExprKind::Int(i) => Some(i as f64),
        IrExprKind::Float(f) => Some(f),
        _ => None,
    }
}

fn numeric(rule: &IrRule, params: &ParamValues, state: &State, e: &IrExpr) -> Option<f64> {
    match crate::eval::value_of(rule, params, state, e) {
        crate::eval::Value::Int(i) => Some(i as f64),
        crate::eval::Value::Float(f) => Some(f),
        _ => None,
    }
}

fn build_curve(
    rule: String,
    clause: usize,
    span: Span,
    op: crate::ast::BinOp,
    at: f64,
    mut values: Vec<f64>,
) -> Threshold {
    use crate::ast::BinOp::*;
    let blocked_at = |t: f64, sorted: &[f64]| -> u64 {
        sorted
            .iter()
            .filter(|v| match op {
                GtEq => **v < t,
                Gt => **v <= t,
                LtEq => **v > t,
                _ => **v >= t,
            })
            .count() as u64
    };
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let reached = *values.last().unwrap_or(&0.0);
    let floor = *values.first().unwrap_or(&0.0);

    // Candidates from the distribution rather than a fixed grid: a gate is
    // interesting exactly where the measured side actually sat, and a grid over
    // an arbitrary range spends most of its points on values nothing was near.
    let mut candidates: Vec<f64> = (1..=9)
        .map(|d| values[(values.len() - 1) * d / 10])
        .collect();
    candidates.push(at);
    candidates.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    candidates.dedup();

    Threshold {
        rule,
        clause,
        span,
        op,
        at,
        blocked: blocked_at(at, &values),
        curve: candidates
            .iter()
            .map(|t| (*t, blocked_at(*t, &values)))
            .collect(),
        reached,
        floor,
    }
}

#[cfg(test)]
mod threshold_tests {
    use super::*;
    use crate::check::check;
    use crate::unit::Unit;

    fn ir_of(src: &str) -> Ir {
        let u = Unit::parse([("t.vy".to_string(), src.to_string())]);
        assert!(u.diags.is_empty(), "{:?}", u.diags);
        check(&u.ast).expect("checks").ir
    }

    fn states(cash: &[i64]) -> Vec<State> {
        cash.iter()
            .map(|c| serde_json::from_str(&format!(r#"{{"scalars":{{"cash":{c}}}}}"#)).unwrap())
            .collect()
    }

    const NO_PARAMS: ParamValues = ParamValues { values: Vec::new() };

    const RULE: &str =
        "rule r {\n priority 1\n category economy\n do scout\n require cash >= 600\n}\n";

    /// A gate that a lower value would clear reports a falling curve.
    #[test]
    fn a_movable_gate_has_a_falling_curve() {
        let ir = ir_of(RULE);
        let s = states(&[0, 100, 200, 400, 500, 700, 800, 900, 1000, 1200]);
        let t = &thresholds(&ir, &NO_PARAMS, &s)[0];

        assert_eq!(t.at, 600.0);
        assert_eq!(t.blocked, 5, "five states held less than 600");
        let at_100 = t.curve.iter().find(|(v, _)| *v <= 100.0).map(|(_, n)| *n);
        assert!(
            at_100.unwrap_or(u64::MAX) < t.blocked,
            "lowering the gate should block less: {:?}",
            t.curve
        );
    }

    /// A gate far above anything the measured side reached is not a threshold
    /// problem: no reduction helps until it drops inside the observed range, and
    /// `reached` is what says so.
    #[test]
    fn a_gate_above_everything_observed_only_moves_inside_the_range() {
        let ir = ir_of(RULE);
        let s = states(&[0, 0, 1, 2, 0, 3, 1, 0, 2, 1]);
        let t = &thresholds(&ir, &NO_PARAMS, &s)[0];

        assert_eq!(t.blocked, 10, "600 blocks every state");
        assert!(t.reached <= 3.0, "reached = {}, want ~3", t.reached);
        for (value, blocked) in &t.curve {
            if *blocked < t.blocked {
                assert!(
                    *value <= t.reached,
                    "{value} frees states but is above anything observed ({}); \
                     the gate is unreachable, not mistuned",
                    t.reached
                );
            }
        }
    }

    /// A comparison between two measured values is not a number anyone can
    /// change, so it gets no curve.
    #[test]
    fn a_gate_without_a_constant_side_is_skipped() {
        let ir = ir_of(
            "rule r {\n priority 1\n category economy\n do scout\n \
             require role-count(harvester) < role-count(refinery)\n}\n",
        );
        let s = states(&[100, 200]);
        assert!(thresholds(&ir, &NO_PARAMS, &s).is_empty());
    }

    /// A gate under an `or` is not why the require was false — the other side
    /// could have carried it — so recommending a change to it would be wrong.
    #[test]
    fn a_gate_under_an_or_is_skipped() {
        let ir = ir_of(
            "rule r {\n priority 1\n category economy\n do scout\n \
             require cash >= 600 or has-role(radar)\n}\n",
        );
        let s = states(&[100, 200]);
        assert!(thresholds(&ir, &NO_PARAMS, &s).is_empty());
    }

    /// Both `and` conjuncts are gates, and both get a curve.
    #[test]
    fn conjuncts_are_found_separately() {
        let ir = ir_of(
            "rule r {\n priority 1\n category economy\n do scout\n \
             require cash >= 600 and power-excess >= 0\n}\n",
        );
        let s: Vec<State> = [(100, -5), (900, 10)]
            .iter()
            .map(|(c, p)| {
                serde_json::from_str(&format!(
                    r#"{{"scalars":{{"cash":{c},"power-excess":{p}}}}}"#
                ))
                .unwrap()
            })
            .collect();
        assert_eq!(thresholds(&ir, &NO_PARAMS, &s).len(), 2);
    }
}
