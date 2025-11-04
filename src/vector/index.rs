//! Vector Index Trait
//!
//! Defines common interface for all vector index types

use crate::Result;
use serde::{Deserialize, Serialize};

/// Vector search result with distance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResult {
    pub row_id: u64,
    pub distance: f32,
}

/// Index type for vector search
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexType {
    /// Brute-force flat index (exact search, O(n))
    Flat,
    /// HNSW approximate nearest neighbor (fast, ~O(log n))
    HNSW,
    /// IVF-PQ (future: billion-scale)
    #[allow(dead_code)]
    IVFPQ,
}

/// Common trait for all vector indexes
pub trait VectorIndex: Send + Sync {
    /// Insert a vector into the index
    fn insert(&mut self, row_id: u64, vector: &[f32]) -> Result<()>;

    /// Search for k nearest neighbors
    fn search(&self, query: &[f32], k: usize) -> Result<Vec<VectorSearchResult>>;

    /// Remove a vector from the index
    fn remove(&mut self, row_id: u64) -> Result<()>;

    /// Get index type
    fn index_type(&self) -> IndexType;

    /// Get number of vectors in index
    fn len(&self) -> usize;

    /// Check if index is empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Serialize index to bytes for storage
    fn serialize(&self) -> Result<Vec<u8>>;

    /// Deserialize index from bytes
    fn deserialize(data: &[u8]) -> Result<Self>
    where
        Self: Sized;
}

/// Helper function to compute distance between vectors
#[inline]
pub fn l2_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

/// Cosine distance (1 - cosine similarity)
#[inline]
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 1.0;
    }

    1.0 - (dot_product / (norm_a * norm_b))
}

/// Inner product distance (negative dot product for similarity)
#[inline]
pub fn inner_product_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    -dot_product
}

/// Inner product (negative for distance-based sorting)
#[inline]
pub fn inner_product(a: &[f32], b: &[f32]) -> f32 {
    -a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l2_distance() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let dist = l2_distance(&a, &b);
        assert!((dist - 1.414).abs() < 0.01);
    }

    #[test]
    fn test_cosine_distance() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let dist = cosine_distance(&a, &b);
        assert!((dist - 1.0).abs() < 0.01);
    }
}
