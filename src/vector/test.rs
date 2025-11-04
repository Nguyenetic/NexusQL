//! Vector Index Tests

#[cfg(test)]
mod tests {
    use crate::vector::index::*;
    use crate::vector::*;

    fn generate_random_vector(dim: usize) -> Vec<f32> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..dim).map(|_| rng.gen::<f32>()).collect()
    }

    #[test]
    fn test_flat_index_basic() {
        let mut index = FlatIndex::new(128);

        // Insert some vectors
        for i in 0..100 {
            let vec = generate_random_vector(128);
            index.insert(i, &vec).unwrap();
        }

        assert_eq!(index.len(), 100);

        // Search
        let query = generate_random_vector(128);
        let results = index.search(&query, 10).unwrap();

        assert_eq!(results.len(), 10);

        // Results should be sorted by distance
        for i in 1..results.len() {
            assert!(results[i].distance >= results[i - 1].distance);
        }
    }

    #[test]
    fn test_hnsw_index_basic() {
        let config = HNSWConfig::default();
        let mut index = HNSWIndex::new(128, config);

        // Insert some vectors
        for i in 0..100 {
            let vec = generate_random_vector(128);
            index.insert(i, &vec).unwrap();
        }

        assert_eq!(index.len(), 100);

        // Search
        let query = generate_random_vector(128);
        let results = index.search(&query, 10).unwrap();

        assert_eq!(results.len(), 10);

        // Results should be sorted by distance
        for i in 1..results.len() {
            assert!(results[i].distance >= results[i - 1].distance);
        }
    }

    #[test]
    fn test_hnsw_accuracy() {
        let config = HNSWConfig::high_recall();
        let mut hnsw_index = HNSWIndex::new(128, config);
        let mut flat_index = FlatIndex::new(128);

        // Insert same vectors into both indices
        let vectors: Vec<Vec<f32>> = (0..500).map(|_| generate_random_vector(128)).collect();

        for (i, vec) in vectors.iter().enumerate() {
            hnsw_index.insert(i as u64, vec).unwrap();
            flat_index.insert(i as u64, vec).unwrap();
        }

        // Search with both indices
        let query = generate_random_vector(128);
        let hnsw_results = hnsw_index.search(&query, 10).unwrap();
        let flat_results = flat_index.search(&query, 10).unwrap();

        // Count how many results match (HNSW is approximate)
        let hnsw_ids: std::collections::HashSet<_> =
            hnsw_results.iter().map(|r| r.row_id).collect();
        let flat_ids: std::collections::HashSet<_> =
            flat_results.iter().map(|r| r.row_id).collect();

        let recall = hnsw_ids.intersection(&flat_ids).count() as f32 / 10.0;

        // HNSW should have at least 70% recall with high_recall config
        assert!(recall >= 0.7, "HNSW recall too low: {}", recall);
    }

    #[test]
    fn test_distance_functions() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];

        // L2 distance
        let l2 = l2_distance(&a, &b);
        assert!((l2 - 5.196).abs() < 0.01);

        // Cosine distance
        let cos = cosine_distance(&a, &b);
        assert!(cos >= 0.0 && cos <= 1.0);

        // Inner product distance
        let ip = inner_product_distance(&a, &b);
        assert!(ip < 0.0); // Should be negative of dot product
    }

    #[test]
    fn test_serialization() {
        let mut index = FlatIndex::new(128);

        for i in 0..50 {
            let vec = generate_random_vector(128);
            index.insert(i, &vec).unwrap();
        }

        // Serialize
        let serialized = index.serialize().unwrap();

        // Deserialize
        let deserialized: FlatIndex = FlatIndex::deserialize(&serialized).unwrap();

        assert_eq!(index.len(), deserialized.len());

        // Search should give same results
        let query = generate_random_vector(128);
        let original_results = index.search(&query, 5).unwrap();
        let restored_results = deserialized.search(&query, 5).unwrap();

        for (a, b) in original_results.iter().zip(restored_results.iter()) {
            assert_eq!(a.row_id, b.row_id);
            assert!((a.distance - b.distance).abs() < 0.001);
        }
    }
}
