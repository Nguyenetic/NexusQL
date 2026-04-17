//! Backwards-compatibility shim.
//!
//! Before the enum refactor `Hypervector` was a struct living in this
//! file. It is now the enum in the parent `hdc` module. This shim
//! re-exports it so `use crate::engine::hdc::hypervector::Hypervector;`
//! keeps compiling. New code should use `crate::engine::hdc::Hypervector`.

pub use crate::engine::hdc::Hypervector;
pub use crate::engine::hdc::variants::bsc::BscVec;
