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
    AircraftCapacity,
    AxisBurned,
    BaseUnderAttack,
    BestAirTarget,
    BestGroundTarget,
    BuildingCount,
    CanBuild,
    CanBuildAnyCombatAircraft,
    CanBuildAnyCombatVehicle,
    CanBuildAnySpecialist,
    CanBuildRole,
    CanBuildTransport,
    CapturableCount,
    Cash,
    CombatAircraftCount,
    CombatVehicleCount,
    CriticalBuildingUnderAttack,
    DamagedBuildings,
    DamagedCombatUnits,
    EnemiesVisible,
    EngineerNearCapturable,
    HarvestersInDanger,
    HasBuilding,
    HasEnemyIntel,
    HasRetreatingUnits,
    HasRole,
    HasScout,
    HasUnit,
    IdleCombatAircraft,
    IdleCombatInfantry,
    IdleCombatLoadedApcs,
    IdleEmptyApcs,
    IdleEngineerLoadedApcs,
    IdleEngineers,
    IdleGroundUnits,
    IdleHarvesters,
    IdleMinelayers,
    IdleNavalUnits,
    IdleScouts,
    IsRushed,
    LostRole,
    MapHasWater,
    NearBaseGroundUnits,
    NearestEnemy,
    OverextendedSquadMembers,
    PowerExcess,
    QueueBusy,
    QueueProducingRole,
    QueueReady,
    ResourcesNearCap,
    RoleCount,
    SpecialistInfantryCount,
    SquadAwayFromBase,
    SquadExists,
    SquadIdleCount,
    SquadNeedsReinforcement,
    SquadReadyRatio,
    SquadThreatRatio,
    SupportPowerReady,
    TransportCount,
    UnassignedIdleAir,
    UnassignedIdleGround,
    UnassignedIdleNaval,
    UnitCount,
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

/// A member of an enum domain.
pub struct Member {
    pub name: &'static str,
}

// ---- builtins ----

/// A pure function. The only two, because they are the only arithmetic the Go
/// compiler does on a doctrine value: 66 calls to `lerp` and 5 to `lerpf`.
///
/// Separate from `Predicate` because they read no state, which is exactly what
/// lets them appear in a `priority` — see `docs/design.md`, "Two phases".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    Lerp,
    Lerpf,
}

pub struct BuiltinSignature {
    pub id: Builtin,
    pub name: &'static str,
    pub params: &'static [ParamType],
    pub ret: Type,
}

pub const BUILTINS: &[BuiltinSignature] = &[
    BuiltinSignature {
        id: Builtin::Lerp,
        name: "lerp",
        params: &[
            ParamType::Exact(Type::Int),
            ParamType::Exact(Type::Int),
            ParamType::Exact(Type::Float),
        ],
        ret: Type::Int,
    },
    BuiltinSignature {
        id: Builtin::Lerpf,
        name: "lerpf",
        params: &[
            ParamType::Exact(Type::Float),
            ParamType::Exact(Type::Float),
            ParamType::Exact(Type::Float),
        ],
        ret: Type::Float,
    },
];

pub fn builtin(name: &str) -> Option<&'static BuiltinSignature> {
    BUILTINS.iter().find(|b| b.name == name)
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
        id: Predicate::AircraftCapacity,
        name: "aircraft-capacity",
        params: &[],
        ret: Type::Int,
    },
    Signature {
        id: Predicate::AxisBurned,
        name: "axis-burned",
        params: &[ParamType::Exact(Type::Enum(Domain::Axis))],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::BaseUnderAttack,
        name: "base-under-attack",
        params: &[],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::BestAirTarget,
        name: "best-air-target",
        params: &[],
        ret: Type::Option(&Type::Enemy),
    },
    Signature {
        id: Predicate::BestGroundTarget,
        name: "best-ground-target",
        params: &[],
        ret: Type::Option(&Type::Enemy),
    },
    Signature {
        id: Predicate::BuildingCount,
        name: "building-count",
        params: &[ParamType::Exact(Type::Enum(Domain::BuildingType))],
        ret: Type::Int,
    },
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
        id: Predicate::CanBuildAnyCombatAircraft,
        name: "can-build-any-combat-aircraft",
        params: &[],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::CanBuildAnyCombatVehicle,
        name: "can-build-any-combat-vehicle",
        params: &[],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::CanBuildAnySpecialist,
        name: "can-build-any-specialist",
        params: &[],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::CanBuildRole,
        name: "can-build-role",
        params: &[ParamType::Exact(Type::Enum(Domain::Role))],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::CanBuildTransport,
        name: "can-build-transport",
        params: &[],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::CapturableCount,
        name: "capturable-count",
        params: &[],
        ret: Type::Int,
    },
    Signature {
        id: Predicate::Cash,
        name: "cash",
        params: &[],
        ret: Type::Int,
    },
    Signature {
        id: Predicate::CombatAircraftCount,
        name: "combat-aircraft-count",
        params: &[],
        ret: Type::Int,
    },
    Signature {
        id: Predicate::CombatVehicleCount,
        name: "combat-vehicle-count",
        params: &[],
        ret: Type::Int,
    },
    Signature {
        id: Predicate::CriticalBuildingUnderAttack,
        name: "critical-building-under-attack",
        params: &[],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::DamagedBuildings,
        name: "damaged-buildings",
        params: &[],
        ret: Type::Collection,
    },
    Signature {
        id: Predicate::DamagedCombatUnits,
        name: "damaged-combat-units",
        params: &[ParamType::Exact(Type::Float)],
        ret: Type::Collection,
    },
    Signature {
        id: Predicate::EnemiesVisible,
        name: "enemies-visible",
        params: &[],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::EngineerNearCapturable,
        name: "engineer-near-capturable",
        params: &[],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::HarvestersInDanger,
        name: "harvesters-in-danger",
        params: &[ParamType::Exact(Type::Float)],
        ret: Type::Collection,
    },
    Signature {
        id: Predicate::HasBuilding,
        name: "has-building",
        params: &[ParamType::Exact(Type::Enum(Domain::BuildingType))],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::HasEnemyIntel,
        name: "has-enemy-intel",
        params: &[],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::HasRetreatingUnits,
        name: "has-retreating-units",
        params: &[],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::HasRole,
        name: "has-role",
        params: &[ParamType::Exact(Type::Enum(Domain::Role))],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::HasScout,
        name: "has-scout",
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
        id: Predicate::IdleCombatAircraft,
        name: "idle-combat-aircraft",
        params: &[],
        ret: Type::Collection,
    },
    Signature {
        id: Predicate::IdleCombatInfantry,
        name: "idle-combat-infantry",
        params: &[],
        ret: Type::Collection,
    },
    Signature {
        id: Predicate::IdleCombatLoadedApcs,
        name: "idle-combat-loaded-apcs",
        params: &[],
        ret: Type::Collection,
    },
    Signature {
        id: Predicate::IdleEmptyApcs,
        name: "idle-empty-apcs",
        params: &[],
        ret: Type::Collection,
    },
    Signature {
        id: Predicate::IdleEngineerLoadedApcs,
        name: "idle-engineer-loaded-apcs",
        params: &[],
        ret: Type::Collection,
    },
    Signature {
        id: Predicate::IdleEngineers,
        name: "idle-engineers",
        params: &[],
        ret: Type::Collection,
    },
    Signature {
        id: Predicate::IdleGroundUnits,
        name: "idle-ground-units",
        params: &[],
        ret: Type::Collection,
    },
    Signature {
        id: Predicate::IdleHarvesters,
        name: "idle-harvesters",
        params: &[],
        ret: Type::Collection,
    },
    Signature {
        id: Predicate::IdleMinelayers,
        name: "idle-minelayers",
        params: &[],
        ret: Type::Collection,
    },
    Signature {
        id: Predicate::IdleNavalUnits,
        name: "idle-naval-units",
        params: &[],
        ret: Type::Collection,
    },
    Signature {
        id: Predicate::IdleScouts,
        name: "idle-scouts",
        params: &[],
        ret: Type::Collection,
    },
    Signature {
        id: Predicate::IsRushed,
        name: "is-rushed",
        params: &[],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::LostRole,
        name: "lost-role",
        params: &[ParamType::Exact(Type::Enum(Domain::Role))],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::MapHasWater,
        name: "map-has-water",
        params: &[],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::NearBaseGroundUnits,
        name: "near-base-ground-units",
        params: &[],
        ret: Type::Collection,
    },
    Signature {
        id: Predicate::NearestEnemy,
        name: "nearest-enemy",
        params: &[],
        ret: Type::Option(&Type::Enemy),
    },
    Signature {
        id: Predicate::OverextendedSquadMembers,
        name: "overextended-squad-members",
        params: &[
            ParamType::Exact(Type::Enum(Domain::SquadName)),
            ParamType::Exact(Type::Float),
        ],
        ret: Type::Collection,
    },
    Signature {
        id: Predicate::PowerExcess,
        name: "power-excess",
        params: &[],
        ret: Type::Int,
    },
    Signature {
        id: Predicate::QueueBusy,
        name: "queue-busy",
        params: &[ParamType::Exact(Type::Enum(Domain::Queue))],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::QueueProducingRole,
        name: "queue-producing-role",
        params: &[ParamType::Exact(Type::Enum(Domain::Role))],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::QueueReady,
        name: "queue-ready",
        params: &[ParamType::Exact(Type::Enum(Domain::Queue))],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::ResourcesNearCap,
        name: "resources-near-cap",
        params: &[],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::RoleCount,
        name: "role-count",
        params: &[ParamType::Exact(Type::Enum(Domain::Role))],
        ret: Type::Int,
    },
    Signature {
        id: Predicate::SpecialistInfantryCount,
        name: "specialist-infantry-count",
        params: &[],
        ret: Type::Int,
    },
    Signature {
        id: Predicate::SquadAwayFromBase,
        name: "squad-away-from-base",
        params: &[
            ParamType::Exact(Type::Enum(Domain::SquadName)),
            ParamType::Exact(Type::Float),
        ],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::SquadExists,
        name: "squad-exists",
        params: &[ParamType::Exact(Type::Enum(Domain::SquadName))],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::SquadIdleCount,
        name: "squad-idle-count",
        params: &[ParamType::Exact(Type::Enum(Domain::SquadName))],
        ret: Type::Int,
    },
    Signature {
        id: Predicate::SquadNeedsReinforcement,
        name: "squad-needs-reinforcement",
        params: &[ParamType::Exact(Type::Enum(Domain::SquadName))],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::SquadReadyRatio,
        name: "squad-ready-ratio",
        params: &[ParamType::Exact(Type::Enum(Domain::SquadName))],
        ret: Type::Float,
    },
    Signature {
        id: Predicate::SquadThreatRatio,
        name: "squad-threat-ratio",
        params: &[
            ParamType::Exact(Type::Enum(Domain::SquadName)),
            ParamType::Exact(Type::Float),
        ],
        ret: Type::Float,
    },
    Signature {
        id: Predicate::SupportPowerReady,
        name: "support-power-ready",
        params: &[ParamType::Exact(Type::Enum(Domain::SupportPower))],
        ret: Type::Bool,
    },
    Signature {
        id: Predicate::TransportCount,
        name: "transport-count",
        params: &[],
        ret: Type::Int,
    },
    Signature {
        id: Predicate::UnassignedIdleAir,
        name: "unassigned-idle-air",
        params: &[],
        ret: Type::Collection,
    },
    Signature {
        id: Predicate::UnassignedIdleGround,
        name: "unassigned-idle-ground",
        params: &[],
        ret: Type::Collection,
    },
    Signature {
        id: Predicate::UnassignedIdleNaval,
        name: "unassigned-idle-naval",
        params: &[],
        ret: Type::Collection,
    },
    Signature {
        id: Predicate::UnitCount,
        name: "unit-count",
        params: &[ParamType::Exact(Type::Enum(Domain::UnitType))],
        ret: Type::Int,
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
pub const AXES: &[Member] = &[
    Member { name: "air" },
    Member { name: "infantry" },
    Member { name: "naval" },
    Member { name: "vehicle" },
];

/// OpenRA order names, not kebab — these are engine identifiers.
pub const SUPPORT_POWERS: &[Member] = &[
    Member {
        name: "GrantExternalConditionPowerInfoOrder",
    },
    Member {
        name: "NukePowerInfoOrder",
    },
    Member {
        name: "SovietParatroopers",
    },
    Member {
        name: "SovietSpyPlane",
    },
    Member {
        name: "UkraineParabombs",
    },
];

pub const SQUAD_DOMAINS: &[Member] = &[
    Member { name: "Ground" },
    Member { name: "Air" },
    Member { name: "Naval" },
];

pub const SQUAD_ROLES: &[Member] = &[Member { name: "Attack" }, Member { name: "Defend" }];

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
        name: "v2-launcher",
    },
    Member {
        name: "war-factory",
    },
];

// ---- lookup ----

pub fn predicate(name: &str) -> Option<&'static Signature> {
    PREDICATES.iter().find(|s| s.name == name)
}

pub fn members(domain: Domain) -> &'static [Member] {
    match domain {
        Domain::Queue => QUEUES,
        Domain::Role => ROLES,
        Domain::BuildingType => BUILDING_TYPES,
        Domain::UnitType => UNIT_TYPES,
        Domain::SquadName => SQUAD_NAMES,
        Domain::Axis => AXES,
        Domain::SupportPower => SUPPORT_POWERS,
        Domain::SquadDomain => SQUAD_DOMAINS,
        Domain::SquadRole => SQUAD_ROLES,
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
        Domain::Axis,
        Domain::SupportPower,
        Domain::SquadDomain,
        Domain::SquadRole,
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
    "cancel-stuck-aircraft",
    "capture-building",
    "defend-base",
    "defend-critical-building",
    "deliver-assault-apc",
    "deploy-mcv",
    "emergency-defend-base",
    "fire-iron-curtain",
    "fire-nuke",
    "fire-parabombs",
    "fire-paratroopers",
    "fire-spy-plane",
    "lay-mines",
    "load-combat-infantry",
    "load-engineer-into-apc",
    "naval-attack-enemy",
    "naval-defend-base",
    "place-building",
    "place-defense",
    "produce-aa-defense",
    "produce-advanced-aircraft",
    "produce-advanced-power",
    "produce-advanced-ship",
    "produce-aircraft",
    "produce-airfield",
    "produce-apc",
    "produce-attack-dog",
    "produce-barracks",
    "produce-basic-aircraft",
    "produce-defense",
    "produce-engineer",
    "produce-flak-truck",
    "produce-flame-tower",
    "produce-gap-generator",
    "produce-grenadier",
    "produce-gunboat",
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
    "produce-scout-vehicle",
    "produce-service-depot",
    "produce-ship",
    "produce-siege-vehicle",
    "produce-specialist-infantry",
    "produce-spy",
    "produce-tech-center",
    "produce-tesla-coil",
    "produce-vehicle",
    "produce-war-factory",
    "repair-buildings",
    "scout",
    "scout-patrol",
    "send-harvesters",
    "unblock-war-factory-egress",
    "unload-apc-near-target",
];

pub fn is_category(name: &str) -> bool {
    CATEGORIES.contains(&name)
}

/// A category's index into `CATEGORIES`.
pub fn category_id(name: &str) -> Option<u32> {
    CATEGORIES.iter().position(|c| *c == name).map(|i| i as u32)
}

pub fn category_name(id: u32) -> &'static str {
    CATEGORIES[id as usize]
}

/// An action's index.
///
/// One space covering both tables: plain actions first, then the parameterised
/// ones, which are built by a factory and so are absent from Go's registry.
pub fn action_id(name: &str) -> Option<u32> {
    if let Some(i) = ACTIONS.iter().position(|a| *a == name) {
        return Some(i as u32);
    }
    ACTION_SIGNATURES
        .iter()
        .position(|a| a.name == name)
        .map(|i| (ACTIONS.len() + i) as u32)
}

pub fn action_name(id: u32) -> &'static str {
    let i = id as usize;
    if i < ACTIONS.len() {
        ACTIONS[i]
    } else {
        ACTION_SIGNATURES[i - ACTIONS.len()].name
    }
}

/// A member's index within its domain.
pub fn member_index(domain: Domain, name: &str) -> Option<u32> {
    members(domain)
        .iter()
        .position(|m| m.name == name)
        .map(|i| i as u32)
}

pub fn member_name(domain: Domain, index: u32) -> &'static str {
    members(domain)[index as usize].name
}

pub fn is_action(name: &str) -> bool {
    ACTIONS.contains(&name) || action_signature(name).is_some()
}

/// An action built by a factory, so it carries arguments.
///
/// `form-squad(ground-attack, Ground, 8, Attack)` cannot be a fixed id the way
/// `scout` can — the arguments vary per doctrine, so a registry entry per
/// parameterisation is not possible. Eleven factories, from `actions.go`.
pub struct ActionSignature {
    pub name: &'static str,
    pub params: &'static [ParamType],
}

pub const ACTION_SIGNATURES: &[ActionSignature] = &[
    ActionSignature {
        name: "form-squad",
        params: &[
            ParamType::Exact(Type::Enum(Domain::SquadName)),
            ParamType::Exact(Type::Enum(Domain::SquadDomain)),
            ParamType::Exact(Type::Int),
            ParamType::Exact(Type::Enum(Domain::SquadRole)),
        ],
    },
    ActionSignature {
        name: "squad-attack-move",
        params: &[ParamType::Exact(Type::Enum(Domain::SquadName))],
    },
    ActionSignature {
        name: "squad-attack-known-base",
        params: &[
            ParamType::Exact(Type::Enum(Domain::SquadName)),
            ParamType::Exact(Type::Float),
        ],
    },
    ActionSignature {
        name: "squad-air-strike",
        params: &[ParamType::Exact(Type::Enum(Domain::SquadName))],
    },
    ActionSignature {
        name: "squad-focus-fire",
        params: &[ParamType::Exact(Type::Enum(Domain::SquadName))],
    },
    ActionSignature {
        name: "squad-disengage",
        params: &[ParamType::Exact(Type::Enum(Domain::SquadName))],
    },
    ActionSignature {
        name: "squad-defend",
        params: &[ParamType::Exact(Type::Enum(Domain::SquadName))],
    },
    ActionSignature {
        name: "recall-overextended",
        params: &[
            ParamType::Exact(Type::Enum(Domain::SquadName)),
            ParamType::Exact(Type::Float),
        ],
    },
    ActionSignature {
        name: "retreat-damaged-units",
        params: &[ParamType::Exact(Type::Float)],
    },
    ActionSignature {
        name: "flee-harvesters",
        params: &[ParamType::Exact(Type::Float)],
    },
    ActionSignature {
        name: "clear-healed-units",
        params: &[ParamType::Exact(Type::Float)],
    },
];

pub fn action_signature(name: &str) -> Option<&'static ActionSignature> {
    ACTION_SIGNATURES.iter().find(|a| a.name == name)
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
