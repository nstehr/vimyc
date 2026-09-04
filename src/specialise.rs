//! The fold-time pass: a rule set plus a doctrine, minus everything that
//! doctrine settles.
//!
//! `CompileDoctrine` decides in Go which rules to emit at all, so a rule set for
//! a land doctrine simply has no naval rules in it. Parameters move that
//! decision into the language as an ordinary `require`, which means the
//! comparison survives folding — `0.4 >= 0.3 && ...` — and a rule whose gate is
//! false becomes a rule that never fires rather than a rule that is not there.
//!
//! This restores the difference. It is the second half of what a `param` is for:
//! `emit` turns a parameter into a number, and this turns what that number
//! decided into structure.

use crate::eval::{Value, static_eval};
use crate::ir::{Ir, IrExpr, IrExprKind, ParamValues};

/// Applies a doctrine, dropping what it rules out.
///
/// A conjunct that depends only on parameters is decided here: false drops the
/// rule, true drops the conjunct. Everything that reads game state is left
/// exactly as it was, because only a tick can answer it.
///
/// In place rather than returning a new `Ir`: this only ever removes, and
/// rebuilding the tree to do that would need `Clone` on every node for no gain.
pub fn specialise(ir: &mut Ir, params: &ParamValues) {
    ir.rules.retain_mut(|rule| {
        let mut alive = true;
        rule.requires.retain(|conjunct| {
            // Once the rule is doomed, leave the rest alone — it is about to be
            // dropped, and evaluating further conjuncts could only mislead a
            // reader of the result.
            if !alive || !is_static(conjunct) {
                return true;
            }
            match static_eval(conjunct, params) {
                Value::Bool(true) => false,
                Value::Bool(false) => {
                    alive = false;
                    true
                }
                other => unreachable!("a require folded to {other:?} rather than a bool"),
            }
        });
        alive
    });
}

/// Whether this expression can be decided without game state.
///
/// The IR counterpart of `check`'s `Phase::Static`, and it has to agree with it:
/// the checker uses that rule to decide what a priority may contain, and this
/// uses it to decide what folds. The two lists are the same three node kinds.
///
/// A binding counts as non-static even when its value is. `let` is rule-scoped
/// and resolving one here would mean carrying the scope through the walk, for a
/// case no real rule set has: a doctrine gate written through a binding.
pub(crate) fn is_static(e: &IrExpr) -> bool {
    match &e.kind {
        IrExprKind::Int(_) | IrExprKind::Float(_) | IrExprKind::Param(_) => true,
        IrExprKind::Predicate(..) | IrExprKind::Member(..) | IrExprKind::Binding(_) => false,
        IrExprKind::Builtin(_, args) => args.iter().all(is_static),
        IrExprKind::Unary(_, operand) => is_static(operand),
        IrExprKind::Binary(_, l, r) => is_static(l) && is_static(r),
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
}
