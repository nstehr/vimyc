//! Turning a lowered rule set into something that can run.
//!
//! An enum rather than a `Backend` trait: the set of targets is closed and known
//! at compile time, so this gets exhaustiveness — adding one becomes a compile
//! error everywhere it matters — with no vtable and no lifetime friction. A
//! trait would also abstract the wrong thing, since the emitters share almost no
//! interface; what they share is the resolution `lower` already did.

use crate::ir::{Ir, ParamValues};

pub mod expr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// expr source, which Vimy's existing engine runs unchanged. The lowest-risk
    /// integration: nothing in the hot path changes.
    Expr,
}

/// What a backend produces. One variant per target.
#[derive(Debug)]
pub enum Artifact {
    Expr(Vec<RuleSource>),
}

/// One rule, ready for the Go side: metadata plus a condition it can evaluate.
///
/// Serialised as-is into the build artifact, so the field names here are the
/// JSON contract Go's loader reads.
#[derive(Debug, serde::Serialize)]
pub struct RuleSource {
    pub name: String,
    pub priority: i64,
    pub category: String,
    pub exclusive: bool,
    /// The action id, with arguments when it is built by a factory.
    pub action: String,
    pub condition: String,
}

/// `params` supplies the doctrine's numbers. An unparameterised rule set takes
/// an empty set, which is what every rule set does today.
pub fn emit(ir: &Ir, params: &ParamValues, target: Target) -> Artifact {
    match target {
        Target::Expr => Artifact::Expr(expr::emit(ir, params)),
    }
}
