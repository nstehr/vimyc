//! The predicate surface: what a condition is allowed to call.
//!
//! A fraction of Go's `RuleEnv` and its 108 methods, many of which are
//! action-only and unusable in a condition. Picking the subset is design work,
//! not transcription.
//!
//! The v0 table, the generic `count`, why collections and pointers never need to
//! exist in the language, and why `squad-ready-ratio` is here despite the seed
//! rules not using it: `docs/design.md`, under "Predicates".
