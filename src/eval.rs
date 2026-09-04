//! Tree-walking interpreter over `Ir`.
//!
//! Also the oracle for any later backend: two implementations that must agree on
//! the whole corpus is a better property test than reading bytecode.
//!
//! Runs on lowered rules, so there are no names left to resolve and no `count`
//! to disambiguate — every remaining failure would be a compiler bug rather than
//! bad input, which is why nothing here reports a diagnostic.

use crate::ast::{BinOp, UnOp};
use crate::env::{self, Predicate};
use crate::ir::{Ir, IrExpr, IrExprKind, IrRule, ParamValue, ParamValues};
use crate::state::State;
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
    pub fired: Vec<&'a IrRule>,
}

/// Runs a rule set against one state.
///
/// A faithful port of Go's `Evaluate`, including the part that matters most for
/// agreement: a rule in a category already claimed by an exclusive rule is
/// **skipped without being evaluated**. Evaluating it anyway would give the same
/// firings but a different count of predicate calls, which the differential test
/// would notice.
pub fn evaluate<'a>(ir: &'a Ir, params: &ParamValues, state: &State) -> Firing<'a> {
    // Resolved once, not per rule: a priority sees only parameters, which do not
    // change within a doctrine window. This is the first of the two phases.
    let mut order: Vec<(i64, &IrRule)> =
        ir.rules.iter().map(|r| (priority(r, params), r)).collect();
    order.sort_by_key(|(p, _)| Reverse(*p));
    let order: Vec<&IrRule> = order.into_iter().map(|(_, r)| r).collect();

    let mut fired_categories: HashSet<u32> = HashSet::new();
    let mut firing = Firing::default();

    for rule in order {
        if fired_categories.contains(&rule.category.0) {
            continue;
        }
        if !rule_fires(rule, params, state) {
            continue;
        }
        firing.fired.push(rule);
        if rule.exclusive {
            fired_categories.insert(rule.category.0);
        }
    }

    firing
}

/// A rule's priority, resolved against a doctrine.
///
/// Separate from `rule_fires` because the phases are separate: this runs once
/// when a doctrine lands, that runs every tick.
pub fn priority(rule: &IrRule, params: &ParamValues) -> i64 {
    match static_eval(&rule.priority, params) {
        Value::Int(n) => n,
        other => unreachable!("priority is {other:?}, which `check` should have rejected"),
    }
}

/// Evaluates an expression that reads no state.
///
/// Takes no `State`, which is the phase separation made structural rather than
/// promised: a priority physically cannot consult the game. The arms it does not
/// handle are the ones `check` rejects in `Phase::Static`.
pub(crate) fn static_eval(e: &IrExpr, params: &ParamValues) -> Value {
    match &e.kind {
        IrExprKind::Int(n) => Value::Int(*n),
        IrExprKind::Float(f) => Value::Float(*f),
        IrExprKind::Param(slot) => param_value(params, *slot),
        IrExprKind::Builtin(id, args) => {
            let args: Vec<Value> = args.iter().map(|a| static_eval(a, params)).collect();
            apply_builtin(*id, &args)
        }
        IrExprKind::Unary(op, operand) => unary(*op, static_eval(operand, params)),
        IrExprKind::Binary(op, l, r) => binary(*op, static_eval(l, params), static_eval(r, params)),
        other => unreachable!("`{other:?}` reads state and cannot be static"),
    }
}

/// One doctrine value, by slot.
fn param_value(params: &ParamValues, slot: u32) -> Value {
    match params.values.get(slot as usize) {
        Some(ParamValue::Int(n)) => Value::Int(*n),
        Some(ParamValue::Float(f)) => Value::Float(*f),
        // A caller supplied fewer values than the rule set declares. Not a
        // compiler bug, but nothing here can carry on without a number.
        None => panic!("no value for parameter slot {slot}"),
    }
}

/// `lerp` and `lerpf`, matching Go's `doctrine.go` exactly — including that
/// `lerp` rounds rather than truncates.
fn apply_builtin(id: env::Builtin, args: &[Value]) -> Value {
    match (id, args) {
        (env::Builtin::Lerp, [min, max, t]) => {
            let (min, max) = (as_f64(*min), as_f64(*max));
            // Saturating, like every other integer path here: a doctrine is not
            // trusted to keep the arithmetic in range.
            Value::Int((min + ((max - min) * as_f64(*t)).round()) as i64)
        }
        (env::Builtin::Lerpf, [min, max, t]) => {
            let (min, max) = (as_f64(*min), as_f64(*max));
            Value::Float(min + (max - min) * as_f64(*t))
        }
        _ => unreachable!("{id:?} with {} arguments", args.len()),
    }
}

/// Whether every `require` holds.
pub fn rule_fires(rule: &IrRule, params: &ParamValues, state: &State) -> bool {
    let ev = Evaluator::for_rule(rule, params, state);
    for require in &rule.requires {
        if !expect_bool(ev.eval(require)) {
            return false;
        }
    }
    true
}

/// Whether each `require` holds, evaluated independently.
///
/// Deliberately does **not** short-circuit, unlike `rule_fires`: this exists to
/// measure which conjuncts a corpus actually exercises, and a conjunct never
/// reached tells you nothing about whether it works.
pub fn conjuncts(rule: &IrRule, params: &ParamValues, state: &State) -> Vec<bool> {
    let ev = Evaluator::for_rule(rule, params, state);
    rule.requires
        .iter()
        .map(|r| expect_bool(ev.eval(r)))
        .collect()
}

/// One rule's evaluation.
///
/// Mirrors `RuleChecker`: the scope is born and dropped with the rule, so a
/// binding cannot leak into the next one.
struct Evaluator<'a> {
    state: &'a State,
    #[allow(dead_code, reason = "read once `Param` and `Builtin` are evaluated")]
    params: &'a ParamValues,
    /// `let` bindings by slot. Lowering numbered them, so nothing here searches
    /// by name — and a binding can only refer to an earlier one, which is why
    /// filling this in order is enough.
    scope: Vec<Value>,
}

impl<'a> Evaluator<'a> {
    /// Evaluates the rule's bindings, leaving the scope ready for its requires.
    fn for_rule(rule: &IrRule, params: &'a ParamValues, state: &'a State) -> Self {
        let mut ev = Evaluator {
            state,
            params,
            scope: Vec::with_capacity(rule.lets.len()),
        };
        for binding in &rule.lets {
            let value = ev.eval(binding);
            ev.scope.push(value);
        }
        ev
    }

    fn eval(&self, e: &IrExpr) -> Value {
        match &e.kind {
            IrExprKind::Int(n) => Value::Int(*n),
            IrExprKind::Float(f) => Value::Float(*f),
            IrExprKind::Binding(slot) => self.scope[*slot as usize],
            IrExprKind::Param(slot) => param_value(self.params, *slot),
            IrExprKind::Builtin(id, args) => {
                let args: Vec<Value> = args.iter().map(|a| self.eval(a)).collect();
                apply_builtin(*id, &args)
            }
            IrExprKind::Predicate(id, args) => self.apply(*id, args),
            // Only ever an argument, read as a name by `key` and `arg_name`.
            IrExprKind::Member(..) => unreachable!("enum member in a value position"),

            IrExprKind::Unary(op, operand) => unary(*op, self.eval(operand)),

            IrExprKind::Binary(op, left, right) => match op {
                BinOp::And => {
                    Value::Bool(expect_bool(self.eval(left)) && expect_bool(self.eval(right)))
                }
                BinOp::Or => {
                    Value::Bool(expect_bool(self.eval(left)) || expect_bool(self.eval(right)))
                }
                _ => binary(*op, self.eval(left), self.eval(right)),
            },
        }
    }

    fn apply(&self, id: Predicate, args: &[IrExpr]) -> Value {
        let st = self.state;
        match id {
            Predicate::AircraftCapacity => Value::Int(st.scalar("aircraft-capacity") as i64),
            Predicate::AxisBurned => Value::Bool(st.call_bool(&key("axis-burned", args))),
            Predicate::BaseUnderAttack => Value::Bool(st.flag("base-under-attack")),
            Predicate::BestAirTarget => Value::Opt(st.is_present("best-air-target")),
            Predicate::BestGroundTarget => Value::Opt(st.is_present("best-ground-target")),
            Predicate::BuildingCount => Value::Int(st.type_count(arg_name(&args[0]))),
            Predicate::CanBuild => Value::Bool(st.call_bool(&key("can-build", args))),
            Predicate::CanBuildAnyCombatAircraft => {
                Value::Bool(st.flag("can-build-any-combat-aircraft"))
            }
            Predicate::CanBuildAnyCombatVehicle => {
                Value::Bool(st.flag("can-build-any-combat-vehicle"))
            }
            Predicate::CanBuildAnySpecialist => Value::Bool(st.flag("can-build-any-specialist")),
            Predicate::CanBuildRole => Value::Bool(st.call_bool(&key("can-build-role", args))),
            Predicate::CanBuildTransport => Value::Bool(st.flag("can-build-transport")),
            Predicate::CapturableCount => Value::Int(st.scalar("capturable-count") as i64),
            Predicate::Cash => Value::Int(st.scalar("cash") as i64),
            Predicate::CombatAircraftCount => Value::Int(st.scalar("combat-aircraft-count") as i64),
            Predicate::CombatVehicleCount => Value::Int(st.scalar("combat-vehicle-count") as i64),
            Predicate::CriticalBuildingUnderAttack => {
                Value::Bool(st.flag("critical-building-under-attack"))
            }
            Predicate::DamagedBuildings => Value::Int(st.collection("damaged-buildings")),
            Predicate::DamagedCombatUnits => {
                Value::Int(st.collection(&key("damaged-combat-units", args)))
            }
            Predicate::EnemiesVisible => Value::Bool(st.flag("enemies-visible")),
            Predicate::EngineerNearCapturable => Value::Bool(st.flag("engineer-near-capturable")),
            Predicate::HarvestersInDanger => {
                Value::Int(st.collection(&key("harvesters-in-danger", args)))
            }
            Predicate::HasBuilding => Value::Bool(st.type_count(arg_name(&args[0])) > 0),
            Predicate::HasEnemyIntel => Value::Bool(st.flag("has-enemy-intel")),
            Predicate::HasRetreatingUnits => Value::Bool(st.flag("has-retreating-units")),
            Predicate::HasRole => Value::Bool(st.call_bool(&key("has-role", args))),
            Predicate::HasScout => Value::Bool(st.flag("has-scout")),
            Predicate::HasUnit => Value::Bool(st.type_count(arg_name(&args[0])) > 0),
            Predicate::IdleCombatAircraft => Value::Int(st.collection("idle-combat-aircraft")),
            Predicate::IdleCombatInfantry => Value::Int(st.collection("idle-combat-infantry")),
            Predicate::IdleCombatLoadedApcs => Value::Int(st.collection("idle-combat-loaded-apcs")),
            Predicate::IdleEmptyApcs => Value::Int(st.collection("idle-empty-apcs")),
            Predicate::IdleEngineerLoadedApcs => {
                Value::Int(st.collection("idle-engineer-loaded-apcs"))
            }
            Predicate::IdleEngineers => Value::Int(st.collection("idle-engineers")),
            Predicate::IdleGroundUnits => Value::Int(st.collection("idle-ground-units")),
            Predicate::IdleHarvesters => Value::Int(st.collection("idle-harvesters")),
            Predicate::IdleMinelayers => Value::Int(st.collection("idle-minelayers")),
            Predicate::IdleNavalUnits => Value::Int(st.collection("idle-naval-units")),
            Predicate::IdleScouts => Value::Int(st.collection("idle-scouts")),
            Predicate::IsRushed => Value::Bool(st.flag("is-rushed")),
            Predicate::LostRole => Value::Bool(st.call_bool(&key("lost-role", args))),
            Predicate::MapHasWater => Value::Bool(st.flag("map-has-water")),
            Predicate::NearBaseGroundUnits => Value::Int(st.collection("near-base-ground-units")),
            Predicate::NearestEnemy => Value::Opt(st.is_present("nearest-enemy")),
            Predicate::OverextendedSquadMembers => {
                Value::Int(st.collection(&key("overextended-squad-members", args)))
            }
            Predicate::PowerExcess => Value::Int(st.scalar("power-excess") as i64),
            Predicate::QueueBusy => Value::Bool(st.call_bool(&key("queue-busy", args))),
            Predicate::QueueProducingRole => {
                Value::Bool(st.call_bool(&key("queue-producing-role", args)))
            }
            Predicate::QueueReady => Value::Bool(st.call_bool(&key("queue-ready", args))),
            Predicate::ResourcesNearCap => Value::Bool(st.flag("resources-near-cap")),
            Predicate::RoleCount => Value::Int(st.call_int(&key("role-count", args))),
            Predicate::SpecialistInfantryCount => {
                Value::Int(st.scalar("specialist-infantry-count") as i64)
            }
            Predicate::SquadAwayFromBase => {
                Value::Bool(st.call_bool(&key("squad-away-from-base", args)))
            }
            Predicate::SquadExists => Value::Bool(st.call_bool(&key("squad-exists", args))),
            Predicate::SquadIdleCount => Value::Int(st.call_int(&key("squad-idle-count", args))),
            Predicate::SquadNeedsReinforcement => {
                Value::Bool(st.call_bool(&key("squad-needs-reinforcement", args)))
            }
            Predicate::SquadReadyRatio => {
                Value::Float(st.call_float(&key("squad-ready-ratio", args)))
            }
            Predicate::SquadThreatRatio => {
                Value::Float(st.call_float(&key("squad-threat-ratio", args)))
            }
            Predicate::SupportPowerReady => {
                Value::Bool(st.call_bool(&key("support-power-ready", args)))
            }
            Predicate::TransportCount => Value::Int(st.scalar("transport-count") as i64),
            Predicate::UnassignedIdleAir => Value::Int(st.collection("unassigned-idle-air")),
            Predicate::UnassignedIdleGround => Value::Int(st.collection("unassigned-idle-ground")),
            Predicate::UnassignedIdleNaval => Value::Int(st.collection("unassigned-idle-naval")),
            Predicate::UnitCount => Value::Int(st.type_count(arg_name(&args[0]))),
        }
    }
}

/// The literal name of an argument in an enum position.
///
/// Panics on anything else, which the type checker has already ruled out.
/// Arithmetic and comparison, shared by the two evaluators.
///
/// Free rather than a method because it never needed `self`, and the static
/// evaluator has no `self` to give it.
fn binary(op: BinOp, l: Value, r: Value) -> Value {
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

/// `not`, negation and `exists`.
fn unary(op: UnOp, v: Value) -> Value {
    match op {
        UnOp::Not => Value::Bool(!expect_bool(v)),
        UnOp::Neg => match v {
            Value::Int(n) => Value::Int(-n),
            Value::Float(f) => Value::Float(-f),
            other => unreachable!("cannot negate {other:?}"),
        },
        UnOp::Exists => match v {
            Value::Opt(present) => Value::Bool(present),
            other => unreachable!("`exists` on {other:?}"),
        },
    }
}

/// The state key a call is recorded under. Arguments in enum positions are
/// literal names; numeric ones are rendered so a float argument keys distinctly.
fn key(name: &str, args: &[IrExpr]) -> String {
    let rendered: Vec<String> = args
        .iter()
        .map(|a| match &a.kind {
            IrExprKind::Member(d, i) => env::member_name(*d, *i).to_string(),
            IrExprKind::Int(n) => n.to_string(),
            IrExprKind::Float(f) => crate::state::render_number(*f),
            other => unreachable!("unexpected argument {other:?}"),
        })
        .collect();
    crate::state::call_key(name, &rendered)
}

fn arg_name(e: &IrExpr) -> &str {
    match &e.kind {
        IrExprKind::Member(d, i) => env::member_name(*d, *i),
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

    fn seed() -> Ir {
        let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/rules/seed.vy"))
            .expect("seed.vy");
        let (tokens, ld) = lex(&src);
        assert!(ld.is_empty(), "{ld:?}");
        let (ast, pd) = parse(&tokens);
        assert!(pd.is_empty(), "{pd:?}");
        crate::lower::lower(&ast)
    }

    /// Keyed state is verbose to write by hand, so these build it from the
    /// predicate calls a rule would make rather than from raw JSON.
    fn state(spec: &[(&str, &str)]) -> State {
        let mut st = State::default();
        for (kind, key) in spec {
            match *kind {
                "flag" => {
                    st.flags.insert(key.to_string());
                }
                "call" => {
                    st.calls_bool.insert(key.to_string());
                }
                "present" => {
                    st.present.insert(key.to_string());
                }
                other => {
                    let (k, v) = key.split_once('=').expect("num needs key=value");
                    match other {
                        "scalar" => {
                            st.scalars.insert(k.to_string(), v.parse().unwrap());
                        }
                        "coll" => {
                            st.collections.insert(k.to_string(), v.parse().unwrap());
                        }
                        "type" => {
                            st.type_counts.insert(k.to_string(), v.parse().unwrap());
                        }
                        _ => unreachable!("unknown spec kind `{other}`"),
                    }
                }
            }
        }
        st
    }

    fn fired(ir: &Ir, st: &State) -> Vec<String> {
        evaluate(ir, &ParamValues::default(), st)
            .fired
            .into_iter()
            .map(|r| r.name.clone())
            .collect()
    }

    #[test]
    fn an_exclusive_rule_blocks_its_category() {
        // Both economy rules qualify here; only the higher-priority one fires,
        // and the other is skipped without being evaluated.
        let st = state(&[
            ("scalar", "cash=5000"),
            ("scalar", "power-excess=40"),
            ("type", "powr=0"),
            ("call", "can-build(Building,powr)"),
            ("call", "can-build(Building,proc)"),
        ]);
        assert_eq!(fired(&seed(), &st), vec!["build-power"]);
    }

    #[test]
    fn non_exclusive_rules_in_one_category_all_fire() {
        // `defend-base` and `attack-idle-units` are both `combat`, neither
        // exclusive.
        let st = state(&[
            ("flag", "base-under-attack"),
            ("present", "nearest-enemy"),
            ("coll", "idle-ground-units=6"),
        ]);
        let names = fired(&seed(), &st);
        assert!(names.contains(&"defend-base".to_string()), "{names:?}");
        assert!(
            names.contains(&"attack-idle-units".to_string()),
            "{names:?}"
        );
    }

    #[test]
    fn firings_come_in_priority_order() {
        let st = state(&[
            ("scalar", "cash=5000"),
            ("scalar", "power-excess=40"),
            ("type", "powr=0"),
            ("type", "e1=0"),
            ("call", "can-build(Building,powr)"),
            ("call", "has-role(barracks)"),
            ("coll", "idle-harvesters=2"),
        ]);
        assert_eq!(
            fired(&seed(), &st),
            vec!["build-power", "produce-infantry", "return-idle-harvesters"]
        );
    }

    #[test]
    fn an_empty_state_fires_nothing() {
        assert!(fired(&seed(), &State::default()).is_empty());
    }

    #[test]
    fn arithmetic_never_panics_on_a_source_file() {
        let rule = |req: &str| {
            let src = format!(
                "rule r {{\n priority 1\n category economy\n do scout\n require {req}\n}}\n"
            );
            let (t, _) = lex(&src);
            let (a, d) = parse(&t);
            assert!(d.is_empty(), "{d:?}");
            crate::lower::lower(&a)
        };

        // Division by zero used to panic here.
        let st = state(&[("scalar", "cash=10")]);
        assert!(fired(&rule("cash / 0 > 1"), &st).is_empty());

        // Saturating rather than overflowing, and identical in debug and release.
        let st = state(&[("scalar", "cash=9223372036854775807")]);
        assert_eq!(fired(&rule("cash * cash > 0"), &st), vec!["r"]);
    }
}
