//! The tree.
//!
//! Every node carries a `Span`. Shape is `struct Expr { kind, span }` with the
//! variants on `ExprKind` — one place for the span, clean matching on `.kind`.
//!
//! Rule fields and the rule/action namespace collision are in `docs/design.md`;
//! the AST shape and why spans have to be here from the start are in
//! `docs/implementation.md`.
