//! Type checking.
//!
//! Bidirectional: most nodes are *synthesised* bottom-up, but enum literals have
//! no type of their own and must be *checked* against what a parameter expects.
//! `docs/design.md` covers the resolution rules under "Types".

use crate::ast::{Action, Ast, BinOp, Expr, ExprKind, Name, ParamKind, Rule, UnOp};
use crate::diag::{Diagnostic, Span};
use crate::env;
use crate::ir::Ir;
use crate::types::{Domain, ParamType, Type};

/// A rule set that can be lowered, plus anything worth saying that did not stop
/// it.
pub struct Checked {
    pub ir: Ir,
    pub warnings: Vec<Diagnostic>,
}

/// Checks a rule set and lowers it.
///
/// The only way to obtain an `Ir`, which is what makes `lower`'s panics sound.
/// Both kinds come back on failure, so an error does not bury a warning beside
/// it. Collects rather than bailing: one bad rule must not hide the rest.
pub fn check(ast: &Ast) -> Result<Checked, Vec<Diagnostic>> {
    let mut checker = Checker::new();
    checker.run(ast);
    let diags = checker.diags;

    if diags.iter().any(Diagnostic::is_error) {
        return Err(diags);
    }
    Ok(Checked {
        ir: crate::lower::lower(ast),
        warnings: diags,
    })
}

/// Cross-rule state. Anything rule-scoped lives on `RuleChecker`, so it cannot
/// outlive the rule it belongs to.
struct Checker {
    diags: Vec<Diagnostic>,
    /// In order: a def may only call an earlier one, which rules out recursion
    /// and so lets inlining terminate.
    defs: Vec<DefSig>,
    /// File-scoped, so unlike a `let` these outlive any one rule.
    params: Vec<(String, Type)>,
}

/// Which of the two evaluations an expression belongs to: a priority resolves
/// once per doctrine, a condition runs every tick.
///
/// `Static` makes that separation a type error rather than a convention — see
/// `docs/design.md`, "Two phases".
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Carries what is being checked, so the diagnostic can name it.
    Static(&'static str),
    Tick,
}

/// Why a name may not be introduced, or `None` if it is free.
///
/// Shared by `bind` and `declare_params`: a `let` and a `param` are different
/// things, but what they may not be called is the same list, and a fourth copy
/// of it would drift.
fn reserved(name: &str) -> Option<String> {
    if env::predicate(name).is_some() {
        return Some(format!("`{name}` is a predicate"));
    }
    if env::builtin(name).is_some() {
        return Some(format!("`{name}` is a builtin"));
    }
    // Enum literals resolve by position, so `powr` would mean the binding in
    // `cash > powr` and the building in `count(powr)`.
    if let Some(d) = env::domains_containing(name).first() {
        return Some(format!("`{name}` is a {}", d.name()));
    }
    None
}

/// A `def`'s signature, once its body has been checked.
struct DefSig {
    name: String,
    params: Vec<Type>,
    ret: Type,
}

/// One rule's checking pass.
///
/// The scope lives here so a stale binding is impossible: born and dropped with
/// the rule, leaving no `clear()` to forget.
struct RuleChecker<'a> {
    diags: &'a mut Vec<Diagnostic>,
    /// A `Vec` because there are no nested scopes and a handful of bindings.
    scope: Vec<(String, Type)>,
    params: &'a [(String, Type)],
    defs: &'a [DefSig],
    phase: Phase,
}

impl Checker {
    fn new() -> Self {
        Checker {
            diags: Vec::new(),
            defs: Vec::new(),
            params: Vec::new(),
        }
    }

    fn run(&mut self, ast: &Ast) {
        // Before any rule, since every rule may refer to them.
        self.declare_params(&ast.params);
        self.declare_defs(&ast.defs);

        for rule in &ast.rules {
            RuleChecker {
                diags: &mut self.diags,
                scope: Vec::new(),
                params: &self.params,
                defs: &self.defs,
                phase: Phase::Tick,
            }
            .rule(rule);
        }
    }

    /// Checks each `def` body and records what it returns, in order, with only
    /// earlier defs in scope.
    fn declare_defs(&mut self, defs: &[crate::ast::Def]) {
        for d in defs {
            let name = &d.name.text;
            if let Some(why) = reserved(name) {
                let msg = format!("{why} and cannot be a def");
                self.diags.push(Diagnostic::error(d.name.span, msg));
                continue;
            }
            if self.defs.iter().any(|s| s.name == *name)
                || self.params.iter().any(|(n, _)| n == name)
            {
                let msg = format!("`{name}` is already declared");
                self.diags.push(Diagnostic::error(d.name.span, msg));
                continue;
            }

            // With the doctrine's parameters rather than in the rule scope:
            // they are substituted before anything evaluates, and an argument is
            // checked in the static phase, which does not consult the scope.
            let mut visible = self.params.clone();
            for p in &d.params {
                let ty = match p.kind {
                    ParamKind::Int => Type::Int,
                    ParamKind::Float => Type::Float,
                };
                if let Some(why) = reserved(&p.name.text) {
                    let msg = format!("{why} and cannot be a def parameter");
                    self.diags.push(Diagnostic::error(p.name.span, msg));
                }
                visible.push((p.name.text.clone(), ty));
            }

            let mut checker = RuleChecker {
                diags: &mut self.diags,
                scope: Vec::new(),
                params: &visible,
                defs: &self.defs,
                phase: Phase::Tick,
            };
            let ret = checker.synth(&d.body);

            self.defs.push(DefSig {
                name: name.clone(),
                params: d
                    .params
                    .iter()
                    .map(|p| match p.kind {
                        ParamKind::Int => Type::Int,
                        ParamKind::Float => Type::Float,
                    })
                    .collect(),
                ret,
            });
        }
    }

    /// Records the declared parameters, rejecting duplicates and any name that
    /// would shadow a predicate or a builtin.
    fn declare_params(&mut self, params: &[crate::ast::Param]) {
        for p in params {
            let name = &p.name.text;
            let ty = match p.kind {
                ParamKind::Int => Type::Int,
                ParamKind::Float => Type::Float,
            };

            // Recorded even when rejected, so one bad declaration does not
            // become an "unknown name" at every use.
            let bad = reserved(name)
                .map(|why| format!("{why} and cannot be a parameter"))
                .or_else(|| {
                    self.params
                        .iter()
                        .any(|(n, _)| n == name)
                        .then(|| format!("`{name}` is declared twice"))
                });
            match bad {
                Some(msg) => {
                    self.diags.push(Diagnostic::error(p.name.span, msg));
                    self.params.push((name.clone(), Type::Error));
                }
                None => self.params.push((name.clone(), ty)),
            }
        }
    }
}

impl<'a> RuleChecker<'a> {
    fn rule(&mut self, rule: &Rule) {
        self.scope.clear();

        // Before the bindings: a priority may see parameters, not `let`s.
        self.priority(&rule.priority);

        for binding in &rule.lets {
            let ty = self.synth(&binding.value);
            self.bind(&binding.name, ty);
        }

        for require in &rule.requires {
            self.check_expr(require, &Type::Bool);
        }

        // A misspelled category would make its own exclusivity group, letting
        // two rules fire that meant to exclude each other.
        if !env::is_category(&rule.category.text) {
            let msg = format!("unknown category `{}`", rule.category.text);
            self.error(rule.category.span, msg);
        }
        self.action(&rule.action);
    }

    /// A priority must be an `Int` decidable before the first tick: parameters,
    /// literals and builtins are in, a predicate is not.
    fn priority(&mut self, e: &Expr) {
        // Restored unconditionally: a leaked `Static` would reject every
        // predicate in the rest of the rule.
        self.phase = Phase::Static("a priority");
        self.check_expr(e, &Type::Int);
        self.phase = Phase::Tick;
    }

    /// An action name, and its arguments when it takes any.
    fn action(&mut self, action: &Action) {
        let name = &action.name.text;
        if let Some(sig) = env::action_signature(name) {
            if action.args.len() != sig.params.len() {
                let msg = format!(
                    "`{name}` expects {} argument(s), got {}",
                    sig.params.len(),
                    action.args.len()
                );
                self.error(action.span, msg);
                return;
            }
            for (arg, want) in action.args.iter().zip(sig.params.iter()) {
                self.check_arg(arg, want);
            }
            return;
        }

        if !env::is_action(name) {
            let msg = format!("unknown action `{name}`");
            let note = suggest(
                name,
                env::ACTIONS
                    .iter()
                    .copied()
                    .chain(env::ACTION_SIGNATURES.iter().map(|a| a.name)),
            );
            self.error(action.name.span, msg + &note);
            return;
        }
        if !action.args.is_empty() {
            let msg = format!("`{name}` takes no arguments");
            self.error(action.span, msg);
        }
    }

    /// Adds a `let` binding, rejecting anything that would shadow. A rule where
    /// `cash` means something else is the confusion this language exists to
    /// prevent.
    fn bind(&mut self, name: &Name, ty: Type) {
        if let Some(why) = reserved(&name.text) {
            let msg = format!("{why} and cannot be rebound");
            self.error(name.span, msg);
            return;
        }
        // Lowering resolves the scope before the parameter table, so without
        // this a shadowed name would mean whatever the passes happened to do.
        if self.params.iter().any(|(n, _)| *n == name.text) {
            let msg = format!("`{}` is a parameter and cannot be rebound", name.text);
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
    /// compatible with everything, so one mistake yields one message.
    fn synth(&mut self, e: &Expr) -> Type {
        match &e.kind {
            ExprKind::Int(_) => Type::Int,
            ExprKind::Float(_) => Type::Float,
            ExprKind::Ident(name) => self.ident(&name.text, name.span),
            // The parser already reported.
            ExprKind::Error => Type::Error,
            ExprKind::Call(n, args) => {
                if n.text == env::COUNT {
                    if let Phase::Static(what) = self.phase {
                        return self.not_static(what, &n.text, n.span);
                    }
                    self.count(args, n.span)
                } else if let Some(sig) = self.defs.iter().find(|d| d.name == n.text) {
                    // Inlined at lowering, so a def is exactly as static as its
                    // body and arguments — no phase rule of its own.
                    let params: Vec<ParamType> = sig
                        .params
                        .iter()
                        .map(|t| ParamType::Exact(t.clone()))
                        .collect();
                    let ret = sig.ret.clone();
                    self.call(&n.text, n.span, args, &params, &ret)
                } else if let Some(sig) = env::builtin(&n.text) {
                    // Legal in either phase: a builtin reads no state, which is
                    // the whole reason it is not a predicate.
                    self.call(&n.text, n.span, args, sig.params, &sig.ret)
                } else if let Some(sig) = env::predicate(&n.text) {
                    if let Phase::Static(what) = self.phase {
                        return self.not_static(what, &n.text, n.span);
                    }
                    self.call(&n.text, n.span, args, sig.params, &sig.ret)
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

                BinOp::Eq | BinOp::NotEq => {
                    let lt = self.synth(left);
                    let rt = self.synth(right);
                    if self.reject_optional(&lt, &rt, e.span) && !lt.compatible(&rt) {
                        let msg = format!("cannot compare {lt} with {rt}");
                        self.error(e.span, msg);
                    }
                    // `Bool` even after an error — a comparison is a bool
                    // whatever its operands were, and saying so keeps one
                    // mistake to one message.
                    Type::Bool
                }

                // Ordering needs numbers where equality does not. Without the
                // split, `base-under-attack < enemies-visible` type checks, and
                // the evaluator would have to invent an ordering on bools or
                // panic on a program the checker accepted.
                BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq => {
                    let lt = self.synth(left);
                    let rt = self.synth(right);
                    if self.reject_optional(&lt, &rt, e.span) {
                        if !lt.is_numeric() || !rt.is_numeric() {
                            let msg = format!("cannot order {lt} and {rt}");
                            self.error(e.span, msg);
                        } else if !lt.compatible(&rt) {
                            let msg = format!("cannot compare {lt} with {rt}");
                            self.error(e.span, msg);
                        }
                    }
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

    /// Reports an optional used as a value, returning whether checking should
    /// continue — so one mistake does not also produce a comparison error.
    fn reject_optional(&mut self, lt: &Type, rt: &Type, span: Span) -> bool {
        if matches!(lt, Type::Option(_)) || matches!(rt, Type::Option(_)) {
            self.error(span, "use `exists` to test an optional value".to_string());
            return false;
        }
        true
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
        // A numeric argument belongs to the static phase. `eval` keys a
        // predicate call by its arguments and `emit` writes an action's into
        // text Go looks up, so neither can wait for a tick to learn what one is.
        // Parameters and `lerp` are the point of allowing more than a literal.
        let outer = self.phase;
        self.phase = Phase::Static("an argument");
        self.check_expr(e, t);
        self.phase = outer;
    }

    /// A bare name: a `let` binding, a zero-argument predicate, or a collection.
    fn ident(&mut self, name: &str, span: Span) -> Type {
        // No bindings exist in the static phase: a priority is checked before
        // the rule's `let`s, precisely because it may not see them.
        if matches!(self.phase, Phase::Tick)
            && let Some((_, ty)) = self.scope.iter().find(|(n, _)| n == name)
        {
            return ty.clone();
        }

        if let Some((_, ty)) = self.params.iter().find(|(n, _)| n == name) {
            return ty.clone();
        }

        if let Some(sig) = env::predicate(name) {
            if let Phase::Static(what) = self.phase {
                return self.not_static(what, name, span);
            }
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
                .chain(self.scope.iter().map(|(n, _)| n.as_str())),
        );
        self.error(span, msg + &note);
        Type::Error
    }

    /// Arity and arguments for anything with a fixed signature — a predicate or
    /// a builtin. They differ in what they may read, not in how they are called.
    fn call(
        &mut self,
        name: &str,
        span: Span,
        args: &[Expr],
        want: &[ParamType],
        ret: &Type,
    ) -> Type {
        if args.len() != want.len() {
            let msg = format!(
                "`{name}` expects {} argument(s), got {}",
                want.len(),
                args.len()
            );
            self.error(span, msg);
            // Arguments left unchecked on purpose: pairing them against
            // parameters at the wrong count reports about ones that are merely
            // in the wrong slot.
            return Type::Error;
        }
        for (arg, w) in args.iter().zip(want.iter()) {
            self.check_arg(arg, w);
        }
        ret.clone()
    }

    /// Reading game state where only a doctrine is available.
    fn not_static(&mut self, what: &str, name: &str, span: Span) -> Type {
        let msg = format!("{what} is fixed when the doctrine lands, so it cannot read `{name}`");
        self.error(span, msg);
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

        // A bare building or unit name counts instances of that type. Anything
        // else must be an expression producing a collection, which is what lets
        // `count(damaged-combat-units(0.5))` work alongside
        // `count(idle-ground-units)`.
        if let ExprKind::Ident(name) = &arg.kind {
            let hits: Vec<Domain> = env::domains_containing(&name.text)
                .into_iter()
                .filter(|d| matches!(d, Domain::BuildingType | Domain::UnitType))
                .collect();
            match hits.len() {
                1 => return Type::Int,
                // Never a silent pick — a name quietly meaning something
                // unintended is the failure this language exists to catch.
                n if n > 1 => {
                    let msg = format!("`{}` is ambiguous across domains", name.text);
                    self.error(name.span, msg);
                    return Type::Error;
                }
                _ => {}
            }
        }

        match self.synth(arg) {
            Type::Collection => Type::Int,
            Type::Error => Type::Error,
            other => {
                let msg = format!("`count` needs a collection or a type name, found {other}");
                self.error(arg.span, msg);
                Type::Error
            }
        }
    }

    fn error(&mut self, span: Span, message: String) {
        self.diags.push(Diagnostic::error(span, message));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;

    /// Every message, whichever side of the split it landed on. Most of these
    /// tests care that a problem is *reported*; `severity_decides_what_lowers`
    /// is where the split itself is pinned.
    fn messages(src: &str) -> Vec<String> {
        let (tokens, ld) = lex(src);
        assert!(ld.is_empty(), "{ld:?}");
        let (ast, pd) = parse(&tokens);
        assert!(pd.is_empty(), "{pd:?}");
        match check(&ast) {
            Ok(c) => c.warnings.into_iter().map(|d| d.message).collect(),
            Err(d) => d.into_iter().map(|d| d.message).collect(),
        }
    }

    #[test]
    fn ordering_needs_numbers() {
        let e = messages(
            "rule r {\n priority 1\n category economy\n do scout\n require base-under-attack < enemies-visible\n}\n",
        );
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(e[0].contains("cannot order"), "{e:?}");
    }

    #[test]
    fn equality_still_works_on_bools() {
        let e = messages(
            "rule r {\n priority 1\n category economy\n do scout\n require base-under-attack == enemies-visible\n}\n",
        );
        assert!(e.is_empty(), "{e:?}");
    }

    /// The split is about soundness, not about how bad the problem is.
    ///
    /// A shadowed rule is near-certainly a mistake, but it lowers to something
    /// that runs — so it warns and an `Ir` still comes back. A typo cannot be
    /// lowered at all, so it errors and no `Ir` exists to misuse.
    #[test]
    fn severity_decides_what_lowers() {
        let shadowed = "rule high {\n priority 9\n category economy exclusive\n do scout\n \
                        require cash >= 1\n}\n\
                        rule low {\n priority 1\n category economy\n do scout\n \
                        require cash >= 1\n require has-role(barracks)\n}\n";
        let (t, _) = lex(shadowed);
        let (ast, pd) = parse(&t);
        assert!(pd.is_empty(), "{pd:?}");
        // The shadowing itself is reported by `specialise::validate`, which
        // needs a doctrine; what matters here is that a warnable rule set still
        // yields an `Ir`.
        let checked = check(&ast).expect("a warning must not stop lowering");
        assert_eq!(checked.ir.rules.len(), 2);

        let typo = "rule r {\n priority 1\n category economy\n do scout\n \
                    require has-role(war-facotry)\n}\n";
        let (t, _) = lex(typo);
        let (ast, pd) = parse(&t);
        assert!(pd.is_empty(), "{pd:?}");
        let diags = check(&ast).err().expect("a typo must stop lowering");
        assert!(diags.iter().all(|d| d.is_error()), "{diags:?}");
    }

    /// `messages` runs the whole of `check`, which lowers on success — so a
    /// rule set that checks cleanly cannot be asserted on until `lower` handles
    /// parameters. These pin the rejections; `a_parameterised_rule_set_checks`
    /// is the positive case, ignored until then.
    fn param_src(decls: &str, priority: &str, require: &str) -> String {
        format!(
            "{decls}rule r {{\n priority {priority}\n category economy\n do scout\n \
             require {require}\n}}\n"
        )
    }

    #[test]
    fn a_priority_cannot_read_game_state() {
        let e = messages(&param_src("", "cash", "cash >= 1"));
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(e[0].contains("cannot read `cash`"), "{e:?}");

        // Nested just as much as bare, and through `count` too.
        let e = messages(&param_src("", "400 + role-count(barracks)", "cash >= 1"));
        assert!(e.iter().any(|m| m.contains("cannot read")), "{e:?}");
        let e = messages(&param_src("", "count(powr)", "cash >= 1"));
        assert!(e.iter().any(|m| m.contains("cannot read")), "{e:?}");
    }

    #[test]
    fn a_priority_must_be_an_int() {
        // `lerpf` is the trap: a builtin, so the phase rule lets it through, but
        // the engine sorts on an integer.
        let e = messages(&param_src(
            "param aggression: float\n",
            "lerpf(200.0, 400.0, aggression)",
            "cash >= 1",
        ));
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(e[0].contains("expected int"), "{e:?}");
    }

    #[test]
    fn a_priority_cannot_see_a_binding() {
        let src = "param aggression: float\n\
                   rule r {\n priority n\n category economy\n do scout\n \
                   let n = 400\n require cash >= 1\n}\n";
        let e = messages(src);
        assert!(e.iter().any(|m| m.contains("unknown name `n`")), "{e:?}");
    }

    /// The phase is restored after the priority, or every predicate in the rest
    /// of the rule would be rejected too.
    ///
    /// Arithmetic rather than `lerp` on purpose: this has to check *and* lower,
    /// and lowering a builtin is not written yet. `200 + 200` exercises the same
    /// set-and-restore.
    #[test]
    fn the_static_phase_does_not_leak_into_the_rule() {
        let e = messages(&param_src(
            "",
            "200 + 200",
            "cash >= 1 and has-role(barracks)",
        ));
        assert!(e.is_empty(), "{e:?}");
    }

    #[test]
    fn a_parameter_cannot_take_a_reserved_name() {
        for (decl, want) in [
            ("param cash: int\n", "is a predicate"),
            ("param lerp: int\n", "is a builtin"),
            ("param powr: int\n", "is a building"),
        ] {
            let e = messages(&param_src(decl, "1", "cash >= 1"));
            assert!(e.iter().any(|m| m.contains(want)), "{decl}: {e:?}");
        }
    }

    #[test]
    fn a_parameter_cannot_be_declared_twice() {
        let e = messages(&param_src(
            "param aggression: float\nparam aggression: int\n",
            "1",
            "cash >= 1",
        ));
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(e[0].contains("declared twice"), "{e:?}");
    }

    #[test]
    fn a_binding_cannot_shadow_a_parameter() {
        let src = "param aggression: float\n\
                   rule r {\n priority 1\n category economy\n do scout\n \
                   let aggression = 3\n require cash >= 1\n}\n";
        let e = messages(src);
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(
            e[0].contains("is a parameter and cannot be rebound"),
            "{e:?}"
        );
    }

    /// A parameter's declared type is load-bearing, not decoration.
    ///
    /// Asserted through a rejection because the accepting case would go on to
    /// lower — `form-squad` wants an `Int` group size, and a float parameter is
    /// exactly the mistake worth catching.
    #[test]
    fn a_parameter_carries_its_declared_type() {
        let src = "param size: float\n\
                   rule r {\n priority 1\n category squad-form\n \
                   do form-squad(naval-attack, Naval, size, Attack)\n \
                   require cash >= 1\n}\n";
        let e = messages(src);
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(e[0].contains("int"), "{e:?}");
    }

    #[test]
    fn a_parameterised_rule_set_checks() {
        let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/rules/params.vy"))
            .expect("params.vy");
        assert!(messages(&src).is_empty());
    }

    /// An argument is settled when the doctrine lands, not during a tick.
    ///
    /// `eval` keys a predicate call by its arguments and `emit` writes an
    /// action's into text Go looks up; neither can wait for the game to answer.
    #[test]
    fn an_argument_cannot_read_game_state() {
        // A predicate's numeric argument.
        let e = messages(&param_src("", "1", "count(damaged-combat-units(cash)) > 0"));
        assert!(
            e.iter().any(|m| m.contains("an argument is fixed")),
            "{e:?}"
        );

        // An action's.
        let src = "rule r {\n priority 1\n category squad-form\n \
                   do form-squad(ground-attack, Ground, cash, Attack)\n \
                   require cash >= 1\n}\n";
        let e = messages(src);
        assert!(
            e.iter().any(|m| m.contains("an argument is fixed")),
            "{e:?}"
        );

        // A binding is not static either, even when its value would be.
        let src = "rule r {\n priority 1\n category micro\n \
                   do retreat-damaged-units(0.5)\n let t = 0.5\n \
                   require count(damaged-combat-units(t)) > 0\n}\n";
        let e = messages(src);
        assert!(e.iter().any(|m| m.contains("unknown name `t`")), "{e:?}");
    }

    /// Arithmetic and parameters are fine, which is the reason the rule is
    /// "static" rather than "a literal".
    #[test]
    fn an_argument_may_be_computed_from_a_doctrine() {
        let e = messages(&param_src(
            "param leash: float\n",
            "1",
            "count(overextended-squad-members(ground-attack, leash)) > 0",
        ));
        assert!(e.is_empty(), "{e:?}");

        let e = messages(&param_src(
            "",
            "1",
            "count(damaged-combat-units(0.25 + 0.25)) > 0",
        ));
        assert!(e.is_empty(), "{e:?}");
    }

    /// A `def` is the language's only abstraction, and it exists so the savings
    /// stack is written once rather than at twenty-one call sites.
    #[test]
    fn a_def_is_checked_like_any_expression() {
        // Arity and argument types are the call's, the result type is the body's.
        let src = "def reserve(cost: int) = cash >= cost\n\
                   rule r {\n priority 1\n category economy\n do scout\n \
                   require reserve(100)\n}\n";
        assert!(messages(src).is_empty(), "{:?}", messages(src));

        let bad = "def reserve(cost: int) = cash >= cost\n\
                   rule r {\n priority 1\n category economy\n do scout\n \
                   require reserve(100, 200)\n}\n";
        let e = messages(bad);
        assert!(e.iter().any(|m| m.contains("expects 1 argument")), "{e:?}");

        // The body's type is what the call has, so a numeric def is not a
        // condition.
        let numeric = "def half(x: float) = x / 2.0\n\
                       rule r {\n priority 1\n category economy\n do scout\n \
                       require half(1.0)\n}\n";
        let e = messages(numeric);
        assert!(e.iter().any(|m| m.contains("expected bool")), "{e:?}");
    }

    #[test]
    fn a_def_cannot_take_a_reserved_or_repeated_name() {
        for (src, want) in [
            ("def cash(x: int) = x > 0\n", "is a predicate"),
            ("def lerp(x: int) = x > 0\n", "is a builtin"),
            (
                "def a(x: int) = x > 0\ndef a(y: int) = y > 1\n",
                "already declared",
            ),
            (
                "param a: float\ndef a(x: int) = x > 0\n",
                "already declared",
            ),
        ] {
            let full = format!(
                "{src}rule r {{\n priority 1\n category economy\n do scout\n \
                 require cash >= 1\n}}\n"
            );
            let e = messages(&full);
            assert!(e.iter().any(|m| m.contains(want)), "{src}: {e:?}");
        }
    }

    /// Only earlier defs are in scope, so a def cannot call itself and inlining
    /// terminates.
    #[test]
    fn a_def_cannot_be_recursive() {
        let src = "def loop(x: int) = loop(x) and cash >= x\n\
                   rule r {\n priority 1\n category economy\n do scout\n \
                   require loop(1)\n}\n";
        let e = messages(src);
        assert!(
            e.iter().any(|m| m.contains("unknown predicate `loop`")),
            "{e:?}"
        );
    }
}
