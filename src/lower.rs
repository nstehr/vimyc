//! `Ast` to `Ir`: resolving every name exactly once.
//!
//! Runs only on a tree that checked, so nothing here reports a diagnostic —
//! an unresolvable name at this point is a bug in `check`, not in the source.
//! That is why `Ir` has no error variant.

use crate::ast::{Action, Ast, Expr, ExprKind, Rule};
use crate::diag::Span;
use crate::env::{self, Predicate};
use crate::ir::{ActionId, CategoryId, Ir, IrAction, IrExpr, IrExprKind, IrParam, IrRule};
use crate::types::{Domain, ParamType, Type};

/// Lowers a checked rule set.
///
/// Crate-private, and reachable only through `check`. That is what makes the
/// panics below sound: an unresolved name here would mean `check` accepted
/// something it should not have, not that a caller skipped it.
/// Substitutes a `def`'s arguments into its body.
///
/// At the AST rather than the IR, which keeps lowering unchanged: the call site
/// becomes an ordinary expression and is lowered in the caller's scope. Capture
/// is impossible because `bind` rejects any binding named after a doctrine
/// parameter or a predicate, and a def body can name nothing else.
fn inline(def: &crate::ast::Def, args: &[Expr]) -> Expr {
    fn subst(e: &Expr, bound: &[(String, &Expr)]) -> Expr {
        let kind = match &e.kind {
            ExprKind::Ident(name) => {
                if let Some((_, arg)) = bound.iter().find(|(n, _)| *n == name.text) {
                    return (*arg).clone();
                }
                ExprKind::Ident(name.clone())
            }
            ExprKind::Call(name, args) => {
                ExprKind::Call(name.clone(), args.iter().map(|a| subst(a, bound)).collect())
            }
            ExprKind::Unary(op, x) => ExprKind::Unary(*op, Box::new(subst(x, bound))),
            ExprKind::Binary(op, l, r) => {
                ExprKind::Binary(*op, Box::new(subst(l, bound)), Box::new(subst(r, bound)))
            }
            other => other.clone(),
        };
        Expr { kind, span: e.span }
    }

    let bound: Vec<(String, &Expr)> = def
        .params
        .iter()
        .map(|p| p.name.text.clone())
        .zip(args)
        .collect();
    subst(&def.body, &bound)
}

pub(crate) fn lower(ast: &Ast) -> Ir {
    let params: Vec<IrParam> = ast
        .params
        .iter()
        .map(|p| IrParam {
            name: p.name.text.clone(),
            kind: p.kind,
            span: p.span,
        })
        .collect();
    // Names in slot order, which is declaration order.
    let names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
    Ir {
        rules: ast
            .rules
            .iter()
            .map(|r| lower_rule(r, &names, &ast.defs))
            .collect(),
        params,
    }
}

/// `params` is the file's parameter names, in slot order. Threaded through
/// because a parameter may appear anywhere an expression may.
fn lower_rule(rule: &Rule, params: &[String], defs: &[crate::ast::Def]) -> IrRule {
    let category = env::category_id(&rule.category.text)
        .unwrap_or_else(|| unreachable!("unknown category `{}`", rule.category.text));

    // Built as we go: a binding may refer to earlier ones but not to itself or
    // to later ones, and its slot is its position here.
    let mut scope: Vec<String> = Vec::with_capacity(rule.lets.len());
    let mut lets = Vec::with_capacity(rule.lets.len());
    for binding in &rule.lets {
        lets.push(lower_expr(&binding.value, &scope, params, defs));
        scope.push(binding.name.text.clone());
    }

    IrRule {
        name: rule.name.text.clone(),
        // Only parameters are in scope: the checker has already rejected a
        // priority that mentions a binding or a predicate.
        priority: lower_expr(&rule.priority, &[], params, defs),

        category: CategoryId(category),
        exclusive: rule.exclusive,
        because: rule.because.clone(),
        action: lower_action(&rule.action, &scope, params, defs),
        requires: rule
            .requires
            .iter()
            .map(|r| lower_expr(r, &scope, params, defs))
            .collect(),
        lets,
        name_span: rule.name.span,
        span: rule.span,
    }
}

fn lower_action(
    action: &Action,
    scope: &[String],
    params: &[String],
    defs: &[crate::ast::Def],
) -> IrAction {
    let id = env::action_id(&action.name.text)
        .unwrap_or_else(|| unreachable!("unknown action `{}`", action.name.text));

    // Arguments lower like any other expression, except that the enum literals
    // among them resolve by parameter position — the same rule as a predicate's,
    // which is what makes `form-squad(ground-attack, Ground, 8, Attack)` work.
    let sig = env::action_signature(&action.name.text);
    let args = action
        .args
        .iter()
        .enumerate()
        .map(|(i, a)| match sig.and_then(|s| s.params.get(i)) {
            Some(want) => lower_arg(a, want, scope, params, defs),
            None => lower_expr(a, scope, params, defs),
        })
        .collect();

    IrAction {
        id: ActionId(id),
        args,
        span: action.span,
    }
}

/// `scope` maps a binding's name to its slot, so `Ident` becomes `Binding(n)`.
fn lower_expr(e: &Expr, scope: &[String], params: &[String], defs: &[crate::ast::Def]) -> IrExpr {
    let kind = match &e.kind {
        ExprKind::Int(n) => IrExprKind::Int(*n),
        ExprKind::Float(f) => IrExprKind::Float(*f),
        ExprKind::Ident(name) => return lower_ident(&name.text, e.span, scope, params),

        ExprKind::Call(name, args) => {
            if name.text == env::COUNT {
                return lower_count(&args[0], e.span, scope, params, defs);
            }
            // A def is expanded here rather than represented in the IR: the
            // emitted rule must look exactly like Go's, which has no defs.
            if let Some(def) = defs.iter().find(|d| d.name.text == name.text) {
                return lower_expr(&inline(def, args), scope, params, defs);
            }
            if let Some(sig) = env::builtin(&name.text) {
                let args = args
                    .iter()
                    .zip(sig.params.iter())
                    .map(|(a, want)| lower_arg(a, want, scope, params, defs))
                    .collect();
                return IrExpr {
                    kind: IrExprKind::Builtin(sig.id, args),
                    span: e.span,
                };
            }
            let sig = env::predicate(&name.text)
                .unwrap_or_else(|| unreachable!("unknown predicate `{}`", name.text));
            let args = args
                .iter()
                .zip(sig.params.iter())
                .map(|(a, want)| lower_arg(a, want, scope, params, defs))
                .collect();
            IrExprKind::Predicate(sig.id, args)
        }

        ExprKind::Unary(op, operand) => {
            IrExprKind::Unary(*op, Box::new(lower_expr(operand, scope, params, defs)))
        }
        ExprKind::Binary(op, l, r) => IrExprKind::Binary(
            *op,
            Box::new(lower_expr(l, scope, params, defs)),
            Box::new(lower_expr(r, scope, params, defs)),
        ),

        ExprKind::Error => unreachable!("lowered a tree that did not check"),
    };
    IrExpr { kind, span: e.span }
}

/// An argument in a position whose parameter type is known, which is what lets a
/// bare name resolve to an enum member.
fn lower_arg(
    e: &Expr,
    want: &ParamType,
    scope: &[String],
    params: &[String],
    defs: &[crate::ast::Def],
) -> IrExpr {
    let ExprKind::Ident(name) = &e.kind else {
        return lower_expr(e, scope, params, defs);
    };

    // A binding shadows nothing — `bind` rejects any name that could — so a name
    // in scope is a binding and never an enum literal.
    if scope.iter().any(|n| n == &name.text) {
        return lower_expr(e, scope, params, defs);
    }

    let domains: &[Domain] = match want {
        ParamType::AnyOf(ds) => ds,
        ParamType::Exact(Type::Enum(d)) => std::slice::from_ref(d),
        // Not an enum position, so the argument is an ordinary expression.
        ParamType::Exact(_) => return lower_expr(e, scope, params, defs),
    };

    for d in domains {
        if let Some(index) = env::member_index(*d, &name.text) {
            return IrExpr {
                kind: IrExprKind::Member(*d, index),
                span: e.span,
            };
        }
    }
    unreachable!("`{}` is not a member of {want}", name.text)
}

/// The one overloaded name: `count(powr)`, `count(e1)` and
/// `count(idle-ground-units)` are three different predicates, and which one is
/// decided here rather than at every consumer. Nothing downstream sees a
/// `count` node.
fn lower_count(
    arg: &Expr,
    span: Span,
    scope: &[String],
    params: &[String],
    defs: &[crate::ast::Def],
) -> IrExpr {
    // A bare building or unit name counts instances of that type.
    if let ExprKind::Ident(name) = &arg.kind
        && !scope.iter().any(|n| n == &name.text)
    {
        for (domain, id) in [
            (Domain::BuildingType, Predicate::BuildingCount),
            (Domain::UnitType, Predicate::UnitCount),
        ] {
            if let Some(index) = env::member_index(domain, &name.text) {
                return IrExpr {
                    kind: IrExprKind::Predicate(
                        id,
                        vec![IrExpr {
                            kind: IrExprKind::Member(domain, index),
                            span: arg.span,
                        }],
                    ),
                    span,
                };
            }
        }
    }

    // Otherwise it counts a collection, and the collection predicate is already
    // the whole expression — `count` itself disappears.
    lower_expr(arg, scope, params, defs)
}

/// A bare name: a binding, a parameter, or a zero-argument predicate.
///
/// Enum literals never reach here — they are resolved in argument position,
/// where the parameter says which domain they belong to.
fn lower_ident(name: &str, span: Span, scope: &[String], params: &[String]) -> IrExpr {
    if let Some(slot) = scope.iter().position(|n| n == name) {
        return IrExpr {
            kind: IrExprKind::Binding(slot as u32),
            span,
        };
    }
    // After the rule scope, because `bind` rejects a binding that would shadow
    // a parameter — so at most one of the two can match, and the order is only
    // about matching what `check` did.
    if let Some(slot) = params.iter().position(|n| n == name) {
        return IrExpr {
            kind: IrExprKind::Param(slot as u32),
            span,
        };
    }
    let sig = env::predicate(name).unwrap_or_else(|| unreachable!("unknown name `{name}`"));
    IrExpr {
        kind: IrExprKind::Predicate(sig.id, Vec::new()),
        span,
    }
}
