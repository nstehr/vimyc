//! Canonical formatting.
//!
//! A pretty-printer over the AST. The language has no comments, so the AST
//! carries everything the source does and a plain printer is lossless.
//!
//! No line breaking yet: the longest seed conjunct is 51 characters. Layout in
//! `docs/design.md`, the algebra to reach for later in `docs/implementation.md`.
