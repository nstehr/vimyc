//! The lowered form: a checked rule set with every name resolved.
//!
//! Sits between `check` and the things that consume a rule set — the evaluator
//! and the emitters — so resolution happens once instead of once per consumer.
//! `docs/implementation.md` covers why under "The IR".
//!
//! There is no `Ident` and no `Error` here, deliberately. A backend cannot
//! forget to handle an unresolved name because there are none, and cannot be
//! handed a tree that failed to check.

use crate::ast::ParamKind;
use crate::diag::Span;
use crate::env::Predicate;
use crate::types::Domain;
use std::collections::HashMap;

/// A whole rule set, in priority order.
#[derive(Debug)]
pub struct Ir {
    /// Doctrine inputs by slot, in declaration order. `IrExpr::Param` indexes
    /// this, and a `ParamValues` supplies one number per entry.
    pub params: Vec<IrParam>,
    pub rules: Vec<IrRule>,
}

#[derive(Debug)]
pub struct IrParam {
    pub name: String,
    pub kind: ParamKind,
    pub span: Span,
}

/// What a doctrine supplies for one parameter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParamValue {
    Int(i64),
    Float(f64),
}

/// A doctrine's numbers, positional so a lookup is an index rather than a hash.
///
/// Built against an `Ir`, since only the parameter list says what order the
/// values go in or how many there should be.
#[derive(Debug, Default, Clone)]
pub struct ParamValues {
    pub values: Vec<ParamValue>,
}

impl ParamValues {
    /// Puts a doctrine's numbers in slot order, reporting anything that does not
    /// line up with what the rule set declares.
    ///
    /// The alternative — trusting the caller to order them — makes a wrong
    /// threshold indistinguishable from a right one, and a doctrine is exactly
    /// the input least worth trusting.
    pub fn bind(ir: &Ir, supplied: &HashMap<String, f64>) -> Result<Self, String> {
        let mut values = Vec::with_capacity(ir.params.len());
        for p in &ir.params {
            let Some(&n) = supplied.get(&p.name) else {
                return Err(format!("no value for parameter `{}`", p.name));
            };
            values.push(match p.kind {
                ParamKind::Float => ParamValue::Float(n),
                // Rejected rather than rounded: `lerp(200, 400, 0.5)` and a
                // group size of 7.5 are different kinds of mistake, and only one
                // of them is the caller's intent.
                ParamKind::Int if n.fract() != 0.0 => {
                    return Err(format!("parameter `{}` is an int, got {n}", p.name));
                }
                ParamKind::Int => ParamValue::Int(n as i64),
            });
        }
        if let Some(extra) = supplied
            .keys()
            .find(|k| !ir.params.iter().any(|p| p.name == **k))
        {
            return Err(format!("`{extra}` is not a parameter of this rule set"));
        }
        Ok(ParamValues { values })
    }
}

#[derive(Debug)]
pub struct IrRule {
    /// Output only — emitted, never compared, so no reason to intern.
    pub name: String,
    /// Resolved once per doctrine rather than per tick: the engine sorts on it,
    /// so it has to be a number before the first evaluation. Restricted by the
    /// checker to parameters, literals and `lerp`.
    pub priority: IrExpr,
    pub category: CategoryId,
    pub exclusive: bool,
    pub action: IrAction,
    /// Bindings by slot, in declaration order. `IrExpr::Binding` indexes this.
    pub lets: Vec<IrExpr>,
    /// Implicitly ANDed.
    pub requires: Vec<IrExpr>,
    pub span: Span,
}

/// An index into `env::CATEGORIES`.
///
/// Interned because exclusivity groups rules by category on every tick, and
/// comparing integers beats comparing strings — the engine does it once per
/// rule per evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CategoryId(pub u32);

#[derive(Debug)]
pub struct IrAction {
    /// An index into `env::ACTIONS`, or the action signature table when it takes
    /// arguments.
    pub id: ActionId,
    /// Empty unless the action is built by a factory.
    pub args: Vec<IrExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionId(pub u32);

#[derive(Debug)]
pub struct IrExpr {
    pub kind: IrExprKind,
    /// Kept because blocked-on analysis points at source: "which conjunct was
    /// false for 1,400 ticks" needs somewhere to aim. Backends ignore it.
    pub span: Span,
}

#[derive(Debug)]
pub enum IrExprKind {
    Int(i64),
    Float(f64),

    /// A resolved predicate and its lowered arguments. `count` never survives
    /// lowering — it becomes whichever of the three it meant.
    Predicate(Predicate, Vec<IrExpr>),

    /// An enum literal, as an index into its domain's table. Already an integer,
    /// which is what a wasm backend would want.
    Member(Domain, u32),

    /// A `let` binding, by slot into `IrRule::lets`.
    Binding(u32),

    /// A doctrine parameter, by slot into `Ir::params`.
    Param(u32),

    /// A pure function — `lerp`, `lerpf`. Kept distinct from `Predicate`
    /// because reading no state is what makes it legal in a priority.
    Builtin(crate::env::Builtin, Vec<IrExpr>),

    Unary(crate::ast::UnOp, Box<IrExpr>),
    Binary(crate::ast::BinOp, Box<IrExpr>, Box<IrExpr>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{check, lexer, parser};

    fn ir(src: &str) -> Ir {
        let (tokens, ld) = lexer::lex(src);
        assert!(ld.is_empty(), "{ld:?}");
        let (ast, pd) = parser::parse(&tokens);
        assert!(pd.is_empty(), "{pd:?}");
        check::check(&ast).expect("checks").ir
    }

    fn decls() -> Ir {
        ir("param aggression: float\nparam size: int\n\
            rule r {\n priority lerp(200, 400, aggression)\n category economy\n \
            do scout\n require cash >= size\n}\n")
    }

    #[test]
    fn values_land_in_declaration_order() {
        let ir = decls();
        let v = ParamValues::bind(
            &ir,
            &HashMap::from([("size".into(), 8.0), ("aggression".into(), 0.5)]),
        )
        .expect("binds");
        // Supplied in the other order on purpose: the map does not decide slots.
        assert_eq!(v.values, vec![ParamValue::Float(0.5), ParamValue::Int(8)]);
    }

    #[test]
    fn a_doctrine_that_does_not_fit_is_rejected() {
        let ir = decls();
        let full = || HashMap::from([("aggression".into(), 0.5), ("size".into(), 8.0)]);

        let mut missing = full();
        missing.remove("size");
        assert!(
            ParamValues::bind(&ir, &missing)
                .unwrap_err()
                .contains("no value for parameter `size`")
        );

        let mut extra = full();
        extra.insert("naval-weight".into(), 0.2);
        assert!(
            ParamValues::bind(&ir, &extra)
                .unwrap_err()
                .contains("not a parameter")
        );

        let mut fractional = full();
        fractional.insert("size".into(), 7.5);
        assert!(
            ParamValues::bind(&ir, &fractional)
                .unwrap_err()
                .contains("is an int")
        );
    }
}
