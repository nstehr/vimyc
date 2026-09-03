//! Tree-walking interpreter.
//!
//! Also the oracle for any later backend: two implementations that must agree on
//! the whole corpus is a better property test than reading bytecode.
//!
//! Assumes the rule set type checked. A `Value` of the wrong shape here is a
//! compiler bug rather than bad input, so this reports nothing and panics on
//! nothing — see `expect_bool`.

use crate::ast::{Ast, BinOp, Expr, ExprKind, Rule, UnOp};
use crate::env::{self, Predicate};
use crate::state::State;
use crate::types::Domain;
use std::cmp::Reverse;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    /// Whether an optional is present. The payload is unreachable from the
    /// language, so there is nothing to carry.
    Opt(bool),
}

/// Which rules fired, in the order they fired.
#[derive(Debug, Default)]
pub struct Firing<'a> {
    pub fired: Vec<&'a Rule>,
}

/// Runs a rule set against one state.
///
/// A faithful port of Go's `Evaluate`, including the part that matters most for
/// agreement: a rule in a category already claimed by an exclusive rule is
/// **skipped without being evaluated**. Evaluating it anyway would give the same
/// firings but a different count of predicate calls, which the differential test
/// would notice.
pub fn evaluate<'a>(ast: &'a Ast, state: &State) -> Firing<'a> {
    let mut order: Vec<&Rule> = ast.rules.iter().collect();
    order.sort_by_key(|r| Reverse(r.priority));

    let mut fired_categories: HashSet<&str> = HashSet::new();
    let mut firing = Firing::default();

    for rule in order {
        if fired_categories.contains(rule.category.text.as_str()) {
            continue;
        }
        if !rule_fires(rule, state) {
            continue;
        }
        firing.fired.push(rule);
        if rule.exclusive {
            fired_categories.insert(rule.category.text.as_str());
        }
    }

    firing
}

/// Whether every `require` holds.
pub fn rule_fires(rule: &Rule, state: &State) -> bool {
    let mut ev = Evaluator {
        state,
        scope: Vec::new(),
    };
    for binding in &rule.lets {
        let value = ev.eval(&binding.value);
        ev.scope.push((binding.name.text.clone(), value));
    }
    for require in &rule.requires {
        if !expect_bool(ev.eval(require)) {
            return false;
        }
    }
    true
}

/// One rule's evaluation.
///
/// Mirrors `RuleChecker`: the scope is born and dropped with the rule, so a
/// binding cannot leak into the next one.
struct Evaluator<'a> {
    state: &'a State,
    /// `let` bindings. `&self` on `eval` is deliberate — evaluation has no
    /// effects, and only binding mutates.
    scope: Vec<(String, Value)>,
}

impl Evaluator<'_> {
    fn eval(&self, e: &Expr) -> Value {
        match &e.kind {
            ExprKind::Int(n) => Value::Int(*n),
            ExprKind::Float(f) => Value::Float(*f),
            ExprKind::Ident(name) => self.lookup(&name.text),
            ExprKind::Call(name, args) => self.eval_call(&name.text, args),

            ExprKind::Unary(op, operand) => match op {
                UnOp::Not => Value::Bool(!expect_bool(self.eval(operand))),
                UnOp::Neg => match self.eval(operand) {
                    Value::Int(n) => Value::Int(-n),
                    Value::Float(f) => Value::Float(-f),
                    other => unreachable!("cannot negate {other:?}"),
                },
                UnOp::Exists => match self.eval(operand) {
                    Value::Opt(present) => Value::Bool(present),
                    other => unreachable!("`exists` on {other:?}"),
                },
            },

            ExprKind::Binary(op, left, right) => match op {
                BinOp::And => {
                    Value::Bool(expect_bool(self.eval(left)) && expect_bool(self.eval(right)))
                }
                BinOp::Or => {
                    Value::Bool(expect_bool(self.eval(left)) || expect_bool(self.eval(right)))
                }
                _ => self.eval_binary(*op, self.eval(left), self.eval(right)),
            },

            ExprKind::Error => unreachable!("evaluated a tree that did not check"),
        }
    }

    fn eval_binary(&self, op: BinOp, l: Value, r: Value) -> Value {
        if let (Value::Bool(a), Value::Bool(b)) = (l, r) {
            return match op {
                BinOp::Eq => Value::Bool(a == b),
                BinOp::NotEq => Value::Bool(a != b),
                other => unreachable!("{other:?} on bools"),
            };
        }

        let (a, b) = (as_f64(l), as_f64(r));
        match op {
            BinOp::Eq => Value::Bool(a == b),
            BinOp::NotEq => Value::Bool(a != b),
            BinOp::Lt => Value::Bool(a < b),
            BinOp::LtEq => Value::Bool(a <= b),
            BinOp::Gt => Value::Bool(a > b),
            BinOp::GtEq => Value::Bool(a >= b),
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                if let (Value::Int(x), Value::Int(y)) = (l, r) {
                    Value::Int(match op {
                        BinOp::Add => x.saturating_add(y),
                        BinOp::Sub => x.saturating_sub(y),
                        BinOp::Mul => x.saturating_mul(y),
                        BinOp::Div => x.checked_div(y).unwrap_or(0),
                        other => unreachable!("not arithmetic: {other:?}"),
                    })
                } else {
                    Value::Float(match op {
                        BinOp::Add => a + b,
                        BinOp::Sub => a - b,
                        BinOp::Mul => a * b,
                        // Floats need no guard: division by zero is infinity,
                        // not a trap.
                        BinOp::Div => a / b,
                        other => unreachable!("not arithmetic: {other:?}"),
                    })
                }
            }
            BinOp::And | BinOp::Or => unreachable!("handled by the caller"),
        }
    }

    /// A call, dispatched on the predicate name.
    ///
    /// Arguments in enum positions are read as *names*, not evaluated:
    /// `barracks` in `has-role(barracks)` is a literal.
    fn eval_call(&self, name: &str, args: &[Expr]) -> Value {
        if name == env::COUNT {
            return Value::Int(self.count(arg_name(&args[0])));
        }
        let sig =
            env::predicate(name).unwrap_or_else(|| unreachable!("unknown predicate `{name}`"));
        self.apply(sig.id, args)
    }

    fn apply(&self, id: Predicate, args: &[Expr]) -> Value {
        let st = self.state;
        match id {
            Predicate::Cash => Value::Int(st.cash),
            Predicate::PowerExcess => Value::Int(st.power_excess),
            Predicate::BaseUnderAttack => Value::Bool(st.base_under_attack),
            Predicate::EnemiesVisible => Value::Bool(st.enemies_visible),
            Predicate::HasEnemyIntel => Value::Bool(st.has_enemy_intel),
            Predicate::NearestEnemy => Value::Opt(st.nearest_enemy),
            Predicate::HasUnit => Value::Bool(st.has_unit(arg_name(&args[0]))),
            Predicate::HasBuilding => Value::Bool(st.has_building(arg_name(&args[0]))),
            Predicate::HasRole => Value::Bool(st.has_role(arg_name(&args[0]))),
            Predicate::CanBuildRole => Value::Bool(st.can_build_role(arg_name(&args[0]))),
            Predicate::QueueBusy => Value::Bool(st.queue_busy(arg_name(&args[0]))),
            Predicate::QueueReady => Value::Bool(st.queue_ready(arg_name(&args[0]))),
            Predicate::CanBuild => {
                Value::Bool(st.can_build(arg_name(&args[0]), arg_name(&args[1])))
            }
            Predicate::SquadReadyRatio => Value::Float(st.squad_ready_ratio(arg_name(&args[0]))),
        }
    }

    fn count(&self, name: &str) -> i64 {
        if env::collection(name).is_some() {
            return self.state.collection_len(name);
        }
        if env::member(Domain::BuildingType, name).is_some() {
            return self.state.building_count(name);
        }
        self.state.unit_count(name)
    }

    /// A bare name: a `let` binding, or a zero-argument predicate.
    ///
    /// Enum literals never reach here — they are read as names in argument
    /// position and never become values.
    fn lookup(&self, name: &str) -> Value {
        if let Some((_, v)) = self.scope.iter().find(|(n, _)| n == name) {
            return *v;
        }
        let sig = env::predicate(name).unwrap_or_else(|| unreachable!("unknown name `{name}`"));
        self.apply(sig.id, &[])
    }
}

/// The literal name of an argument in an enum position.
///
/// Panics on anything else, which the type checker has already ruled out.
fn arg_name(e: &Expr) -> &str {
    match &e.kind {
        ExprKind::Ident(name) => &name.text,
        other => unreachable!("expected a name, got {other:?}"),
    }
}

fn as_f64(v: Value) -> f64 {
    match v {
        Value::Int(n) => n as f64,
        Value::Float(f) => f,
        other => unreachable!("expected a number, got {other:?}"),
    }
}

/// Unwraps a `Value` the checker guaranteed is a bool.
fn expect_bool(v: Value) -> bool {
    match v {
        Value::Bool(b) => b,
        // Not a diagnostic: reaching here means the checker let through
        // something it should not have.
        other => unreachable!("expected bool, got {other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;

    fn seed() -> Ast {
        let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/rules/seed.vy"))
            .expect("seed.vy");
        let (tokens, ld) = lex(&src);
        assert!(ld.is_empty(), "{ld:?}");
        let (ast, pd) = parse(&tokens);
        assert!(pd.is_empty(), "{pd:?}");
        ast
    }

    fn fired(ast: &Ast, json: &str) -> Vec<String> {
        let state: State = serde_json::from_str(json).expect("state");
        evaluate(ast, &state)
            .fired
            .into_iter()
            .map(|r| r.name.text.clone())
            .collect()
    }

    #[test]
    fn an_exclusive_rule_blocks_its_category() {
        // Both economy rules are eligible here; only the higher-priority one
        // fires, and the other is skipped without being evaluated.
        let names = fired(
            &seed(),
            r#"{"cash": 5000, "power_excess": 40, "buildings": {"fact": 1},
                "can_build": ["Building/powr", "Building/proc"]}"#,
        );
        assert_eq!(names, vec!["build-power"]);
    }

    #[test]
    fn non_exclusive_rules_in_one_category_all_fire() {
        // `defend-base` and `attack-idle-units` are both `combat`, neither
        // exclusive.
        let names = fired(
            &seed(),
            r#"{"base_under_attack": true, "nearest_enemy": true,
                "collections": {"idle-ground-units": 6}}"#,
        );
        assert!(names.contains(&"defend-base".to_string()), "{names:?}");
        assert!(
            names.contains(&"attack-idle-units".to_string()),
            "{names:?}"
        );
    }

    #[test]
    fn firings_come_in_priority_order() {
        let names = fired(
            &seed(),
            r#"{"cash": 5000, "power_excess": 40, "buildings": {"fact": 1},
                "can_build": ["Building/powr"], "roles": ["barracks"],
                "collections": {"idle-harvesters": 2}}"#,
        );
        assert_eq!(
            names,
            vec!["build-power", "produce-infantry", "return-idle-harvesters"]
        );
    }

    #[test]
    fn an_empty_state_fires_nothing() {
        assert!(fired(&seed(), "{}").is_empty());
    }

    #[test]
    fn arithmetic_never_panics_on_a_source_file() {
        let ast = |src: &str| {
            let (t, _) = lex(src);
            let (a, d) = parse(&t);
            assert!(d.is_empty(), "{d:?}");
            a
        };
        let rule = |req: &str| {
            format!("rule r {{\n priority 1\n category economy\n do scout\n require {req}\n}}\n")
        };

        // Division by zero used to panic here.
        assert!(fired(&ast(&rule("cash / 0 > 1")), r#"{"cash": 10}"#).is_empty());
        // Saturating rather than overflowing, and identical in debug and release.
        assert_eq!(
            fired(
                &ast(&rule("cash * cash > 0")),
                r#"{"cash": 9223372036854775807}"#
            ),
            vec!["r"]
        );
    }
}
