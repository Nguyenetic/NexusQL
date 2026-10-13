use nexusql::{NexusSQL, Value};

async fn scalar(db: &NexusSQL, sql: &str) -> Value {
    db.execute(sql).await.unwrap().rows[0].values[0].clone()
}

#[tokio::test]
async fn geometry_constructors_and_measures() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    for (sql, expected) in [
        (
            "SELECT ST_AsText(ST_Point(3, 4))",
            Value::from("POINT(3 4)"),
        ),
        ("SELECT ST_X(ST_Point(3, 4))", Value::from(3.0)),
        ("SELECT ST_Y(ST_Point(3, 4))", Value::from(4.0)),
        (
            "SELECT ST_Distance(ST_Point(0, 0), ST_Point(3, 4))",
            Value::from(5.0),
        ),
        (
            "SELECT ST_DWithin(ST_Point(0, 0), ST_Point(3, 4), 5)",
            Value::from(true),
        ),
        (
            "SELECT ST_DWithin(ST_Point(0, 0), ST_Point(3, 4), 4.99)",
            Value::from(false),
        ),
        (
            "SELECT ST_Area(ST_MakeEnvelope(0, 0, 10, 20))",
            Value::from(200.0),
        ),
        (
            "SELECT ST_AsText(ST_Centroid(ST_MakeEnvelope(0, 0, 10, 20)))",
            Value::from("POINT(5 10)"),
        ),
        (
            "SELECT ST_AsText(ST_Envelope(ST_Point(3, 4)))",
            Value::from("POINT(3 4)"),
        ),
        (
            "SELECT ST_SRID(ST_GeomFromText('POINT(3 4)', 3857))",
            Value::from(3857),
        ),
        (
            "SELECT ST_X(CAST('POINT(3 4)' AS GEOMETRY))",
            Value::from(3.0),
        ),
    ] {
        assert_eq!(scalar(&db, sql).await, expected, "{sql}");
    }
}

#[tokio::test]
async fn exact_topology_including_holes_and_boundaries() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    let polygon = "ST_GeomFromText('POLYGON((0 0,10 0,10 10,0 10,0 0),(3 3,7 3,7 7,3 7,3 3))')";
    for (point, contains, intersects) in [
        ("1 1", true, true),
        ("0 5", false, true),
        ("5 5", false, false),
        ("3 5", false, true),
        ("11 5", false, false),
    ] {
        for (function, args, expected) in [
            (
                "ST_Contains",
                format!("{polygon}, ST_GeomFromText('POINT({point})')"),
                contains,
            ),
            (
                "ST_Within",
                format!("ST_GeomFromText('POINT({point})'), {polygon}"),
                contains,
            ),
            (
                "ST_Intersects",
                format!("{polygon}, ST_GeomFromText('POINT({point})')"),
                intersects,
            ),
        ] {
            assert_eq!(
                scalar(&db, &format!("SELECT {function}({args})")).await,
                Value::from(expected)
            );
        }
    }
    assert_eq!(
        scalar(&db, &format!("SELECT ST_Area({polygon})")).await,
        Value::from(84.0)
    );
    assert_eq!(
        scalar(
            &db,
            &format!("SELECT ST_Distance({polygon}, ST_Point(5, 5))")
        )
        .await,
        Value::from(2.0)
    );
    assert_eq!(
        scalar(
            &db,
            "SELECT ST_Contains(ST_MakeEnvelope(0,0,10,10), ST_MakeEnvelope(0,0,10,10))"
        )
        .await,
        Value::from(true)
    );
}

#[tokio::test]
async fn geometry_nulls_and_invalid_inputs() {
    let db = NexusSQL::new(":memory:").await.unwrap();
    for sql in [
        "SELECT ST_Contains(NULL, ST_Point(0,0))",
        "SELECT ST_GeomFromText('broken')",
        "SELECT ST_Point('oops', 2)",
        "SELECT ST_X(ST_MakeEnvelope(0,0,2,2))",
        "SELECT ST_GeomFromText('POINT Z (1 2 3)')",
        "SELECT ST_Distance(ST_Point(0,0), ST_GeomFromText('POINT(1 1)', 4326))",
    ] {
        assert_eq!(scalar(&db, sql).await, Value::Null, "{sql}");
    }
    db.execute("CREATE TABLE points (id INT, geom GEOMETRY)")
        .await
        .unwrap();
    assert!(db
        .execute("INSERT INTO points VALUES (1, 'broken')")
        .await
        .is_err());
    db.execute("INSERT INTO points VALUES (1, 'POINT(1 2)')")
        .await
        .unwrap();
    assert!(matches!(
        scalar(&db, "SELECT geom FROM points").await,
        Value::Geometry(_)
    ));
}

#[tokio::test]
async fn geometry_parameters_bulk_insert_and_disk_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("geometry");
    let db = NexusSQL::new(path.to_str().unwrap()).await.unwrap();
    db.execute("CREATE TABLE points (id INT, geom GEOMETRY)")
        .await
        .unwrap();
    db.execute_with_params(
        "INSERT INTO points VALUES ($1, ST_GeomFromText($2, $3))",
        Some(vec![1.into(), "POINT(1.25 2.5)".into(), 3857.into()]),
    )
    .await
    .unwrap();
    db.bulk_insert(
        "points",
        &["id".into(), "geom".into()],
        vec![2.into(), "SRID=3857;POINT(3 4)".into()],
        1,
    )
    .await
    .unwrap();
    let expected = db
        .execute("SELECT geom FROM points ORDER BY id")
        .await
        .unwrap()
        .rows;
    db.shutdown().await.unwrap();
    let db = NexusSQL::new(path.to_str().unwrap()).await.unwrap();
    assert_eq!(
        db.execute("SELECT geom FROM points ORDER BY id")
            .await
            .unwrap()
            .rows,
        expected
    );
    assert!(
        db.execute("INSERT INTO points VALUES (3, 'broken')")
            .await
            .is_err(),
        "geometry schema must survive reopen"
    );
    assert_eq!(
        scalar(&db, "SELECT ST_SRID(geom) FROM points WHERE id = 2").await,
        Value::from(3857)
    );
    db.execute("UPDATE points SET geom = 'POINT(9 8)' WHERE id = 1")
        .await
        .unwrap();
    assert_eq!(
        scalar(&db, "SELECT ST_X(geom) FROM points WHERE id = 1").await,
        Value::from(9.0)
    );
}
