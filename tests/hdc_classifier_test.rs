use nexusql::{NexusSQL, Value};

async fn scalar(db: &NexusSQL, sql: &str) -> Value {
    db.execute(sql).await.unwrap().rows[0].values[0].clone()
}

async fn train_from_example_rows(db: &NexusSQL) -> Value {
    db.execute(
        "CREATE TABLE examples (zone TEXT, switch_name TEXT, port TEXT, vlan INT, model BLOB)",
    )
    .await
    .unwrap();
    db.execute("INSERT INTO examples VALUES ('ZONE-A','SW-ALPHA','Gi1/0/1',101,NULL), ('ZONE-B','SW-BETA','Gi1/0/1',202,NULL)").await.unwrap();
    // Exactly one example row per label. All learning is expressed in SQL.
    db.execute("UPDATE examples SET model = HDC_LEARN(NULL, zone, switch_name, port, vlan)")
        .await
        .unwrap();
    scalar(db, "SELECT HDC_MERGE(a.model,b.model) FROM examples a JOIN examples b ON a.zone != b.zone WHERE a.zone = 'ZONE-A' AND b.zone = 'ZONE-B'").await
}

#[tokio::test]
async fn learns_one_switch_port_row_per_zone_and_predicts_unseen_ports() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    let model = train_from_example_rows(&db).await;
    assert!(matches!(model, Value::Blob(_)));
    db.execute("CREATE TABLE classifier (model BLOB)")
        .await
        .unwrap();
    db.execute_with_params("INSERT INTO classifier VALUES ($1)", Some(vec![model]))
        .await
        .unwrap();
    db.execute("CREATE TABLE ports (id INT, switch_name TEXT, port TEXT, vlan INT)")
        .await
        .unwrap();
    db.execute(
        "INSERT INTO ports VALUES (1,'SW-ALPHA','Gi1/0/17',101), (2,'SW-BETA','Gi1/0/23',202)",
    )
    .await
    .unwrap();
    let result = db.execute("SELECT p.id, HDC_PREDICT(c.model,p.switch_name,p.port,p.vlan) AS zone, HDC_CONFIDENCE(c.model,p.switch_name,p.port,p.vlan) AS confidence FROM ports p CROSS JOIN classifier c ORDER BY p.id").await.unwrap();
    for (row, expected) in result.rows.iter().zip(["ZONE-A", "ZONE-B"]) {
        assert_eq!(row.get("zone"), Some(&Value::from(expected)));
        let confidence = row.get("confidence").unwrap().as_f64().unwrap();
        assert!(confidence > 0.1 && confidence <= 1.0, "{row:?}");
    }
    let exact = scalar(
        &db,
        "SELECT HDC_CONFIDENCE(model,'SW-ALPHA','Gi1/0/1',101) FROM classifier",
    )
    .await
    .as_f64()
    .unwrap();
    assert!(exact > result.rows[0].get("confidence").unwrap().as_f64().unwrap());
}

#[tokio::test]
async fn learning_updates_are_explicit_persistent_and_transactional() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("classifier");
    let db = NexusSQL::new(path.to_str().unwrap()).await.unwrap();
    db.execute("CREATE TABLE classifier (model BLOB)")
        .await
        .unwrap();
    db.execute("INSERT INTO classifier VALUES (HDC_LEARN(NULL,'ZONE-A','SW-ALPHA','Gi1/0/1',101))")
        .await
        .unwrap();
    db.execute("UPDATE classifier SET model = HDC_LEARN(model,'ZONE-B','SW-BETA','Gi1/0/1',202)")
        .await
        .unwrap();
    let before = scalar(&db, "SELECT model FROM classifier").await;
    db.execute("BEGIN").await.unwrap();
    db.execute("UPDATE classifier SET model = HDC_LEARN(NULL,'REPLACED','SW-BETA','Gi1/0/1',202)")
        .await
        .unwrap();
    db.execute("ROLLBACK").await.unwrap();
    assert_eq!(scalar(&db, "SELECT model FROM classifier").await, before);
    // Explicitly save the restored model: the pre-existing disk engine does not
    // defer DML persistence until COMMIT, independently of classifier support.
    db.execute("UPDATE classifier SET model = model")
        .await
        .unwrap();
    db.shutdown().await.unwrap();
    let db = NexusSQL::new(path.to_str().unwrap()).await.unwrap();
    assert_eq!(scalar(&db, "SELECT model FROM classifier").await, before);
    assert_eq!(
        scalar(
            &db,
            "SELECT HDC_PREDICT(model,'SW-BETA','Gi1/0/22',202) FROM classifier"
        )
        .await,
        Value::from("ZONE-B")
    );
    db.execute("UPDATE classifier SET model = HDC_LEARN(model,'ZONE-B','SW-BETA','Gi1/0/1',202)")
        .await
        .unwrap();
    assert_eq!(
        scalar(&db, "SELECT model FROM classifier").await,
        before,
        "relearning the same label replaces its single prototype deterministically"
    );
}

#[tokio::test]
async fn feature_roles_and_tie_confidence() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    let model = "HDC_LEARN(HDC_LEARN(NULL,'A','west','east',101),'B','east','west',101)";
    assert_eq!(
        scalar(
            &db,
            &format!("SELECT HDC_PREDICT({model},'west','east',101)")
        )
        .await,
        Value::from("A")
    );
    assert_eq!(
        scalar(
            &db,
            &format!("SELECT HDC_PREDICT({model},'east','west',101)")
        )
        .await,
        Value::from("B")
    );
    let tied = "HDC_LEARN(HDC_LEARN(NULL,'Z','same'),'A','same')";
    assert_eq!(
        scalar(&db, &format!("SELECT HDC_PREDICT({tied},'same')")).await,
        Value::from("A")
    );
    assert_eq!(
        scalar(&db, &format!("SELECT HDC_CONFIDENCE({tied},'same')")).await,
        Value::from(0.0)
    );
    assert_eq!(
        scalar(
            &db,
            "SELECT HDC_CONFIDENCE(HDC_LEARN(NULL,'A','same'),'same')"
        )
        .await,
        Value::from(1.0)
    );
}

#[tokio::test]
async fn classifier_rejects_invalid_models_and_feature_shapes() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    for sql in [
        "SELECT HDC_PREDICT(NULL,'port')",
        "SELECT HDC_LEARN(NULL,'', 'port')",
        "SELECT HDC_LEARN(NULL,'A',NULL)",
        "SELECT HDC_LEARN(NULL,'A','')",
        "SELECT HDC_LEARN(HDC_LEARN(NULL,'A','switch','port'),'B','switch')",
        "SELECT HDC_PREDICT(HDC_LEARN(NULL,'A','switch','port'),'switch')",
        "SELECT HDC_MERGE(HDC_LEARN(NULL,'A','switch'),HDC_LEARN(NULL,'B','switch','port'))",
    ] {
        assert_eq!(scalar(&db, sql).await, Value::Null, "{sql}");
    }
    let bad = db
        .execute_with_params(
            "SELECT HDC_PREDICT($1,'port')",
            Some(vec![Value::Blob(b"NXHDC001broken".to_vec())]),
        )
        .await
        .unwrap();
    assert_eq!(bad.rows[0].values[0], Value::Null);
}

#[tokio::test]
async fn models_are_isolated_between_databases() {
    let a = NexusSQL::new(":memory:").await.unwrap();
    let b = NexusSQL::new(":memory:").await.unwrap();
    for (db, label) in [(&a, "ZONE-A"), (&b, "ZONE-B")] {
        db.execute("CREATE TABLE model (value BLOB)").await.unwrap();
        db.execute_with_params(
            "INSERT INTO model VALUES (HDC_LEARN(NULL,$1,'SW-01',1))",
            Some(vec![label.into()]),
        )
        .await
        .unwrap();
    }
    assert_eq!(
        scalar(&a, "SELECT HDC_PREDICT(value,'SW-01',1) FROM model").await,
        Value::from("ZONE-A")
    );
    assert_eq!(
        scalar(&b, "SELECT HDC_PREDICT(value,'SW-01',1) FROM model").await,
        Value::from("ZONE-B")
    );
}
