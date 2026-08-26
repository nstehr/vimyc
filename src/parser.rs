//! Tokens to tree. Hand-written recursive descent.
//!
//! Returns `(Ast, Vec<Diagnostic>)`, not `Result`. On failure, record the
//! diagnostic, skip to the next token that can start a construct, and carry on;
//! `ExprKind::Error` stands in for whatever failed to parse.
//!
//! Decide that before writing the first function — it is a shape, not a feature,
//! and retrofitting it means rewriting every parse method. Reasoning and the
//! precedence table are in `docs/implementation.md`.

use crate::diag::Span;

pub struct Token {
    kind: TokenKind,
    span: Span,
}

pub enum TokenKind {
    Identifier(String),
    Number(i64),
    Plus,
    Minus,
    Asterisk,
    Slash,
    LParen,
    RParen,
}
