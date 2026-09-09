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
    /// Every keyword, for anything that needs the list rather than the lookup.
    ///
    /// `keywords_match_the_lookup` keeps the two in step.
    pub const KEYWORDS: &'static [&'static str] = &[
        "rule",
        "priority",
        "category",
        "exclusive",
        "do",
        "require",
        "because",
        "let",
        "and",
        "or",
        "not",
        "exists",
        "param",
        "def",
        "int",
        "float",
    ];

    /// The operators, longest first — which is the order a lexer must try them
    /// in and the order a regex alternation must list them in, so `<=` is not
    /// read as `<` followed by `=`.
    pub const OPERATORS: &'static [&'static str] =
        &["<=", ">=", "==", "!=", "<", ">", "+", "-", "*", "/", "="];

    /// Everything else with a fixed spelling.
    pub const PUNCTUATION: &'static [&'static str] = &["{", "}", "(", ")", ",", ":"];

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

    /// The vocabulary the lexer actually produces, against the tables that
    /// describe it. The dashboard builds its highlighting from those tables, so
    /// a spelling missing from them renders as plain text while still compiling.
    #[test]
    fn the_token_tables_cover_what_the_lexer_lexes() {
        use crate::lexer::lex;

        for spelling in TokenKind::OPERATORS
            .iter()
            .chain(TokenKind::PUNCTUATION)
            .chain(TokenKind::KEYWORDS)
        {
            let (tokens, diags) = lex(spelling);
            assert!(diags.is_empty(), "`{spelling}` does not lex: {diags:?}");
            // One token and the terminator: a spelling that lexes as two is one
            // the table has wrong, which is how `<=` would go astray.
            assert_eq!(
                tokens.len(),
                2,
                "`{spelling}` lexed as {} tokens",
                tokens.len() - 1
            );
        }

        // And the reverse: every punctuation byte the lexer accepts is listed.
        // `.` and `"` are absent on purpose — they only occur inside a float or
        // a string, never alone.
        for c in "{}(),:+-*/<>=".chars() {
            let s = c.to_string();
            assert!(
                TokenKind::OPERATORS.contains(&s.as_str())
                    || TokenKind::PUNCTUATION.contains(&s.as_str()),
                "the lexer accepts `{c}` but no table lists it"
            );
        }
    }

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
