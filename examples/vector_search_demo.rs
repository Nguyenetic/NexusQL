//! Vector Search Demo
//!
//! Demonstrates HNSW vs Flat index performance

use nexusql::vector::index::*;
use nexusql::vector::*;
use std::time::Instant;

fn generate_random_vector(dim: usize) -> Vec<f32> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..dim).map(|_| rng.gen::<f32>()).collect()
}

fn main() {
    println!("=== NexusQL Vector Search Demo ===\n");

    let dim = 128;
    let num_vectors = 10_000;
    let num_queries = 100;
    let k = 10;

    println!("Configuration:");
    println!("  Dimension: {}", dim);
    println!("  Vectors: {}", num_vectors);
    println!("  Queries: {}", num_queries);
    println!("  Top-K: {}\n", k);

    // Generate test data
    println!(
        "Generating {} random {}-dimensional vectors...",
        num_vectors, dim
    );
    let vectors: Vec<Vec<f32>> = (0..num_vectors)
        .map(|_| generate_random_vector(dim))
        .collect();

    let queries: Vec<Vec<f32>> = (0..num_queries)
        .map(|_| generate_random_vector(dim))
        .collect();

    // Test Flat Index
    println!("\n--- Flat Index (Brute Force) ---");
    let mut flat_index = FlatIndex::new(dim);

    let start = Instant::now();
    for (i, vec) in vectors.iter().enumerate() {
        flat_index.insert(i as u64, vec).unwrap();
    }
    let insert_time = start.elapsed();
    println!(
        "Insert time: {:?} ({:.2} vec/s)",
        insert_time,
        num_vectors as f64 / insert_time.as_secs_f64()
    );

    let start = Instant::now();
    let mut flat_results = Vec::new();
    for query in &queries {
        flat_results.push(flat_index.search(query, k).unwrap());
    }
    let search_time = start.elapsed();
    println!(
        "Search time: {:?} ({:.2} queries/s)",
        search_time,
        num_queries as f64 / search_time.as_secs_f64()
    );
    println!("Avg query time: {:?}", search_time / num_queries as u32);

    // Test HNSW Index with default config
    println!("\n--- HNSW Index (Default Config) ---");
    let config = HNSWConfig::default();
    println!(
        "Config: M={}, ef_construction={}, ef_search={}",
        config.m, config.ef_construction, config.ef_search
    );

    let mut hnsw_index = HNSWIndex::new(dim, config);

    let start = Instant::now();
    for (i, vec) in vectors.iter().enumerate() {
        hnsw_index.insert(i as u64, vec).unwrap();
    }
    let insert_time = start.elapsed();
    println!(
        "Insert time: {:?} ({:.2} vec/s)",
        insert_time,
        num_vectors as f64 / insert_time.as_secs_f64()
    );

    let start = Instant::now();
    let mut hnsw_results = Vec::new();
    for query in &queries {
        hnsw_results.push(hnsw_index.search(query, k).unwrap());
    }
    let search_time = start.elapsed();
    println!(
        "Search time: {:?} ({:.2} queries/s)",
        search_time,
        num_queries as f64 / search_time.as_secs_f64()
    );
    println!("Avg query time: {:?}", search_time / num_queries as u32);

    // Calculate recall
    let mut total_recall = 0.0;
    for (flat_res, hnsw_res) in flat_results.iter().zip(hnsw_results.iter()) {
        let flat_ids: std::collections::HashSet<_> = flat_res.iter().map(|r| r.row_id).collect();
        let hnsw_ids: std::collections::HashSet<_> = hnsw_res.iter().map(|r| r.row_id).collect();
        let recall = hnsw_ids.intersection(&flat_ids).count() as f32 / k as f32;
        total_recall += recall;
    }
    let avg_recall = total_recall / num_queries as f32;
    println!("Recall@{}: {:.2}%", k, avg_recall * 100.0);

    // Test HNSW with high recall config
    println!("\n--- HNSW Index (High Recall Config) ---");
    let config = HNSWConfig::high_recall();
    println!(
        "Config: M={}, ef_construction={}, ef_search={}",
        config.m, config.ef_construction, config.ef_search
    );

    let mut hnsw_index = HNSWIndex::new(dim, config);

    let start = Instant::now();
    for (i, vec) in vectors.iter().enumerate() {
        hnsw_index.insert(i as u64, vec).unwrap();
    }
    let insert_time = start.elapsed();
    println!(
        "Insert time: {:?} ({:.2} vec/s)",
        insert_time,
        num_vectors as f64 / insert_time.as_secs_f64()
    );

    let start = Instant::now();
    let mut hnsw_results = Vec::new();
    for query in &queries {
        hnsw_results.push(hnsw_index.search(query, k).unwrap());
    }
    let search_time = start.elapsed();
    println!(
        "Search time: {:?} ({:.2} queries/s)",
        search_time,
        num_queries as f64 / search_time.as_secs_f64()
    );
    println!("Avg query time: {:?}", search_time / num_queries as u32);

    // Calculate recall
    let mut total_recall = 0.0;
    for (flat_res, hnsw_res) in flat_results.iter().zip(hnsw_results.iter()) {
        let flat_ids: std::collections::HashSet<_> = flat_res.iter().map(|r| r.row_id).collect();
        let hnsw_ids: std::collections::HashSet<_> = hnsw_res.iter().map(|r| r.row_id).collect();
        let recall = hnsw_ids.intersection(&flat_ids).count() as f32 / k as f32;
        total_recall += recall;
    }
    let avg_recall = total_recall / num_queries as f32;
    println!("Recall@{}: {:.2}%", k, avg_recall * 100.0);

    // Test HNSW with fast config
    println!("\n--- HNSW Index (Fast Config) ---");
    let config = HNSWConfig::fast();
    println!(
        "Config: M={}, ef_construction={}, ef_search={}",
        config.m, config.ef_construction, config.ef_search
    );

    let mut hnsw_index = HNSWIndex::new(dim, config);

    let start = Instant::now();
    for (i, vec) in vectors.iter().enumerate() {
        hnsw_index.insert(i as u64, vec).unwrap();
    }
    let insert_time = start.elapsed();
    println!(
        "Insert time: {:?} ({:.2} vec/s)",
        insert_time,
        num_vectors as f64 / insert_time.as_secs_f64()
    );

    let start = Instant::now();
    let mut hnsw_results = Vec::new();
    for query in &queries {
        hnsw_results.push(hnsw_index.search(query, k).unwrap());
    }
    let search_time = start.elapsed();
    println!(
        "Search time: {:?} ({:.2} queries/s)",
        search_time,
        num_queries as f64 / search_time.as_secs_f64()
    );
    println!("Avg query time: {:?}", search_time / num_queries as u32);

    // Calculate recall
    let mut total_recall = 0.0;
    for (flat_res, hnsw_res) in flat_results.iter().zip(hnsw_results.iter()) {
        let flat_ids: std::collections::HashSet<_> = flat_res.iter().map(|r| r.row_id).collect();
        let hnsw_ids: std::collections::HashSet<_> = hnsw_res.iter().map(|r| r.row_id).collect();
        let recall = hnsw_ids.intersection(&flat_ids).count() as f32 / k as f32;
        total_recall += recall;
    }
    let avg_recall = total_recall / num_queries as f32;
    println!("Recall@{}: {:.2}%", k, avg_recall * 100.0);

    println!("\n=== Demo Complete ===");
}
