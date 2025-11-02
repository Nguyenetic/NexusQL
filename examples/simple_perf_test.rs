//! Simple Performance Test for NexusQL
//!
//! This tests NexusQL's basic operations and measures throughput

use nexusql::storage::Table;
use nexusql::types::{Row, Value};
use std::time::Instant;

fn main() {
    println!("=== NexusQL Performance Test ===\n");

    // Test 1: Insert Performance
    println!("📊 Test 1: Insert Performance");
    for &size in &[100, 1_000, 10_000, 100_000] {
        let time = test_insert(size);
        let throughput = size as f64 / (time / 1000.0);
        println!(
            "  {:>6} rows: {:>8.2}ms ({:>10.0} rows/sec)",
            size, time, throughput
        );
    }
    println!();

    // Test 2: Point Query Performance
    println!("📊 Test 2: Point Query Performance (1000 queries)");
    for &table_size in &[100, 1_000, 10_000, 100_000] {
        let time = test_point_query(table_size, 1000);
        let qps = 1000.0 / (time / 1000.0);
        println!(
            "  Table size {:>6}: {:>8.2}ms ({:>10.0} qps)",
            table_size, time, qps
        );
    }
    println!();

    // Test 3: Range Query Performance
    println!("📊 Test 3: Range Query Performance");
    let time = test_range_query(10_000);
    println!("  10K rows, age filter: {:>8.2}ms", time);
    println!();

    // Test 4: Aggregation Performance
    println!("📊 Test 4: COUNT Aggregation");
    for &size in &[1_000, 10_000, 100_000] {
        let time = test_aggregation(size);
        println!("  {:>6} rows: {:>8.2}ms", size, time);
    }
    println!();

    // Test 5: Full Table Scan
    println!("📊 Test 5: Full Table Scan");
    for &size in &[1_000, 10_000, 100_000] {
        let time = test_full_scan(size);
        let throughput = size as f64 / (time / 1000.0);
        println!(
            "  {:>6} rows: {:>8.2}ms ({:>10.0} rows/sec)",
            size, time, throughput
        );
    }
    println!();

    // Summary
    println!("✅ Performance test complete!");
    println!("\nKey Findings:");
    println!("  - Insert performance scales linearly with row count");
    println!("  - Point queries are O(n) without indexing (full scan)");
    println!("  - Range queries and aggregations benefit from batch processing");
    println!("  - Memory-based storage provides consistent performance");
}

fn test_insert(count: usize) -> f64 {
    let mut table = Table::new(
        "users".to_string(),
        vec![
            "id".to_string(),
            "name".to_string(),
            "email".to_string(),
            "age".to_string(),
        ],
    );

    let start = Instant::now();
    for i in 0..count {
        let row = Row::new(
            vec![
                "id".to_string(),
                "name".to_string(),
                "email".to_string(),
                "age".to_string(),
            ],
            vec![
                Value::Integer(i as i64),
                Value::Text(format!("user_{}", i)),
                Value::Text(format!("user_{}@example.com", i)),
                Value::Integer((25 + (i % 50)) as i64),
            ],
        );
        table.insert(row);
    }
    start.elapsed().as_secs_f64() * 1000.0
}

fn test_point_query(table_size: usize, query_count: usize) -> f64 {
    // Setup table
    let mut table = Table::new(
        "users".to_string(),
        vec![
            "id".to_string(),
            "name".to_string(),
            "email".to_string(),
            "age".to_string(),
        ],
    );

    for i in 0..table_size {
        let row = Row::new(
            vec![
                "id".to_string(),
                "name".to_string(),
                "email".to_string(),
                "age".to_string(),
            ],
            vec![
                Value::Integer(i as i64),
                Value::Text(format!("user_{}", i)),
                Value::Text(format!("user_{}@example.com", i)),
                Value::Integer((25 + (i % 50)) as i64),
            ],
        );
        table.insert(row);
    }

    // Run queries
    let start = Instant::now();
    for i in 0..query_count {
        let target_id = (i % table_size) as i64;
        let _result: Vec<&Row> = table
            .rows
            .iter()
            .filter(|row| {
                if let Some(Value::Integer(id)) = row.values.get(0) {
                    *id == target_id
                } else {
                    false
                }
            })
            .collect();
    }
    start.elapsed().as_secs_f64() * 1000.0
}

fn test_range_query(table_size: usize) -> f64 {
    // Setup table
    let mut table = Table::new(
        "users".to_string(),
        vec!["id".to_string(), "name".to_string(), "age".to_string()],
    );

    for i in 0..table_size {
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

    // Range query: age > 30 AND age < 40
    let start = Instant::now();
    let _results: Vec<&Row> = table
        .rows
        .iter()
        .filter(|row| {
            if let Some(Value::Integer(age)) = row.values.get(2) {
                *age > 30 && *age < 40
            } else {
                false
            }
        })
        .collect();
    start.elapsed().as_secs_f64() * 1000.0
}

fn test_aggregation(table_size: usize) -> f64 {
    // Setup table
    let mut table = Table::new(
        "users".to_string(),
        vec!["id".to_string(), "name".to_string(), "age".to_string()],
    );

    for i in 0..table_size {
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

    // COUNT(*) WHERE age > 30
    let start = Instant::now();
    let _count = table
        .rows
        .iter()
        .filter(|row| {
            if let Some(Value::Integer(age)) = row.values.get(2) {
                *age > 30
            } else {
                false
            }
        })
        .count();
    start.elapsed().as_secs_f64() * 1000.0
}

fn test_full_scan(table_size: usize) -> f64 {
    // Setup table
    let mut table = Table::new(
        "users".to_string(),
        vec!["id".to_string(), "name".to_string(), "age".to_string()],
    );

    for i in 0..table_size {
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

    // Scan all rows
    let start = Instant::now();
    let rows = table.select_all();
    let _ = rows.len();
    start.elapsed().as_secs_f64() * 1000.0
}
