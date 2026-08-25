//! The predicate surface: what a condition is allowed to call.
//!
//! A deliberate fraction of Go's `RuleEnv`, which has 108 methods — many are
//! action-only and unusable in a condition. Choosing the subset is design work,
//! not transcription.
//!
//! The v0 table, the generic `count`, why collections and pointers do not need to
//! exist in the language, and why `squad-ready-ratio` is here despite the seed
//! rules not using it: `docs/design.md`, under "Predicates".
