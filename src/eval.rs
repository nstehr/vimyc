//! Tree-walking interpreter.
//!
//! Also the oracle for any later backend. Keep it even if a wasm backend
//! appears: two implementations that have to agree across the whole corpus is a
//! better property test than reading bytecode.
//!
//! Needs a mocked env trait for the tests; see `docs/implementation.md`.
