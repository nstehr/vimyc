//! Turning a lowered rule set into something that can run.
//!
//! An enum rather than a `Backend` trait: the set of targets is closed, so this
//! gets exhaustiveness for free. A trait would also abstract the wrong thing —
//! the emitters share almost no interface, only the resolution `lower` did.

use crate::ir::{Ir, ParamValues};

pub mod expr;
pub mod vy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// expr source, which Vimy's engine runs unchanged.
    Expr,
    /// The rule set printed back as `.vy`, after lowering and specialising —
    /// what the source became for the doctrine that ran. For reading, not
    /// running.
    Vy,
}

#[derive(Debug)]
pub enum Artifact {
    Expr(Vec<RuleSource>),
    Vy(Vec<String>),
}

/// Serialised as-is into the build artifact, so these field names are the JSON
/// contract Go's loader reads.
#[derive(Debug, serde::Serialize)]
pub struct RuleSource {
    pub name: String,
    pub priority: i64,
    pub category: String,
    pub exclusive: bool,
    /// One win in `share` of its exclusive category's wins. Skipped when
    /// unrationed so the artifact is byte-identical for every rule that does
    /// not use it -- which is what lets the field ship without moving a digest.
    #[serde(skip_serializing_if = "is_zero")]
    pub share: i64,
    /// Why the rule exists. Absent unless the source said.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub because: Option<String>,
    /// The action id, with arguments when it is built by a factory.
    pub action: String,
    pub condition: String,
    /// The same rule as `.vy`, for the dashboard and the archive. Emitted from
    /// the same `Ir` in the same pass as `condition`, so the two cannot drift.
    pub source: String,
}

fn is_zero(n: &i64) -> bool {
    *n == 0
}

/// An unparameterised rule set takes an empty `params`.
pub fn emit(ir: &Ir, params: &ParamValues, target: Target) -> Artifact {
    match target {
        Target::Expr => Artifact::Expr(expr::emit(ir, params)),
        Target::Vy => Artifact::Vy(vy::emit(ir, params)),
    }
}
