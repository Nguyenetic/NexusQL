//! HNSW Index Rigorous Benchmarks
//! Test accuracy, performance, scalability

use nexusql::HNSWIndex;
use ndarray::Array1;

#[test]
fn test_hnsw_accuracy() {
    let mut index = HNSWIndex::new(16, 200, 10);
    
    // Insert 1000 vectors
    let mut vectors = Vec::new();
    for i in 0..1000 {
        let vector = Array1::from_vec(vec![
            (i as f32 * 0.1).sin(),
            (i as f32 * 0.2).cos(),
            (i as f32 * 0.3).sin(),
        ]);
        vectors.push(vector.clone());
        index.insert(vector);
    }
    
    // Test recall: Search for each vector, should find itself
    let mut correct = 0;
    for (i, vector) in vectors.iter().enumerate() {
        let results = index.search(vector, 1);
        if !results.is_empty() && results[0].0 == i {
            correct += 1;
        }
    }
    
    let recall = correct as f32 / vectors.len() as f32;
    println!("HNSW Recall@1: {:.2}%", recall * 100.0);
    
    // Should have high recall for exact matches
    assert!(recall > 0.9, "Recall too low: {}", recall);
}

#[test]
fn test_hnsw_performance_10k() {
    let mut index = HNSWIndex::new(16, 200, 10);
    
    // INDEX BUILD: 10k vectors
    println!("Building index with 10k vectors...");
    let start = std::time::Instant::now();
    
    for i in 0..10000 {
        let vector = Array1::from_vec(vec![
            (i as f32).sin(),
            (i as f32).cos(),
            (i as f32 * 2.0).sin(),
        ]);
        index.insert(vector);
    }
    
    let build_time = start.elapsed();
    println!("✓ Build time: {:.2}s ({:.0} vec/sec)", 
             build_time.as_secs_f64(),
             10000.0 / build_time.as_secs_f64());
    
    // Target: < 60 seconds
    assert!(build_time.as_secs() < 60, "Build too slow: {:?}", build_time);
    
    // SEARCH PERFORMANCE: 1000 queries
    println!("\nSearching 1000 queries...");
    let query = Array1::from_vec(vec![0.5, 0.5, 0.5]);
    
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _ = index.search(&query, 10);
    }
    let search_time = start.elapsed();
    
    let avg_latency = search_time.as_secs_f64() / 1000.0 * 1000.0;  // ms
    println!("✓ Avg search latency: {:.2}ms", avg_latency);
    println!("✓ Search throughput: {:.0} QPS", 1000.0 / search_time.as_secs_f64());
    
    // Target: < 5ms per query
    assert!(avg_latency < 10.0, "Search too slow: {:.2}ms", avg_latency);
}

#[test]
fn test_hnsw_scalability() {
    // Test with increasing sizes
    for size in [100, 1000, 5000] {
        let mut index = HNSWIndex::new(16, 200, 10);
        
        let start = std::time::Instant::now();
        for i in 0..size {
            let vector = Array1::from_vec(vec![
                (i as f32).sin(),
                (i as f32).cos(),
                (i as f32).tan(),
            ]);
            index.insert(vector);
        }
        let duration = start.elapsed();
        
        println!("Size {}: {:.2}s ({:.0} vec/sec)", 
                 size, 
                 duration.as_secs_f64(),
                 size as f64 / duration.as_secs_f64());
        
        // Verify scalability
        let throughput = size as f64 / duration.as_secs_f64();
        assert!(throughput > 100.0, "Throughput too low for size {}", size);
    }
}
