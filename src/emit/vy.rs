//! Printing the IR back as `.vy`.
//!
//! Not a formatter: this prints the rule set *after* lowering and specialising —
//! numbers folded, doctrine-settled gates gone, `count(powr)` now
//! `building-count(powr)`. The `cargo expand` of this compiler.
//!
//! Which makes it what the dashboard should show. It rendered expr before, so
//! reading it meant translating back into the language the rules are written in.
//! `emitted_vy_round_trips` keeps "what you read is what runs" a property.

use crate::env;
use crate::ir::{Ir, IrExpr, IrExprKind, IrRule, ParamValues};
use crate::types::Domain;

/// One rule, printed.
pub fn emit(ir: &Ir, params: &ParamValues) -> Vec<String> {
    ir.rules.iter().map(|r| rule(r, params)).collect()
}

/// The whole set, as a file that parses.
pub fn emit_file(ir: &Ir, params: &ParamValues) -> String {
    emit(ir, params).join("\n\n") + "\n"
}

/// One rule as `.vy`, for the expr backend to carry alongside its condition.
pub(crate) fn rule_source(r: &IrRule, params: &ParamValues) -> String {
    rule(r, params)
}

fn rule(r: &IrRule, params: &ParamValues) -> String {
    let mut out = format!("rule {} {{\n", r.name);
    out.push_str(&format!(
        "  priority {}\n",
        crate::eval::priority(r, params)
    ));
    out.push_str(&format!(
        "  category {}{}\n",
        env::category_name(r.category.0),
        if r.exclusive { " exclusive" } else { "" }
    ));
    if let Some(why) = &r.because {
        // The one field written to be read, so it goes where a reader looks
        // first — above the mechanics, as in the hand-written sources.
        out.push_str(&format!("  because {}\n", quote(why)));
    }
    out.push_str(&format!("  do {}\n", action(r, params)));

    // Bindings are inlined by lowering, so there are no `let`s left to print;
    // a binding that was used twice appears twice, which is the honest picture
    // of what runs.
    for require in &r.requires {
        out.push_str("  require ");
        expr(require, r, params, PREC_LOWEST, &mut out);
        out.push('\n');
    }
    if r.requires.is_empty() {
        // Vacuously true, and a rule with no `require` is legal — say so rather
        // than printing something that reads as a truncation.
        out.push_str("  require true\n");
    }
    out.push('}');
    out
}

fn action(r: &IrRule, params: &ParamValues) -> String {
    let name = env::action_name(r.action.id.0);
    if r.action.args.is_empty() {
        return name.to_string();
    }
    let args: Vec<String> = r
        .action
        .args
        .iter()
        .map(|a| {
            let mut s = String::new();
            expr(a, r, params, PREC_ATOM, &mut s);
            s
        })
        .collect();
    format!("{name}({})", args.join(", "))
}

// The language's own precedence, loosest first. Mirrors the grammar rather than
// expr's, because this prints `.vy`.
const PREC_LOWEST: u8 = 0;
const PREC_OR: u8 = 1;
const PREC_AND: u8 = 2;
const PREC_CMP: u8 = 3;
const PREC_ADD: u8 = 4;
const PREC_MUL: u8 = 5;
const PREC_UNARY: u8 = 6;
const PREC_ATOM: u8 = 7;

fn binding_power(op: crate::ast::BinOp) -> u8 {
    use crate::ast::BinOp::*;
    match op {
        Or => PREC_OR,
        And => PREC_AND,
        Eq | NotEq | Lt | LtEq | Gt | GtEq => PREC_CMP,
        Add | Sub => PREC_ADD,
        Mul | Div => PREC_MUL,
    }
}

fn expr(e: &IrExpr, r: &IrRule, params: &ParamValues, parent: u8, out: &mut String) {
    match &e.kind {
        IrExprKind::Int(n) => out.push_str(&n.to_string()),
        IrExprKind::Float(f) => out.push_str(&crate::state::render_number(*f)),
        IrExprKind::Bool(b) => out.push_str(if *b { "true" } else { "false" }),

        // Folded, like everything else the doctrine settled.
        IrExprKind::Param(_) | IrExprKind::Builtin(..) => {
            out.push_str(&fold(e, params));
        }

        IrExprKind::Member(domain, index) => out.push_str(&member(*domain, *index)),

        // `let` does not survive lowering, so the value takes its place.
        IrExprKind::Binding(slot) => expr(&r.lets[*slot as usize], r, params, parent, out),

        IrExprKind::Predicate(id, args) => {
            let sig = env::PREDICATES
                .iter()
                .find(|s| s.id == *id)
                .unwrap_or_else(|| unreachable!("predicate has no signature"));

            // A collection is only ever counted, and `count(...)` is how the
            // language says that.
            let counted = sig.ret == crate::types::Type::Collection;
            if counted {
                out.push_str("count(");
            }
            out.push_str(sig.name);
            if !args.is_empty() {
                out.push('(');
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    expr(a, r, params, PREC_ATOM, out);
                }
                out.push(')');
            }
            if counted {
                out.push(')');
            }
        }

        IrExprKind::Unary(op, operand) => {
            use crate::ast::UnOp::*;
            let wrap = parent > PREC_UNARY;
            if wrap {
                out.push('(');
            }
            match op {
                Not => out.push_str("not "),
                Neg => out.push('-'),
                Exists => out.push_str("exists "),
            }
            expr(operand, r, params, PREC_UNARY, out);
            if wrap {
                out.push(')');
            }
        }

        IrExprKind::Binary(op, l, rhs) => {
            let prec = binding_power(*op);
            let wrap = prec < parent;
            if wrap {
                out.push('(');
            }
            expr(l, r, params, prec, out);
            out.push_str(&format!(" {} ", spelling(*op)));
            // The right operand binds one tighter, so `a - (b - c)` keeps its
            // parentheses while `(a - b) - c` loses them.
            expr(rhs, r, params, prec + 1, out);
            if wrap {
                out.push(')');
            }
        }
    }
}

fn spelling(op: crate::ast::BinOp) -> &'static str {
    use crate::ast::BinOp::*;
    match op {
        Or => "or",
        And => "and",
        Eq => "==",
        NotEq => "!=",
        Lt => "<",
        LtEq => "<=",
        Gt => ">",
        GtEq => ">=",
        Add => "+",
        Sub => "-",
        Mul => "*",
        Div => "/",
    }
}

/// A member in the language's spelling, which is where it differs from the expr
/// backend: kebab throughout, and no quotes.
fn member(domain: Domain, index: u32) -> String {
    env::member_name(domain, index).to_string()
}

fn fold(e: &IrExpr, params: &ParamValues) -> String {
    match crate::eval::static_eval(e, params) {
        crate::eval::Value::Int(n) => n.to_string(),
        crate::eval::Value::Float(f) => crate::state::render_number(f),
        crate::eval::Value::Bool(b) => (if b { "true" } else { "false" }).to_string(),
        other => unreachable!("folded to {other:?}"),
    }
}

/// `because` is the only string in the language, and it has no escapes — so a
/// quote inside one would produce something that does not parse.
fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "'"))
}
