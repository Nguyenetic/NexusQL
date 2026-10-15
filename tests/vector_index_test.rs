//! HNSW vector indexes through SQL: kNN answers, maintenance by DML,
//! transactions, and the definition surviving reopen.
use nexusql::{NexusSQL, ResultSet, Value};
use tempfile::TempDir;

const DIM: usize = 24;

/// Deterministic pseudo-random vectors (xorshift), so failures reproduce.
fn vectors(n: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut state = seed | 1;
    (0..n)
        .map(|_| {
            (0..DIM)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    (state % 2000) as f32 / 1000.0 - 1.0
                })
                .collect()
        })
        .collect()
}

fn ids(result: &ResultSet) -> Vec<i64> {
    result
        .rows
        .iter()
        .map(|row| row.get("id").and_then(Value::as_i64).unwrap())
        .collect()
}

async fn seed(db: &NexusSQL, table: &str, data: &[Vec<f32>], index: Option<&str>) {
    db.execute(&format!(
        "CREATE TABLE {table} (id INT, project TEXT, e VECTOR({DIM}))"
    ))
    .await
    .unwrap();
    if let Some(options) = index {
        db.execute(&format!(
            "CREATE INDEX {table}_hnsw ON {table} USING hnsw (e) {options}"
        ))
        .await
        .unwrap();
    }
    db.execute("BEGIN").await.unwrap();
    for (i, v) in data.iter().enumerate() {
        db.execute_with_params(
            &format!("INSERT INTO {table} (id, project, e) VALUES ($1, $2, $3)"),
            Some(vec![
                Value::Integer(i as i64),
                Value::Text(format!("p{}", i % 10)),
                Value::Vector(v.clone()),
            ]),
        )
        .await
        .unwrap();
    }
    db.execute("COMMIT").await.unwrap();
}

async fn knn(db: &NexusSQL, sql: &str, q: &[f32]) -> Vec<i64> {
    ids(&db
        .execute_with_params(sql, Some(vec![Value::Vector(q.to_vec())]))
        .await
        .unwrap())
}

#[tokio::test]
async fn hnsw_results_match_the_exact_scan() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    let data = vectors(800, 7);
    seed(&db, "exact", &data, None).await;
    seed(&db, "indexed", &data, Some("WITH (metric = 'cosine')")).await;
    let queries = vectors(30, 99);
    let (mut hits, mut total) = (0, 0);
    for q in &queries {
        let want = knn(&db, "SELECT id FROM exact ORDER BY e <=> $1 LIMIT 10", q).await;
        let got = knn(&db, "SELECT id FROM indexed ORDER BY e <=> $1 LIMIT 10", q).await;
        assert_eq!(got.len(), 10);
        hits += got.iter().filter(|id| want.contains(id)).count();
        total += want.len();
    }
    let recall = hits as f64 / total as f64;
    assert!(recall >= 0.9, "recall@10 {recall}");
    // A stored vector is its own nearest neighbor, and the function form works.
    let got = knn(
        &db,
        "SELECT id, cosine_distance(e, $1) AS d FROM indexed ORDER BY d LIMIT 1",
        &data[123],
    )
    .await;
    assert_eq!(got, [123]);
}

#[tokio::test]
async fn filters_and_maintenance() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    let data = vectors(300, 3);
    seed(&db, "v", &data, Some("WITH (m = 8, ef_search = 64)")).await;
    let q = data[40].clone();
    // Build the graph, then change the table under it.
    assert_eq!(
        knn(&db, "SELECT id FROM v ORDER BY e <-> $1 LIMIT 1", &q).await,
        [40]
    );
    db.execute("DELETE FROM v WHERE id = 40").await.unwrap();
    let after = knn(&db, "SELECT id FROM v ORDER BY e <-> $1 LIMIT 3", &q).await;
    assert!(!after.contains(&40));
    db.execute_with_params(
        "UPDATE v SET e = $1 WHERE id = 7",
        Some(vec![Value::Vector(q.clone())]),
    )
    .await
    .unwrap();
    assert_eq!(
        knn(&db, "SELECT id FROM v ORDER BY e <-> $1 LIMIT 1", &q).await,
        [7]
    );
    db.execute_with_params(
        "INSERT INTO v (id, project, e) VALUES (1000, 'p0', $1)",
        Some(vec![Value::Vector(q.iter().map(|x| x + 0.001).collect())]),
    )
    .await
    .unwrap();
    assert_eq!(
        knn(&db, "SELECT id FROM v ORDER BY e <-> $1 LIMIT 2", &q).await,
        [7, 1000]
    );
    // WHERE is applied to the candidates; a selective filter still fills k.
    let p3 = knn(
        &db,
        "SELECT id FROM v WHERE project = 'p3' ORDER BY e <-> $1 LIMIT 5",
        &q,
    )
    .await;
    assert_eq!(p3.len(), 5);
    assert!(p3.iter().all(|id| id % 10 == 3));
    // The p3 rows were not modified: the true nearest is computable here.
    let l2 = |v: &[f32]| {
        v.iter()
            .zip(&q)
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f32>()
    };
    let exact = (0..data.len())
        .filter(|i| i % 10 == 3)
        .min_by(|a, b| l2(&data[*a]).total_cmp(&l2(&data[*b])))
        .unwrap();
    assert_eq!(p3[0], exact as i64);
}

#[tokio::test]
async fn rollback_and_null_vectors() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    let data = vectors(100, 11);
    seed(&db, "v", &data, Some("")).await;
    let far = vec![50.0; DIM];
    knn(&db, "SELECT id FROM v ORDER BY e <-> $1 LIMIT 1", &far).await;
    db.execute("BEGIN").await.unwrap();
    db.execute_with_params(
        "INSERT INTO v (id, project, e) VALUES (500, 'p0', $1)",
        Some(vec![Value::Vector(far.clone())]),
    )
    .await
    .unwrap();
    assert_eq!(
        knn(&db, "SELECT id FROM v ORDER BY e <-> $1 LIMIT 1", &far).await,
        [500]
    );
    db.execute("ROLLBACK").await.unwrap();
    assert_ne!(
        knn(&db, "SELECT id FROM v ORDER BY e <-> $1 LIMIT 1", &far).await,
        [500]
    );
    // A NULL vector cannot be indexed; the query scans and still answers.
    db.execute("INSERT INTO v (id, project, e) VALUES (600, 'p0', NULL)")
        .await
        .unwrap();
    let got = knn(&db, "SELECT id FROM v ORDER BY e <-> $1 LIMIT 3", &data[5]).await;
    assert!(got.contains(&5), "{got:?}");
}

#[tokio::test]
async fn bad_options_are_rejected() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    db.execute(&format!("CREATE TABLE v (id INT, e VECTOR({DIM}))"))
        .await
        .unwrap();
    let err = db
        .execute("CREATE INDEX h ON v USING hnsw (e) WITH (metric = 'manhattan')")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("unknown HNSW metric"), "{err}");
    assert!(db
        .execute("CREATE INDEX h ON v USING hnsw (e) WITH (m = 0)")
        .await
        .is_err());
}

#[tokio::test]
async fn definition_survives_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("db");
    let path = path.to_str().unwrap();
    let data = vectors(200, 5);
    {
        let db = NexusSQL::new(path).await.unwrap();
        seed(&db, "v", &data, Some("WITH (metric = 'cosine')")).await;
        db.shutdown().await.unwrap();
    }
    let db = NexusSQL::new(path).await.unwrap();
    assert_eq!(
        knn(&db, "SELECT id FROM v ORDER BY e <=> $1 LIMIT 1", &data[77]).await,
        [77]
    );
    // Re-creating the index under the same name replaces the definition.
    db.execute("CREATE INDEX v_hnsw ON v USING hnsw (e)")
        .await
        .unwrap();
    assert_eq!(
        knn(&db, "SELECT id FROM v ORDER BY e <-> $1 LIMIT 1", &data[77]).await,
        [77]
    );
    db.shutdown().await.unwrap();
}

/// Timing at AMen scale (20k x 768). Run with
/// `cargo test --release --test vector_index_test -- --ignored --nocapture`.
#[tokio::test]
#[ignore]
async fn scale_20k_768() {
    use std::time::Instant;
    let db = NexusSQL::new(":memory:").await.unwrap();
    db.execute("CREATE TABLE v (id INT, e VECTOR(768))")
        .await
        .unwrap();
    db.execute("CREATE INDEX vh ON v USING hnsw (e) WITH (metric = 'cosine')")
        .await
        .unwrap();
    let mut state: u64 = 42;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % 2000) as f32 / 1000.0 - 1.0
    };
    // Clustered data, closer to real embeddings than uniform noise.
    let centers: Vec<Vec<f32>> = (0..200)
        .map(|_| (0..768).map(|_| next()).collect())
        .collect();
    let data: Vec<Vec<f32>> = (0..20_000)
        .map(|i| centers[i % 200].iter().map(|c| c + 0.6 * next()).collect())
        .collect();
    db.execute("BEGIN").await.unwrap();
    for (i, v) in data.iter().enumerate() {
        db.execute_with_params(
            "INSERT INTO v (id, e) VALUES ($1, $2)",
            Some(vec![Value::Integer(i as i64), Value::Vector(v.clone())]),
        )
        .await
        .unwrap();
    }
    db.execute("COMMIT").await.unwrap();
    let q = |i: usize| vec![Value::Vector(data[i].iter().map(|x| x + 0.01).collect())];
    let t = Instant::now();
    db.execute_with_params("SELECT id FROM v ORDER BY e <=> $1 LIMIT 10", Some(q(0)))
        .await
        .unwrap();
    println!("first query (builds graph): {:?}", t.elapsed());
    let t = Instant::now();
    let (mut hits, mut total) = (0, 0);
    for i in (0..20_000).step_by(1000) {
        let got = ids(&db
            .execute_with_params("SELECT id FROM v ORDER BY e <=> $1 LIMIT 10", Some(q(i)))
            .await
            .unwrap());
        hits += usize::from(got.first() == Some(&(i as i64)));
        total += 1;
    }
    println!(
        "20 indexed queries: {:?}, self-recall@1 {hits}/{total}",
        t.elapsed()
    );
    let t = Instant::now();
    db.execute_with_params(
        "SELECT id FROM v ORDER BY inner_product(e, $1) LIMIT 10",
        Some(q(0)),
    )
    .await
    .unwrap();
    println!("one exact scan: {:?}", t.elapsed());
}
