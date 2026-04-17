//! BSC — Binary Spatter Codes.
//!
//! Vectors: `H ∈ {0, 1}^D`, stored as packed `u64` chunks.
//! - Binding: componentwise XOR
//! - Bundling: majority vote across a stack of vectors
//! - Permutation: cyclic bit shift
//! - Similarity: `1 − 2·d_H / D` (Hamming distance normalized to [−1, +1])
//!
//! Kanerva 1996. Fastest variant to compute on CPU (XOR + popcnt).

use super::HvOps;
use crate::engine::hdc::error::HdcError;
use rand::{RngCore, SeedableRng};

/// Binary Spatter Code hypervector. Bits are packed into `u64` chunks —
/// 64 bits per chunk, bit `i` lives in `data[i/64]` at position `i%64`.
/// Unused bits in the final chunk are always zero.
#[derive(Debug, Clone)]
pub struct BscVec {
    pub(crate) data: Vec<u64>,
    pub(crate) dim: usize,
}

impl BscVec {
    /// Convenience: build from a `u64` seed. Uses `StdRng` so the same
    /// seed produces the same vector across runs.
    pub fn random_seeded(dim: usize, seed: u64) -> Self {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        <Self as HvOps>::random(dim, &mut rng)
    }

    /// Read bit `i` (0-indexed). Returns `true` for 1, `false` for 0.
    #[inline]
    pub fn get_bit(&self, i: usize) -> bool {
        debug_assert!(i < self.dim, "bit index out of range");
        (self.data[i / 64] >> (i % 64)) & 1 == 1
    }

    /// Number of `u64` chunks needed for `dim` bits.
    #[inline]
    fn chunks_for(dim: usize) -> usize {
        (dim + 63) / 64
    }

    /// Zero out bits beyond `dim` in the final chunk. Keeps the on-disk
    /// representation canonical so equal vectors have equal bytes.
    #[inline]
    fn mask_tail(data: &mut [u64], dim: usize) {
        let extra_bits = data.len() * 64 - dim;
        if extra_bits > 0 && !data.is_empty() {
            let keep = 64 - extra_bits;
            let mask: u64 = if keep == 0 { 0 } else { (!0u64) >> extra_bits };
            let last = data.len() - 1;
            data[last] &= mask;
        }
    }
}

impl HvOps for BscVec {
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
        let data: Vec<u64> = self
            .data
            .iter()
            .zip(&other.data)
            .map(|(a, b)| a ^ b)
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

        // Per-bit population count. u32 is enough for ≥ 4 billion summands.
        let mut counts = vec![0u32; dim];
        for v in vectors {
            for i in 0..dim {
                let chunk = v.data[i / 64];
                let bit = (chunk >> (i % 64)) & 1;
                counts[i] += bit as u32;
            }
        }

        let n = vectors.len() as u32;
        let threshold = n / 2;
        let mut data = vec![0u64; Self::chunks_for(dim)];
        for i in 0..dim {
            let bit = if counts[i] > threshold {
                1u64
            } else if counts[i] < threshold {
                0u64
            } else {
                // Deterministic tiebreak on position parity — matches the
                // plan spec. Avoids needing an RNG at bundling time.
                (i as u64) & 1
            };
            data[i / 64] |= bit << (i % 64);
        }
        Self::mask_tail(&mut data, dim);
        Ok(Self { data, dim })
    }

    fn permute(&self, shift: isize) -> Self {
        // Bit-level cyclic shift. O(D) — can be sped up to O(D/64) using
        // rotate_left on packed chunks; PR 2 or later optimization work.
        let dim = self.dim;
        if dim == 0 {
            return self.clone();
        }
        let s = ((shift.rem_euclid(dim as isize)) as usize) % dim;
        if s == 0 {
            return self.clone();
        }
        let mut data = vec![0u64; Self::chunks_for(dim)];
        for i in 0..dim {
            let chunk = self.data[i / 64];
            let bit = (chunk >> (i % 64)) & 1;
            let j = (i + s) % dim;
            data[j / 64] |= bit << (j % 64);
        }
        Self::mask_tail(&mut data, dim);
        Self { data, dim }
    }

    fn similarity(&self, other: &Self) -> Result<f32, HdcError> {
        if self.dim != other.dim {
            return Err(HdcError::DimensionMismatch {
                left: self.dim,
                right: other.dim,
            });
        }
        // Tail bits are kept zero by construction, so `count_ones()` on
        // the XOR gives true Hamming distance without a bit-range check.
        let hamming: u32 = self
            .data
            .iter()
            .zip(&other.data)
            .map(|(a, b)| (a ^ b).count_ones())
            .sum();
        let normalized = hamming as f32 / self.dim as f32;
        Ok(1.0 - 2.0 * normalized)
    }

    fn random<R: RngCore>(dim: usize, rng: &mut R) -> Self {
        let n_chunks = Self::chunks_for(dim);
        let mut data = vec![0u64; n_chunks];
        for chunk in data.iter_mut() {
            *chunk = rng.next_u64();
        }
        Self::mask_tail(&mut data, dim);
        Self { data, dim }
    }

    fn zero(dim: usize) -> Self {
        Self {
            data: vec![0u64; Self::chunks_for(dim)],
            dim,
        }
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(8 + self.data.len() * 8);
        bytes.extend_from_slice(&(self.dim as u64).to_le_bytes());
        for chunk in &self.data {
            bytes.extend_from_slice(&chunk.to_le_bytes());
        }
        bytes
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, HdcError> {
        if bytes.len() < 8 {
            return Err(HdcError::Serialization(
                "BSC: header missing (need 8 bytes for dim)".into(),
            ));
        }
        let dim = u64::from_le_bytes(bytes[0..8].try_into().unwrap()) as usize;
        let n_chunks = Self::chunks_for(dim);
        let expected = 8 + n_chunks * 8;
        if bytes.len() != expected {
            return Err(HdcError::Serialization(format!(
                "BSC: expected {expected} bytes for dim={dim}, got {}",
                bytes.len()
            )));
        }
        let mut data = Vec::with_capacity(n_chunks);
        for i in 0..n_chunks {
            let start = 8 + i * 8;
            let chunk = u64::from_le_bytes(bytes[start..start + 8].try_into().unwrap());
            data.push(chunk);
        }
        Ok(Self { data, dim })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hypervector_creation() {
        let hv = BscVec::random_seeded(1000, 42);
        assert_eq!(hv.dim(), 1000);
        // Packed: 1000 bits = ceil(1000/64) = 16 u64 chunks
        assert_eq!(hv.data.len(), 16);
    }

    #[test]
    fn test_binding_produces_orthogonal_output() {
        let hv1 = BscVec::random_seeded(10_000, 1);
        let hv2 = BscVec::random_seeded(10_000, 2);
        let bound = hv1.bind(&hv2).unwrap();

        // XOR of two random bit strings is itself a random bit string,
        // so similarity to either input should be within 4σ of 0.
        // σ ≈ 1/√D = 0.01, so |sim| < 0.05 with overwhelming probability.
        let s1 = bound.similarity(&hv1).unwrap();
        let s2 = bound.similarity(&hv2).unwrap();
        assert!(s1.abs() < 0.05, "bind not orthogonal to input 1: {s1}");
        assert!(s2.abs() < 0.05, "bind not orthogonal to input 2: {s2}");
    }

    #[test]
    fn test_binding_is_self_inverse() {
        // BSC binding is XOR, so (A ⊕ B) ⊕ B == A exactly.
        let a = BscVec::random_seeded(10_000, 11);
        let b = BscVec::random_seeded(10_000, 22);
        let bound = a.bind(&b).unwrap();
        let recovered = bound.bind(&b).unwrap();
        assert_eq!(recovered.data, a.data);
    }

    #[test]
    fn test_bundling_similar_to_inputs() {
        let hv1 = BscVec::random_seeded(10_000, 1);
        let hv2 = BscVec::random_seeded(10_000, 2);
        let bundled = BscVec::bundle(&[&hv1, &hv2]).unwrap();

        // Bundle of 2 random vectors is similar to each input (theoretical
        // similarity ≈ 0.5 for majority vote with position-parity tiebreak).
        assert!(
            bundled.similarity(&hv1).unwrap() > 0.4,
            "bundled not similar to hv1"
        );
        assert!(
            bundled.similarity(&hv2).unwrap() > 0.4,
            "bundled not similar to hv2"
        );
    }

    #[test]
    fn test_quasi_orthogonality_of_random_vectors() {
        let a = BscVec::random_seeded(10_000, 999);
        let b = BscVec::random_seeded(10_000, 1000);
        let s = a.similarity(&b).unwrap();
        assert!(
            s.abs() < 0.05,
            "random 10k-D vectors should be quasi-orthogonal, got {s}"
        );
    }

    #[test]
    fn test_roundtrip_bytes() {
        let v = BscVec::random_seeded(10_000, 12345);
        let bytes = v.to_bytes();
        let v2 = BscVec::from_bytes(&bytes).unwrap();
        assert_eq!(v.dim, v2.dim);
        assert_eq!(v.data, v2.data);
    }

    #[test]
    fn test_dimension_mismatch_errors() {
        let a = BscVec::random_seeded(1000, 1);
        let b = BscVec::random_seeded(2000, 2);
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
    fn test_empty_bundle_errors() {
        assert!(matches!(
            BscVec::bundle(&[]),
            Err(HdcError::EmptyBundle)
        ));
    }

    #[test]
    fn test_mask_tail_keeps_form_canonical() {
        // dim=100 uses 2 u64 chunks; bits 100..128 must be zero on
        // every public-API output.
        let v = BscVec::random_seeded(100, 7);
        assert_eq!(v.data.len(), 2);
        let extra_mask: u64 = !0u64 << (100 - 64);
        assert_eq!(
            v.data[1] & extra_mask, 0,
            "tail bits leaked past dim boundary"
        );
    }
}
