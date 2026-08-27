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

/// Note the absence of `Eq`: `Float` holds an `f64`, and `NaN != NaN` means
/// floats are only `PartialEq`. `==` still works, which is all the parser needs;
/// what is lost is using a `TokenKind` as a `HashMap` key.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // literals and names
    Identifier(String),
    Number(i64),
    Float(f64),
    /// Contents only — the surrounding quotes are not kept.
    Str(String),

    // keywords
    Rule,
    Priority,
    Category,
    Exclusive,
    Do,
    Require,
    Because,
    Let,
    And,
    Or,
    Not,
    Exists,

    // punctuation
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,

    // arithmetic
    Plus,
    Minus,
    Asterisk,
    Slash,

    // comparison
    EqEq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,

    Eof,
}

impl TokenKind {
    /// Maps an already-scanned identifier to its keyword kind, if it is one.
    pub fn keyword(s: &str) -> Option<TokenKind> {
        Some(match s {
            "rule" => TokenKind::Rule,
            "priority" => TokenKind::Priority,
            "category" => TokenKind::Category,
            "exclusive" => TokenKind::Exclusive,
            "do" => TokenKind::Do,
            "require" => TokenKind::Require,
            "because" => TokenKind::Because,
            "let" => TokenKind::Let,
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "not" => TokenKind::Not,
            "exists" => TokenKind::Exists,
            _ => return None,
        })
    }
}
