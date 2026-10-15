//! INSERT/UPDATE/DELETE report how many rows they changed.
use nexusql::{NexusSQL, Value};

#[tokio::test]
async fn dml_reports_rows_affected() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    db.execute("CREATE TABLE t (id INT, name TEXT)")
        .await
        .unwrap();
    // Literal INSERT takes the fast bulk path; parameterized INSERT the executor.
    let r = db
        .execute("INSERT INTO t VALUES (1, 'a'), (2, 'b'), (3, 'c')")
        .await
        .unwrap();
    assert_eq!(r.rows_affected, 3);
    let r = db
        .execute_with_params(
            "INSERT INTO t VALUES (?, ?)",
            Some(vec![Value::Integer(4), Value::Text("d".into())]),
        )
        .await
        .unwrap();
    assert_eq!(r.rows_affected, 1);
    let r = db
        .execute("UPDATE t SET name = 'x' WHERE id >= 3")
        .await
        .unwrap();
    assert_eq!(r.rows_affected, 2);
    let r = db.execute("DELETE FROM t WHERE name = 'x'").await.unwrap();
    assert_eq!(r.rows_affected, 2);
    let r = db.execute("DELETE FROM t WHERE id = 99").await.unwrap();
    assert_eq!(r.rows_affected, 0);
    let r = db.execute("SELECT * FROM t").await.unwrap();
    assert_eq!(r.rows.len(), 2);
}

#[tokio::test]
async fn literal_and_bulk_inserts_assign_auto_increment_ids() {
    for ddl in [
        "CREATE TABLE n (id SERIAL, body TEXT)",
        "CREATE TABLE n (id INTEGER PRIMARY KEY AUTOINCREMENT, body TEXT)",
    ] {
        let db = NexusSQL::new(":memory:").await.unwrap();
        db.execute(ddl).await.unwrap();
        // Literal INSERT uses the direct bulk path, which skipped AUTO_INCREMENT.
        db.execute("INSERT INTO n (body) VALUES ('a'), ('b')")
            .await
            .unwrap();
        db.execute_with_params(
            "INSERT INTO n (body) VALUES ($1)",
            Some(vec![Value::Text("c".into())]),
        )
        .await
        .unwrap();
        db.bulk_insert("n", &["body".to_string()], vec![Value::Text("d".into())], 1)
            .await
            .unwrap();
        let ids: Vec<_> = db
            .execute("SELECT id FROM n")
            .await
            .unwrap()
            .rows
            .iter()
            .map(|r| r.get("id").cloned().unwrap())
            .collect();
        assert_eq!(
            ids,
            (1..=4).map(Value::Integer).collect::<Vec<_>>(),
            "{ddl}"
        );
    }
}
