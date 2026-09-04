//! Turning a lowered rule set into something that can run.
//!
//! An enum rather than a `Backend` trait: the set of targets is closed and known
//! at compile time, so this gets exhaustiveness — adding one becomes a compile
//! error everywhere it matters — with no vtable and no lifetime friction. A
//! trait would also abstract the wrong thing, since the emitters share almost no
//! interface; what they share is the resolution `lower` already did.

use crate::ir::Ir;

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
#[derive(Debug)]
pub struct RuleSource {
    pub name: String,
    pub priority: i64,
    pub category: String,
    pub exclusive: bool,
    /// The action id, with arguments when it is built by a factory.
    pub action: String,
    pub condition: String,
}

pub fn emit(ir: &Ir, target: Target) -> Artifact {
    match target {
        Target::Expr => Artifact::Expr(expr::emit(ir)),
    }
}
