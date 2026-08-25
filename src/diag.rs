//! Spans and diagnostic rendering.
//!
//! `Span` is a byte range: `Copy`, `u32` fields, merged with `to()`. Write it
//! first — the lexer cannot emit tokens without it. The rendering half can wait
//! until the lexer is producing real errors to point at.
//!
//! `docs/implementation.md` covers why byte offsets rather than line/column, and
//! the character-versus-byte trap in column counting.
