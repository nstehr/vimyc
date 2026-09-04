//! The lowered form: a checked rule set with every name resolved.
//!
//! Sits between `check` and the things that consume a rule set — the evaluator
//! and the emitters — so resolution happens once instead of once per consumer.
//! `docs/implementation.md` covers why under "The IR".
//!
//! There is no `Ident` and no `Error` here, deliberately. A backend cannot
//! forget to handle an unresolved name because there are none, and cannot be
//! handed a tree that failed to check.

use crate::diag::Span;
use crate::env::Predicate;
use crate::types::Domain;

/// A whole rule set, in priority order.
#[derive(Debug)]
pub struct Ir {
    pub rules: Vec<IrRule>,
}

#[derive(Debug)]
pub struct IrRule {
    /// Output only — emitted, never compared, so no reason to intern.
    pub name: String,
    pub priority: i64,
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

    Unary(crate::ast::UnOp, Box<IrExpr>),
    Binary(crate::ast::BinOp, Box<IrExpr>, Box<IrExpr>),
}
