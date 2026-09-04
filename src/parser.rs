//! Tokens to tree. Hand-written recursive descent.
//!
//! Collects errors rather than bailing, so one bad construct does not hide the
//! rest of the file. `ExprKind::Error` stands in for whatever failed to parse,
//! and later stages skip those nodes instead of cascading off them.
//!
//! Reasoning is in `docs/implementation.md`.

use crate::ast::{Action, Ast, BinOp, Expr, ExprKind, Let, Name, Rule, UnOp};
use crate::diag::{Diagnostic, Span};
use crate::token::{Token, TokenKind};

pub fn parse(tokens: &[Token]) -> (Ast, Vec<Diagnostic>) {
    let mut parser = Parser::new(tokens);
    let ast = parser.parse();
    (ast, parser.diags)
}

/// Borrows the token slice: a `Parser` lives only for one `parse` call.
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

        let brace = self.peek_span();
        if !self.eat(&TokenKind::LBrace) {
            self.error(brace, "expected `{` after the rule name".into());
            return None;
        }

        let mut priority: Option<i64> = None;
        let mut category: Option<Name> = None;
        let mut exclusive = false;
        let mut action: Option<Action> = None;
        let mut because: Option<String> = None;
        let mut lets: Vec<Let> = Vec::new();
        let mut requires: Vec<Expr> = Vec::new();

        // Every arm must consume at least one token, or this loop spins.
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
                    if let Some(a) = self.action() {
                        if action.is_some() {
                            self.error(kw, "duplicate `do`".into());
                        }
                        action = Some(a);
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
                TokenKind::Let => {
                    let kw = self.peek_span();
                    self.bump(); // `let`
                    if let Some(name) = self.name() {
                        self.expect(&TokenKind::Eq);
                        let value = self.expr();
                        lets.push(Let {
                            span: kw.to(value.span),
                            name,
                            value,
                        });
                    }
                }
                TokenKind::Require => {
                    self.bump(); // `require`
                    requires.push(self.expr());
                }
                _ => {
                    let span = self.peek_span();
                    self.error(span, "expected a rule field or `require`".into());
                    self.bump(); // progress
                }
            }
        }

        let close = self.peek_span();
        if !self.eat(&TokenKind::RBrace) {
            self.error(close, "expected `}` to close the rule".into());
            return None;
        }

        // Bad input, not a compiler bug, so these report rather than panic.
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
        self.or_expr()
    }

    fn or_expr(&mut self) -> Expr {
        let mut left = self.and_expr();
        while self.eat(&TokenKind::Or) {
            let right = self.and_expr();
            left = binary(BinOp::Or, left, right);
        }
        left
    }

    fn and_expr(&mut self) -> Expr {
        let mut left = self.cmp_expr();
        while self.eat(&TokenKind::And) {
            let right = self.cmp_expr();
            left = binary(BinOp::And, left, right);
        }
        left
    }

    fn cmp_expr(&mut self) -> Expr {
        let mut left = self.add_expr();
        loop {
            let op = match self.peek() {
                TokenKind::EqEq => BinOp::Eq,
                TokenKind::NotEq => BinOp::NotEq,
                TokenKind::Lt => BinOp::Lt,
                TokenKind::LtEq => BinOp::LtEq,
                TokenKind::Gt => BinOp::Gt,
                TokenKind::GtEq => BinOp::GtEq,
                _ => break,
            };
            self.bump();
            let right = self.add_expr();
            left = binary(op, left, right);
        }
        left
    }

    fn add_expr(&mut self) -> Expr {
        let mut left = self.mul_expr();
        loop {
            let op = match self.peek() {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let right = self.mul_expr();
            left = binary(op, left, right);
        }
        left
    }

    fn mul_expr(&mut self) -> Expr {
        let mut left = self.unary();
        loop {
            let op = match self.peek() {
                TokenKind::Asterisk => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                _ => break,
            };
            self.bump();
            let right = self.unary();
            left = binary(op, left, right);
        }
        left
    }

    /// Right-recursive so `not not x` nests — prefix operators have nothing to
    /// be left-associative about.
    fn unary(&mut self) -> Expr {
        let start = self.peek_span();
        let op = match self.peek() {
            TokenKind::Not => UnOp::Not,
            TokenKind::Minus => UnOp::Neg,
            TokenKind::Exists => UnOp::Exists,
            _ => return self.primary(),
        };
        self.bump();
        let operand = self.unary();
        Expr {
            span: start.to(operand.span),
            kind: ExprKind::Unary(op, Box::new(operand)),
        }
    }

    /// Where the ladder bottoms out. A parenthesised expression restarts it from
    /// the loosest level, which is why parentheses override precedence.
    fn primary(&mut self) -> Expr {
        let span = self.peek_span();
        match *self.peek() {
            TokenKind::Number(n) => {
                self.bump();
                Expr {
                    kind: ExprKind::Int(n),
                    span,
                }
            }
            TokenKind::Float(f) => {
                self.bump();
                Expr {
                    kind: ExprKind::Float(f),
                    span,
                }
            }
            TokenKind::LParen => {
                self.bump();
                let inner = self.expr();
                let close = self.peek_span();
                if !self.eat(&TokenKind::RParen) {
                    self.error(close, "expected `)`".into());
                    return error_expr(span.to(close));
                }
                Expr {
                    kind: inner.kind,
                    span: span.to(close),
                }
            }
            TokenKind::Identifier(_) => self.name_or_call(),
            _ => {
                self.error(span, "expected an expression".into());
                self.bump(); // progress, so a bad token cannot stall a loop
                error_expr(span)
            }
        }
    }

    /// A bare name, or a call if a `(` follows. Kept distinct in the tree — see
    /// `docs/implementation.md`.
    fn name_or_call(&mut self) -> Expr {
        let Some(name) = self.name() else {
            let span = self.peek_span();
            return error_expr(span);
        };

        if !self.at(&TokenKind::LParen) {
            return Expr {
                span: name.span,
                kind: ExprKind::Ident(name),
            };
        }

        self.bump(); // `(`
        let mut args = Vec::new();
        if !self.at(&TokenKind::RParen) {
            loop {
                args.push(self.expr());
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
                // A trailing comma ends the list rather than demanding another.
                if self.at(&TokenKind::RParen) || self.at_end() {
                    break;
                }
            }
        }

        let close = self.peek_span();
        let start = name.span;
        if !self.eat(&TokenKind::RParen) {
            self.error(close, "expected `)` to close the argument list".into());
            return error_expr(start.to(close));
        }

        Expr {
            span: start.to(close),
            kind: ExprKind::Call(name, args),
        }
    }

    // ---- cursor ----

    /// Cannot go out of bounds: the stream ends with `Eof` and `bump` never
    /// advances past it.
    fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn at(&self, kind: &TokenKind) -> bool {
        self.peek() == kind
    }

    fn at_end(&self) -> bool {
        self.at(&TokenKind::Eof)
    }

    /// Where a diagnostic about the next token should point.
    fn peek_span(&self) -> Span {
        self.tokens[self.pos].span
    }

    /// Consumes the next token, clamping at `Eof` so a truncated file cannot
    /// run the cursor off the end.
    fn bump(&mut self) -> &'a Token {
        let t = &self.tokens[self.pos];
        if !self.at_end() {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    /// Reports on a mismatch but skips nothing: recovery is the caller's job.
    fn expect(&mut self, kind: &TokenKind) -> bool {
        if self.eat(kind) {
            return true;
        }
        let span = self.peek_span();
        self.error(span, format!("expected {kind:?}"));
        false
    }

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

    /// An action name, with arguments when it is built by a factory.
    fn action(&mut self) -> Option<Action> {
        let name = self.name()?;
        let start = name.span;
        if !self.at(&TokenKind::LParen) {
            return Some(Action {
                span: start,
                name,
                args: Vec::new(),
            });
        }

        self.bump(); // `(`
        let mut args = Vec::new();
        if !self.at(&TokenKind::RParen) {
            loop {
                args.push(self.expr());
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
                if self.at(&TokenKind::RParen) || self.at_end() {
                    break;
                }
            }
        }
        let close = self.peek_span();
        if !self.eat(&TokenKind::RParen) {
            self.error(close, "expected `)` to close the argument list".into());
        }
        Some(Action {
            span: start.to(close),
            name,
            args,
        })
    }

    /// Returns an owned `String` rather than an interned symbol: `because` text
    /// is prose that gets stored and printed, never compared or looked up.
    fn string_literal(&mut self) -> Option<String> {
        let span = self.peek_span();
        if let TokenKind::Str(text) = self.peek() {
            let text = text.clone();
            self.bump();
            Some(text)
        } else {
            self.error(span, "expected a quoted string after `because`".into());
            None
        }
    }

    fn name(&mut self) -> Option<Name> {
        let span = self.peek_span();
        if let TokenKind::Identifier(text) = self.peek() {
            let text = text.clone();
            self.bump();
            Some(Name { text, span })
        } else {
            self.error(span, "expected a name".into());
            None
        }
    }

    // ---- errors ----

    fn error(&mut self, span: Span, message: String) {
        self.diags.push(Diagnostic::error(span, message));
    }

    /// Panic-mode recovery: skip to the next `rule`. Only `parse` calls this,
    /// and `rule` is the only token that can start a top-level construct. A rule
    /// body needs a different break set, but its loop handles errors inline and
    /// never comes through here. See `docs/implementation.md`, "Error recovery".
    fn recover(&mut self) {
        // Deliberately no unconditional bump first: that would swallow the very
        // rule this is aiming for. Progress is still guaranteed, because `rule`
        // returns `None` on a `rule` token only after consuming it.
        while !self.at_end() && !self.at(&TokenKind::Rule) {
            self.bump();
        }
    }
}

/// Folds two operands into a binary node spanning both.
///
/// A free function, not a method: `&mut self` would clash with operands already
/// moved out of the parser.
fn binary(op: BinOp, left: Expr, right: Expr) -> Expr {
    Expr {
        // Must precede `kind`, which moves the operands. Struct fields evaluate
        // top to bottom.
        span: left.span.to(right.span),
        kind: ExprKind::Binary(op, Box::new(left), Box::new(right)),
    }
}

/// Stands in for an expression that failed to parse, so later stages can skip
/// the hole rather than cascading fresh errors off it.
fn error_expr(span: Span) -> Expr {
    Expr {
        kind: ExprKind::Error,
        span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn parse_str(src: &str) -> (Ast, Vec<Diagnostic>) {
        let (toks, lex_diags) = lex(src);
        assert!(lex_diags.is_empty(), "lexing failed: {lex_diags:?}");
        parse(&toks)
    }

    // Both of these hung at one point. They assert little; what they guard
    // against is the parser looping forever, which no other test would catch.

    #[test]
    fn junk_at_top_level_terminates() {
        let (ast, diags) = parse_str("require cash >= 300\n");
        assert!(!diags.is_empty());
        assert!(ast.rules.is_empty());
    }

    #[test]
    fn stray_brace_terminates() {
        let (_, diags) = parse_str("}\n");
        assert!(!diags.is_empty());
    }

    #[test]
    fn recovers_to_the_next_rule() {
        let (ast, diags) = parse_str(
            "garbage here\nrule build-power {\n  priority 800\n  category economy\n  do produce-power-plant\n  require cash >= 300\n}\n",
        );
        assert!(!diags.is_empty(), "the junk should be reported");
        assert_eq!(ast.rules.len(), 1, "the rule after it should still parse");
        assert_eq!(ast.rules[0].name.text, "build-power");
    }

    #[test]
    fn a_rule_missing_a_field_does_not_eat_the_next_one() {
        let (ast, diags) = parse_str(
            "rule broken {\n  category economy\n  do thing\n  require cash >= 300\n}\n\nrule after-it {\n  priority 700\n  category economy\n  do other\n  require cash >= 100\n}\n",
        );
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert_eq!(
            ast.rules.len(),
            1,
            "the rule after a broken one must still parse"
        );
        assert_eq!(ast.rules[0].name.text, "after-it");
    }
}
