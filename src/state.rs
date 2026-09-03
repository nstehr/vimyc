//! Game state, as a condition can observe it.
//!
//! A plain data struct rather than a trait, because the same state has to be
//! expressed in both Rust and Go for the differential test — so it has to be
//! JSON, and a trait is not.
//!
//! Absence is meaningful throughout: a role not in the set is not held, a count
//! not in the map is zero. That keeps fixtures small and matches how a partial
//! observation of a game actually looks.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct State {
    pub cash: i64,
    pub power_excess: i64,
    pub base_under_attack: bool,
    pub enemies_visible: bool,
    pub has_enemy_intel: bool,
    /// Whether `nearest-enemy` is present. The payload is unreachable from the
    /// language, so it is not modelled.
    pub nearest_enemy: bool,

    /// Unit type to count, e.g. `{"e1": 7}`.
    pub units: HashMap<String, i64>,
    pub buildings: HashMap<String, i64>,

    pub roles: HashSet<String>,
    pub buildable_roles: HashSet<String>,
    pub queues_busy: HashSet<String>,
    pub queues_ready: HashSet<String>,
    /// `"Building/powr"` — a pair, flattened because JSON keys cannot be tuples.
    pub can_build: HashSet<String>,

    /// Collection name to length, e.g. `{"idle-ground-units": 5}`.
    pub collections: HashMap<String, i64>,
    pub squad_ready: HashMap<String, f64>,
}

impl State {
    pub fn unit_count(&self, ty: &str) -> i64 {
        self.units.get(ty).copied().unwrap_or(0)
    }

    pub fn building_count(&self, ty: &str) -> i64 {
        self.buildings.get(ty).copied().unwrap_or(0)
    }

    pub fn collection_len(&self, name: &str) -> i64 {
        self.collections.get(name).copied().unwrap_or(0)
    }

    pub fn has_unit(&self, ty: &str) -> bool {
        self.unit_count(ty) > 0
    }

    pub fn has_building(&self, ty: &str) -> bool {
        self.building_count(ty) > 0
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.contains(role)
    }

    pub fn can_build_role(&self, role: &str) -> bool {
        self.buildable_roles.contains(role)
    }

    pub fn queue_busy(&self, queue: &str) -> bool {
        self.queues_busy.contains(queue)
    }

    pub fn queue_ready(&self, queue: &str) -> bool {
        self.queues_ready.contains(queue)
    }

    pub fn can_build(&self, queue: &str, item: &str) -> bool {
        self.can_build.contains(&format!("{queue}/{item}"))
    }

    /// Matches Go's `SquadReadyRatio`: exactly 0.0 for an unknown squad, so a
    /// caller cannot tell "no squad" from "squad ready 0%". Preserved rather
    /// than improved — see `docs/design.md`.
    pub fn squad_ready_ratio(&self, squad: &str) -> f64 {
        self.squad_ready.get(squad).copied().unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_entries_default_rather_than_failing() {
        let s = State::default();
        assert_eq!(s.unit_count("e1"), 0);
        assert_eq!(s.collection_len("idle-ground-units"), 0);
        assert_eq!(s.squad_ready_ratio("ground-attack"), 0.0);
        assert!(!s.has_role("barracks"));
        assert!(!s.can_build("Building", "powr"));
    }

    #[test]
    fn fixtures_parse() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/states");
        let mut seen = 0;
        for entry in std::fs::read_dir(dir).expect("testdata/states") {
            let path = entry.expect("entry").path();
            let text = std::fs::read_to_string(&path).expect("read");
            let state: State =
                serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let _ = state.cash;
            seen += 1;
        }
        assert!(seen >= 3, "expected the state fixtures, found {seen}");
    }

    #[test]
    fn unknown_fields_are_rejected() {
        // Fixtures are hand-written, so a typo has to fail loudly rather than
        // silently defaulting.
        let r: Result<State, _> = serde_json::from_str(r#"{"cashh": 5}"#);
        assert!(r.is_err());
    }

    #[test]
    fn can_build_is_keyed_on_the_pair() {
        let s: State = serde_json::from_str(r#"{"can_build": ["Building/powr"]}"#).expect("parse");
        assert!(s.can_build("Building", "powr"));
        assert!(!s.can_build("Infantry", "powr"));
        assert!(!s.can_build("Building", "weap"));
    }
}
