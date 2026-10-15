//! Full-text search over 20,000 short chat-like messages.
//!
//! Compares the FULLTEXT index against an unindexed scan of the same table and
//! against SQLite FTS5 (bundled rusqlite) for the same ranked, filtered query:
//!   SELECT id, bm25(..), snippet(..) WHERE <match> AND project = 'p3'
//!   ORDER BY bm25 LIMIT 10
//! plus unfiltered top-5 searches for terms in thousands of messages
//! (`top5_*`), the shape a memory search runs for a common word.
//! NexusQL runs through the in-memory Executor, so WAL I/O is excluded, as is
//! SQL parsing. Run: cargo bench --bench fulltext_search

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nexusql::engine::Executor;
use nexusql::query::evaluator::QueryParams;
use nexusql::{QueryParser, Row, Statement, Value};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::time::Duration;

const DOCUMENTS: usize = 20_000;

fn vocabulary(rng: &mut StdRng) -> Vec<String> {
    const SYLLABLES: &[&str] = &[
        "ka", "lo", "mi", "ra", "ten", "dor", "sel", "qua", "fin", "bo", "zu", "pre", "lis", "mon",
        "tri", "vel",
    ];
    (0..3000)
        .map(|_| {
            (0..rng.gen_range(2..4))
                .map(|_| SYLLABLES[rng.gen_range(0..SYLLABLES.len())])
                .collect()
        })
        .collect()
}

/// Deterministic corpus with a Zipf-like word distribution, a few fixed
/// phrases, and some Japanese messages.
fn corpus() -> Vec<(i64, String, String)> {
    let mut rng = StdRng::seed_from_u64(0x00F7_5000);
    let words = vocabulary(&mut rng);
    (0..DOCUMENTS)
        .map(|id| {
            let project = format!("p{}", id % 8);
            let text = if id % 20 == 0 {
                "東京のサーバーでデータベースの検索インデックスを再構築しました".to_string()
            } else {
                let mut parts: Vec<&str> = (0..rng.gen_range(6..24))
                    .map(|_| {
                        // Rank r is chosen with probability ~ 1/r.
                        let r = (words.len() as f64).powf(rng.gen::<f64>()) as usize;
                        words[r.min(words.len() - 1)].as_str()
                    })
                    .collect();
                if id % 7 == 0 {
                    parts.push("checkpoint wal recovery");
                }
                if id % 997 == 0 {
                    parts.push("zebracorn");
                }
                // A frequent term: ~6,300 matching messages.
                if id % 3 == 1 {
                    parts.push("access");
                }
                parts.join(" ")
            };
            (id as i64, project, text)
        })
        .collect()
}

fn executor(indexed: bool) -> Executor {
    let mut ex = Executor::new();
    let p = QueryParams::new();
    ex.execute(
        &QueryParser::parse("CREATE TABLE docs (id INT, project TEXT, content TEXT)").unwrap(),
        &p,
    )
    .unwrap();
    let table = ex.get_tables_mut().get_mut("docs").unwrap();
    let columns = table.columns.clone();
    table.insert_batch(
        corpus()
            .into_iter()
            .map(|(id, project, text)| {
                Row::new(
                    columns.clone(),
                    vec![Value::Integer(id), Value::Text(project), Value::Text(text)],
                )
            })
            .collect(),
    );
    if indexed {
        ex.execute(&create_index(), &p).unwrap();
    }
    ex
}

fn create_index() -> Statement {
    QueryParser::parse("CREATE INDEX docs_fts ON docs USING fulltext (content)").unwrap()
}

const QUERIES: &[(&str, &str)] = &[
    ("rare", "zebracorn"),
    ("phrase", "\"wal recovery\""),
    ("prefix", "kalo*"),
    ("boolean", "checkpoint AND NOT recovery OR 検索"),
    ("common", "kaka"),
    ("frequent", "access"),
];

/// Unfiltered ranked top-5 with snippets over terms matching thousands of rows.
const TOP5: &[(&str, &str)] = &[("frequent", "access"), ("frequent_or", "access OR kaka")];

fn search_sql(query: &str) -> Statement {
    QueryParser::parse(&format!(
        "SELECT id, bm25(content, '{q}') AS score, snippet(content, '{q}', '[', ']', '...', 10) AS s \
         FROM docs WHERE content MATCH '{q}' AND project = 'p3' ORDER BY score LIMIT 10",
        q = query.replace('\'', "''")
    ))
    .unwrap()
}

fn top5_sql(query: &str) -> Statement {
    QueryParser::parse(&format!(
        "SELECT id, bm25(content, '{q}') AS score, snippet(content, '{q}', '[', ']', '...', 10) AS s \
         FROM docs WHERE content MATCH '{q}' ORDER BY score LIMIT 5",
        q = query.replace('\'', "''")
    ))
    .unwrap()
}

fn sqlite() -> rusqlite::Connection {
    let db = rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE VIRTUAL TABLE docs USING fts5(content, project UNINDEXED, tokenize = 'unicode61');",
    )
    .unwrap();
    let tx = db.unchecked_transaction().unwrap();
    {
        let mut insert = tx
            .prepare("INSERT INTO docs(rowid, content, project) VALUES (?1, ?2, ?3)")
            .unwrap();
        for (id, project, text) in corpus() {
            insert
                .execute(rusqlite::params![id, text, project])
                .unwrap();
        }
    }
    tx.commit().unwrap();
    db
}

fn fulltext(c: &mut Criterion) {
    let p = QueryParams::new();
    let mut group = c.benchmark_group("fulltext_20k");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    let mut plain = executor(false);
    let create = create_index();
    group.bench_function("create_index", |b| {
        b.iter(|| black_box(plain.execute(&create, &p).unwrap()))
    });

    let mut indexed = executor(true);
    let mut scan = executor(false);
    let sqlite = sqlite();
    for (name, query) in QUERIES {
        let statement = search_sql(query);
        let hits = indexed.execute(&statement, &p).unwrap().rows.len();
        assert_eq!(hits, scan.execute(&statement, &p).unwrap().rows.len());
        group.bench_function(format!("query_{name}_indexed"), |b| {
            b.iter(|| black_box(indexed.execute(&statement, &p).unwrap()))
        });
        group.bench_function(format!("query_{name}_scan"), |b| {
            b.iter(|| black_box(scan.execute(&statement, &p).unwrap()))
        });
        // FTS5 uses its own syntax for the same queries.
        let fts5_query = match *name {
            "boolean" => "(checkpoint NOT recovery) OR 検索",
            _ => query,
        };
        let mut stmt = sqlite
            .prepare(
                "SELECT rowid, bm25(docs) AS score, snippet(docs, 0, '[', ']', '...', 10) \
                 FROM docs WHERE docs MATCH ?1 AND project = 'p3' ORDER BY score LIMIT 10",
            )
            .unwrap();
        group.bench_function(format!("query_{name}_sqlite_fts5"), |b| {
            b.iter(|| {
                let rows: Vec<i64> = stmt
                    .query_map([fts5_query], |row| row.get(0))
                    .unwrap()
                    .map(Result::unwrap)
                    .collect();
                black_box(rows)
            })
        });
    }

    for (name, query) in TOP5 {
        let statement = top5_sql(query);
        let ids = |rows: &[Row]| -> Vec<Value> {
            rows.iter().map(|r| r.get("id").cloned().unwrap()).collect()
        };
        assert_eq!(
            ids(&indexed.execute(&statement, &p).unwrap().rows),
            ids(&scan.execute(&statement, &p).unwrap().rows)
        );
        group.bench_function(format!("top5_{name}_indexed"), |b| {
            b.iter(|| black_box(indexed.execute(&statement, &p).unwrap()))
        });
        let mut stmt = sqlite
            .prepare(
                "SELECT rowid, bm25(docs) AS score, snippet(docs, 0, '[', ']', '...', 10)                  FROM docs WHERE docs MATCH ?1 ORDER BY score LIMIT 5",
            )
            .unwrap();
        group.bench_function(format!("top5_{name}_sqlite_fts5"), |b| {
            b.iter(|| {
                let rows: Vec<i64> = stmt
                    .query_map([*query], |row| row.get(0))
                    .unwrap()
                    .map(Result::unwrap)
                    .collect();
                black_box(rows)
            })
        });
    }

    // Incremental maintenance: one parameterized INSERT into the indexed table.
    let insert = QueryParser::parse("INSERT INTO docs VALUES ($1, $2, $3)").unwrap();
    let mut next = DOCUMENTS as i64;
    group.bench_function("insert_one_indexed", |b| {
        b.iter(|| {
            next += 1;
            let params = QueryParams::with_positional(vec![
                Value::Integer(next),
                Value::Text("p1".into()),
                Value::Text("a freshly imported chat message about wal recovery".into()),
            ]);
            black_box(indexed.execute(&insert, &params).unwrap())
        })
    });
    group.finish();
}

criterion_group!(benches, fulltext);
criterion_main!(benches);
