//! Text to tokens.
//!
//! Collects errors rather than bailing, so one bad character does not hide the
//! rest of the file.
//!
//! The kebab-identifier rule — `ground-defense` is one token, `size - 1` is
//! three — is in `docs/design.md` under "Names".
use crate::diag::{Diagnostic, Span};
use crate::token::{Token, TokenKind};

pub fn lex(source: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    Lexer::new(source).run()
}

/// Borrows the source: a `Lexer` is dropped before `lex` returns. `SourceFile`
/// owns its text because it outlives the call.
struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    /// Byte offset of the token being scanned.
    start: usize,
    /// Byte offset of the next unconsumed byte.
    pos: usize,
    tokens: Vec<Token>,
    diags: Vec<Diagnostic>,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Lexer {
            src,
            bytes: src.as_bytes(),
            start: 0,
            pos: 0,
            tokens: Vec::new(),
            diags: Vec::new(),
        }
    }

    fn run(mut self) -> (Vec<Token>, Vec<Diagnostic>) {
        while !self.at_end() {
            self.start = self.pos;
            self.scan_token();
        }

        // Empty span, so an "unexpected end of file" diagnostic can point here.
        self.start = self.pos;
        self.push(TokenKind::Eof);

        (self.tokens, self.diags)
    }

    fn scan_token(&mut self) {
        let c = self.bytes[self.pos];

        // Maximal munch. Must run before the one-character arms, or `>=` lexes
        // as `>` then `=`.
        let pair = match (c, self.peek_at(self.pos + 1)) {
            (b'=', Some(b'=')) => Some(TokenKind::EqEq),
            (b'!', Some(b'=')) => Some(TokenKind::NotEq),
            (b'<', Some(b'=')) => Some(TokenKind::LtEq),
            (b'>', Some(b'=')) => Some(TokenKind::GtEq),
            _ => None,
        };
        if let Some(kind) = pair {
            self.pos += 2;
            self.push(kind);
            return;
        }

        match c {
            // No line tracking needed: spans are byte offsets.
            b' ' | b'\t' | b'\r' | b'\n' => {
                self.pos += 1;
            }

            b'(' => self.single(TokenKind::LParen),
            b')' => self.single(TokenKind::RParen),
            b'{' => self.single(TokenKind::LBrace),
            b'}' => self.single(TokenKind::RBrace),
            b',' => self.single(TokenKind::Comma),
            b'+' => self.single(TokenKind::Plus),
            b'*' => self.single(TokenKind::Asterisk),
            b'/' => self.single(TokenKind::Slash),
            b'<' => self.single(TokenKind::Lt),
            b'>' => self.single(TokenKind::Gt),

            b'=' => self.single(TokenKind::Eq),

            b'-' => self.single(TokenKind::Minus),

            b'0'..=b'9' => self.number(),

            c if c.is_ascii_alphabetic() => self.identifier(),

            b'"' => self.string(),

            _ => {
                self.pos += 1;
                let text = self.lexeme().to_string();
                self.error(format!("unexpected character `{text}`"));
            }
        }
    }

    fn number(&mut self) {
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.bump();
        }
        if self.peek() == Some(b'.')
            && self
                .peek_at(self.pos + 1)
                .is_some_and(|b| b.is_ascii_digit())
        {
            self.bump(); // the `.`
            while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                self.bump();
            }
        }
        let text = self.lexeme();
        let kind = if text.contains('.') {
            match text.parse::<f64>() {
                Ok(n) => TokenKind::Float(n),
                Err(_) => {
                    self.error(format!("`{text}` is not a valid number"));
                    TokenKind::Float(0.0)
                }
            }
        } else {
            match text.parse::<i64>() {
                Ok(n) => TokenKind::Number(n),
                Err(_) => {
                    self.error(format!("number `{text}` does not fit in a 64-bit integer"));
                    TokenKind::Number(0)
                }
            }
        };
        self.push(kind);
    }

    fn identifier(&mut self) {
        loop {
            match self.peek() {
                Some(b) if b.is_ascii_alphanumeric() => {
                    self.bump();
                }
                // Two bytes of lookahead, so it cannot fold into the arm
                // above. See docs/design.md, "Names".
                Some(b'-')
                    if self
                        .peek_at(self.pos + 1)
                        .is_some_and(|b| b.is_ascii_alphabetic()) =>
                {
                    self.bump();
                }
                _ => break,
            }
        }
        let text = self.lexeme();
        let kind =
            TokenKind::keyword(text).unwrap_or_else(|| TokenKind::Identifier(text.to_string()));
        self.push(kind);
    }

    fn string(&mut self) {
        self.bump(); // the opening quote

        while self.peek().is_some_and(|b| b != b'"') {
            self.bump();
        }

        if self.at_end() {
            self.error("unterminated string literal".to_string());
            let text = self.lexeme();
            self.push(TokenKind::Str(text[1..].to_string()));
            return;
        }

        self.bump(); // the closing quote

        let text = self.lexeme();
        let contents = text[1..text.len() - 1].to_string();
        self.push(TokenKind::Str(contents));
    }

    fn at_end(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn peek(&self) -> Option<u8> {
        self.peek_at(self.pos)
    }

    /// `Option` rather than indexing: a `!` on the last byte of the file would
    /// otherwise panic while checking whether a `=` follows.
    fn peek_at(&self, at: usize) -> Option<u8> {
        self.bytes.get(at).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }

    /// The source text of the token being scanned.
    ///
    /// Safe to slice: both ends were found by matching ASCII, which cannot occur
    /// inside a multi-byte sequence, so neither lands mid-character.
    fn lexeme(&self) -> &'a str {
        &self.src[self.start..self.pos]
    }

    /// Consumes one byte and pushes a token spanning it.
    fn single(&mut self, kind: TokenKind) {
        self.pos += 1;
        self.push(kind);
    }

    /// Pushes a token spanning `self.start..self.pos`.
    fn push(&mut self, kind: TokenKind) {
        self.tokens.push(Token {
            kind,
            span: Span {
                start: self.start as u32,
                end: self.pos as u32,
            },
        });
    }

    /// Records a diagnostic spanning `self.start..self.pos`.
    fn error(&mut self, message: String) {
        self.diags.push(Diagnostic {
            message,
            span: Span {
                start: self.start as u32,
                end: self.pos as u32,
            },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::TokenKind;

    /// Token kinds, `Eof` stripped. `emits_eof_at_end` is what pins `Eof` down;
    /// every other test here ignores it.
    fn kinds(src: &str) -> Vec<TokenKind> {
        let (toks, diags) = lex(src);
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
        toks.into_iter()
            .map(|t| t.kind)
            .filter(|k| *k != TokenKind::Eof)
            .collect()
    }

    /// Token kinds paired with their (start, end) byte offsets, `Eof` stripped.
    fn spans(src: &str) -> Vec<(TokenKind, u32, u32)> {
        let (toks, _) = lex(src);
        toks.into_iter()
            .filter(|t| t.kind != TokenKind::Eof)
            .map(|t| (t.kind, t.span.start, t.span.end))
            .collect()
    }

    fn ident(s: &str) -> TokenKind {
        TokenKind::Identifier(s.to_string())
    }

    // ---- trivial input ----

    #[test]
    fn empty_input_yields_nothing() {
        assert_eq!(kinds(""), vec![]);
    }

    #[test]
    fn whitespace_only_yields_nothing() {
        assert_eq!(kinds("   \n\t  \r\n "), vec![]);
    }

    // ---- numbers ----

    #[test]
    fn single_digit() {
        assert_eq!(kinds("7"), vec![TokenKind::Number(7)]);
    }

    #[test]
    fn multi_digit() {
        assert_eq!(kinds("2000"), vec![TokenKind::Number(2000)]);
    }

    #[test]
    fn oversized_integer_reports_instead_of_panicking() {
        // Bad input, not a compiler bug: it reports rather than unwraps.
        let (toks, diags) = lex("99999999999999999999");
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert_eq!(toks[0].kind, TokenKind::Number(0));
    }

    // ---- identifiers ----

    #[test]
    fn simple_identifier() {
        assert_eq!(kinds("cash"), vec![ident("cash")]);
    }

    #[test]
    fn identifier_may_contain_digits() {
        // `e1` is a real UnitType, so a digit straight after a letter stays in.
        assert_eq!(kinds("e1"), vec![ident("e1")]);
    }

    #[test]
    fn several_identifiers_separated_by_space() {
        assert_eq!(
            kinds("cash powr proc"),
            vec![ident("cash"), ident("powr"), ident("proc")]
        );
    }

    #[test]
    fn identifier_may_start_with_a_capital() {
        // Queue literals are capitalised by convention. The lexer doesn't care;
        // only the enum tables in types.rs do.
        assert_eq!(
            kinds("Building Infantry"),
            vec![ident("Building"), ident("Infantry")]
        );
    }

    #[test]
    fn keywords_are_case_sensitive() {
        // Case-insensitive matching would swallow capitalised enum literals.
        assert_eq!(kinds("Rule Require"), vec![ident("Rule"), ident("Require")]);
    }

    // ---- the kebab rule (docs/design.md, "Names") ----

    #[test]
    fn hyphen_between_letters_stays_in_the_identifier() {
        assert_eq!(kinds("ground-defense"), vec![ident("ground-defense")]);
    }

    #[test]
    fn multiple_hyphens() {
        assert_eq!(
            kinds("attack-known-base-ground"),
            vec![ident("attack-known-base-ground")]
        );
    }

    #[test]
    fn spaced_hyphen_is_subtraction() {
        assert_eq!(
            kinds("size - 1"),
            vec![ident("size"), TokenKind::Minus, TokenKind::Number(1)]
        );
    }

    #[test]
    fn hyphen_followed_by_digit_is_subtraction() {
        // No whitespace, but a digit follows, so the identifier ends.
        assert_eq!(
            kinds("size-1"),
            vec![ident("size"), TokenKind::Minus, TokenKind::Number(1)]
        );
    }

    #[test]
    fn hyphen_followed_by_letter_is_absorbed_even_when_wrong() {
        // The documented footgun: types.rs catches this, not the lexer.
        assert_eq!(kinds("size-one"), vec![ident("size-one")]);
    }

    #[test]
    fn hyphen_after_a_paren_is_subtraction() {
        // `)` is not an identifier character, so there is nothing to continue.
        assert_eq!(
            kinds("aircraft-capacity() - 1"),
            vec![
                ident("aircraft-capacity"),
                TokenKind::LParen,
                TokenKind::RParen,
                TokenKind::Minus,
                TokenKind::Number(1),
            ]
        );
    }

    #[test]
    fn leading_hyphen_is_an_operator() {
        assert_eq!(kinds("-5"), vec![TokenKind::Minus, TokenKind::Number(5)]);
    }

    // ---- operators and punctuation ----

    #[test]
    fn arithmetic_operators() {
        assert_eq!(
            kinds("+ - * /"),
            vec![
                TokenKind::Plus,
                TokenKind::Minus,
                TokenKind::Asterisk,
                TokenKind::Slash
            ]
        );
    }

    #[test]
    fn call_shaped_input() {
        assert_eq!(
            kinds("count(powr)"),
            vec![
                ident("count"),
                TokenKind::LParen,
                ident("powr"),
                TokenKind::RParen
            ]
        );
    }

    // ---- spans ----

    #[test]
    #[ignore = "stage 6"]
    fn spans_cover_exactly_the_token_text() {
        //                     0123456
        assert_eq!(
            spans("size-1"),
            vec![
                (ident("size"), 0, 4),
                (TokenKind::Minus, 4, 5),
                (TokenKind::Number(1), 5, 6),
            ]
        );
    }

    #[test]
    #[ignore = "stage 6"]
    fn spans_skip_leading_whitespace() {
        assert_eq!(spans("  cash"), vec![(ident("cash"), 2, 6)]);
    }

    #[test]
    #[ignore = "stage 6"]
    fn spans_are_byte_offsets_not_character_counts() {
        // The em-dash is three bytes, so `cash` starts at 4, not 2.
        assert_eq!(spans("\u{2014} cash"), vec![(ident("cash"), 4, 8)]);
    }

    // ---- error recovery ----

    #[test]
    fn unknown_character_reports_and_continues() {
        let (toks, diags) = lex("cash @ powr");
        assert_eq!(diags.len(), 1, "expected exactly one diagnostic: {diags:?}");
        assert_eq!(diags[0].span, crate::diag::Span { start: 5, end: 6 });
        // The point of collecting rather than bailing: `powr` still gets lexed.
        let ks: Vec<_> = toks
            .into_iter()
            .map(|t| t.kind)
            .filter(|k| *k != TokenKind::Eof)
            .collect();
        assert_eq!(ks, vec![ident("cash"), ident("powr")]);
    }

    #[test]
    fn several_unknown_characters_all_report() {
        let (_, diags) = lex("@ cash #");
        assert_eq!(
            diags.len(),
            2,
            "one diagnostic per bad character: {diags:?}"
        );
    }

    // ---- braces and commas ----

    #[test]
    fn braces_and_comma() {
        assert_eq!(
            kinds("{ , }"),
            vec![TokenKind::LBrace, TokenKind::Comma, TokenKind::RBrace]
        );
    }

    #[test]
    fn rule_header_shape() {
        assert_eq!(
            kinds("rule build-power {"),
            vec![TokenKind::Rule, ident("build-power"), TokenKind::LBrace]
        );
    }

    // ---- comparison operators ----

    #[test]
    fn two_character_comparisons() {
        assert_eq!(
            kinds("== != <= >="),
            vec![
                TokenKind::EqEq,
                TokenKind::NotEq,
                TokenKind::LtEq,
                TokenKind::GtEq
            ]
        );
    }

    #[test]
    fn bare_equals_is_its_own_token() {
        assert_eq!(
            kinds("let size = 5"),
            vec![
                TokenKind::Let,
                ident("size"),
                TokenKind::Eq,
                TokenKind::Number(5)
            ]
        );
    }

    #[test]
    fn double_equals_still_wins() {
        // Maximal munch: `==` must not lex as two `Eq`.
        assert_eq!(kinds("= =="), vec![TokenKind::Eq, TokenKind::EqEq]);
    }

    #[test]
    fn one_character_comparisons() {
        assert_eq!(kinds("< >"), vec![TokenKind::Lt, TokenKind::Gt]);
    }

    #[test]
    fn longest_match_wins() {
        // `>=` must not lex as `>` then `=`. Peek before committing to `Gt`.
        assert_eq!(
            kinds("cash >= 300"),
            vec![ident("cash"), TokenKind::GtEq, TokenKind::Number(300)]
        );
    }

    #[test]
    fn less_than_followed_by_a_number_is_not_lteq() {
        assert_eq!(
            kinds("count(e1) < 10"),
            vec![
                ident("count"),
                TokenKind::LParen,
                ident("e1"),
                TokenKind::RParen,
                TokenKind::Lt,
                TokenKind::Number(10),
            ]
        );
    }

    // ---- keywords ----

    #[test]
    fn keywords_are_their_own_kinds() {
        assert_eq!(
            kinds("rule priority category exclusive do require because let"),
            vec![
                TokenKind::Rule,
                TokenKind::Priority,
                TokenKind::Category,
                TokenKind::Exclusive,
                TokenKind::Do,
                TokenKind::Require,
                TokenKind::Because,
                TokenKind::Let,
            ]
        );
    }

    #[test]
    fn operator_keywords() {
        assert_eq!(
            kinds("and or not exists"),
            vec![
                TokenKind::And,
                TokenKind::Or,
                TokenKind::Not,
                TokenKind::Exists
            ]
        );
    }

    #[test]
    fn a_word_that_merely_starts_with_a_keyword_is_an_identifier() {
        // Scan the whole identifier, then look it up. Matching keyword text
        // directly would split `rules` into `rule` + `s`.
        assert_eq!(
            kinds("rules required nothing"),
            vec![ident("rules"), ident("required"), ident("nothing")]
        );
    }

    #[test]
    fn a_keyword_inside_a_kebab_name_is_not_a_keyword() {
        // `do` is a keyword, but `do-thing` is one identifier.
        assert_eq!(kinds("do-thing"), vec![ident("do-thing")]);
    }

    // ---- strings ----

    #[test]
    fn string_literal_holds_its_contents_without_quotes() {
        assert_eq!(
            kinds(r#"because "game 16""#),
            vec![TokenKind::Because, TokenKind::Str("game 16".to_string())]
        );
    }

    #[test]
    fn string_span_includes_both_quotes() {
        //     0123456
        assert_eq!(spans(r#""ab""#), vec![(TokenKind::Str("ab".into()), 0, 4)]);
    }

    #[test]
    fn string_may_hold_multibyte_characters() {
        assert_eq!(
            kinds("\"an em-dash \u{2014} here\""),
            vec![TokenKind::Str("an em-dash \u{2014} here".to_string())]
        );
    }

    #[test]
    fn unterminated_string_reports_and_does_not_hang() {
        let (_, diags) = lex("\"never closed");
        assert_eq!(diags.len(), 1, "expected one diagnostic: {diags:?}");
    }

    // ---- floats ----

    #[test]
    fn float_literal() {
        assert_eq!(kinds("0.7"), vec![TokenKind::Float(0.7)]);
    }

    #[test]
    fn integer_stays_an_integer() {
        // Only a `.` followed by a digit makes it a float.
        assert_eq!(kinds("300"), vec![TokenKind::Number(300)]);
    }

    #[test]
    fn float_in_context() {
        assert_eq!(
            kinds("squad-ready-ratio >= 0.7"),
            vec![
                ident("squad-ready-ratio"),
                TokenKind::GtEq,
                TokenKind::Float(0.7)
            ]
        );
    }

    // ---- end of input ----

    #[test]
    fn emits_eof_at_end() {
        let (toks, _) = lex("cash");
        let last = toks.last().expect("always at least an Eof");
        assert_eq!(last.kind, TokenKind::Eof);
        // Empty span at the end of the input, so an "unexpected end of file"
        // diagnostic has somewhere to point.
        assert_eq!(last.span, crate::diag::Span { start: 4, end: 4 });
    }

    #[test]
    fn empty_input_is_just_eof() {
        let (toks, _) = lex("");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].kind, TokenKind::Eof);
    }
}
