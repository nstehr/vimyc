//! Type checking.
//!
//! `Int`, `Float`, `Bool`, the domain enums, `Option<T>`. The job is turning
//! what expr accepts today into errors: a misspelled enum literal, an enum of
//! the wrong domain, an option used as a bool, an ambiguous `count(...)`.
//!
//! Resolution rules and the whole-rule-set checks that come later are in
//! `docs/design.md`, under "Types".
