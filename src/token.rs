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
    /// Every keyword, for anything that needs the list rather than the lookup —
    /// the syntax highlighting in Vimy's dashboard is generated from this, so it
    /// cannot fall behind the language.
    ///
    /// `keywords_match_the_lookup` keeps the two in step.
    pub const KEYWORDS: &'static [&'static str] = &[
        "rule", "priority", "category", "exclusive", "do", "require", "because", "let", "and",
        "or", "not", "exists", "param", "def", "int", "float",
    ];

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
#[cfg(test)]
mod tests {
    use super::TokenKind;

    /// The list and the lookup are two spellings of one fact, and the list is
    /// what the dashboard's highlighting is built from — so a keyword added to
    /// one and not the other would show up as a word that stops being a keyword
    /// on screen while still being one to the compiler.
    #[test]
    fn keywords_match_the_lookup() {
        for k in TokenKind::KEYWORDS {
            assert!(
                TokenKind::keyword(k).is_some(),
                "`{k}` is listed but the lexer does not know it"
            );
        }
        // And nothing the lexer knows is missing from the list. There is no way
        // to enumerate the lookup, so this checks the words a `.vy` file may
        // contain: every keyword is lower-case ASCII, and the list is sorted by
        // nothing in particular, so length is the only cheap invariant left.
        assert_eq!(
            TokenKind::KEYWORDS.len(),
            16,
            "a keyword was added or removed; update KEYWORDS and this count"
        );
    }
}
