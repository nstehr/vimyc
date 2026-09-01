//! Types.
//!
//! No inference and no generics, so this is equality checking rather than
//! unification. Resolution rules are in `docs/design.md` under "Types".

use std::fmt;

/// The closed set of enum domains. Members live in `env`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    Queue,
    Role,
    BuildingType,
    UnitType,
    SquadName,
}

impl Domain {
    pub fn name(self) -> &'static str {
        match self {
            Domain::Queue => "queue",
            Domain::Role => "role",
            Domain::BuildingType => "building",
            Domain::UnitType => "unit",
            Domain::SquadName => "squad",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    Bool,

    /// A member of one of the enum domains.
    Enum(Domain),

    /// An opaque game object, only ever reached through `Option`.
    Enemy,

    /// `exists` is the only thing that consumes one; it is not a `Bool`.
    ///
    /// `&'static` rather than `Box` so signatures can live in a `const` table —
    /// `Box::new` is not const. Nothing builds an optional at runtime.
    Option(&'static Type),

    /// Only ever counted; the element type is unmodelled because nothing can
    /// reach an element.
    Collection,

    /// Compatible with everything, so one mistake reports once rather than
    /// cascading. Produced by a failed check and by `ExprKind::Error`.
    Error,
}

impl Type {
    /// `Error` matches anything — see the variant.
    pub fn compatible(&self, other: &Type) -> bool {
        matches!(self, Type::Error) || matches!(other, Type::Error) || self == other
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Int | Type::Float | Type::Error)
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Int => write!(f, "int"),
            Type::Float => write!(f, "float"),
            Type::Bool => write!(f, "bool"),
            Type::Enum(d) => write!(f, "{}", d.name()),
            Type::Enemy => write!(f, "enemy"),
            Type::Option(t) => write!(f, "optional {t}"),
            Type::Collection => write!(f, "collection"),
            Type::Error => write!(f, "<error>"),
        }
    }
}

/// What a parameter accepts.
///
/// Separate from `Type` because a parameter may span several domains and a
/// *return* type never can — keeping them apart makes that unrepresentable
/// rather than merely unused.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamType {
    Exact(Type),
    /// A member of any one of these domains, which must be disjoint so the name
    /// alone decides which.
    AnyOf(&'static [Domain]),
}

impl fmt::Display for ParamType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParamType::Exact(t) => write!(f, "{t}"),
            ParamType::AnyOf(ds) => {
                let names: Vec<&str> = ds.iter().map(|d| d.name()).collect();
                write!(f, "{}", names.join(" or "))
            }
        }
    }
}
