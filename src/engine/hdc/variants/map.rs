//! MAP — Multiply-Add-Permute.
//!
//! Vectors: `H ∈ {−1, +1}^D`, stored as `Vec<i8>` in memory and
//! 1-bit-per-element packed on disk.
//!
//! - Binding: `(A ⊗ B)_i = A_i · B_i` — Hadamard product, **self-
//!   inverse**: `(A ⊗ B) ⊗ B == A` exactly.
//! - Bundling: `sign(Σ A_k)` with deterministic tiebreak on zeros.
//! - Permutation: cyclic shift by `s` positions.
//! - Similarity: `S(A, B) = (1/D) · Σ A_i · B_i ∈ [−1, +1]` — plain cosine.
//!
//! Why MAP vs BSC: the signed similarity range lets "opposite" concepts
//! score near −1 (not 0 as in BSC), which makes analogical reasoning
//! and signed-class classifiers cleaner. Same per-op cost as BSC.

use super::HvOps;
use crate::engine::hdc::error::HdcError;
use rand::{RngCore, SeedableRng};

/// Multiply-Add-Permute hypervector. Each element is exactly ±1.
#[derive(Debug, Clone)]
pub struct MapVec {
    pub(crate) data: Vec<i8>,
    pub(crate) dim: usize,
}

impl MapVec {
    /// Convenience: seed-based random vector. Uses `StdRng` so the same
    /// seed produces the same vector across runs.
    pub fn random_seeded(dim: usize, seed: u64) -> Self {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        <Self as HvOps>::random(dim, &mut rng)
    }

    /// Build from a raw `Vec<i8>`. Values are clipped to ±1 (non-negative
    /// becomes +1, negative becomes −1) so the invariant holds.
    pub fn from_raw(data: Vec<i8>) -> Self {
        let dim = data.len();
        let data = data
            .into_iter()
            .map(|x| if x >= 0 { 1 } else { -1 })
            .collect();
        Self { data, dim }
    }

    #[inline]
    fn packed_byte_count(dim: usize) -> usize {
        (dim + 7) / 8
    }
}

impl HvOps for MapVec {
    fn dim(&self) -> usize {
        self.dim
    }

    fn bind(&self, other: &Self) -> Result<Self, HdcError> {
        if self.dim != other.dim {
            return Err(HdcError::DimensionMismatch {
                left: self.dim,
                right: other.dim,
            });
        }
        let data = self
            .data
            .iter()
            .zip(&other.data)
            .map(|(&a, &b)| a * b)
            .collect();
        Ok(Self {
            data,
            dim: self.dim,
        })
    }

    fn bundle(vectors: &[&Self]) -> Result<Self, HdcError> {
        if vectors.is_empty() {
            return Err(HdcError::EmptyBundle);
        }
        let dim = vectors[0].dim;
        for v in vectors.iter().skip(1) {
            if v.dim != dim {
                return Err(HdcError::DimensionMismatch {
                    left: dim,
                    right: v.dim,
                });
            }
        }

        // i32 is safe for up to ~2B summands; production use cases are
        // well under that, and each step is +1 or −1.
        let mut sums = vec![0i32; dim];
        for v in vectors {
            for (s, &x) in sums.iter_mut().zip(&v.data) {
                *s += x as i32;
            }
        }

        let data: Vec<i8> = sums
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                match s.cmp(&0) {
                    std::cmp::Ordering::Greater => 1,
                    std::cmp::Ordering::Less => -1,
                    // Deterministic tiebreak on position parity — matches
                    // BSC's convention. Same caveat: avoids an RNG at
                    // bundling time but makes bundling of perfectly
                    // balanced inputs position-dependent.
                    std::cmp::Ordering::Equal => {
                        if i & 1 == 0 {
                            1
                        } else {
                            -1
                        }
                    }
                }
            })
            .collect();

        Ok(Self { data, dim })
    }

    fn permute(&self, shift: isize) -> Self {
        let dim = self.dim;
        if dim == 0 {
            return self.clone();
        }
        let s = ((shift.rem_euclid(dim as isize)) as usize) % dim;
        if s == 0 {
            return self.clone();
        }
        // Rotate right by s: new[(i+s) mod dim] = old[i]
        let mut data = vec![0i8; dim];
        for i in 0..dim {
            let j = (i + s) % dim;
            data[j] = self.data[i];
        }
        Self { data, dim }
    }

    fn similarity(&self, other: &Self) -> Result<f32, HdcError> {
        if self.dim != other.dim {
            return Err(HdcError::DimensionMismatch {
                left: self.dim,
                right: other.dim,
            });
        }
        // i64 because 10_000 · (±1) · (±1) fits easily and we want
        // headroom for larger dims without overflow concerns.
        let dot: i64 = self
            .data
            .iter()
            .zip(&other.data)
            .map(|(&a, &b)| (a as i64) * (b as i64))
            .sum();
        Ok(dot as f32 / self.dim as f32)
    }

    fn random<R: RngCore>(dim: usize, rng: &mut R) -> Self {
        // One random bit per dimension → ±1. Pull 64 bits at a time.
        let mut data = Vec::with_capacity(dim);
        let mut i = 0;
        while i < dim {
            let word = rng.next_u64();
            let take = (dim - i).min(64);
            for bit in 0..take {
                let is_pos = ((word >> bit) & 1) == 1;
                data.push(if is_pos { 1 } else { -1 });
            }
            i += take;
        }
        Self { data, dim }
    }

    fn zero(dim: usize) -> Self {
        // All +1. MAP has no true zero under binding (0 · x = 0 would
        // corrupt). Callers needing a bundling identity should use an
        // empty bundle test instead of this sentinel.
        Self {
            data: vec![1i8; dim],
            dim,
        }
    }

    fn to_bytes(&self) -> Vec<u8> {
        let packed = Self::packed_byte_count(self.dim);
        let mut bytes = Vec::with_capacity(8 + packed);
        bytes.extend_from_slice(&(self.dim as u64).to_le_bytes());
        for chunk_idx in 0..packed {
            let mut byte = 0u8;
            for bit_idx in 0..8 {
                let i = chunk_idx * 8 + bit_idx;
                if i < self.dim && self.data[i] > 0 {
                    byte |= 1 << bit_idx;
                }
            }
            bytes.push(byte);
        }
        bytes
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, HdcError> {
        if bytes.len() < 8 {
            return Err(HdcError::Serialization(
                "MAP: header missing (need 8 bytes for dim)".into(),
            ));
        }
        let dim = u64::from_le_bytes(bytes[0..8].try_into().unwrap()) as usize;
        let packed = Self::packed_byte_count(dim);
        let expected = 8 + packed;
        if bytes.len() != expected {
            return Err(HdcError::Serialization(format!(
                "MAP: expected {expected} bytes for dim={dim}, got {}",
                bytes.len()
            )));
        }
        let mut data = Vec::with_capacity(dim);
        for i in 0..dim {
            let byte = bytes[8 + i / 8];
            let bit = (byte >> (i % 8)) & 1;
            data.push(if bit == 1 { 1i8 } else { -1i8 });
        }
        Ok(Self { data, dim })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_gives_bipolar_values() {
        let v = MapVec::random_seeded(1000, 42);
        assert_eq!(v.dim, 1000);
        assert_eq!(v.data.len(), 1000);
        assert!(v.data.iter().all(|&x| x == 1 || x == -1));
    }

    #[test]
    fn quasi_orthogonality_of_random_pair() {
        // Two random 10k-D MAP vectors should satisfy |sim| < 4σ where
        // σ = 1/√D = 0.01.
        let a = MapVec::random_seeded(10_000, 1);
        let b = MapVec::random_seeded(10_000, 2);
        let s = a.similarity(&b).unwrap();
        assert!(s.abs() < 0.05, "not quasi-orthogonal: {s}");
    }

    #[test]
    fn binding_produces_orthogonal_output() {
        let a = MapVec::random_seeded(10_000, 11);
        let b = MapVec::random_seeded(10_000, 22);
        let bound = a.bind(&b).unwrap();
        assert!(
            bound.similarity(&a).unwrap().abs() < 0.05,
            "bind should be orthogonal to input A"
        );
        assert!(
            bound.similarity(&b).unwrap().abs() < 0.05,
            "bind should be orthogonal to input B"
        );
    }

    #[test]
    fn binding_is_self_inverse() {
        // MAP binding is Hadamard product and ±1 · ±1 = ±1, so
        // (A ⊗ B) ⊗ B == A exactly.
        let a = MapVec::random_seeded(10_000, 31);
        let b = MapVec::random_seeded(10_000, 32);
        let bound = a.bind(&b).unwrap();
        let recovered = bound.bind(&b).unwrap();
        assert_eq!(recovered.data, a.data);
        assert_eq!(recovered.similarity(&a).unwrap(), 1.0);
    }

    #[test]
    fn bundling_is_similar_to_each_component() {
        let a = MapVec::random_seeded(10_000, 1);
        let b = MapVec::random_seeded(10_000, 2);
        let c = MapVec::random_seeded(10_000, 3);
        let bundle = MapVec::bundle(&[&a, &b, &c]).unwrap();
        // Theoretical: S(bundle, each_input) ≈ 1/√N = 1/√3 ≈ 0.577
        for v in [&a, &b, &c] {
            let s = bundle.similarity(v).unwrap();
            assert!(s > 0.3, "bundle should be similar to component: sim={s}");
        }
    }

    #[test]
    fn bundle_of_opposites_cancels() {
        // A ⊕ (−A) tied at every position; parity tiebreak makes it
        // half +1, half −1 → similarity near 0 with both.
        let a = MapVec::random_seeded(10_000, 77);
        let neg = MapVec {
            data: a.data.iter().map(|x| -x).collect(),
            dim: a.dim,
        };
        let bundle = MapVec::bundle(&[&a, &neg]).unwrap();
        let s = bundle.similarity(&a).unwrap();
        // Exactly 0 is unlikely; parity tiebreak gives ~0.
        assert!(
            s.abs() < 0.05,
            "balanced opposing bundle should score near 0, got {s}"
        );
    }

    #[test]
    fn permute_is_invertible() {
        let a = MapVec::random_seeded(10_000, 13);
        for shift in [1isize, 7, 63, 100, 9999] {
            let rotated = a.permute(shift);
            let back = rotated.permute(-shift);
            assert_eq!(back.data, a.data, "shift={shift} round-trip failed");
        }
    }

    #[test]
    fn permute_is_orthogonal() {
        let a = MapVec::random_seeded(10_000, 23);
        for shift in [1isize, 7, 63, 100, 5000] {
            let p = a.permute(shift);
            let s = p.similarity(&a).unwrap();
            assert!(
                s.abs() < 0.05,
                "ρ(A) should be orthogonal to A for shift={shift}: sim={s}"
            );
        }
    }

    #[test]
    fn permute_preserves_pairwise_similarity() {
        let a = MapVec::random_seeded(10_000, 31);
        let b = MapVec::random_seeded(10_000, 32);
        let base = a.similarity(&b).unwrap();
        for shift in [1isize, 10, 64, 999] {
            let ra = a.permute(shift);
            let rb = b.permute(shift);
            let after = ra.similarity(&rb).unwrap();
            assert!(
                (after - base).abs() < 1e-6,
                "similarity must be preserved under ρ (shift={shift})"
            );
        }
    }

    #[test]
    fn similarity_to_self_is_one() {
        let a = MapVec::random_seeded(1000, 99);
        assert_eq!(a.similarity(&a).unwrap(), 1.0);
    }

    #[test]
    fn similarity_to_negation_is_minus_one() {
        let a = MapVec::random_seeded(1000, 99);
        let neg = MapVec {
            data: a.data.iter().map(|x| -x).collect(),
            dim: a.dim,
        };
        assert_eq!(a.similarity(&neg).unwrap(), -1.0);
    }

    #[test]
    fn roundtrip_bytes() {
        let v = MapVec::random_seeded(10_000, 12345);
        let bytes = v.to_bytes();
        // 8 header + ceil(10000/8) = 1250 packed = 1258 bytes
        assert_eq!(bytes.len(), 8 + 1250);
        let v2 = MapVec::from_bytes(&bytes).unwrap();
        assert_eq!(v.dim, v2.dim);
        assert_eq!(v.data, v2.data);
    }

    #[test]
    fn roundtrip_works_on_non_multiple_of_8_dim() {
        // dim=100 uses ceil(100/8)=13 bytes; the last byte has 4 unused
        // high bits that must be zeroed on write for canonical form.
        let v = MapVec::random_seeded(100, 7);
        let bytes = v.to_bytes();
        assert_eq!(bytes.len(), 8 + 13);
        let v2 = MapVec::from_bytes(&bytes).unwrap();
        assert_eq!(v.data, v2.data);
    }

    #[test]
    fn dimension_mismatch_errors() {
        let a = MapVec::random_seeded(1000, 1);
        let b = MapVec::random_seeded(2000, 2);
        assert!(matches!(
            a.bind(&b),
            Err(HdcError::DimensionMismatch { .. })
        ));
        assert!(matches!(
            a.similarity(&b),
            Err(HdcError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn empty_bundle_errors() {
        assert!(matches!(MapVec::bundle(&[]), Err(HdcError::EmptyBundle)));
    }

    #[test]
    fn from_raw_clips_to_bipolar() {
        let v = MapVec::from_raw(vec![5, -3, 0, 127, -127]);
        assert_eq!(v.dim, 5);
        // 0 is treated as +1 by the "x >= 0" rule
        assert_eq!(v.data, vec![1, -1, 1, 1, -1]);
    }
}
