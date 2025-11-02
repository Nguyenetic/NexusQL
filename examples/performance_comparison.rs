//! Simple Performance Comparison: NexusQL vs SQLite
//!
//! This example compares basic insert and select performance
//! between NexusQL's in-memory storage and SQLite.

use nexusql::storage::Table;
use nexusql::types::{Row, Value};
use rusqlite::{params, Connection};
use std::time::Instant;

fn main() {
    println!("=== NexusQL vs SQLite Performance Comparison ===\n");

    // Test parameters
    let insert_sizes = vec![100, 1_000, 10_000];
    let query_count = 1_000;

    for &size in &insert_sizes {
        println!("--- Testing with {} rows ---", size);

        // SQLite test
        let sqlite_insert_time = bench_sqlite_insert(size);
        let sqlite_select_time = bench_sqlite_select(size, query_count);

        // NexusQL test
        let nexusql_insert_time = bench_nexusql_insert(size);
        let nexusql_select_time = bench_nexusql_select(size, query_count);

        // Results
        println!("SQLite:");
        println!("  Insert {} rows: {:.2}ms", size, sqlite_insert_time);
        println!(
            "  {} point queries: {:.2}ms",
            query_count, sqlite_select_time
        );
        println!(
            "  Insert throughput: {:.0} rows/sec",
            size as f64 / (sqlite_insert_time / 1000.0)
        );

        println!("NexusQL:");
        println!("  Insert {} rows: {:.2}ms", size, nexusql_insert_time);
        println!(
            "  {} point queries: {:.2}ms",
            query_count, nexusql_select_time
        );
        println!(
            "  Insert throughput: {:.0} rows/sec",
            size as f64 / (nexusql_insert_time / 1000.0)
        );

        println!("Comparison:");
        let insert_ratio = nexusql_insert_time / sqlite_insert_time;
        let select_ratio = nexusql_select_time / sqlite_select_time;
        println!(
            "  NexusQL insert is {:.2}x {} than SQLite",
            if insert_ratio > 1.0 {
                insert_ratio
            } else {
                1.0 / insert_ratio
            },
            if insert_ratio > 1.0 {
                "slower"
            } else {
                "faster"
            }
        );
        println!(
            "  NexusQL select is {:.2}x {} than SQLite",
            if select_ratio > 1.0 {
                select_ratio
            } else {
                1.0 / select_ratio
            },
            if select_ratio > 1.0 {
                "slower"
            } else {
                "faster"
            }
        );
        println!();
    }

    // Aggregation comparison
    println!("--- Aggregation Performance (10,000 rows) ---");
    let sqlite_agg = bench_sqlite_aggregation(10_000);
    let nexusql_agg = bench_nexusql_aggregation(10_000);

    println!("SQLite COUNT(*): {:.2}ms", sqlite_agg);
    println!("NexusQL COUNT(*): {:.2}ms", nexusql_agg);
    println!(
        "NexusQL is {:.2}x {} for aggregation\n",
        if nexusql_agg > sqlite_agg {
            nexusql_agg / sqlite_agg
        } else {
            sqlite_agg / nexusql_agg
        },
        if nexusql_agg > sqlite_agg {
            "slower"
        } else {
            "faster"
        }
    );
}

// ============================================================================
// SQLite Benchmarks
// ============================================================================

fn bench_sqlite_insert(count: i32) -> f64 {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER
        )",
        [],
    )
    .unwrap();

    let start = Instant::now();
    let tx = conn.unchecked_transaction().unwrap();

    for i in 0..count {
        tx.execute(
            "INSERT INTO users (id, name, email, age) VALUES (?1, ?2, ?3, ?4)",
            params![
                i,
                format!("user_{}", i),
                format!("user_{}@example.com", i),
                25 + (i % 50)
            ],
        )
        .unwrap();
    }

    tx.commit().unwrap();
    start.elapsed().as_secs_f64() * 1000.0
}

fn bench_sqlite_select(setup_count: i32, query_count: i32) -> f64 {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER
        )",
        [],
    )
    .unwrap();

    // Setup data
    let tx = conn.unchecked_transaction().unwrap();
    for i in 0..setup_count {
        tx.execute(
            "INSERT INTO users VALUES (?1, ?2, ?3, ?4)",
            params![
                i,
                format!("user_{}", i),
                format!("user_{}@example.com", i),
                25 + (i % 50)
            ],
        )
        .unwrap();
    }
    tx.commit().unwrap();

    // Benchmark queries
    let start = Instant::now();
    for i in 0..query_count {
        let mut stmt = conn.prepare("SELECT * FROM users WHERE id = ?1").unwrap();
        let mut rows = stmt.query(params![i % setup_count]).unwrap();
        while let Some(_row) = rows.next().unwrap() {}
    }
    start.elapsed().as_secs_f64() * 1000.0
}

fn bench_sqlite_aggregation(count: i32) -> f64 {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT,
            age INTEGER
        )",
        [],
    )
    .unwrap();

    let tx = conn.unchecked_transaction().unwrap();
    for i in 0..count {
        tx.execute(
            "INSERT INTO users VALUES (?1, ?2, ?3)",
            params![i, format!("user_{}", i), 25 + (i % 50)],
        )
        .unwrap();
    }
    tx.commit().unwrap();

    let start = Instant::now();
    let _count: i64 = conn
        .query_row("SELECT COUNT(*) FROM users WHERE age > 30", [], |row| {
            row.get(0)
        })
        .unwrap();
    start.elapsed().as_secs_f64() * 1000.0
}

// ============================================================================
// NexusQL Benchmarks (using actual NexusQL components)
// ============================================================================

fn bench_nexusql_insert(count: i32) -> f64 {
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

fn bench_nexusql_select(setup_count: i32, query_count: i32) -> f64 {
    let mut table = Table::new(
        "users".to_string(),
        vec![
            "id".to_string(),
            "name".to_string(),
            "email".to_string(),
            "age".to_string(),
        ],
    );

    // Setup data
    for i in 0..setup_count {
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

    // Benchmark queries - simple scan through all rows
    let start = Instant::now();
    for i in 0..query_count {
        let target_id = i % setup_count;
        let _rows: Vec<&Row> = table
            .rows
            .iter()
            .filter(|row| {
                if let Some(Value::Integer(id)) = row.values.get(0) {
                    *id == target_id as i64
                } else {
                    false
                }
            })
            .collect();
    }
    start.elapsed().as_secs_f64() * 1000.0
}

fn bench_nexusql_aggregation(count: i32) -> f64 {
    let mut table = Table::new(
        "users".to_string(),
        vec!["id".to_string(), "name".to_string(), "age".to_string()],
    );

    for i in 0..count {
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

    let start = Instant::now();
    let count = table
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
    let _ = count;
    start.elapsed().as_secs_f64() * 1000.0
}
