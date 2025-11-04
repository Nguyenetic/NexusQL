//! Flat (Brute-Force) Vector Index
//!
//! Simple linear scan for exact nearest neighbor search
//! - O(n) query time
//! - 100% recall
//! - Good for <10K vectors

use super::index::{l2_distance, IndexType, VectorIndex, VectorSearchResult};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Flat index storing all vectors in memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatIndex {
    /// Map of row_id to vector
    vectors: HashMap<u64, Vec<f32>>,
    /// Dimensionality of vectors
    dimension: usize,
}

impl FlatIndex {
    /// Create a new flat index
    pub fn new(dimension: usize) -> Self {
        Self {
            vectors: HashMap::new(),
            dimension,
        }
    }

    /// Get dimension
    pub fn dimension(&self) -> usize {
        self.dimension
    }
}

impl VectorIndex for FlatIndex {
    fn insert(&mut self, row_id: u64, vector: &[f32]) -> Result<()> {
        if vector.len() != self.dimension {
            return Err(crate::NexusError::QueryError(format!(
                "Vector dimension mismatch: expected {}, got {}",
                self.dimension,
                vector.len()
            )));
        }

        self.vectors.insert(row_id, vector.to_vec());
        Ok(())
    }

    fn search(&self, query: &[f32], k: usize) -> Result<Vec<VectorSearchResult>> {
        if query.len() != self.dimension {
            return Err(crate::NexusError::QueryError(format!(
                "Query dimension mismatch: expected {}, got {}",
                self.dimension,
                query.len()
            )));
        }

        // Compute distances for all vectors
        let mut results: Vec<VectorSearchResult> = self
            .vectors
            .iter()
            .map(|(&row_id, vector)| {
                let distance = l2_distance(query, vector);
                VectorSearchResult { row_id, distance }
            })
            .collect();

        // Sort by distance (ascending)
        results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());

        // Return top k
        results.truncate(k);
        Ok(results)
    }

    fn remove(&mut self, row_id: u64) -> Result<()> {
        self.vectors.remove(&row_id);
        Ok(())
    }

    fn index_type(&self) -> IndexType {
        IndexType::Flat
    }

    fn len(&self) -> usize {
        self.vectors.len()
    }

    fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(&self).map_err(|e| crate::NexusError::SerializeError(e))
    }

    fn deserialize(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data).map_err(|e| crate::NexusError::SerializeError(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flat_index_insert_search() {
        let mut index = FlatIndex::new(3);

        // Insert vectors
        index.insert(1, &[1.0, 0.0, 0.0]).unwrap();
        index.insert(2, &[0.0, 1.0, 0.0]).unwrap();
        index.insert(3, &[0.0, 0.0, 1.0]).unwrap();

        assert_eq!(index.len(), 3);

        // Search for nearest to [1.0, 0.1, 0.0]
        let results = index.search(&[1.0, 0.1, 0.0], 2).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].row_id, 1); // Closest
        assert_eq!(results[1].row_id, 2); // Second closest
    }

    #[test]
    fn test_flat_index_remove() {
        let mut index = FlatIndex::new(2);

        index.insert(1, &[1.0, 0.0]).unwrap();
        index.insert(2, &[0.0, 1.0]).unwrap();

        assert_eq!(index.len(), 2);

        index.remove(1).unwrap();
        assert_eq!(index.len(), 1);

        let results = index.search(&[1.0, 0.0], 1).unwrap();
        assert_eq!(results[0].row_id, 2);
    }
}
