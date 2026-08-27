//! The tree.
//!
//! Every node carries a `Span`. Shape is `struct Expr { kind, span }` with the
//! variants on `ExprKind` — one place for the span, clean matching on `.kind`.
//!
//! Rule fields and the rule/action namespace collision are in `docs/design.md`;
//! the AST shape and why spans have to be here from the start are in
//! `docs/implementation.md`.

use crate::diag::Span;

/// A whole source file: a list of rules.
#[derive(Debug)]
pub struct Ast {
    pub rules: Vec<Rule>,
}

/// A name as written, with where it was written.
///
/// Distinct from a bare `String` so that a rule name, a category and an enum
/// literal all keep their span — the type checker needs somewhere to point when
/// one of them does not resolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

#[derive(Debug)]
pub struct Rule {
    pub name: Name,
    pub priority: i64,
    pub category: Name,
    /// A modifier on `category`; absent means false.
    pub exclusive: bool,
    /// The action to run. Exactly one per rule.
    pub action: Name,
    /// Optional rationale. The language has no comments, so this is where the
    /// "why" lives — and unlike a comment it survives into an archived rule set.
    pub because: Option<String>,
    /// Rule-scoped bindings. Empty in v0; `let` is not implemented yet.
    pub lets: Vec<Let>,
    /// Implicitly ANDed. One `require` per line, no shorthand.
    pub requires: Vec<Expr>,
    /// The whole rule, `rule` keyword through closing brace.
    pub span: Span,
}

#[derive(Debug)]
pub struct Let {
    pub name: Name,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum ExprKind {
    Int(i64),
    Float(f64),

    /// A bare name: a zero-argument predicate, a `let` binding, or an enum
    /// literal. Which one is a question for `types`, not the parser.
    ///
    /// Deliberately not folded into `Call(name, vec![])` — the tree stays
    /// faithful to what was written, which matters when pointing a caret at it.
    Ident(Name),

    /// `queue-busy(Building)`, `can-build(Building, powr)`.
    Call(Name, Vec<Expr>),

    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),

    /// Stands in for whatever failed to parse, so the type checker can skip the
    /// hole rather than cascading fresh errors off it. Carries a span like any
    /// other node.
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
