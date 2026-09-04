//! The lowered form: a checked rule set with every name resolved once, rather
//! than once per consumer. `docs/implementation.md` covers why under "The IR".
//!
//! No `Ident` and no `Error`, deliberately: a backend cannot forget to handle an
//! unresolved name when there are none.

use crate::ast::ParamKind;
use crate::diag::Span;
use crate::env::Predicate;
use crate::types::Domain;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Ir {
    /// In declaration order, which is the order `IrExpr::Param` indexes.
    pub params: Vec<IrParam>,
    pub rules: Vec<IrRule>,
}

#[derive(Debug)]
pub struct IrParam {
    pub name: String,
    pub kind: ParamKind,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParamValue {
    Int(i64),
    Float(f64),
}

/// A doctrine's numbers, positional so a lookup is an index.
#[derive(Debug, Default, Clone)]
pub struct ParamValues {
    pub values: Vec<ParamValue>,
}

impl ParamValues {
    /// Orders a doctrine's numbers against what the rule set declares.
    ///
    /// Trusting the caller to order them would make a wrong threshold
    /// indistinguishable from a right one.
    pub fn bind(ir: &Ir, supplied: &HashMap<String, f64>) -> Result<Self, String> {
        let mut values = Vec::with_capacity(ir.params.len());
        for p in &ir.params {
            let Some(&n) = supplied.get(&p.name) else {
                return Err(format!("no value for parameter `{}`", p.name));
            };
            values.push(match p.kind {
                ParamKind::Float if !n.is_finite() => {
                    return Err(format!("parameter `{}` is not a number: {n}", p.name));
                }
                ParamKind::Float => ParamValue::Float(n),
                // Rejected rather than rounded: `lerp(200, 400, 0.5)` and a
                // group size of 7.5 are different kinds of mistake, and only one
                // of them is the caller's intent.
                ParamKind::Int if n.fract() != 0.0 => {
                    return Err(format!("parameter `{}` is an int, got {n}", p.name));
                }
                // `as` saturates rather than failing, so an out-of-range value
                // would silently become i64::MAX.
                ParamKind::Int if !n.is_finite() || n < i64::MIN as f64 || n > i64::MAX as f64 => {
                    return Err(format!("parameter `{}` is out of range: {n}", p.name));
                }
                ParamKind::Int => ParamValue::Int(n as i64),
            });
        }
        // Extra values are not an error: a doctrine carries thirty numbers and
        // any one rule set uses a handful. A misspelling still surfaces, as the
        // parameter it meant to supply going missing.
        Ok(ParamValues { values })
    }
}

#[derive(Debug)]
pub struct IrRule {
    /// Output only — emitted, never compared, so no reason to intern.
    pub name: String,
    /// Resolved once per doctrine, not per tick: the engine sorts on it.
    pub priority: IrExpr,
    pub category: CategoryId,
    pub exclusive: bool,
    /// Why the rule exists. Reaches the dashboard; see `docs/design.md`.
    pub because: Option<String>,
    pub action: IrAction,
    /// Bindings by slot, in declaration order. `IrExpr::Binding` indexes this.
    pub lets: Vec<IrExpr>,
    /// Implicitly ANDed.
    pub requires: Vec<IrExpr>,
    /// Kept apart from `span` so a whole-set diagnostic can point at the name
    /// rather than underlining the entire rule.
    pub name_span: Span,
    pub span: Span,
}

/// An index into `env::CATEGORIES`. Interned because exclusivity groups by
/// category once per rule per tick.
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
    /// Only ever produced by folding: the language has no boolean literal.
    Bool(bool),

    /// `count` never survives lowering — it becomes whichever of the three
    /// predicates it meant.
    Predicate(Predicate, Vec<IrExpr>),

    /// An enum literal, as an index into its domain's table.
    Member(Domain, u32),

    /// By slot into `IrRule::lets`.
    Binding(u32),
    /// By slot into `Ir::params`.
    Param(u32),

    /// Distinct from `Predicate` because reading no state is what makes it
    /// legal in a priority.
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

    /// `as i64` saturates rather than failing, so an out-of-range value would
    /// silently arrive as i64::MAX.
    #[test]
    fn a_number_an_int_cannot_hold_is_rejected() {
        let ir = decls();
        for bad in [1e30, -1e30, f64::INFINITY, f64::NAN] {
            let supplied = HashMap::from([("aggression".into(), 0.5), ("size".into(), bad)]);
            let err = ParamValues::bind(&ir, &supplied).unwrap_err();
            assert!(
                err.contains("out of range") || err.contains("is an int"),
                "{bad}: {err}"
            );
        }
        // A float parameter has its own hole: not every f64 is a number.
        let supplied = HashMap::from([("aggression".into(), f64::NAN), ("size".into(), 8.0)]);
        assert!(
            ParamValues::bind(&ir, &supplied)
                .unwrap_err()
                .contains("not a number")
        );
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

        // A value the rule set does not declare is ignored, not rejected.
        let mut extra = full();
        extra.insert("naval-weight".into(), 0.2);
        assert!(ParamValues::bind(&ir, &extra).is_ok());

        let mut fractional = full();
        fractional.insert("size".into(), 7.5);
        assert!(
            ParamValues::bind(&ir, &fractional)
                .unwrap_err()
                .contains("is an int")
        );
    }
}
