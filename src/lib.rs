//! A typed language for Vimy's AI rule conditions.
//!
//! ```text
//! source ──lexer──> tokens ──parser──> ast ──types──> checked ast ──eval──> bool
//! ```
//!
//! Design decisions live in `docs/design.md`; how it is put together, and the
//! decisions that are expensive to reverse, in `docs/implementation.md`.

pub mod ast;
pub mod check;
pub mod diag;
pub mod env;
pub mod eval;
pub mod fmt;
pub mod lexer;
pub mod parser;
pub mod token;
pub mod types;
