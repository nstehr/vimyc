//! Text to tokens.
//!
//! Returns `(Vec<Token>, Vec<Diagnostic>)` — collect errors, never bail on the
//! first one.
//!
//! The one non-obvious rule is kebab identifiers versus `-` as subtraction:
//! `ground-defense` is one token, `size - 1` is two. See `docs/design.md`, under
//! "Names", for the exact rule and the case where it bites.
//!
//! There is no comment syntax to lex. `because "..."` is a field on a rule and
//! carries what `//` otherwise would.
use crate::diag::{Diagnostic, SourceFile};
use crate::token::Token;
pub fn lex(source: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    todo!()
}
