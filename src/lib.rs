//! A typed language for Vimy's AI rule conditions.
//!
//! ```text
//! files ──unit──> ast ──check──> ir ──┬──eval──> bool
//!                                     └──emit──> expr
//! ```
//!
//! `unit` is `lexer` then `parser`, once per file, merged: a rule set is
//! written across several files and checked as one.
//!
//! Design decisions live in `docs/design.md`; how it is put together, and the
//! decisions that are expensive to reverse, in `docs/implementation.md`.

pub mod ast;
pub mod check;
pub mod diag;
pub mod emit;
pub mod env;
pub mod eval;
pub mod fmt;
pub mod ir;
pub mod lexer;
pub mod lower;
pub mod parser;
pub mod specialise;
pub mod state;
pub mod token;
pub mod types;
pub mod unit;
