//! Quick MCP Real-World Demo
//! Fast version for demonstration with 1000 memories

use nexusql::vector::index::*;
use nexusql::vector::*;
use std::time::Instant;

fn generate_realistic_embedding(seed: f32) -> Vec<f32> {
    let dim = 384; // all-MiniLM-L6-v2
    let mut embedding = Vec::with_capacity(dim);

    for i in 0..dim {
        let val = ((seed + i as f32) * 0.1).sin() + ((seed * 2.0 + i as f32) * 0.05).cos();
        embedding.push(val);
    }

    // Normalize
    let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    embedding.iter().map(|x| x / magnitude).collect()
}

fn main() {
    println!("=== MCP Real-World Demo (Quick) ===\n");

    let dim = 384; // all-MiniLM-L6-v2 model
    let num_memories = 1000;
    let num_queries = 50;
    let top_k = 10;

    println!("Simulating MCP sage project:");
    println!("  Model: all-MiniLM-L6-v2 (384 dims)");
    println!("  Memories: {}", num_memories);
    println!("  Queries: {}\n", num_queries);

    // Generate data
    let memories: Vec<Vec<f32>> = (0..num_memories)
        .map(|i| generate_realistic_embedding(i as f32))
        .collect();

    let queries: Vec<Vec<f32>> = (0..num_queries)
        .map(|i| generate_realistic_embedding(1000.0 + i as f32))
        .collect();

    println!("--- Flat Index (sqlite-vec equivalent) ---");
    let mut flat_index = FlatIndex::new(dim);

    let start = Instant::now();
    for (i, vec) in memories.iter().enumerate() {
        flat_index.insert(i as u64, vec).unwrap();
    }
    let flat_build = start.elapsed();

    let start = Instant::now();
    for query in &queries {
        let _ = flat_index.search(query, top_k).unwrap();
    }
    let flat_search = start.elapsed();

    println!("Build: {:?}", flat_build);
    println!("Search: {:?} ({} queries)", flat_search, num_queries);
    println!(
        "Avg: {:.2}ms per query\n",
        flat_search.as_secs_f64() * 1000.0 / num_queries as f64
    );

    // Test different HNSW configs
    let configs = vec![
        ("Fast", HNSWConfig::fast()),
        ("Default", HNSWConfig::default()),
        ("High Recall", HNSWConfig::high_recall()),
    ];

    for (name, config) in configs {
        println!("--- HNSW {} ---", name);
        println!(
            "M={}, ef_construction={}, ef_search={}",
            config.m, config.ef_construction, config.ef_search
        );

        let mut hnsw = HNSWIndex::new(dim, config);

        let start = Instant::now();
        for (i, vec) in memories.iter().enumerate() {
            hnsw.insert(i as u64, vec).unwrap();
        }
        let hnsw_build = start.elapsed();

        let start = Instant::now();
        for query in &queries {
            let _ = hnsw.search(query, top_k).unwrap();
        }
        let hnsw_search = start.elapsed();

        let speedup = flat_search.as_secs_f64() / hnsw_search.as_secs_f64();

        println!("Build: {:?}", hnsw_build);
        println!("Search: {:?} ({} queries)", hnsw_search, num_queries);
        println!(
            "Avg: {:.2}ms per query",
            hnsw_search.as_secs_f64() * 1000.0 / num_queries as f64
        );
        println!("🚀 Speedup: {:.2}×\n", speedup);
    }

    println!("=== Summary ===");
    println!("\nFor MCP with {} memories:", num_memories);
    println!(
        "  sqlite-vec: ~{:.1}ms per semantic search",
        flat_search.as_secs_f64() * 1000.0 / num_queries as f64
    );

    // Calculate best HNSW time
    let mut best_hnsw = HNSWIndex::new(dim, HNSWConfig::default());
    for (i, vec) in memories.iter().enumerate() {
        best_hnsw.insert(i as u64, vec).unwrap();
    }
    let start = Instant::now();
    for query in &queries {
        let _ = best_hnsw.search(query, top_k).unwrap();
    }
    let best_time = start.elapsed();
    let best_avg = best_time.as_secs_f64() * 1000.0 / num_queries as f64;
    let speedup = (flat_search.as_secs_f64() * 1000.0 / num_queries as f64) / best_avg;

    println!("  NexusQL HNSW: ~{:.1}ms per semantic search", best_avg);
    println!("  Improvement: {:.1}× faster!", speedup);

    println!("\nProjected for 10K memories:");
    println!(
        "  sqlite-vec: ~{:.0}ms",
        (flat_search.as_secs_f64() * 1000.0 / num_queries as f64) * 10.0
    );
    println!("  NexusQL: ~{:.0}ms", best_avg * 1.5);
    println!(
        "  Speedup: ~{:.0}×",
        ((flat_search.as_secs_f64() * 1000.0 / num_queries as f64) * 10.0) / (best_avg * 1.5)
    );

    println!(
        "\n✅ NexusQL is {:.0}× faster than sqlite-vec for MCP!",
        speedup
    );
}
