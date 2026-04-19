//! VSA variants (BSC, MAP, HRR, FHRR, SCH) and the common HvOps trait.
//!
//! Each variant is a concrete struct implementing `HvOps`. Public-facing code
//! uses the `Hypervector` enum (see parent `mod.rs`) which dispatches to the
//! matching variant at runtime — runtime dispatch at SQL boundaries, static
//! dispatch inside each variant's math.

pub mod bsc;
pub mod map;

use crate::engine::hdc::error::HdcError;
use rand::RngCore;

/// Operations every VSA variant must support.
///
/// This is the internal abstraction. User-facing types go through the
/// `Hypervector` enum so SQL callers don't need to care which variant is
/// stored.
pub trait HvOps: Sized + Clone + Send + Sync {
    /// Dimensionality of the conceptual vector space (not the storage
    /// length — BSC packs 64 bits per u64).
    fn dim(&self) -> usize;

    /// Binding: `A ⊗ B`. Produces a vector that is quasi-orthogonal to
    /// both inputs but carries enough information to recover either if
    /// the other is known.
    fn bind(&self, other: &Self) -> Result<Self, HdcError>;

    /// Bundling: `A ⊕ B ⊕ ...`. Produces a vector similar (but not
    /// identical) to each input. Useful for representing sets / multisets
    /// of items.
    fn bundle(vectors: &[&Self]) -> Result<Self, HdcError>;

    /// Permutation: `ρ(A)`. Rotates the vector by `shift` positions.
    /// `shift != 0` produces a vector quasi-orthogonal to the input.
    /// Used to encode sequence position.
    fn permute(&self, shift: isize) -> Self;

    /// Similarity: `S(A, B)`. Range `[-1, +1]` for most variants;
    /// BSC returns `[-1, +1]` via the `1 - 2·d_H/D` normalization.
    fn similarity(&self, other: &Self) -> Result<f32, HdcError>;

    /// Generate a random hypervector from a seeded RNG.
    fn random<R: RngCore>(dim: usize, rng: &mut R) -> Self;

    /// Zero / identity-for-bundling. Not all variants have a meaningful
    /// zero; treat this as a convenience, not an algebraic guarantee.
    fn zero(dim: usize) -> Self;

    /// Serialize for persistence. Must include the dimension so
    /// `from_bytes` can reconstruct without external metadata.
    fn to_bytes(&self) -> Vec<u8>;

    /// Deserialize. Reads the embedded dimension header.
    fn from_bytes(bytes: &[u8]) -> Result<Self, HdcError>;
}
