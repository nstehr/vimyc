//! Emitting expr source, which Vimy's engine already runs.
//!
//! The inverse of `vimy-core/rules/translate.go`, and the first backend because
//! Go's evaluation path is unchanged by it.
//!
//! The one place Go's spellings appear: the language is uniformly kebab and the
//! conversion happens here, so nothing upstream has to know.

use crate::ast::{BinOp, UnOp};
use crate::emit::RuleSource;
use crate::env;
use crate::ir::{Ir, IrExpr, IrExprKind, IrRule, ParamValues};
use crate::types::{Domain, Type};

pub fn emit(ir: &Ir, params: &ParamValues) -> Vec<RuleSource> {
    ir.rules.iter().map(|r| emit_rule(r, params)).collect()
}

fn emit_rule(rule: &IrRule, params: &ParamValues) -> RuleSource {
    // Vacuously true, and expr will not compile an empty string
    // (`unexpected token EOF`). Reached by a rule with no `require`, and by one
    // whose every conjunct was a gate `specialise` folded away.
    let mut condition = if rule.requires.is_empty() {
        "true".to_string()
    } else {
        String::new()
    };
    for (i, require) in rule.requires.iter().enumerate() {
        if i > 0 {
            condition.push_str(" && ");
        }
        // A conjunct sits at `&&`'s precedence, so it parenthesises itself if
        // it is looser.
        emit_expr(require, rule, params, PREC_AND, &mut condition);
    }

    RuleSource {
        name: rule.name.clone(),
        priority: crate::eval::priority(rule, params),
        category: env::category_name(rule.category.0).to_string(),
        exclusive: rule.exclusive,
        because: rule.because.clone(),
        action: emit_action(rule, params),
        condition,
    }
}

/// A predicate's argument, folded rather than emitted as an expression: Go keys
/// a recorded call by the literal in its source, so the projection can answer
/// `DamagedCombatUnits(0.5)` and not `DamagedCombatUnits((0.25 + 0.25))`.
fn emit_arg(e: &IrExpr, rule: &IrRule, params: &ParamValues, out: &mut String) {
    match &e.kind {
        IrExprKind::Member(..) => emit_expr(e, rule, params, PREC_ATOM, out),
        _ if crate::specialise::is_static(e) => out.push_str(&fold(e, params)),
        // A collection argument to `count`, which lowering has already folded
        // into the predicate itself, so nothing else should reach here.
        _ => emit_expr(e, rule, params, PREC_ATOM, out),
    }
}

/// A parameter or builtin, resolved to the number the doctrine gives it.
///
/// Reuses the priority evaluator, so a folded threshold and a folded priority
/// cannot disagree about what `lerp` means.
fn fold(e: &IrExpr, params: &ParamValues) -> String {
    match crate::eval::static_eval(e, params) {
        crate::eval::Value::Int(n) => n.to_string(),
        crate::eval::Value::Float(f) => crate::state::render_number(f),
        other => unreachable!("folded to {other:?}, which is not a number"),
    }
}

/// The action as Go's `actionSrc` spells it: the language's own form, since Go
/// looks this up rather than evaluating it.
fn emit_action(rule: &IrRule, params: &ParamValues) -> String {
    let name = env::action_name(rule.action.id.0);
    if rule.action.args.is_empty() {
        return name.to_string();
    }
    let args: Vec<String> = rule
        .action
        .args
        .iter()
        .map(|a| {
            let mut s = String::new();
            emit_bare(a, rule, params, &mut s);
            s
        })
        .collect();
    format!("{name}({})", args.join(", "))
}

/// An action argument, in the language's spelling rather than Go's.
fn emit_bare(e: &IrExpr, rule: &IrRule, params: &ParamValues, out: &mut String) {
    match &e.kind {
        IrExprKind::Int(n) => out.push_str(&n.to_string()),
        IrExprKind::Float(f) => out.push_str(&crate::state::render_number(*f)),
        IrExprKind::Bool(b) => out.push_str(if *b { "true" } else { "false" }),

        // The other binding time — the engine answering `Aggression()` per
        // tick — would emit a call here; `docs/design.md` covers why it stays
        // open.
        IrExprKind::Param(_) | IrExprKind::Builtin(..) => out.push_str(&fold(e, params)),
        IrExprKind::Member(domain, index) => out.push_str(env::member_name(*domain, *index)),
        IrExprKind::Binding(slot) => emit_bare(&rule.lets[*slot as usize], rule, params, out),
        // Static but not a literal — `retreat-damaged-units(0.25 + 0.25)`, or
        // anything computed from a parameter. Go looks an action up by this
        // text, so it has to be the number.
        _ if crate::specialise::is_static(e) => out.push_str(&fold(e, params)),
        other => unreachable!("action argument is not static: {other:?}"),
    }
}

// Precedence, matching the grammar. A child is parenthesised when it binds
// looser than the position it sits in.
const PREC_OR: u8 = 1;
const PREC_AND: u8 = 2;
const PREC_CMP: u8 = 3;
const PREC_ADD: u8 = 4;
const PREC_MUL: u8 = 5;
const PREC_UNARY: u8 = 6;
const PREC_ATOM: u8 = 7;

fn precedence(op: BinOp) -> u8 {
    match op {
        BinOp::Or => PREC_OR,
        BinOp::And => PREC_AND,
        BinOp::Eq | BinOp::NotEq | BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq => PREC_CMP,
        BinOp::Add | BinOp::Sub => PREC_ADD,
        BinOp::Mul | BinOp::Div => PREC_MUL,
    }
}

fn operator(op: BinOp) -> &'static str {
    match op {
        BinOp::Or => "||",
        BinOp::And => "&&",
        BinOp::Eq => "==",
        BinOp::NotEq => "!=",
        BinOp::Lt => "<",
        BinOp::LtEq => "<=",
        BinOp::Gt => ">",
        BinOp::GtEq => ">=",
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
    }
}

/// Renders one expression as expr source.
///
/// Parenthesised by precedence rather than everywhere: this output is read by
/// people debugging a rule set, and `Cash() >= 300` beats `((Cash()) >= (300))`.
fn emit_expr(e: &IrExpr, rule: &IrRule, params: &ParamValues, parent: u8, out: &mut String) {
    match &e.kind {
        IrExprKind::Int(n) => out.push_str(&n.to_string()),
        IrExprKind::Float(f) => out.push_str(&crate::state::render_number(*f)),
        IrExprKind::Bool(b) => out.push_str(if *b { "true" } else { "false" }),

        // Folded to the number the doctrine supplied. The other binding time —
        // the engine answering `Aggression()` at evaluation time — would emit a
        // call here instead; `docs/design.md` covers why that stays open.
        IrExprKind::Param(_) | IrExprKind::Builtin(..) => out.push_str(&fold(e, params)),

        // Enum literals are quoted strings on Go's side, in Go's spelling.
        IrExprKind::Member(domain, index) => {
            out.push('"');
            out.push_str(&wire_member(*domain, *index));
            out.push('"');
        }

        IrExprKind::Binding(slot) => emit_binding(rule, params, *slot, parent, out),

        IrExprKind::Predicate(id, args) => {
            let sig = env::PREDICATES
                .iter()
                .find(|s| s.id == *id)
                .unwrap_or_else(|| unreachable!("predicate has no signature"));

            // A collection can only ever be counted, so it is always `len(…)`.
            let collection = sig.ret == Type::Collection;
            if collection {
                out.push_str("len(");
            }
            out.push_str(&go_name(sig.name));
            out.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                emit_arg(a, rule, params, out);
            }
            out.push(')');
            if collection {
                out.push(')');
            }
        }

        IrExprKind::Unary(op, operand) => match op {
            // expr has no `exists`; an optional is compared against nil.
            UnOp::Exists => {
                let wrap = PREC_CMP < parent;
                if wrap {
                    out.push('(');
                }
                emit_expr(operand, rule, params, PREC_ATOM, out);
                out.push_str(" != nil");
                if wrap {
                    out.push(')');
                }
            }
            UnOp::Not | UnOp::Neg => {
                out.push(if matches!(op, UnOp::Not) { '!' } else { '-' });
                emit_expr(operand, rule, params, PREC_UNARY, out);
            }
        },

        IrExprKind::Binary(op, l, r) => {
            let prec = precedence(*op);
            let wrap = prec < parent;
            if wrap {
                out.push('(');
            }
            emit_expr(l, rule, params, prec, out);
            out.push(' ');
            out.push_str(operator(*op));
            out.push(' ');
            // The right operand binds one tighter, so `a - (b - c)` keeps its
            // parentheses while `(a - b) - c` loses them.
            emit_expr(r, rule, params, prec + 1, out);
            if wrap {
                out.push(')');
            }
        }
    }
}

/// expr has no bindings, so one is replaced by what it was bound to.
///
/// Terminates because a binding may only refer to earlier ones. Used twice, it
/// is emitted twice — `let` earns its place in the source, not the output.
fn emit_binding(rule: &IrRule, params: &ParamValues, slot: u32, parent: u8, out: &mut String) {
    emit_expr(&rule.lets[slot as usize], rule, params, parent, out);
}

/// Predicates whose Go spelling capitalises an acronym: `apcs` could be `Apcs`
/// or `APCs`, and nothing recovers that from the kebab name.
///
/// `go_name_matches_the_manifest` holds this to exactly the set that needs it.
pub const ACRONYMS: &[(&str, &str)] = &[
    ("idle-combat-loaded-apcs", "IdleCombatLoadedAPCs"),
    ("idle-empty-apcs", "IdleEmptyAPCs"),
    ("idle-engineer-loaded-apcs", "IdleEngineerLoadedAPCs"),
];

/// A predicate in Go's spelling: `can-build-role` becomes `CanBuildRole`.
pub fn go_name(kebab: &str) -> String {
    match ACRONYMS.iter().find(|(k, _)| *k == kebab) {
        Some((_, go)) => (*go).to_string(),
        None => pascal_case(kebab),
    }
}

fn pascal_case(kebab: &str) -> String {
    kebab
        .split('-')
        .map(|part| {
            let mut c = part.chars();
            match c.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + c.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// A member in Go's spelling. Roles are snake there; the rest are engine
/// identifiers that both sides share.
fn wire_member(domain: Domain, index: u32) -> String {
    let name = env::member_name(domain, index);
    match domain {
        Domain::Role => name.replace('-', "_"),
        _ => name.to_string(),
    }
}
