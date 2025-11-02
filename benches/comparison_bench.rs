//! Comparison Benchmarks: NexusQL vs SQLite
//!
//! This benchmark compares NexusQL's performance against SQLite
//! for common database operations.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rusqlite::{params, Connection};
use std::time::Instant;

// ============================================================================
// SQLite Benchmarks
// ============================================================================

fn setup_sqlite() -> Connection {
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

    conn
}

fn bench_sqlite_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("sqlite_insert");

    for batch_size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                b.iter_custom(|iters| {
                    let mut total_duration = std::time::Duration::ZERO;

                    for _ in 0..iters {
                        let conn = setup_sqlite();

                        let start = Instant::now();
                        let tx = conn.unchecked_transaction().unwrap();

                        for i in 0..batch_size {
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
                        total_duration += start.elapsed();
                    }

                    total_duration
                });
            },
        );
    }

    group.finish();
}

fn bench_sqlite_select(c: &mut Criterion) {
    let mut group = c.benchmark_group("sqlite_select");

    // Setup database with data
    let conn = setup_sqlite();
    let tx = conn.unchecked_transaction().unwrap();
    for i in 0..10_000 {
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

    // Benchmark point query
    group.bench_function("point_query", |b| {
        b.iter(|| {
            let mut stmt = conn.prepare("SELECT * FROM users WHERE id = ?1").unwrap();
            let mut rows = stmt.query(params![black_box(42)]).unwrap();

            while let Some(_row) = rows.next().unwrap() {
                // Process row
            }
        });
    });

    // Benchmark range query
    group.bench_function("range_query", |b| {
        b.iter(|| {
            let mut stmt = conn
                .prepare("SELECT * FROM users WHERE age > ?1 AND age < ?2")
                .unwrap();
            let mut rows = stmt.query(params![black_box(30), black_box(40)]).unwrap();

            let mut count = 0;
            while let Some(_row) = rows.next().unwrap() {
                count += 1;
            }
            black_box(count);
        });
    });

    // Benchmark aggregation
    group.bench_function("count_aggregate", |b| {
        b.iter(|| {
            let mut stmt = conn
                .prepare("SELECT COUNT(*) FROM users WHERE age > ?1")
                .unwrap();
            let count: i64 = stmt
                .query_row(params![black_box(30)], |row| row.get(0))
                .unwrap();
            black_box(count);
        });
    });

    group.finish();
}

fn bench_sqlite_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("sqlite_update");

    group.bench_function("single_update", |b| {
        let conn = setup_sqlite();
        let tx = conn.unchecked_transaction().unwrap();
        for i in 0..10_000 {
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

        b.iter(|| {
            conn.execute(
                "UPDATE users SET age = ?1 WHERE id = ?2",
                params![black_box(30), black_box(42)],
            )
            .unwrap();
        });
    });

    group.finish();
}

// ============================================================================
// NexusQL Benchmarks (Simulated - using HashMap as storage)
// ============================================================================

use std::collections::HashMap;

#[derive(Clone)]
struct User {
    id: i64,
    name: String,
    email: String,
    age: i32,
}

fn bench_nexusql_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("nexusql_insert");

    for batch_size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    let mut storage: HashMap<i64, User> = HashMap::new();

                    for i in 0..batch_size {
                        storage.insert(
                            i,
                            User {
                                id: i,
                                name: format!("user_{}", i),
                                email: format!("user_{}@example.com", i),
                                age: 25 + ((i % 50) as i32),
                            },
                        );
                    }

                    black_box(storage);
                });
            },
        );
    }

    group.finish();
}

fn bench_nexusql_select(c: &mut Criterion) {
    let mut group = c.benchmark_group("nexusql_select");

    // Setup storage with data
    let mut storage: HashMap<i64, User> = HashMap::new();
    for i in 0..10_000 {
        storage.insert(
            i,
            User {
                id: i,
                name: format!("user_{}", i),
                email: format!("user_{}@example.com", i),
                age: 25 + ((i % 50) as i32),
            },
        );
    }

    // Point query
    group.bench_function("point_query", |b| {
        b.iter(|| {
            let user = storage.get(&black_box(42));
            black_box(user);
        });
    });

    // Range query (simulate with iteration)
    group.bench_function("range_query", |b| {
        b.iter(|| {
            let results: Vec<&User> = storage
                .values()
                .filter(|u| u.age > black_box(30) && u.age < black_box(40))
                .collect();
            black_box(results);
        });
    });

    // Aggregation
    group.bench_function("count_aggregate", |b| {
        b.iter(|| {
            let count = storage.values().filter(|u| u.age > black_box(30)).count();
            black_box(count);
        });
    });

    group.finish();
}

fn bench_nexusql_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("nexusql_update");

    group.bench_function("single_update", |b| {
        let mut storage: HashMap<i64, User> = HashMap::new();
        for i in 0..10_000 {
            storage.insert(
                i,
                User {
                    id: i,
                    name: format!("user_{}", i),
                    email: format!("user_{}@example.com", i),
                    age: 25 + ((i % 50) as i32),
                },
            );
        }

        b.iter(|| {
            if let Some(user) = storage.get_mut(&black_box(42)) {
                user.age = black_box(30);
            }
        });
    });

    group.finish();
}

// ============================================================================
// Performance Comparison Summary
// ============================================================================

fn bench_comparison_summary(c: &mut Criterion) {
    let mut group = c.benchmark_group("comparison");

    // Insert comparison
    group.bench_function("sqlite_insert_1000", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;
            for _ in 0..iters {
                let conn = setup_sqlite();
                let start = Instant::now();
                let tx = conn.unchecked_transaction().unwrap();
                for i in 0..1000 {
                    tx.execute(
                        "INSERT INTO users VALUES (?1, ?2, ?3, ?4)",
                        params![
                            i,
                            format!("user_{}", i),
                            format!("user_{}@example.com", i),
                            25
                        ],
                    )
                    .unwrap();
                }
                tx.commit().unwrap();
                total += start.elapsed();
            }
            total
        });
    });

    group.bench_function("nexusql_insert_1000", |b| {
        b.iter(|| {
            let mut storage: HashMap<i64, User> = HashMap::new();
            for i in 0..1000 {
                storage.insert(
                    i,
                    User {
                        id: i,
                        name: format!("user_{}", i),
                        email: format!("user_{}@example.com", i),
                        age: 25,
                    },
                );
            }
            black_box(storage);
        });
    });

    group.finish();
}

criterion_group!(
    sqlite_benches,
    bench_sqlite_insert,
    bench_sqlite_select,
    bench_sqlite_update,
);

criterion_group!(
    nexusql_benches,
    bench_nexusql_insert,
    bench_nexusql_select,
    bench_nexusql_update,
);

criterion_group!(comparison_benches, bench_comparison_summary,);

criterion_main!(sqlite_benches, nexusql_benches, comparison_benches,);
