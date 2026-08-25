//! Tree-walking interpreter.
//!
//! Also the oracle for any later backend — keep it even if a wasm backend
//! appears, since two implementations that must agree on the whole corpus is a
//! better property test than reading bytecode.
//!
//! Needs a mocked env trait for the tests; see `docs/implementation.md`.
