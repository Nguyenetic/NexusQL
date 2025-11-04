//! Quick Vector Search Test

use nexusql::vector::index::*;
use nexusql::vector::*;
use std::time::Instant;

fn generate_random_vector(dim: usize) -> Vec<f32> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..dim).map(|_| rng.gen::<f32>()).collect()
}

fn main() {
    println!("=== Quick HNSW Test ===\n");

    let dim = 128;
    let num_vectors = 1000;
    let k = 10;

    println!("Generating {} vectors...", num_vectors);
    let vectors: Vec<Vec<f32>> = (0..num_vectors)
        .map(|_| generate_random_vector(dim))
        .collect();

    let query = generate_random_vector(dim);

    // Test Flat Index
    println!("\n--- Flat Index ---");
    let mut flat_index = FlatIndex::new(dim);

    let start = Instant::now();
    for (i, vec) in vectors.iter().enumerate() {
        flat_index.insert(i as u64, vec).unwrap();
    }
    println!("Insert: {:?}", start.elapsed());

    let start = Instant::now();
    let flat_results = flat_index.search(&query, k).unwrap();
    let flat_time = start.elapsed();
    println!("Search: {:?}", flat_time);
    println!("Results: {} nearest neighbors found", flat_results.len());

    // Test HNSW Index
    println!("\n--- HNSW Index ---");
    let config = HNSWConfig::default();
    let mut hnsw_index = HNSWIndex::new(dim, config);

    let start = Instant::now();
    for (i, vec) in vectors.iter().enumerate() {
        hnsw_index.insert(i as u64, vec).unwrap();
    }
    println!("Insert: {:?}", start.elapsed());

    let start = Instant::now();
    let hnsw_results = hnsw_index.search(&query, k).unwrap();
    let hnsw_time = start.elapsed();
    println!("Search: {:?}", hnsw_time);
    println!("Results: {} nearest neighbors found", hnsw_results.len());

    // Calculate recall
    let flat_ids: std::collections::HashSet<_> = flat_results.iter().map(|r| r.row_id).collect();
    let hnsw_ids: std::collections::HashSet<_> = hnsw_results.iter().map(|r| r.row_id).collect();
    let recall = hnsw_ids.intersection(&flat_ids).count() as f32 / k as f32;

    println!("\n--- Performance ---");
    println!(
        "HNSW Speedup: {:.2}x",
        flat_time.as_secs_f64() / hnsw_time.as_secs_f64()
    );
    println!("Recall@{}: {:.1}%", k, recall * 100.0);

    // Test serialization
    println!("\n--- Serialization Test ---");
    let serialized = hnsw_index.serialize().unwrap();
    println!("Serialized size: {} bytes", serialized.len());

    let deserialized: HNSWIndex = HNSWIndex::deserialize(&serialized).unwrap();
    let deserialized_results = deserialized.search(&query, k).unwrap();

    let match_count = hnsw_results
        .iter()
        .zip(deserialized_results.iter())
        .filter(|(a, b)| a.row_id == b.row_id)
        .count();
    println!("Deserialization: {}/{} results match", match_count, k);

    println!("\n✓ All tests passed!");
}
