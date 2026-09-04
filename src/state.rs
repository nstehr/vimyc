//! Game state, as a condition can observe it.
//!
//! A projection of Go's `RuleEnv`, not of `model.GameState`: every entry answers
//! "what does this env method return", regardless of where Go got the answer.
//! That is what keeps `Memory` — squad membership, enemy intel, all of it
//! mutating mid-tick — entirely on the Go side. See `docs/design.md`,
//! "Evaluation semantics".
//!
//! Keyed by predicate rather than one field per concept. Named fields would put
//! the hand-maintenance the manifest exists to remove back into this file:
//! adding a predicate would touch `State`, the Go projector and `eval::apply`.
//! The cost is a state that is harder to read by eye, which `Display` answers.
//!
//! Absence is meaningful: a flag not present is false, a count not present is
//! zero.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct State {
    /// Zero-argument predicates returning a number, e.g. `cash`.
    pub scalars: HashMap<String, f64>,
    /// Zero-argument predicates returning true. Absent means false.
    pub flags: HashSet<String>,
    /// Collections, by length. Keyed the same way as `calls_*` when the
    /// collection takes arguments: `damaged-combat-units(0.5)`.
    pub collections: HashMap<String, i64>,
    /// Options that are present, e.g. `nearest-enemy`.
    pub present: HashSet<String>,

    /// Calls returning true, keyed `name(arg,arg)`, e.g. `has-role(barracks)`.
    pub calls_bool: HashSet<String>,
    pub calls_int: HashMap<String, i64>,
    pub calls_float: HashMap<String, f64>,

    /// Counts of a building or unit type, e.g. `{"e1": 7}`. Separate because
    /// `count(e1)` names a type rather than calling a predicate.
    pub type_counts: HashMap<String, i64>,
}

/// How one argument is rendered into a lookup key.
///
/// Go builds its keys from the literal text in a condition and normalises
/// numbers to their shortest exact form; this has to agree, or a lookup misses
/// and reads the zero default. `0.10` and `0.1` are one threshold.
///
/// Public so `tests/manifest.rs` can check it against the keys Go records rather
/// than reimplementing it — a test that mirrors the logic cannot catch it
/// changing.
pub fn render_number(f: f64) -> String {
    format!("{f}")
}

/// The key a call is recorded under. Go builds the same string, so this is the
/// one place the format is defined on the Rust side.
pub fn call_key(name: &str, args: &[String]) -> String {
    if args.is_empty() {
        name.to_string()
    } else {
        format!("{name}({})", args.join(","))
    }
}

impl State {
    pub fn scalar(&self, name: &str) -> f64 {
        self.scalars.get(name).copied().unwrap_or(0.0)
    }

    pub fn flag(&self, name: &str) -> bool {
        self.flags.contains(name)
    }

    pub fn is_present(&self, name: &str) -> bool {
        self.present.contains(name)
    }

    pub fn collection(&self, key: &str) -> i64 {
        self.collections.get(key).copied().unwrap_or(0)
    }

    pub fn call_bool(&self, key: &str) -> bool {
        self.calls_bool.contains(key)
    }

    pub fn call_int(&self, key: &str) -> i64 {
        self.calls_int.get(key).copied().unwrap_or(0)
    }

    /// Exactly 0.0 for anything unrecorded, matching Go's `SquadReadyRatio` —
    /// a caller cannot tell "no squad" from "squad ready 0%". Preserved rather
    /// than improved; see `docs/design.md`.
    pub fn call_float(&self, key: &str) -> f64 {
        self.calls_float.get(key).copied().unwrap_or(0.0)
    }

    pub fn type_count(&self, ty: &str) -> i64 {
        self.type_counts.get(ty).copied().unwrap_or(0)
    }
}

/// Keyed state is hard to read in a failing test; this makes it skimmable.
impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts: Vec<String> = Vec::new();
        let sorted = |m: Vec<String>| {
            let mut m = m;
            m.sort();
            m
        };
        for k in sorted(self.scalars.keys().cloned().collect()) {
            parts.push(format!("{k}={}", self.scalars[&k]));
        }
        for k in sorted(self.type_counts.keys().cloned().collect()) {
            parts.push(format!("count({k})={}", self.type_counts[&k]));
        }
        for k in sorted(self.collections.keys().cloned().collect()) {
            parts.push(format!("|{k}|={}", self.collections[&k]));
        }
        for k in sorted(self.calls_int.keys().cloned().collect()) {
            parts.push(format!("{k}={}", self.calls_int[&k]));
        }
        for k in sorted(self.calls_float.keys().cloned().collect()) {
            parts.push(format!("{k}={}", self.calls_float[&k]));
        }
        for k in sorted(self.flags.iter().cloned().collect()) {
            parts.push(k);
        }
        for k in sorted(self.present.iter().cloned().collect()) {
            parts.push(format!("exists {k}"));
        }
        for k in sorted(self.calls_bool.iter().cloned().collect()) {
            parts.push(k);
        }
        write!(f, "{}", parts.join(" "))
    }
}
