//! Type checking.
//!
//! Bidirectional: most nodes are *synthesised* bottom-up, but enum literals have
//! no type of their own and must be *checked* against what a parameter expects.
//! `docs/design.md` covers the resolution rules under "Types".

use crate::ast::{Ast, BinOp, Expr, ExprKind, Name, Rule, UnOp};
use crate::diag::{Diagnostic, Span};
use crate::env;
use crate::types::{ParamType, Type};

/// Collects rather than bailing, so one bad rule does not hide the rest.
pub fn check(ast: &Ast) -> Vec<Diagnostic> {
    let mut checker = Checker::new();
    checker.run(ast);
    checker.diags
}

/// Cross-rule state. Anything scoped to a single rule lives on `RuleChecker`
/// instead, so it cannot outlive the rule it belongs to.
struct Checker {
    diags: Vec<Diagnostic>,
}

/// One rule's checking pass.
///
/// The scope lives here rather than on `Checker` so a stale binding is
/// impossible: it is born and dropped with the rule, leaving no `clear()` to
/// forget. Diagnostics are borrowed because they outlive any one rule.
struct RuleChecker<'a> {
    diags: &'a mut Vec<Diagnostic>,
    /// A `Vec` because there are no nested scopes and a handful of bindings.
    scope: Vec<(String, Type)>,
}

impl Checker {
    fn new() -> Self {
        Checker { diags: Vec::new() }
    }

    fn run(&mut self, ast: &Ast) {
        for rule in &ast.rules {
            RuleChecker {
                diags: &mut self.diags,
                scope: Vec::new(),
            }
            .rule(rule);
        }
    }
}

impl<'a> RuleChecker<'a> {
    fn rule(&mut self, rule: &Rule) {
        self.scope.clear();

        for binding in &rule.lets {
            let ty = self.synth(&binding.value);
            self.bind(&binding.name, ty);
        }

        for require in &rule.requires {
            self.check_expr(require, &Type::Bool);
        }

        // Closed tables, not types: a misspelled category would otherwise make a
        // new exclusivity group and let two rules fire that meant to exclude
        // each other.
        if !env::is_category(&rule.category.text) {
            let msg = format!("unknown category `{}`", rule.category.text);
            self.error(rule.category.span, msg);
        }
        if !env::is_action(&rule.action.text) {
            let msg = format!("unknown action `{}`", rule.action.text);
            self.error(rule.action.span, msg);
        }
    }

    /// Adds a `let` binding, rejecting anything that would shadow.
    ///
    /// A rule where `cash` means something other than cash is the confusion this
    /// language exists to prevent.
    fn bind(&mut self, name: &Name, ty: Type) {
        if env::predicate(&name.text).is_some() || env::collection(&name.text).is_some() {
            let msg = format!("`{}` is a predicate and cannot be rebound", name.text);
            self.error(name.span, msg);
            return;
        }
        // Enum literals resolve by position, not through scope — so `powr`
        // would mean the binding in `cash > powr` and the building in
        // `count(powr)`. One name, two meanings, one rule.
        let domains = env::domains_containing(&name.text);
        if let Some(d) = domains.first() {
            let msg = format!("`{}` is a {} and cannot be rebound", name.text, d.name());
            self.error(name.span, msg);
            return;
        }
        if self.scope.iter().any(|(n, _)| *n == name.text) {
            let msg = format!("`{}` is already bound in this rule", name.text);
            self.error(name.span, msg);
            return;
        }
        self.scope.push((name.text.clone(), ty));
    }

    // ---- expressions ----

    /// Bottom-up: the type this expression has.
    ///
    /// Returns `Type::Error` on failure, having already reported. `Error` is
    /// compatible with everything, so one mistake yields one message rather than
    /// cascading through every enclosing expression.
    fn synth(&mut self, e: &Expr) -> Type {
        match &e.kind {
            ExprKind::Int(_) => Type::Int,
            ExprKind::Float(_) => Type::Float,
            ExprKind::Ident(name) => self.ident(&name.text, name.span),
            // The parser already reported.
            ExprKind::Error => Type::Error,
            ExprKind::Call(n, args) => {
                if n.text == "count" {
                    self.count(args, n.span)
                } else if let Some(sig) = env::predicate(&n.text) {
                    if args.len() != sig.params.len() {
                        let msg = format!(
                            "`{}` expects {} argument(s), got {}",
                            n.text,
                            sig.params.len(),
                            args.len()
                        );
                        self.error(n.span, msg);
                        // Arguments left unchecked on purpose: pairing them
                        // against parameters at the wrong count reports about
                        // ones that are merely in the wrong slot.
                        return Type::Error;
                    }
                    for (arg, want) in args.iter().zip(sig.params.iter()) {
                        self.check_arg(arg, want);
                    }
                    sig.ret.clone()
                } else {
                    let msg = format!("unknown predicate `{}`", n.text);
                    self.error(n.span, msg);
                    Type::Error
                }
            }
            ExprKind::Unary(op, operand) => match op {
                UnOp::Not => {
                    self.check_expr(operand, &Type::Bool);
                    Type::Bool
                }
                UnOp::Neg => {
                    let t = self.synth(operand);
                    if t.is_numeric() {
                        t
                    } else {
                        let msg = format!("cannot negate {t}");
                        self.error(operand.span, msg);
                        Type::Error
                    }
                }
                // Why optionals are their own type: expr lets
                // `nearest-enemy` stand in for a bool, and this does not.
                UnOp::Exists => match self.synth(operand) {
                    Type::Option(_) => Type::Bool,
                    Type::Error => Type::Error,
                    t => {
                        let msg = format!("`exists` needs an optional value, found {t}");
                        self.error(operand.span, msg);
                        Type::Error
                    }
                },
            },

            ExprKind::Binary(op, left, right) => match op {
                BinOp::And | BinOp::Or => {
                    self.check_expr(left, &Type::Bool);
                    self.check_expr(right, &Type::Bool);
                    Type::Bool
                }

                BinOp::Eq | BinOp::NotEq | BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq => {
                    let lt = self.synth(left);
                    let rt = self.synth(right);
                    if matches!(lt, Type::Option(_)) || matches!(rt, Type::Option(_)) {
                        self.error(e.span, "use `exists` to test an optional value".to_string());
                    } else if !lt.compatible(&rt) {
                        let msg = format!("cannot compare {lt} with {rt}");
                        self.error(e.span, msg);
                    }
                    // `Bool` even after an error — a comparison is a bool
                    // whatever its operands were, and saying so keeps one
                    // mistake to one message.
                    Type::Bool
                }

                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                    let lt = self.synth(left);
                    let rt = self.synth(right);
                    if !lt.is_numeric() || !rt.is_numeric() {
                        let msg = format!("arithmetic needs numbers, found {lt} and {rt}");
                        self.error(e.span, msg);
                        return Type::Error;
                    }
                    // Widening, so `cash * 0.5` works once doctrine parameters
                    // exist.
                    if lt == Type::Float || rt == Type::Float {
                        Type::Float
                    } else {
                        Type::Int
                    }
                }
            },
        }
    }

    /// Top-down: whether this expression fits an expected type.
    ///
    /// Bare enum literals cannot be synthesised — `powr` has no type until a
    /// parameter says it is a building.
    fn check_expr(&mut self, e: &Expr, want: &Type) {
        if let (Type::Enum(domain), ExprKind::Ident(name)) = (want, &e.kind) {
            if env::member(*domain, &name.text).is_none() {
                let msg = format!("unknown {}: `{}`", domain.name(), name.text);
                let note = suggest(&name.text, env::members(*domain).iter().map(|m| m.name));
                self.error(name.span, msg + &note);
            }
            return;
        }

        let got = self.synth(e);
        if !got.compatible(want) {
            let msg = format!("expected {want}, found {got}");
            self.error(e.span, msg);
        }
    }

    /// An argument, whose expected type may span several domains.
    fn check_arg(&mut self, e: &Expr, want: &ParamType) {
        let ParamType::Exact(t) = want else {
            // `AnyOf` only occurs in enum positions, so this must be a name.
            let ExprKind::Ident(name) = &e.kind else {
                let got = self.synth(e);
                let msg = format!("expected {want}, found {got}");
                self.error(e.span, msg);
                return;
            };
            if env::resolve_param_member(want, &name.text).is_err() {
                let msg = format!("unknown {want}: `{}`", name.text);
                let candidates: Vec<&str> = match want {
                    ParamType::AnyOf(ds) => ds
                        .iter()
                        .flat_map(|d| env::members(*d).iter().map(|m| m.name))
                        .collect(),
                    ParamType::Exact(_) => Vec::new(),
                };
                let note = suggest(&name.text, candidates.into_iter());
                self.error(name.span, msg + &note);
            }
            return;
        };
        self.check_expr(e, t);
    }

    /// A bare name: a `let` binding, a zero-argument predicate, or a collection.
    fn ident(&mut self, name: &str, span: Span) -> Type {
        if let Some((_, ty)) = self.scope.iter().find(|(n, _)| n == name) {
            return ty.clone();
        }

        if let Some(sig) = env::predicate(name) {
            if sig.params.is_empty() {
                return sig.ret.clone();
            }
            let msg = format!(
                "`{name}` takes {} argument(s); write `{name}(...)`",
                sig.params.len()
            );
            self.error(span, msg);
            return Type::Error;
        }

        if env::collection(name).is_some() {
            return Type::Collection;
        }

        // Outside an argument position there is nothing to say which domain an
        // enum literal belongs to.
        let domains = env::domains_containing(name);
        if let Some(d) = domains.first() {
            let msg = format!(
                "`{name}` is a {} and only means something as an argument",
                d.name()
            );
            self.error(span, msg);
            return Type::Error;
        }

        let msg = format!("unknown name `{name}`");
        let note = suggest(
            name,
            env::PREDICATES
                .iter()
                .map(|s| s.name)
                .chain(env::COLLECTIONS.iter().map(|c| c.name))
                .chain(self.scope.iter().map(|(n, _)| n.as_str())),
        );
        self.error(span, msg + &note);
        Type::Error
    }

    /// Overloaded across buildings, units and collections, so it cannot be a row
    /// in `PREDICATES`.
    fn count(&mut self, args: &[Expr], span: Span) -> Type {
        let [arg] = args else {
            let msg = format!("`count` expects 1 argument, got {}", args.len());
            self.error(span, msg);
            return Type::Error;
        };

        let ExprKind::Ident(name) = &arg.kind else {
            self.error(
                arg.span,
                "`count` takes a name, not an expression".to_string(),
            );
            return Type::Error;
        };

        let mut hits: Vec<&str> = Vec::new();
        if env::collection(&name.text).is_some() {
            hits.push("collection");
        }
        for d in env::domains_containing(&name.text) {
            if matches!(
                d,
                crate::types::Domain::BuildingType | crate::types::Domain::UnitType
            ) {
                hits.push(d.name());
            }
        }

        match hits.len() {
            1 => Type::Int,
            0 => {
                let msg = format!("`{}` is not something that can be counted", name.text);
                let note = suggest(
                    &name.text,
                    env::COLLECTIONS
                        .iter()
                        .map(|c| c.name)
                        .chain(env::BUILDING_TYPES.iter().map(|m| m.name))
                        .chain(env::UNIT_TYPES.iter().map(|m| m.name)),
                );
                self.error(name.span, msg + &note);
                Type::Error
            }
            // Never a silent pick — a name quietly meaning something
            // unintended is the failure this language exists to catch.
            _ => {
                let msg = format!(
                    "`{}` is ambiguous — it is a {}",
                    name.text,
                    hits.join(" and a ")
                );
                self.error(name.span, msg);
                Type::Error
            }
        }
    }

    fn error(&mut self, span: Span, message: String) {
        self.diags.push(Diagnostic { message, span });
    }
}

impl Checker {
    // ---- whole-rule-set passes ----
    //
    // The checks Go structurally cannot do. See `docs/design.md`.

    fn check_squad_references(&mut self, _ast: &Ast) {
        todo!("every squad-exists(X) needs a reachable form-squad(X, ...)")
    }

    fn check_priority_collisions(&mut self, _ast: &Ast) {
        todo!("two rules sharing a category and a priority order nondeterministically")
    }

    fn check_shadowed_rules(&mut self, _ast: &Ast) {
        todo!("a rule below a higher-priority exclusive one in the same category")
    }
}

/// A " (did you mean `x`?)" note, or nothing when nothing is close.
///
/// The caller passes the candidates, so `has-role(war-facotry)` is compared
/// against the 52 roles rather than all 147 names. That scoping is what makes
/// the suggestion useful rather than noise.
fn suggest<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> String {
    // Short names need a near-exact match; long ones tolerate a transposition.
    let budget = (name.len() / 3).max(1);
    let best = candidates
        .map(|c| (edit_distance(name, c), c))
        .filter(|(d, _)| *d <= budget)
        .min_by_key(|(d, _)| *d);
    match best {
        Some((_, c)) => format!(" (did you mean `{c}`?)"),
        None => String::new(),
    }
}

/// Levenshtein distance, two rows rather than a full matrix.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];

    for (i, ca) in a.chars().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}
