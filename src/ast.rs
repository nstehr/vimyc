//! The tree.
//!
//! Every node carries a `Span`. Rule fields and the rule/action namespace
//! collision are in `docs/design.md`; why spans are here from day one, rather
//! than retrofitted, is in `docs/implementation.md`.

use crate::diag::Span;

#[derive(Debug)]
pub struct Ast {
    /// Doctrine inputs, file-scoped and constant within a doctrine window.
    pub params: Vec<Param>,
    /// Named expressions, inlined at their call sites.
    pub defs: Vec<Def>,
    pub rules: Vec<Rule>,
}

/// A `def`: one expression, given a name and some arguments.
///
/// The language's only abstraction, and it exists for one shape — Go's
/// `buildCashCondition`, whose five conditional clauses appear at twenty-one
/// call sites differing only in a unit cost.
#[derive(Debug)]
pub struct Def {
    pub name: Name,
    pub params: Vec<DefParam>,
    pub body: Expr,
    pub span: Span,
}

#[derive(Debug)]
pub struct DefParam {
    pub name: Name,
    pub kind: ParamKind,
    pub span: Span,
}

/// A `param` declaration.
#[derive(Debug)]
pub struct Param {
    pub name: Name,
    pub kind: ParamKind,
    pub span: Span,
}

/// What a parameter holds. Numbers only — the `[]string` preferences a doctrine
/// also carries are consumed by `SetPreferences`, never by a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    Int,
    Float,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

#[derive(Debug)]
pub struct Rule {
    pub name: Name,
    /// An expression so a doctrine can set it, restricted by the checker to
    /// parameters, literals and `lerp` — see `docs/design.md`.
    pub priority: Expr,
    pub category: Name,
    /// A modifier on `category`; absent means false.
    pub exclusive: bool,
    /// The action to run. Exactly one per rule.
    pub action: Action,
    /// Why the rule exists. The language has no comments; unlike one, this
    /// survives into an archived rule set.
    pub because: Option<String>,
    /// Rule-scoped bindings.
    pub lets: Vec<Let>,
    /// Implicitly ANDed.
    pub requires: Vec<Expr>,
    pub span: Span,
}

/// What `do` names.
///
/// Arguments because eleven actions are built by a factory —
/// `form-squad(ground-attack, Ground, 8, Attack)` — and their arguments vary per
/// doctrine, so they cannot be fixed ids. `args` is empty for the rest.
#[derive(Debug)]
pub struct Action {
    pub name: Name,
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug)]
pub struct Let {
    pub name: Name,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Int(i64),
    Float(f64),

    /// A zero-argument predicate, a `let` binding, or an enum literal. Which
    /// one is `types`' problem, not the parser's.
    Ident(Name),

    /// `queue-busy(Building)`, `can-build(Building, powr)`.
    Call(Name, Vec<Expr>),

    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),

    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    /// `not`
    Not,
    /// Unary `-`
    Neg,
    /// `exists nearest-enemy` — true when an optional value is present.
    Exists,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    And,
    Or,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Add,
    Sub,
    Mul,
    Div,
}
