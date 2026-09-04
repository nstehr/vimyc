//! What the lexer produces and the parser consumes.
//!
//! Its own module so neither of them has to depend on the other.

use crate::diag::Span;

#[derive(Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// `PartialEq` but no `Eq`, because `Float` holds an `f64` and `NaN != NaN`.
/// The parser only ever compares kinds, so the cost is not being able to use one
/// as a `HashMap` key.
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
    Param,
    Def,
    /// The type in a parameter declaration. Named apart from `Int`/`Float`,
    /// which are literals.
    IntType,
    FloatType,

    // punctuation
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Colon,

    // arithmetic
    Plus,
    Minus,
    Asterisk,
    Slash,

    /// Bare `=`, as in `let size = 5`. Distinct from `EqEq`.
    Eq,

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
            "param" => TokenKind::Param,
            "def" => TokenKind::Def,
            "int" => TokenKind::IntType,
            "float" => TokenKind::FloatType,
            _ => return None,
        })
    }
}
