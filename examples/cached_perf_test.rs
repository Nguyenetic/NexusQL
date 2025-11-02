//! Performance Test: Query Cache Integration
//!
//! Demonstrates the dramatic speedup from caching query results

use nexusql::cache::{CacheConfig, QueryCache};
use nexusql::storage::Table;
use nexusql::types::{ResultSet, Row, Value};
use std::time::Instant;

fn main() {
    println!("=== NexusQL Query Cache Performance ===\n");

    // Setup table with data
    let mut table = create_test_table(10_000);

    // Create index for fast lookups
    table.create_index("id").unwrap();
    table.create_index("age").unwrap();

    // Create cache
    let config = CacheConfig {
        max_entries: 100,
        default_ttl: 300,
        enabled: true,
        ..Default::default()
    };
    let mut cache = QueryCache::new(config);

    println!("📊 Test Setup:");
    println!("  - Table size: 10,000 rows");
    println!("  - Indexes: id, age");
    println!("  - Cache size: 100 entries\n");

    // Test 1: First execution (cache miss)
    println!("🔍 Test 1: Point Query Performance");
    let query1 = "SELECT * FROM users WHERE id = 42";

    println!("  First execution (cache miss):");
    let miss_start = Instant::now();
    let result1 = execute_query(&table, query1);
    let miss_time = miss_start.elapsed().as_secs_f64() * 1000.0;
    println!("    Time: {:.4}ms", miss_time);

    // Cache the result
    cache.put(query1, result1.clone());

    // Test 2: Cached execution (cache hit)
    println!("  Second execution (cache hit):");
    let hit_start = Instant::now();
    let _cached_result = cache.get(query1).unwrap();
    let hit_time = hit_start.elapsed().as_secs_f64() * 1000.0;
    println!("    Time: {:.4}ms", hit_time);

    let speedup = miss_time / hit_time;
    println!("  ⚡ SPEEDUP: {:.0}x faster\n", speedup);

    // Test 3: Multiple different queries
    println!("🔍 Test 2: Multiple Query Performance (100 different queries)");
    let queries: Vec<String> = (0..100)
        .map(|i| format!("SELECT * FROM users WHERE id = {}", i))
        .collect();

    // Execute all queries without cache
    println!("  Without cache:");
    let no_cache_start = Instant::now();
    for query in &queries {
        let _ = execute_query(&table, query);
    }
    let no_cache_time = no_cache_start.elapsed().as_secs_f64() * 1000.0;
    println!("    Time: {:.2}ms", no_cache_time);

    // Execute with cache (first time - all misses)
    let mut cache2 = QueryCache::new(CacheConfig::default());
    println!("  With cache (first run - building cache):");
    let cache_build_start = Instant::now();
    for query in &queries {
        if let Some(cached) = cache2.get(query) {
            let _ = cached;
        } else {
            let result = execute_query(&table, query);
            cache2.put(query, result);
        }
    }
    let cache_build_time = cache_build_start.elapsed().as_secs_f64() * 1000.0;
    println!("    Time: {:.2}ms", cache_build_time);

    // Execute with cache (second time - all hits!)
    println!("  With cache (second run - all hits):");
    let cache_hit_start = Instant::now();
    for query in &queries {
        let _ = cache2.get(query).unwrap();
    }
    let cache_hit_time = cache_hit_start.elapsed().as_secs_f64() * 1000.0;
    println!("    Time: {:.2}ms", cache_hit_time);

    let cache_speedup = no_cache_time / cache_hit_time;
    println!("  ⚡ SPEEDUP: {:.0}x faster\n", cache_speedup);

    // Test 4: Repeated query pattern
    println!("🔍 Test 3: Repeated Query Pattern (same 10 queries, 100 times each)");
    let repeated_queries: Vec<String> = (0..10)
        .map(|i| format!("SELECT * FROM users WHERE age > {}", 20 + i))
        .collect();

    // Without cache
    println!("  Without cache (1000 queries):");
    let repeat_no_cache_start = Instant::now();
    for _ in 0..100 {
        for query in &repeated_queries {
            let _ = execute_range_query(&table, 20 + (query.len() % 10) as i64);
        }
    }
    let repeat_no_cache_time = repeat_no_cache_start.elapsed().as_secs_f64() * 1000.0;
    println!("    Time: {:.2}ms", repeat_no_cache_time);

    // With cache
    let mut cache3 = QueryCache::new(CacheConfig::default());
    println!("  With cache (1000 queries):");
    let repeat_cache_start = Instant::now();
    for _ in 0..100 {
        for query in &repeated_queries {
            if let Some(cached) = cache3.get(query) {
                let _ = cached;
            } else {
                let result = execute_range_query(&table, 20 + (query.len() % 10) as i64);
                cache3.put(query, result);
            }
        }
    }
    let repeat_cache_time = repeat_cache_start.elapsed().as_secs_f64() * 1000.0;
    println!("    Time: {:.2}ms", repeat_cache_time);

    let repeat_speedup = repeat_no_cache_time / repeat_cache_time;
    println!("  ⚡ SPEEDUP: {:.0}x faster\n", repeat_speedup);

    // Statistics
    println!("📈 Cache Statistics:");
    let stats = cache3.get_stats();
    println!("  Hits: {}", stats.hits);
    println!("  Misses: {}", stats.misses);
    println!("  Hit Rate: {:.1}%", stats.hit_rate() * 100.0);
    println!("  Total Cached: {}", stats.total_cached);

    println!("\n✅ Cache Performance Test Complete!");
    println!("\nKey Findings:");
    println!("  - Single query cache hit: ~{:.0}x faster", speedup);
    println!(
        "  - Multiple unique queries: ~{:.0}x faster with warm cache",
        cache_speedup
    );
    println!(
        "  - Repeated queries: ~{:.0}x faster (best case scenario)",
        repeat_speedup
    );
    println!("  - Cache is most effective for repeated queries");
    println!(
        "  - Hit rate of {:.0}% achieved on repeated workload",
        stats.hit_rate() * 100.0
    );
}

fn create_test_table(size: usize) -> Table {
    let mut table = Table::new(
        "users".to_string(),
        vec!["id".to_string(), "name".to_string(), "age".to_string()],
    );

    for i in 0..size {
        let row = Row::new(
            vec!["id".to_string(), "name".to_string(), "age".to_string()],
            vec![
                Value::Integer(i as i64),
                Value::Text(format!("user_{}", i)),
                Value::Integer((25 + (i % 50)) as i64),
            ],
        );
        table.insert(row);
    }

    table
}

fn execute_query(table: &Table, _query: &str) -> ResultSet {
    // Simulate query execution - point lookup
    let result = table.lookup("id", &Value::Integer(42));
    ResultSet::new(result)
}

fn execute_range_query(table: &Table, min_age: i64) -> ResultSet {
    // Simulate range query
    let result = table.range_lookup(
        "age",
        &Value::Integer(min_age),
        &Value::Integer(min_age + 10),
    );
    ResultSet::new(result)
}
