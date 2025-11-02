//! Hyperdimensional Computing Operations
//! PROPRIETARY - Energy-Efficient Intelligence

use std::ops::{Add, BitXor};

/// Hypervector - High-dimensional binary vector
#[derive(Debug, Clone)]
pub struct Hypervector {
    pub data: Vec<bool>,
    pub dimension: usize,
}

impl Hypervector {
    /// Create random hypervector
    pub fn random(dimension: usize, seed: u64) -> Self {
        let mut data = Vec::with_capacity(dimension);
        let mut rng = seed;
        
        for _ in 0..dimension {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            data.push((rng & 0x1) == 1);
        }
        
        Self { data, dimension }
    }
    
    /// Create zero hypervector
    pub fn zeros(dimension: usize) -> Self {
        Self {
            data: vec![false; dimension],
            dimension,
        }
    }
    
    /// Binding operation (combines two concepts)
    pub fn bind(&self, other: &Hypervector) -> Hypervector {
        assert_eq!(self.dimension, other.dimension);
        
        let data: Vec<bool> = self.data.iter()
            .zip(&other.data)
            .map(|(a, b)| a ^ b)  // XOR for binary
            .collect();
        
        Hypervector {
            data,
            dimension: self.dimension,
        }
    }
    
    /// Bundling operation (superposition)
    pub fn bundle(vectors: &[Hypervector]) -> Hypervector {
        if vectors.is_empty() {
            return Hypervector::zeros(10000);
        }
        
        let dimension = vectors[0].dimension;
        let mut counts = vec![0i32; dimension];
        
        for hv in vectors {
            for (i, &bit) in hv.data.iter().enumerate() {
                counts[i] += if bit { 1 } else { -1 };
            }
        }
        
        let data: Vec<bool> = counts.iter().map(|&c| c > 0).collect();
        
        Hypervector { data, dimension }
    }
    
    /// Hamming distance (for similarity)
    pub fn hamming_distance(&self, other: &Hypervector) -> usize {
        self.data.iter()
            .zip(&other.data)
            .filter(|(a, b)| a != b)
            .count()
    }
    
    /// Similarity (normalized)
    pub fn similarity(&self, other: &Hypervector) -> f32 {
        let distance = self.hamming_distance(other);
        1.0 - (distance as f32 / self.dimension as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hypervector_creation() {
        let hv = Hypervector::random(1000, 42);
        assert_eq!(hv.dimension, 1000);
        assert_eq!(hv.data.len(), 1000);
    }
    
    #[test]
    fn test_binding() {
        let hv1 = Hypervector::random(1000, 1);
        let hv2 = Hypervector::random(1000, 2);
        let bound = hv1.bind(&hv2);
        
        // Bound vector should be different from both inputs
        assert!(bound.similarity(&hv1) < 0.6);
        assert!(bound.similarity(&hv2) < 0.6);
    }
    
    #[test]
    fn test_bundling() {
        let hv1 = Hypervector::random(1000, 1);
        let hv2 = Hypervector::random(1000, 2);
        let bundled = Hypervector::bundle(&[hv1.clone(), hv2.clone()]);
        
        // Bundled vector should be similar to inputs
        assert!(bundled.similarity(&hv1) > 0.4);
        assert!(bundled.similarity(&hv2) > 0.4);
    }
}
