//! Tokens to tree. Hand-written recursive descent.
//!
//! Returns `(Ast, Vec<Diagnostic>)`, not `Result`. On failure, record the
//! diagnostic, skip to the next token that can start a construct, and carry on;
//! `ExprKind::Error` stands in for whatever failed to parse.
//!
//! Reasoning and the precedence table are in `docs/implementation.md`.

use crate::ast::{Ast, Expr, Let, Name, Rule};
use crate::diag::{Diagnostic, Span};
use crate::token::{Token, TokenKind};

pub fn parse(tokens: &[Token]) -> (Ast, Vec<Diagnostic>) {
    let mut parser = Parser::new(tokens);
    let ast = parser.parse();
    (ast, parser.diags)
}

/// Borrows the token slice: a `Parser` lives only for one `parse` call, so the
/// lifetime never escapes. Same reasoning as `Lexer`.
struct Parser<'a> {
    tokens: &'a [Token],
    /// Index of the next unconsumed token.
    pos: usize,
    diags: Vec<Diagnostic>,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Parser {
            tokens,
            pos: 0,
            diags: Vec::new(),
        }
    }

    /// `rule*` until `Eof`.
    fn parse(&mut self) -> Ast {
        let mut rules = Vec::new();
        while !self.at_end() {
            match self.rule() {
                Some(r) => rules.push(r),
                None => self.recover(),
            }
        }
        Ast { rules }
    }

    fn rule(&mut self) -> Option<Rule> {
        let start = self.peek_span();
        if !self.eat(&TokenKind::Rule) {
            self.error(start, "expected `rule`".into());
            return None;
        }

        let name = self.name()?;

        // A fresh span per check: `start` is where the rule began, not where
        // this token is, and a diagnostic has to point at what was actually
        // found.
        let brace = self.peek_span();
        if !self.eat(&TokenKind::LBrace) {
            self.error(brace, "expected `{` after the rule name".into());
            return None;
        }

        let mut priority: Option<i64> = None;
        let mut category: Option<Name> = None;
        let mut exclusive = false;
        let mut action: Option<Name> = None;
        let mut because: Option<String> = None;
        let mut lets: Vec<Let> = Vec::new();
        let mut requires: Vec<Expr> = Vec::new();

        // `at_end` guards against a missing `}` running off the end. Every arm
        // must consume at least one token, or this loop spins.
        while !self.at(&TokenKind::RBrace) && !self.at_end() {
            match self.peek() {
                TokenKind::Priority => {
                    let kw = self.peek_span();
                    self.bump(); // `priority`
                    if let Some(n) = self.number() {
                        if priority.is_some() {
                            self.error(kw, "duplicate `priority`".into());
                        }
                        priority = Some(n);
                    }
                }
                TokenKind::Category => {
                    let kw = self.peek_span();
                    self.bump(); // `category`
                    if let Some(n) = self.name() {
                        if category.is_some() {
                            self.error(kw, "duplicate `category`".into());
                        }
                        category = Some(n);
                        exclusive = self.eat(&TokenKind::Exclusive);
                    }
                }
                TokenKind::Do => {
                    let kw = self.peek_span();
                    self.bump(); // `do`
                    if let Some(n) = self.name() {
                        if action.is_some() {
                            self.error(kw, "duplicate `do`".into());
                        }
                        action = Some(n);
                    }
                }
                TokenKind::Because => {
                    let kw = self.peek_span();
                    self.bump(); // `because`
                    if let Some(text) = self.string_literal() {
                        if because.is_some() {
                            self.error(kw, "duplicate `because`".into());
                        }
                        because = Some(text);
                    }
                }
                TokenKind::Let => todo!("let NAME = expr"),
                TokenKind::Require => todo!("require expr"),
                _ => {
                    let span = self.peek_span();
                    self.error(span, "expected a rule field or `require`".into());
                    self.bump(); // progress: never leave this arm without consuming
                }
            }
        }

        let close = self.peek_span();
        if !self.eat(&TokenKind::RBrace) {
            self.error(close, "expected `}` to close the rule".into());
            return None;
        }

        // Missing fields are diagnostics rather than panics — a rule without a
        // priority is bad input, not a compiler bug.
        let priority = match priority {
            Some(p) => p,
            None => {
                self.error(name.span, "rule has no `priority`".into());
                return None;
            }
        };
        let category = match category {
            Some(c) => c,
            None => {
                self.error(name.span, "rule has no `category`".into());
                return None;
            }
        };
        let action = match action {
            Some(a) => a,
            None => {
                self.error(name.span, "rule has no `do`".into());
                return None;
            }
        };

        Some(Rule {
            name,
            priority,
            category,
            exclusive,
            action,
            because,
            lets,
            requires,
            span: start.to(close),
        })
    }

    // ---- expressions ----
    //
    // Precedence, loosest to tightest. Each level parses the one below it, then
    // loops while it sees an operator at its own level:
    //
    //   or
    //   and
    //   == != < <= > >=
    //   + -
    //   * /
    //   unary: not, -, exists
    //   primary: literal, name, call, ( ... )

    fn expr(&mut self) -> Expr {
        todo!("start at the loosest level")
    }

    // ---- cursor ----

    /// The next token's kind. Never `None`: the stream always ends with `Eof`,
    /// and `pos` never advances past it.
    fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn at(&self, kind: &TokenKind) -> bool {
        self.peek() == kind
    }

    fn at_end(&self) -> bool {
        self.at(&TokenKind::Eof)
    }

    /// The span of the next token — where a diagnostic about it should point.
    fn peek_span(&self) -> Span {
        self.tokens[self.pos].span
    }

    /// Consumes the next token and returns it.
    fn bump(&mut self) -> &'a Token {
        let t = &self.tokens[self.pos];
        if !self.at_end() {
            self.pos += 1;
        }
        t
    }

    /// Consumes the next token if it is `kind`, and reports whether it did.
    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    /// Consumes `kind` or reports a diagnostic. Does not skip anything —
    /// recovery is the caller's decision.
    fn expect(&mut self, kind: &TokenKind) -> bool {
        todo!("eat, or self.error(...) describing what was wanted")
    }

    /// Consumes an integer literal, or reports and returns `None`.
    ///
    /// No clone needed, unlike `string_literal`: `i64` is `Copy`, so it comes
    /// straight out of the borrowed token.
    fn number(&mut self) -> Option<i64> {
        let span = self.peek_span();
        if let TokenKind::Number(n) = *self.peek() {
            self.bump();
            Some(n)
        } else {
            self.error(span, "expected a number".into());
            None
        }
    }

    /// Consumes a string literal and returns its contents, or reports and
    /// returns `None`.
    ///
    /// Returns an owned `String` rather than a symbol: `because` text is unique
    /// prose that is stored and printed, never compared or looked up, so there
    /// is nothing for interning to save. Names are the opposite — see
    /// `docs/design.md`.
    fn string_literal(&mut self) -> Option<String> {
        let span = self.peek_span();
        // `Str` owns its contents and the token slice is borrowed, so the text
        // has to be cloned out rather than moved.
        if let TokenKind::Str(text) = self.peek() {
            let text = text.clone();
            self.bump();
            Some(text)
        } else {
            self.error(span, "expected a quoted string after `because`".into());
            None
        }
    }

    /// Consumes an identifier, or reports and returns `None`.
    fn name(&mut self) -> Option<Name> {
        todo!("TokenKind::Identifier becomes a Name carrying its span")
    }

    // ---- errors ----

    fn error(&mut self, span: Span, message: String) {
        self.diags.push(Diagnostic { message, span });
    }

    /// Panic-mode recovery: skip forward to a token that can legally start a
    /// construct, so one bad line does not cascade. See
    /// `docs/implementation.md`, "Error recovery".
    fn recover(&mut self) {
        todo!("skip to Rule / Require / Let / Do / Priority / Category / Because / RBrace / Eof")
    }
}
