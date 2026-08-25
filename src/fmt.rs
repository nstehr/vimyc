//! Canonical formatting.
//!
//! A pretty-printer over the AST. The language has no comments, so the AST
//! carries everything the source does and a plain printer is lossless — no
//! concrete syntax tree, no trivia.
//!
//! v0 needs no line breaking: the longest seed conjunct is 51 characters. Layout
//! rules are in `docs/design.md`; the document algebra to reach for when
//! breaking eventually matters is in `docs/implementation.md`.
