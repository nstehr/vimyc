//! The fold-time pass: a rule set plus a doctrine, minus everything that
//! doctrine settles.
//!
//! Go decides which rules to emit at all, so a land doctrine's rule set simply
//! has no naval rules. A `param` moves that decision into the language as an
//! ordinary `require`, and without this pass the comparison would survive
//! folding — a rule that never fires rather than a rule that is not there.

use crate::ast::BinOp;
use crate::diag::Diagnostic;
use crate::eval::{Value, static_eval};
use crate::ir::{Ir, IrExpr, IrExprKind, IrRule, ParamValues};

/// Applies a doctrine, dropping what it rules out.
///
/// A conjunct decidable from parameters alone is settled here: false drops the
/// rule, true drops the conjunct. Anything reading game state is untouched.
///
/// In place because this only ever removes; rebuilding would need `Clone` on
/// every node for no gain.
pub fn specialise(ir: &mut Ir, params: &ParamValues) {
    ir.rules.retain_mut(|rule| {
        for binding in &mut rule.lets {
            simplify(binding, params);
        }
        let mut alive = true;
        rule.requires.retain_mut(|conjunct| {
            // The rule is about to be dropped; simplifying the rest would only
            // mislead whoever reads the result.
            if !alive {
                return true;
            }
            simplify(conjunct, params);
            match conjunct.kind {
                IrExprKind::Bool(true) => false,
                IrExprKind::Bool(false) => {
                    alive = false;
                    true
                }
                _ => true,
            }
        });
        alive
    });
}

/// Folds what the doctrine settles, in place.
///
/// The absorption step is why folding alone is not enough. Go appends a clause
/// conditionally, which reads here as `floor <= 0 or role-count(pillbox) >=
/// floor`; folding leaves `0 <= 0 || RoleCount("pillbox") >= 0`, where Go emits
/// no clause at all.
fn simplify(e: &mut IrExpr, params: &ParamValues) {
    if is_static(e) {
        let span = e.span;
        e.kind = match static_eval(e, params) {
            Value::Int(n) => IrExprKind::Int(n),
            Value::Float(f) => IrExprKind::Float(f),
            Value::Bool(b) => IrExprKind::Bool(b),
            // The payload of an optional is unreachable from the language, so
            // nothing static can have this type.
            Value::Opt(_) => unreachable!("a static expression produced an optional"),
        };
        e.span = span;
        return;
    }

    match &mut e.kind {
        IrExprKind::Unary(_, operand) => simplify(operand, params),
        IrExprKind::Predicate(_, args) | IrExprKind::Builtin(_, args) => {
            for a in args {
                simplify(a, params);
            }
        }
        IrExprKind::Binary(op, l, r) => {
            simplify(l, params);
            simplify(r, params);
            // One side may now be a constant even though the whole is not.
            if let Some(kind) = absorb(*op, l, r) {
                e.kind = kind;
            }
        }
        _ => {}
    }
}

/// `true && x` is `x`, `false || x` is `x`, and the other two settle the whole
/// expression. `None` when neither side is a constant.
fn absorb(op: BinOp, l: &mut IrExpr, r: &mut IrExpr) -> Option<IrExprKind> {
    // The placeholder is arbitrary: the node it replaces is being discarded.
    let take = |side: &mut IrExpr| std::mem::replace(&mut side.kind, IrExprKind::Bool(false));
    match (op, as_bool(l), as_bool(r)) {
        (BinOp::And, Some(false), _) | (BinOp::And, _, Some(false)) => {
            Some(IrExprKind::Bool(false))
        }
        (BinOp::Or, Some(true), _) | (BinOp::Or, _, Some(true)) => Some(IrExprKind::Bool(true)),
        (BinOp::And, Some(true), _) | (BinOp::Or, Some(false), _) => Some(take(r)),
        (BinOp::And, _, Some(true)) | (BinOp::Or, _, Some(false)) => Some(take(l)),
        _ => None,
    }
}

fn as_bool(e: &IrExpr) -> Option<bool> {
    match e.kind {
        IrExprKind::Bool(b) => Some(b),
        _ => None,
    }
}

/// Whether this expression can be decided without game state.
///
/// Must agree with `check`'s `Phase::Static` — same three node kinds. A binding
/// counts as non-static even when its value is: resolving one would mean
/// carrying the rule scope through the walk, for a case no rule set has.
pub(crate) fn is_static(e: &IrExpr) -> bool {
    match &e.kind {
        IrExprKind::Int(_) | IrExprKind::Float(_) | IrExprKind::Bool(_) => true,
        IrExprKind::Param(_) => true,
        IrExprKind::Predicate(..) | IrExprKind::Member(..) | IrExprKind::Binding(_) => false,
        IrExprKind::Builtin(_, args) => args.iter().all(is_static),
        IrExprKind::Unary(_, operand) => is_static(operand),
        IrExprKind::Binary(_, l, r) => is_static(l) && is_static(r),
    }
}

/// The checks that need a whole rule set *and* its doctrine.
///
/// They compare priorities, so in `check` they silently skipped every rule whose
/// priority was a `lerp` — after the port, nearly all of them. Warnings rather
/// than errors: both describe a rule set that runs and is probably wrong.
pub fn validate(ir: &Ir, params: &ParamValues) -> Vec<Diagnostic> {
    let resolved: Vec<(i64, &IrRule)> = ir
        .rules
        .iter()
        .map(|r| (crate::eval::priority(r, params), r))
        .collect();

    let mut diags = Vec::new();
    collisions(&resolved, &mut diags);
    shadowed(&resolved, &mut diags);
    diags
}

/// Two rules sharing a category and a priority.
///
/// Go's `sort.Slice` is not stable, so equal priorities order arbitrarily —
/// within a category that decides which of the two an exclusive rule blocks, and
/// can land differently between runs of the same rule set.
fn collisions(rules: &[(i64, &IrRule)], diags: &mut Vec<Diagnostic>) {
    for (i, (pa, a)) in rules.iter().enumerate() {
        for (pb, b) in &rules[i + 1..] {
            if pa == pb && a.category == b.category {
                let msg = format!(
                    "`{}` and `{}` share priority {pa} in category `{}`, so their order is undefined",
                    a.name,
                    b.name,
                    crate::env::category_name(a.category.0)
                );
                diags.push(Diagnostic::warning(b.name_span, msg));
            }
        }
    }
}

/// A rule that can never fire because an exclusive rule above it always fires
/// first.
///
/// Sound but narrow: containment of conjuncts, not implication. Anything
/// cleverer needs a solver.
fn shadowed(rules: &[(i64, &IrRule)], diags: &mut Vec<Diagnostic>) {
    // Every pair, not just earlier ones: "higher" means priority, not position.
    for (i, (lp, lower)) in rules.iter().enumerate() {
        for (j, (hp, higher)) in rules.iter().enumerate() {
            if i == j || !higher.exclusive || higher.category != lower.category || hp <= lp {
                continue;
            }
            let covered = higher
                .requires
                .iter()
                .all(|h| lower.requires.iter().any(|l| same_expr(h, l)));
            if covered {
                let msg = format!(
                    "`{}` can never fire: `{}` is exclusive, higher priority, and its conditions are implied by these",
                    lower.name, higher.name
                );
                diags.push(Diagnostic::warning(lower.name_span, msg));
            }
        }
    }
}

/// Structural equality, ignoring spans. On the IR, so `count(powr)` and
/// `building-count(powr)` are one conjunct.
fn same_expr(a: &IrExpr, b: &IrExpr) -> bool {
    match (&a.kind, &b.kind) {
        (IrExprKind::Int(x), IrExprKind::Int(y)) => x == y,
        (IrExprKind::Float(x), IrExprKind::Float(y)) => x == y,
        (IrExprKind::Bool(x), IrExprKind::Bool(y)) => x == y,
        (IrExprKind::Param(x), IrExprKind::Param(y)) => x == y,
        (IrExprKind::Binding(x), IrExprKind::Binding(y)) => x == y,
        (IrExprKind::Member(dx, ix), IrExprKind::Member(dy, iy)) => dx == dy && ix == iy,
        (IrExprKind::Predicate(x, xs), IrExprKind::Predicate(y, ys)) => {
            x == y && xs.len() == ys.len() && xs.iter().zip(ys).all(|(p, q)| same_expr(p, q))
        }
        (IrExprKind::Builtin(x, xs), IrExprKind::Builtin(y, ys)) => {
            x == y && xs.len() == ys.len() && xs.iter().zip(ys).all(|(p, q)| same_expr(p, q))
        }
        (IrExprKind::Unary(xo, x), IrExprKind::Unary(yo, y)) => xo == yo && same_expr(x, y),
        (IrExprKind::Binary(xo, xl, xr), IrExprKind::Binary(yo, yl, yr)) => {
            xo == yo && same_expr(xl, yl) && same_expr(xr, yr)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{check, emit, lexer, parser};
    use std::collections::HashMap;

    /// Two naval rules behind a `naval-weight` gate, plus one that is not gated.
    fn rule_set() -> Ir {
        let src = "param naval-weight: float\n\
                   rule gated {\n priority 100\n category combat\n \
                   do squad-attack-move(naval-attack)\n \
                   require naval-weight >= 0.3\n require map-has-water()\n}\n\
                   rule gate-only {\n priority 90\n category squad-form\n \
                   do squad-defend(naval-attack)\n require naval-weight >= 0.3\n}\n\
                   rule ungated {\n priority 80\n category economy\n do scout\n \
                   require cash >= 300\n}\n";
        let (tokens, ld) = lexer::lex(src);
        assert!(ld.is_empty(), "{ld:?}");
        let (ast, pd) = parser::parse(&tokens);
        assert!(pd.is_empty(), "{pd:?}");
        check::check(&ast).expect("checks").ir
    }

    fn with(weight: f64) -> Ir {
        let mut ir = rule_set();
        let params = ParamValues::bind(&ir, &HashMap::from([("naval-weight".to_string(), weight)]))
            .expect("binds");
        specialise(&mut ir, &params);
        ir
    }

    fn names(ir: &Ir) -> Vec<&str> {
        ir.rules.iter().map(|r| r.name.as_str()).collect()
    }

    #[test]
    fn a_false_gate_removes_the_rule() {
        let ir = with(0.1);
        assert_eq!(names(&ir), vec!["ungated"]);
    }

    #[test]
    fn a_true_gate_removes_only_itself() {
        let ir = with(0.4);
        assert_eq!(names(&ir), vec!["gated", "gate-only", "ungated"]);

        // The gate is gone; what needs a tick to answer is untouched.
        assert_eq!(ir.rules[0].requires.len(), 1);
        assert!(matches!(
            ir.rules[0].requires[0].kind,
            IrExprKind::Predicate(..)
        ));
        assert_eq!(
            ir.rules[2].requires.len(),
            1,
            "an ungated rule is untouched"
        );
    }

    /// A rule that was nothing but its gate still has to compile. expr rejects
    /// an empty condition outright, so the emitter writes `true`.
    #[test]
    fn a_rule_that_was_only_a_gate_survives_as_true() {
        let ir = with(0.4);
        let gate_only = ir.rules.iter().find(|r| r.name == "gate-only").unwrap();
        assert!(gate_only.requires.is_empty());

        let emit::Artifact::Expr(rules) =
            emit::emit(&ir, &ParamValues::default(), emit::Target::Expr);
        let emitted = rules.iter().find(|r| r.name == "gate-only").unwrap();
        assert_eq!(emitted.condition, "true");
    }

    /// The pass only ever removes what a doctrine decided, so a rule set with no
    /// parameters comes out exactly as it went in.
    #[test]
    fn a_rule_set_without_parameters_is_unchanged() {
        let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/rules/seed.vy"))
            .expect("seed.vy");
        let (tokens, _) = lexer::lex(&src);
        let (ast, _) = parser::parse(&tokens);
        let mut ir = check::check(&ast).expect("checks").ir;

        let before: Vec<(String, usize)> = ir
            .rules
            .iter()
            .map(|r| (r.name.clone(), r.requires.len()))
            .collect();
        specialise(&mut ir, &ParamValues::default());
        let after: Vec<(String, usize)> = ir
            .rules
            .iter()
            .map(|r| (r.name.clone(), r.requires.len()))
            .collect();
        assert_eq!(before, after);
    }

    /// A gate can be a conjunction or a disjunction, and those short-circuit —
    /// which `binary` cannot do, since its operands are already values.
    #[test]
    fn a_compound_gate_folds() {
        let src = "param naval-weight: float\nparam aggression: float\n\
                   rule both {\n priority 1\n category combat\n \
                   do squad-defend(naval-attack)\n \
                   require naval-weight >= 0.3 and aggression > 0.5\n \
                   require squad-exists(naval-attack)\n}\n\
                   rule either {\n priority 2\n category combat\n \
                   do squad-attack-move(naval-attack)\n \
                   require naval-weight >= 0.9 or aggression > 0.5\n \
                   require squad-exists(naval-attack)\n}\n";
        let (tokens, _) = lexer::lex(src);
        let (ast, pd) = parser::parse(&tokens);
        assert!(pd.is_empty(), "{pd:?}");

        for (naval, aggression, want) in [
            (0.4, 0.7, vec!["both", "either"]),
            // `both` fails its conjunction; `either` still holds by aggression.
            (0.1, 0.7, vec!["either"]),
            // Neither disjunct holds.
            (0.1, 0.2, Vec::new()),
        ] {
            let mut ir = check::check(&ast).expect("checks").ir;
            let params = ParamValues::bind(
                &ir,
                &HashMap::from([
                    ("naval-weight".to_string(), naval),
                    ("aggression".to_string(), aggression),
                ]),
            )
            .expect("binds");
            specialise(&mut ir, &params);
            assert_eq!(names(&ir), want, "naval {naval}, aggression {aggression}");
        }
    }

    /// An action argument may be any static expression, and Go finds the
    /// function by this text — so it has to be the number, not the sum.
    #[test]
    fn a_computed_action_argument_is_folded() {
        let src = "rule r {\n priority 1\n category micro\n \
                   do retreat-damaged-units(0.25 + 0.25)\n \
                   require count(damaged-combat-units(0.5)) > 0\n}\n";
        let (tokens, _) = lexer::lex(src);
        let (ast, _) = parser::parse(&tokens);
        let ir = check::check(&ast).expect("checks").ir;
        let emit::Artifact::Expr(rules) =
            emit::emit(&ir, &ParamValues::default(), emit::Target::Expr);
        assert_eq!(rules[0].action, "retreat-damaged-units(0.5)");
    }

    /// Go's `baseDefenseFloorClause`: a conjunct that is only partly settled by
    /// the doctrine. Folding alone leaves `0 <= 0 || RoleCount(...) >= 0`,
    /// which Go does not emit at all.
    #[test]
    fn a_partly_static_conjunct_collapses() {
        let src = "param floor: int\n\
                   rule attack {\n priority 1\n category combat\n \
                   do squad-attack-move(ground-attack)\n \
                   require squad-exists(ground-attack)\n \
                   require floor <= 0 or role-count(pillbox) >= floor\n}\n";
        let (tokens, _) = lexer::lex(src);
        let (ast, pd) = parser::parse(&tokens);
        assert!(pd.is_empty(), "{pd:?}");

        for (floor, want) in [
            // The disjunct holds outright, so the clause is gone.
            (0, "SquadExists(\"ground-attack\")"),
            // It cannot be settled, so it stays — without the dead `2 <= 0`.
            (
                2,
                "SquadExists(\"ground-attack\") && RoleCount(\"pillbox\") >= 2",
            ),
        ] {
            let mut ir = check::check(&ast).expect("checks").ir;
            let params = ParamValues::bind(
                &ir,
                &HashMap::from([("floor".to_string(), f64::from(floor))]),
            )
            .expect("binds");
            specialise(&mut ir, &params);
            let emit::Artifact::Expr(rules) = emit::emit(&ir, &params, emit::Target::Expr);
            assert_eq!(rules[0].condition, want, "floor {floor}");
        }
    }

    /// `validate` runs where these checks now live: after a doctrine, so the
    /// priorities are numbers.
    fn warnings(src: &str) -> Vec<String> {
        warnings_with(src, &HashMap::new())
    }

    fn warnings_with(src: &str, doctrine: &HashMap<String, f64>) -> Vec<String> {
        let (tokens, ld) = lexer::lex(src);
        assert!(ld.is_empty(), "{ld:?}");
        let (ast, pd) = parser::parse(&tokens);
        assert!(pd.is_empty(), "{pd:?}");
        let mut ir = check::check(&ast).expect("checks").ir;
        let params = ParamValues::bind(&ir, doctrine).expect("binds");
        specialise(&mut ir, &params);
        validate(&ir, &params)
            .into_iter()
            .map(|d| d.message)
            .collect()
    }

    #[test]
    fn equal_priorities_in_one_category_are_reported() {
        let e = warnings(
            "rule a {\n priority 5\n category economy\n do scout\n require cash >= 1\n}\n\
             rule b {\n priority 5\n category economy\n do scout\n require cash >= 2\n}\n",
        );
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(e[0].contains("share priority 5"), "{e:?}");
    }

    #[test]
    fn equal_priorities_in_different_categories_are_fine() {
        let e = warnings(
            "rule a {\n priority 5\n category economy\n do scout\n require cash >= 1\n}\n\
             rule b {\n priority 5\n category combat\n do scout\n require cash >= 2\n}\n",
        );
        assert!(e.is_empty(), "{e:?}");
    }

    /// The reason these moved out of `check`. Two `lerp` priorities are only
    /// equal once a doctrine says what they are, so this was silent before.
    #[test]
    fn a_collision_between_doctrine_set_priorities_is_found() {
        let src = "param aggression: float\n\
                   rule a {\n priority lerp(200, 400, aggression)\n category combat\n \
                   do squad-attack-move(ground-attack)\n require cash >= 1\n}\n\
                   rule b {\n priority lerp(200, 400, aggression)\n category combat\n \
                   do squad-defend(ground-attack)\n require cash >= 2\n}\n";
        let e = warnings_with(src, &HashMap::from([("aggression".to_string(), 0.5)]));
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(e[0].contains("share priority 300"), "{e:?}");

        // And a doctrine that separates them is not a collision at all, which is
        // the case no source-level check could ever decide.
        let src = "param aggression: float\n\
                   rule a {\n priority lerp(200, 400, aggression)\n category combat\n \
                   do squad-attack-move(ground-attack)\n require cash >= 1\n}\n\
                   rule b {\n priority 300\n category combat\n \
                   do squad-defend(ground-attack)\n require cash >= 2\n}\n";
        let e = warnings_with(src, &HashMap::from([("aggression".to_string(), 0.9)]));
        assert!(e.is_empty(), "{e:?}");
    }

    #[test]
    fn a_rule_under_a_broader_exclusive_one_can_never_fire() {
        // `a` requires strictly less than `b`, so whenever `b` would fire `a`
        // already has, and `a` is exclusive.
        let e = warnings(
            "rule a {\n priority 9\n category economy exclusive\n do scout\n \
             require cash >= 1\n}\n\
             rule b {\n priority 5\n category economy\n do scout\n \
             require cash >= 1\n require has-role(barracks)\n}\n",
        );
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(e[0].contains("can never fire"), "{e:?}");
    }

    #[test]
    fn shadowing_is_found_whichever_order_the_rules_appear() {
        // "Higher" means priority, not position, so both orderings must report.
        let hi = "rule a {\n priority 9\n category economy exclusive\n do scout\n \
                  require cash >= 1\n}\n";
        let lo = "rule b {\n priority 5\n category economy\n do scout\n \
                  require cash >= 1\n require has-role(barracks)\n}\n";
        let forward = warnings(&format!("{hi}{lo}"));
        let reversed = warnings(&format!("{lo}{hi}"));
        assert_eq!(forward.len(), 1, "{forward:?}");
        assert_eq!(reversed.len(), 1, "low-priority rule first: {reversed:?}");
        assert_eq!(forward, reversed);
    }

    #[test]
    fn a_narrower_rule_above_does_not_shadow() {
        // `a` requires *more* than `b`, so `b` can still fire on its own.
        let e = warnings(
            "rule a {\n priority 9\n category economy exclusive\n do scout\n \
             require cash >= 1\n require has-role(barracks)\n}\n\
             rule b {\n priority 5\n category economy\n do scout\n require cash >= 1\n}\n",
        );
        assert!(e.is_empty(), "{e:?}");
    }

    #[test]
    fn a_non_exclusive_rule_above_does_not_shadow() {
        let e = warnings(
            "rule a {\n priority 9\n category economy\n do scout\n require cash >= 1\n}\n\
             rule b {\n priority 5\n category economy\n do scout\n \
             require cash >= 1\n require has-role(barracks)\n}\n",
        );
        assert!(e.is_empty(), "{e:?}");
    }

    /// Comparing the IR rather than the source makes this stricter in the useful
    /// direction: `count(powr)` and `building-count(powr)` are one conjunct.
    #[test]
    fn shadowing_sees_through_two_spellings_of_one_conjunct() {
        let e = warnings(
            "rule a {\n priority 9\n category economy exclusive\n do scout\n \
             require count(powr) > 0\n}\n\
             rule b {\n priority 5\n category economy\n do scout\n \
             require building-count(powr) > 0\n require has-role(barracks)\n}\n",
        );
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(e[0].contains("can never fire"), "{e:?}");
    }
}
