use nexusql::{NexusSQL, Value};

#[tokio::test]
async fn short_device_names_rank_by_maxsim() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    db.execute("CREATE TABLE names (name TEXT)").await.unwrap();
    db.execute("INSERT INTO names VALUES ('EECM-210'), ('PRINTER'), ('EECM021'), ('EECM-021')")
        .await
        .unwrap();
    let query = "SELECT name, COLBERT_SCORE($1, name, 'lightweight') AS score FROM names ORDER BY score DESC";
    let result = db
        .execute_with_params(query, Some(vec!["EECM-021".into()]))
        .await
        .unwrap();
    let names: Vec<_> = result
        .rows
        .iter()
        .map(|r| r.values[0].as_str().unwrap())
        .collect();
    assert_eq!(names, ["EECM-021", "EECM021", "EECM-210", "PRINTER"]);
    let scores: Vec<_> = result
        .rows
        .iter()
        .map(|r| r.values[1].as_f64().unwrap())
        .collect();
    assert_eq!(scores[0], 1.0);
    assert!(scores.windows(2).all(|pair| pair[0] > pair[1]));
    assert_eq!(
        result.rows,
        db.execute_with_params(query, Some(vec!["EECM-021".into()]))
            .await
            .unwrap()
            .rows
    );
}

#[tokio::test]
async fn japanese_labels_and_identifier_typos() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    for (query, close, unrelated) in [
        ("第一工場カメラ", "第一工場カメラ01", "会議室プリンター"),
        ("EEDT0158", "EEDT0159", "EECM0210"),
    ] {
        let result = db
            .execute_with_params(
                "SELECT COLBERT_SCORE($1,$2) AS near, COLBERT_SCORE($1,$3) AS far",
                Some(vec![query.into(), close.into(), unrelated.into()]),
            )
            .await
            .unwrap();
        let row = &result.rows[0];
        assert!(
            row.values[0].as_f64().unwrap() > row.values[1].as_f64().unwrap(),
            "{query}"
        );
    }
}

#[tokio::test]
async fn lightweight_input_contract() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    for (sql, expected) in [
        ("SELECT COLBERT_SCORE('', 'camera')", Value::from(0.0)),
        ("SELECT COLBERT_SCORE('camera', '')", Value::from(0.0)),
        ("SELECT COLBERT_SCORE(NULL, 'camera')", Value::Null),
        ("SELECT COLBERT_SCORE(123, 'camera')", Value::Null),
        (
            "SELECT COLBERT_SCORE('camera', 'camera', 'unknown')",
            Value::Null,
        ),
        ("SELECT COLBERT_SCORE('Camera', 'CAMERA')", Value::from(1.0)),
    ] {
        assert_eq!(db.execute(sql).await.unwrap().rows[0].values[0], expected);
    }
    let result = db
        .execute_with_params(
            "SELECT COLBERT_SCORE($1, 'camera')",
            Some(vec!["x".repeat(257).into()]),
        )
        .await
        .unwrap();
    assert_eq!(result.rows[0].values[0], Value::Null);
}

#[tokio::test]
async fn a_few_hundred_names_are_searchable_without_a_model() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    db.execute("CREATE TABLE devices (name TEXT)")
        .await
        .unwrap();
    let rows = (0..300)
        .map(|i| Value::from(format!("EECM-{i:03}")))
        .collect();
    db.bulk_insert("devices", &["name".into()], rows, 300)
        .await
        .unwrap();
    let result = db.execute("SELECT name, COLBERT_SCORE('EECM-021', name) AS score FROM devices ORDER BY score DESC LIMIT 1").await.unwrap();
    assert_eq!(
        result.rows[0].values,
        vec!["EECM-021".into(), Value::from(1.0)]
    );
}
