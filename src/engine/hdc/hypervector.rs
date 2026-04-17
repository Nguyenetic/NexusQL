//! Backwards-compatibility shim.
//!
//! Before the variant refactor this file held the BSC struct as
//! `Hypervector`. The struct now lives in `variants::bsc::BscVec`;
//! the rest of the crate imports `Hypervector` from this module or
//! from the parent `engine::hdc` module (both paths work).

pub use crate::engine::hdc::variants::bsc::BscVec as Hypervector;
