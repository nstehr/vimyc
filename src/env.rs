//! The predicate surface and the enum domains: what a condition may name.
//!
//! A deliberate fraction of Go's `RuleEnv`, which has 108 methods — many are
//! action-only and unusable in a condition. Which ones belong here is design
//! work; see `docs/design.md` under "Predicates".
//!
//! Everything is a static table, and every name is the source spelling. There is
//! no mapping to Go's spellings here on purpose: the language defines its own
//! vocabulary, and a consumer adapts to it. See `docs/design.md` under "Names".

use crate::types::{Domain, ParamType, Type};

/// Identifies a predicate without going through its spelling.
///
/// Dispatching on this rather than on `&str` means adding a predicate is a
/// compile error in the evaluator rather than a runtime panic when a rule
/// finally names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Predicate {
    Cash,
    PowerExcess,
    BaseUnderAttack,
    EnemiesVisible,
    HasEnemyIntel,
    HasUnit,
    HasBuilding,
    HasRole,
    CanBuildRole,
    QueueBusy,
    QueueReady,
    CanBuild,
    SquadReadyRatio,
    NearestEnemy,
}

/// Not a `Signature`: `count` is overloaded across three domains and resolved by
/// its argument, so it has no single row.
pub const COUNT: &str = "count";

/// One predicate's signature.
pub struct Signature {
    pub id: Predicate,
    pub name: &'static str,
    pub params: &'static [ParamType],
    pub ret: Type,
}

/// Where a predicate returning a collection lowers to `len(...)`.
pub struct CollectionDef {
    pub name: &'static str,
}

/// A member of an enum domain.
pub struct Member {
    pub name: &'static str,
}

// ---- predicates ----

/// What a production queue can be asked to build. Disjoint by construction:
/// `BUILDING_TYPES` and `UNIT_TYPES` share no names, which is what lets
/// `ParamType::AnyOf` resolve from the name alone.
pub const BUILDABLE: &[Domain] = &[Domain::BuildingType, Domain::UnitType];

/// v0: everything `DefaultRules()` uses, plus `squad-ready-ratio` so the `Float`
/// path is exercised — see `docs/design.md`.
///
/// `count` is absent on purpose: it is overloaded across three domains and
/// resolved by its argument, so it cannot be a row here.
pub const PREDICATES: &[Signature] = &[
    Signature {
        id: Predicate::Cash,
        name: "cash",
        params: &[],
        ret: Type::Int,
    },
    Signature {
        id: Predicate::PowerExcess,
        name: "power-excess",
        params: &[],
        ret: Type::Int,
    },
    Signature {
        id: Predicate::BaseUnderAttack,
        name: "base-under-attack",
        params: &[],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::EnemiesVisible,
        name: "enemies-visible",
        params: &[],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::HasEnemyIntel,
        name: "has-enemy-intel",
        params: &[],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::HasUnit,
        name: "has-unit",
        params: &[ParamType::Exact(Type::Enum(Domain::UnitType))],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::HasBuilding,
        name: "has-building",
        params: &[ParamType::Exact(Type::Enum(Domain::BuildingType))],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::HasRole,
        name: "has-role",
        params: &[ParamType::Exact(Type::Enum(Domain::Role))],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::CanBuildRole,
        name: "can-build-role",
        params: &[ParamType::Exact(Type::Enum(Domain::Role))],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::QueueBusy,
        name: "queue-busy",
        params: &[ParamType::Exact(Type::Enum(Domain::Queue))],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::QueueReady,
        name: "queue-ready",
        params: &[ParamType::Exact(Type::Enum(Domain::Queue))],
        ret: Type::Bool,
    },
    // A building or a unit depending on the queue: Go's
    // `CanBuild("Infantry", "e1")` is as real as `CanBuild("Building", "powr")`.
    Signature {
        id: Predicate::CanBuild,
        name: "can-build",
        params: &[
            ParamType::Exact(Type::Enum(Domain::Queue)),
            ParamType::AnyOf(BUILDABLE),
        ],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::SquadReadyRatio,
        name: "squad-ready-ratio",
        params: &[ParamType::Exact(Type::Enum(Domain::SquadName))],
        ret: Type::Float,
    },
    Signature {
        id: Predicate::NearestEnemy,
        name: "nearest-enemy",
        params: &[],
        ret: Type::Option(&Type::Enemy),
    },
];

/// Only reachable through `count(...)`, which is why the language needs no list
/// type — see `docs/design.md`.
pub const COLLECTIONS: &[CollectionDef] = &[
    CollectionDef {
        name: "idle-ground-units",
    },
    CollectionDef {
        name: "idle-harvesters",
    },
    CollectionDef {
        name: "damaged-buildings",
    },
];

// ---- enum domains ----

/// Capitalised, unlike every other domain, so `can-build(Building, powr)` reads
/// as two different things.
pub const QUEUES: &[Member] = &[
    Member { name: "Building" },
    Member { name: "Defense" },
    Member { name: "Vehicle" },
    Member { name: "Infantry" },
    Member { name: "Ship" },
    Member { name: "Aircraft" },
];

/// OpenRA actor names. Not kebab — these are engine identifiers, not words.
pub const BUILDING_TYPES: &[Member] = &[
    Member { name: "fact" },
    Member { name: "powr" },
    Member { name: "proc" },
    Member { name: "weap" },
];

pub const UNIT_TYPES: &[Member] = &[Member { name: "e1" }, Member { name: "mcv" }];

pub const SQUAD_NAMES: &[Member] = &[
    Member {
        name: "ground-attack",
    },
    Member {
        name: "ground-defense",
    },
    Member { name: "air-attack" },
    Member {
        name: "naval-attack",
    },
];

/// From `roles.go`. Kebab in source, snake on the wire.
pub const ROLES: &[Member] = &[
    Member { name: "aa-defense" },
    Member {
        name: "advanced-aircraft",
    },
    Member {
        name: "advanced-power",
    },
    Member { name: "airfield" },
    Member { name: "apc" },
    Member { name: "artillery" },
    Member { name: "attack-dog" },
    Member { name: "badr" },
    Member { name: "barracks" },
    Member {
        name: "basic-aircraft",
    },
    Member {
        name: "camo-pillbox",
    },
    Member {
        name: "construction-yard",
    },
    Member { name: "cruiser" },
    Member { name: "demo-truck" },
    Member { name: "destroyer" },
    Member { name: "engineer" },
    Member { name: "flak-truck" },
    Member {
        name: "flame-tower",
    },
    Member {
        name: "flamethrower",
    },
    Member {
        name: "gap-generator",
    },
    Member { name: "grenadier" },
    Member { name: "gunboat" },
    Member { name: "harvester" },
    Member { name: "heavy-tank" },
    Member {
        name: "iron-curtain",
    },
    Member { name: "kennel" },
    Member { name: "light-tank" },
    Member { name: "mad-tank" },
    Member { name: "medic" },
    Member {
        name: "medium-tank",
    },
    Member { name: "minelayer" },
    Member {
        name: "missile-silo",
    },
    Member {
        name: "missile-sub",
    },
    Member { name: "naval-yard" },
    Member { name: "ore-silo" },
    Member { name: "pillbox" },
    Member {
        name: "power-plant",
    },
    Member { name: "radar" },
    Member { name: "ranger" },
    Member { name: "refinery" },
    Member {
        name: "rocket-soldier",
    },
    Member {
        name: "service-depot",
    },
    Member {
        name: "shock-trooper",
    },
    Member { name: "spy" },
    Member { name: "submarine" },
    Member { name: "tanya" },
    Member {
        name: "tech-center",
    },
    Member { name: "tesla-coil" },
    Member { name: "tesla-tank" },
    Member { name: "tran" },
    Member { name: "turret" },
    Member {
        name: "war-factory",
    },
];

// ---- lookup ----

pub fn predicate(name: &str) -> Option<&'static Signature> {
    PREDICATES.iter().find(|s| s.name == name)
}

pub fn collection(name: &str) -> Option<&'static CollectionDef> {
    COLLECTIONS.iter().find(|c| c.name == name)
}

pub fn members(domain: Domain) -> &'static [Member] {
    match domain {
        Domain::Queue => QUEUES,
        Domain::Role => ROLES,
        Domain::BuildingType => BUILDING_TYPES,
        Domain::UnitType => UNIT_TYPES,
        Domain::SquadName => SQUAD_NAMES,
    }
}

pub fn member(domain: Domain, name: &str) -> Option<&'static Member> {
    members(domain).iter().find(|m| m.name == name)
}

/// Every domain a name belongs to. More than one is an ambiguity for the caller
/// to report rather than silently resolve.
pub fn domains_containing(name: &str) -> Vec<Domain> {
    const ALL: &[Domain] = &[
        Domain::Queue,
        Domain::Role,
        Domain::BuildingType,
        Domain::UnitType,
        Domain::SquadName,
    ];
    ALL.iter()
        .copied()
        .filter(|d| member(*d, name).is_some())
        .collect()
}

/// Resolves an identifier against what a parameter accepts.
///
/// `Ok(None)` means the parameter is not an enum position, so the argument is an
/// ordinary expression. `Err` carries the domains searched, for the diagnostic.
pub fn resolve_param_member(
    param: &ParamType,
    name: &str,
) -> Result<Option<(Domain, &'static Member)>, Vec<Domain>> {
    match param {
        ParamType::Exact(Type::Enum(d)) => match member(*d, name) {
            Some(m) => Ok(Some((*d, m))),
            None => Err(vec![*d]),
        },
        ParamType::Exact(_) => Ok(None),
        ParamType::AnyOf(domains) => {
            let hits: Vec<(Domain, &Member)> = domains
                .iter()
                .filter_map(|d| member(*d, name).map(|m| (*d, m)))
                .collect();
            match hits.len() {
                1 => Ok(Some(hits[0])),
                // More than one means the domains were not disjoint after all —
                // a table bug rather than a source error.
                _ => Err(domains.to_vec()),
            }
        }
    }
}

// ---- categories and actions ----

/// Closed rather than open.
///
/// Go validates nothing here, so a typo silently creates a *new* exclusivity
/// group and two rules meant to exclude each other both fire. The cost of
/// closing it is that adding a category means editing this list.
pub const CATEGORIES: &[&str] = &[
    "air-combat",
    "aircraft-maintenance",
    "capture",
    "combat",
    "defense",
    "economy",
    "emergency-defense",
    "ground-attack-choice",
    "harvester",
    "maintenance",
    "micro",
    "minelayer",
    "naval-combat",
    "produce-aircraft",
    "produce-infantry",
    "produce-ship",
    "produce-vehicle",
    "production",
    "rebuild",
    "recon",
    "setup",
    "squad-form",
    "superweapon",
    "superweapon-build",
    "transport",
    "vehicle-maintenance",
];

/// From Go's `ActionRegistry`. Naming one that does not exist means the rule
/// fires and does nothing.
pub const ACTIONS: &[&str] = &[
    "air-attack-enemy",
    "air-attack-known-base",
    "air-defend-base",
    "attack-known-base-ground",
    "attack-move-ground",
    "capture-building",
    "defend-base",
    "deploy-mcv",
    "emergency-defend-base",
    "fire-iron-curtain",
    "fire-nuke",
    "fire-parabombs",
    "fire-paratroopers",
    "fire-spy-plane",
    "lay-mines",
    "naval-attack-enemy",
    "place-building",
    "place-defense",
    "produce-aa-defense",
    "produce-advanced-aircraft",
    "produce-advanced-power",
    "produce-advanced-ship",
    "produce-aircraft",
    "produce-airfield",
    "produce-attack-dog",
    "produce-barracks",
    "produce-defense",
    "produce-engineer",
    "produce-flame-tower",
    "produce-grenadier",
    "produce-harvester",
    "produce-heavy-vehicle",
    "produce-infantry",
    "produce-iron-curtain",
    "produce-kennel",
    "produce-mad-tank",
    "produce-mcv",
    "produce-minelayer",
    "produce-missile-silo",
    "produce-naval-yard",
    "produce-ore-silo",
    "produce-power-plant",
    "produce-radar",
    "produce-refinery",
    "produce-rocket-soldier",
    "produce-service-depot",
    "produce-ship",
    "produce-specialist-infantry",
    "produce-spy",
    "produce-tech-center",
    "produce-tesla-coil",
    "produce-vehicle",
    "produce-war-factory",
    "repair-buildings",
    "scout",
    "send-harvesters",
];

pub fn is_category(name: &str) -> bool {
    CATEGORIES.contains(&name)
}

pub fn is_action(name: &str) -> bool {
    ACTIONS.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `AnyOf` resolves by name, which only works while its domains are
    /// disjoint.
    #[test]
    fn buildable_domains_are_disjoint() {
        for b in BUILDING_TYPES {
            assert!(
                member(Domain::UnitType, b.name).is_none(),
                "`{}` is both a building and a unit",
                b.name
            );
        }
    }

    /// The tables are what make a typo an error rather than a silently-new
    /// exclusivity group.
    #[test]
    fn seed_categories_and_actions_are_known() {
        for c in [
            "setup",
            "economy",
            "production",
            "combat",
            "recon",
            "maintenance",
            "harvester",
        ] {
            assert!(is_category(c), "`{c}` should be a known category");
        }
        for a in [
            "deploy-mcv",
            "place-building",
            "produce-power-plant",
            "scout",
            "send-harvesters",
        ] {
            assert!(is_action(a), "`{a}` should be a known action");
        }
        assert!(!is_category("ecomomy"));
        assert!(!is_action("produce-powr-plant"));
    }

    #[test]
    fn can_build_accepts_a_building_or_a_unit() {
        let sig = predicate("can-build").expect("can-build is defined");
        let item = &sig.params[1];
        assert!(matches!(resolve_param_member(item, "powr"), Ok(Some(_))));
        assert!(matches!(resolve_param_member(item, "e1"), Ok(Some(_))));
        assert!(resolve_param_member(item, "nonsense").is_err());
    }
}
