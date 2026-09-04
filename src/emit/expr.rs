//! Emitting expr source, which Vimy's engine already runs.
//!
//! The inverse of `vimy-core/rules/translate.go`, and the reason it is the first
//! backend: Go's evaluation path is unchanged, so everything the differential
//! already verified stays verified.
//!
//! This is the one place Go's spellings appear. `docs/design.md` keeps the
//! language uniformly kebab and puts the conversion on emission — so predicates
//! become PascalCase methods here, roles become snake, and nothing upstream has
//! to know.

use crate::ast::{BinOp, UnOp};
use crate::emit::RuleSource;
use crate::env;
use crate::ir::{Ir, IrExpr, IrExprKind, IrRule, ParamValues};
use crate::types::{Domain, Type};

pub fn emit(ir: &Ir, params: &ParamValues) -> Vec<RuleSource> {
    ir.rules.iter().map(|r| emit_rule(r, params)).collect()
}

fn emit_rule(rule: &IrRule, params: &ParamValues) -> RuleSource {
    let mut condition = String::new();
    for (i, require) in rule.requires.iter().enumerate() {
        if i > 0 {
            condition.push_str(" && ");
        }
        // Each `require` is a conjunct, so it sits at `&&`'s precedence and
        // parenthesises itself if it is looser.
        emit_expr(require, rule, PREC_AND, &mut condition);
    }

    RuleSource {
        name: rule.name.clone(),
        priority: crate::eval::priority(rule, params),
        category: env::category_name(rule.category.0).to_string(),
        exclusive: rule.exclusive,
        action: emit_action(rule),
        condition,
    }
}

/// The action as Go's `actionSrc` spells it: the language's own form, since Go
/// looks this up rather than evaluating it.
fn emit_action(rule: &IrRule) -> String {
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
            emit_bare(a, rule, &mut s);
            s
        })
        .collect();
    format!("{name}({})", args.join(", "))
}

/// An action argument, in the language's spelling rather than Go's.
fn emit_bare(e: &IrExpr, rule: &IrRule, out: &mut String) {
    match &e.kind {
        IrExprKind::Int(n) => out.push_str(&n.to_string()),
        IrExprKind::Float(f) => out.push_str(&crate::state::render_number(*f)),

        // Folded to the number the doctrine supplied. The other binding time —
        // the engine answering `Aggression()` at evaluation time — would emit a
        // call here instead; `docs/design.md` covers why that stays open.
        IrExprKind::Param(slot) => todo!("fold param slot {slot}"),
        IrExprKind::Builtin(id, args) => todo!("fold {id:?} over {} args", args.len()),
        IrExprKind::Member(domain, index) => out.push_str(env::member_name(*domain, *index)),
        IrExprKind::Binding(slot) => emit_bare(&rule.lets[*slot as usize], rule, out),
        other => unreachable!("action argument is not a literal: {other:?}"),
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
fn emit_expr(e: &IrExpr, rule: &IrRule, parent: u8, out: &mut String) {
    match &e.kind {
        IrExprKind::Int(n) => out.push_str(&n.to_string()),
        IrExprKind::Float(f) => out.push_str(&crate::state::render_number(*f)),

        // Folded to the number the doctrine supplied. The other binding time —
        // the engine answering `Aggression()` at evaluation time — would emit a
        // call here instead; `docs/design.md` covers why that stays open.
        IrExprKind::Param(slot) => todo!("fold param slot {slot}"),
        IrExprKind::Builtin(id, args) => todo!("fold {id:?} over {} args", args.len()),

        // Enum literals are quoted strings on Go's side, in Go's spelling.
        IrExprKind::Member(domain, index) => {
            out.push('"');
            out.push_str(&wire_member(*domain, *index));
            out.push('"');
        }

        IrExprKind::Binding(slot) => emit_binding(rule, *slot, parent, out),

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
                emit_expr(a, rule, PREC_ATOM, out);
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
                emit_expr(operand, rule, PREC_ATOM, out);
                out.push_str(" != nil");
                if wrap {
                    out.push(')');
                }
            }
            UnOp::Not | UnOp::Neg => {
                out.push(if matches!(op, UnOp::Not) { '!' } else { '-' });
                emit_expr(operand, rule, PREC_UNARY, out);
            }
        },

        IrExprKind::Binary(op, l, r) => {
            let prec = precedence(*op);
            let wrap = prec < parent;
            if wrap {
                out.push('(');
            }
            emit_expr(l, rule, prec, out);
            out.push(' ');
            out.push_str(operator(*op));
            out.push(' ');
            // The right operand binds one tighter, so `a - (b - c)` keeps its
            // parentheses while `(a - b) - c` loses them.
            emit_expr(r, rule, prec + 1, out);
            if wrap {
                out.push(')');
            }
        }
    }
}

/// expr has no bindings, so one is replaced by what it was bound to.
///
/// Terminates because bindings are rule-scoped and may only refer to earlier
/// ones. A binding used twice is emitted twice, which is why `let` is worth
/// keeping in the source even though it vanishes here.
fn emit_binding(rule: &IrRule, slot: u32, parent: u8, out: &mut String) {
    emit_expr(&rule.lets[slot as usize], rule, parent, out);
}

/// Predicates whose Go spelling capitalises an acronym, which no rule can
/// recover from the kebab name — `apcs` could be `Apcs` or `APCs`.
///
/// `go_name_matches_the_manifest` holds this to exactly the set that needs it,
/// so an entry cannot go stale and a new acronym cannot be forgotten.
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
