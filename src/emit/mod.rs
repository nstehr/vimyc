//! Turning a lowered rule set into something that can run.
//!
//! An enum rather than a `Backend` trait: the set of targets is closed, so this
//! gets exhaustiveness for free. A trait would also abstract the wrong thing —
//! the emitters share almost no interface, only the resolution `lower` did.

use crate::ir::{Ir, ParamValues};

pub mod expr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// expr source, which Vimy's engine runs unchanged.
    Expr,
}

#[derive(Debug)]
pub enum Artifact {
    Expr(Vec<RuleSource>),
}

/// Serialised as-is into the build artifact, so these field names are the JSON
/// contract Go's loader reads.
#[derive(Debug, serde::Serialize)]
pub struct RuleSource {
    pub name: String,
    pub priority: i64,
    pub category: String,
    pub exclusive: bool,
    /// Why the rule exists. Absent unless the source said.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub because: Option<String>,
    /// The action id, with arguments when it is built by a factory.
    pub action: String,
    pub condition: String,
}

/// An unparameterised rule set takes an empty `params`.
pub fn emit(ir: &Ir, params: &ParamValues, target: Target) -> Artifact {
    match target {
        Target::Expr => Artifact::Expr(expr::emit(ir, params)),
    }
}
