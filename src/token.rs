//! What the lexer produces and the parser consumes.
//!
//! Its own module rather than living with either: the lexer emits these and the
//! parser reads them, so putting them in one would make that module a dependency
//! of the other for no reason.
//!
//! Every token carries a `Span`. See `docs/design.md` under "Names" for the
//! kebab-identifier rule, which is the one place tokenising is not obvious.

use crate::diag::Span;

#[derive(Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, PartialEq, Eq)]
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
