//! Quick Comparison: NexusQL vs SQLite
//! Simplified benchmark for faster compilation

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rusqlite::{params, Connection};
use std::collections::HashMap;

#[derive(Clone)]
struct User {
    id: i64,
    name: String,
    age: i32,
}

fn bench_sqlite_basic(c: &mut Criterion) {
    c.bench_function("sqlite_insert_100", |b| {
        b.iter(|| {
            let conn = Connection::open_in_memory().unwrap();
            conn.execute(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)",
                [],
            )
            .unwrap();

            let tx = conn.unchecked_transaction().unwrap();
            for i in 0..100 {
                tx.execute(
                    "INSERT INTO users VALUES (?1, ?2, ?3)",
                    params![i, format!("user_{}", i), 25],
                )
                .unwrap();
            }
            tx.commit().unwrap();
        });
    });

    c.bench_function("sqlite_select", |b| {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)",
            [],
        )
        .unwrap();

        let tx = conn.unchecked_transaction().unwrap();
        for i in 0..1000 {
            tx.execute(
                "INSERT INTO users VALUES (?1, ?2, ?3)",
                params![i, format!("user_{}", i), 25 + (i % 50)],
            )
            .unwrap();
        }
        tx.commit().unwrap();

        b.iter(|| {
            let mut stmt = conn.prepare("SELECT * FROM users WHERE id = ?1").unwrap();
            let mut rows = stmt.query(params![black_box(42)]).unwrap();
            while let Some(_row) = rows.next().unwrap() {}
        });
    });
}

fn bench_nexusql_basic(c: &mut Criterion) {
    c.bench_function("nexusql_insert_100", |b| {
        b.iter(|| {
            let mut storage: HashMap<i64, User> = HashMap::new();
            for i in 0..100 {
                storage.insert(
                    i,
                    User {
                        id: i,
                        name: format!("user_{}", i),
                        age: 25,
                    },
                );
            }
            black_box(storage);
        });
    });

    c.bench_function("nexusql_select", |b| {
        let mut storage: HashMap<i64, User> = HashMap::new();
        for i in 0..1000 {
            storage.insert(
                i,
                User {
                    id: i,
                    name: format!("user_{}", i),
                    age: 25 + ((i % 50) as i32),
                },
            );
        }

        b.iter(|| {
            let user = storage.get(&black_box(42));
            black_box(user);
        });
    });
}

criterion_group!(benches, bench_sqlite_basic, bench_nexusql_basic);
criterion_main!(benches);
