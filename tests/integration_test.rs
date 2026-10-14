//! Integration tests for NexusSQL

use nexusql::{NexusSQL, Value};

#[tokio::test]
async fn test_initialization() {
    let dir = tempfile::tempdir().unwrap();
    let db = NexusSQL::new(dir.path().to_str().unwrap()).await.unwrap();
    let engines = db.list_engines();

    assert!(engines.contains(&"default".to_string()));
    assert!(engines.contains(&"colbert".to_string()));
    assert!(engines.contains(&"hdc".to_string()));
    assert_eq!(engines.len(), 3);

    db.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_basic_query() {
    let dir = tempfile::tempdir().unwrap();
    let db = NexusSQL::new(dir.path().to_str().unwrap()).await.unwrap();

    db.execute("CREATE TABLE test (id INT, name TEXT)")
        .await
        .unwrap();
    db.execute("INSERT INTO test VALUES (1, 'Alice')")
        .await
        .unwrap();

    let result = db.execute("SELECT * FROM test").await.unwrap();

    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].values,
        vec![Value::Integer(1), Value::Text("Alice".to_string())]
    );

    db.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_colbert_routing() {
    let dir = tempfile::tempdir().unwrap();
    let db = NexusSQL::new(dir.path().to_str().unwrap()).await.unwrap();

    // ColBERT serves a seeded demo corpus; this tests comment-hint dispatch,
    // not SQL filtering or automatic routing of semantic queries.
    let result = db.execute("SELECT 1 /* ENGINE: colbert */").await.unwrap();

    assert_eq!(result.rows.len(), 3);
    let mut ids: Vec<_> = result
        .rows
        .iter()
        .map(|row| row.get("doc_id").unwrap().as_i64().unwrap())
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, vec![1, 2, 3]);
    for row in &result.rows {
        assert!(row.get("score").unwrap().as_f64().unwrap().is_finite());
    }

    db.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_hdc_routing() {
    let dir = tempfile::tempdir().unwrap();
    let db = NexusSQL::new(dir.path().to_str().unwrap()).await.unwrap();

    // The HDC engine classifies its seeded demo input when explicitly selected.
    let result = db.execute("SELECT 1 /* ENGINE: hdc */").await.unwrap();

    assert_eq!(result.rows.len(), 1);
    assert!(matches!(
        result.rows[0].get("class"),
        Some(Value::Text(class)) if class == "category_a" || class == "category_b"
    ));
    let confidence = result.rows[0].get("confidence").unwrap().as_f64().unwrap();
    assert!(confidence > 0.0 && confidence <= 1.0);

    db.shutdown().await.unwrap();
}
