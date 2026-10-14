use geo_types::{Point, Rect};
use nexusql::engine::Executor;
use nexusql::geometry::GeometryValue;
use nexusql::index::{GiSTIndex, GiSTPredicate, GiSTStats};
use nexusql::query::ast::{OnConflict, Statement};
use nexusql::query::evaluator::QueryParams;
use nexusql::storage::{IndexType, Table};
use nexusql::{NexusSQL, QueryParser, ResultSet, Row, Value};
use rand::{rngs::StdRng, Rng, SeedableRng};

fn sql(ex: &mut Executor, text: &str) -> ResultSet {
    ex.execute(&QueryParser::parse(text).unwrap(), &QueryParams::new())
        .unwrap()
}

fn values(result: ResultSet) -> Vec<Vec<Value>> {
    result.rows.into_iter().map(|row| row.values).collect()
}

fn index(table: &Table) -> &GiSTIndex {
    let IndexType::GiST(index) = &table.typed_indexes["shapes_geom"] else {
        panic!("expected GiST")
    };
    index
}

fn stats(ex: &Executor) -> GiSTStats {
    index(&ex.get_tables()["shapes"]).stats()
}

fn fixture() -> Executor {
    let mut ex = Executor::new();
    sql(
        &mut ex,
        "CREATE TABLE shapes (id INT, geom GEOMETRY, note TEXT)",
    );
    sql(
        &mut ex,
        "CREATE INDEX shapes_geom ON shapes USING gist (geom)",
    );
    ex
}

fn point(x: f64, y: f64) -> Value {
    Value::Geometry(GeometryValue::new(Point::new(x, y).into(), None).unwrap())
}

fn random_geometry(rng: &mut StdRng) -> Value {
    // A small coordinate grid deliberately produces duplicate bounding boxes.
    let x = rng.gen_range(-20..=20) as f64;
    let y = rng.gen_range(-20..=20) as f64;
    match rng.gen_range(0..20) {
        0 => Value::Null,
        1 => Value::Geometry(GeometryValue::from_wkt("POLYGON EMPTY").unwrap()),
        2..=7 => Value::Geometry(
            GeometryValue::new(
                Rect::new([x, y], [x + 3.0, y + 5.0]).to_polygon().into(),
                None,
            )
            .unwrap(),
        ),
        8 => Value::Geometry(
            GeometryValue::from_wkt("POLYGON((0 0,10 0,10 10,0 10,0 0),(3 3,7 3,7 7,3 7,3 3))")
                .unwrap(),
        ),
        _ => point(x, y),
    }
}

fn compare_scan(ex: &mut Executor, statement: &Statement, params: &QueryParams) {
    let before = stats(ex).search_count;
    let indexed = values(ex.execute(statement, params).unwrap());
    assert!(stats(ex).search_count > before, "must actually probe GiST");
    let saved = ex
        .get_tables_mut()
        .get_mut("shapes")
        .unwrap()
        .typed_indexes
        .remove("shapes_geom")
        .unwrap();
    let scanned = values(ex.execute(statement, params).unwrap());
    ex.get_tables_mut()
        .get_mut("shapes")
        .unwrap()
        .typed_indexes
        .insert("shapes_geom".into(), saved);
    assert_eq!(indexed, scanned);
}

#[test]
fn ten_thousand_mixed_operations_match_scan_after_every_batch() {
    let mut ex = fixture();
    let mut rng = StdRng::seed_from_u64(0x0052_5452_4545);
    let insert = QueryParser::parse("INSERT INTO shapes VALUES ($1, $2, 'device')").unwrap();
    let update = QueryParser::parse("UPDATE shapes SET geom = $2 WHERE id = $1").unwrap();
    let delete = QueryParser::parse("DELETE FROM shapes WHERE id = $1").unwrap();
    let window = QueryParser::parse(
        "SELECT id FROM shapes WHERE ST_Intersects(geom, ST_MakeEnvelope($1,$2,$3,$4)) ORDER BY id",
    )
    .unwrap();
    let distance = QueryParser::parse(
        "SELECT id FROM shapes WHERE ST_DWithin(geom, ST_Point($1,$2), $3) ORDER BY id",
    )
    .unwrap();
    let mut snapshot: Option<Vec<Row>> = None;
    let mut next_id = 0i64;
    let mut counts = [0; 4];
    let initial_rebuilds = stats(&ex).full_rebuild_count;
    for step in 1..=10_000 {
        let choice = rng.gen_range(0..100);
        let rows = &ex.get_tables()["shapes"].rows;
        let id = if rows.is_empty() {
            Value::Integer(-1)
        } else {
            rows[rng.gen_range(0..rows.len())].values[0].clone()
        };
        match choice {
            0..=34 => {
                let params = QueryParams::with_positional(vec![
                    Value::Integer(next_id),
                    random_geometry(&mut rng),
                ]);
                next_id += 1;
                ex.execute(&insert, &params).unwrap();
                counts[0] += 1;
            }
            35..=64 => {
                ex.execute(
                    &update,
                    &QueryParams::with_positional(vec![id, random_geometry(&mut rng)]),
                )
                .unwrap();
                counts[1] += 1;
            }
            65..=89 => {
                ex.execute(&delete, &QueryParams::with_positional(vec![id]))
                    .unwrap();
                counts[2] += 1;
            }
            _ => {
                if let Some(expected) = snapshot.take() {
                    if rng.gen_bool(0.8) {
                        sql(&mut ex, "ROLLBACK");
                        assert_eq!(ex.get_tables()["shapes"].rows, expected);
                        counts[3] += 1;
                        compare_scan(
                            &mut ex,
                            &window,
                            &QueryParams::with_positional(vec![
                                (-100).into(),
                                (-100).into(),
                                100.into(),
                                100.into(),
                            ]),
                        );
                    } else {
                        sql(&mut ex, "COMMIT");
                    }
                } else {
                    snapshot = Some(ex.get_tables()["shapes"].rows.clone());
                    sql(&mut ex, "BEGIN");
                }
            }
        }
        assert_eq!(stats(&ex).full_rebuild_count, initial_rebuilds);
        if step % 25 == 0 {
            for _ in 0..4 {
                let x = rng.gen_range(-30..30);
                let y = rng.gen_range(-30..30);
                let w = rng.gen_range(1..20);
                let h = rng.gen_range(1..20);
                compare_scan(
                    &mut ex,
                    &window,
                    &QueryParams::with_positional(vec![
                        x.into(),
                        y.into(),
                        (x + w).into(),
                        (y + h).into(),
                    ]),
                );
                compare_scan(
                    &mut ex,
                    &distance,
                    &QueryParams::with_positional(vec![x.into(), y.into(), w.into()]),
                );
            }
        }
    }
    if let Some(expected) = snapshot {
        sql(&mut ex, "ROLLBACK");
        assert_eq!(ex.get_tables()["shapes"].rows, expected);
    }
    assert!(counts.iter().all(|count| *count > 100), "{counts:?}");
}

#[test]
fn update_touches_one_entry_and_validation_failure_changes_nothing() {
    let mut ex = fixture();
    sql(&mut ex, "INSERT INTO shapes VALUES (1, ST_Point(1,1), 'POINT(9 9)'), (2, ST_Point(2,2), 'broken'), (3, ST_Point(2,2), 'same bounds')");
    let before = stats(&ex);
    sql(
        &mut ex,
        "UPDATE shapes SET geom = ST_Point(5,5) WHERE id = 2",
    );
    let after = stats(&ex);
    assert_eq!(after.full_rebuild_count, before.full_rebuild_count);
    assert_eq!(after.bulk_load_count, before.bulk_load_count);
    assert_eq!(after.insert_count - before.insert_count, 1);
    assert_eq!(after.remove_count - before.remove_count, 1);
    assert_eq!(after.entries_indexed, 3);
    sql(&mut ex, "UPDATE shapes SET note = 'changed' WHERE id = 3");
    sql(&mut ex, "UPDATE shapes SET geom = geom");
    sql(
        &mut ex,
        "UPDATE shapes SET geom = ST_Point(0,0) WHERE id = -1",
    );
    assert_eq!(stats(&ex).insert_count, after.insert_count);
    assert_eq!(stats(&ex).remove_count, after.remove_count);

    let rows = ex.get_tables()["shapes"].rows.clone();
    let statement = QueryParser::parse("UPDATE shapes SET geom = note WHERE id < 3").unwrap();
    assert!(ex.execute(&statement, &QueryParams::new()).is_err());
    assert_eq!(ex.get_tables()["shapes"].rows, rows);
    assert_eq!(stats(&ex).insert_count, after.insert_count);
    assert_eq!(stats(&ex).remove_count, after.remove_count);
    compare_scan(
        &mut ex,
        &QueryParser::parse(
            "SELECT id FROM shapes WHERE ST_DWithin(geom,ST_Point(0,0),10) ORDER BY id",
        )
        .unwrap(),
        &QueryParams::new(),
    );
}

#[test]
fn delete_and_replace_preserve_duplicate_bounds_offsets_and_knn_ties() {
    let mut ex = fixture();
    sql(&mut ex, "INSERT INTO shapes VALUES (1, ST_Point(1,1), ''), (2, ST_Point(1,1), ''), (3, NULL, ''), (4, ST_Point(1,1), ''), (5, ST_Point(1,1), '')");
    let before = stats(&ex);
    sql(&mut ex, "DELETE FROM shapes WHERE id = 1 OR id = 3");
    let after = stats(&ex);
    assert_eq!(after.remove_count - before.remove_count, 1);
    assert_eq!(after.insert_count, before.insert_count);
    assert_eq!(after.full_rebuild_count, before.full_rebuild_count);
    let mut replace =
        QueryParser::parse("INSERT INTO shapes VALUES (4, ST_Point(9,9), '')").unwrap();
    if let Statement::Insert { on_conflict, .. } = &mut replace {
        *on_conflict = Some(OnConflict::Replace);
    }
    ex.execute(&replace, &QueryParams::new()).unwrap();
    assert_eq!(stats(&ex).full_rebuild_count, before.full_rebuild_count);
    assert_eq!(stats(&ex).remove_count - after.remove_count, 1);
    assert_eq!(stats(&ex).insert_count - after.insert_count, 1);
    assert_eq!(
        values(sql(&mut ex, "SELECT id FROM shapes")),
        vec![vec![2.into()], vec![5.into()], vec![4.into()]]
    );
    compare_scan(
        &mut ex,
        &QueryParser::parse("SELECT id FROM shapes ORDER BY geom <-> ST_Point(0,0) LIMIT 2")
            .unwrap(),
        &QueryParams::new(),
    );
    sql(&mut ex, "DELETE FROM shapes");
    assert_eq!(stats(&ex).entries_indexed, 0);
    assert_eq!(stats(&ex).full_rebuild_count, before.full_rebuild_count);
    sql(&mut ex, "INSERT INTO shapes VALUES (6, ST_Point(1,1), '')");
    assert_eq!(
        index(&ex.get_tables()["shapes"]).search(&GiSTPredicate::Point { x: 1.0, y: 1.0 }),
        vec![0]
    );
}

#[test]
fn bulk_create_append_and_snapshot_rollback_keep_cached_entries_consistent() {
    let mut ex = fixture();
    let table = ex.get_tables_mut().get_mut("shapes").unwrap();
    let batch: Vec<_> = (0..1_000)
        .map(|i| {
            Row::new(
                table.columns.clone(),
                vec![i.into(), point(i as f64, 1.0), Value::Null],
            )
        })
        .collect();
    table.insert_batch(batch);
    assert_eq!(index(table).stats().bulk_load_count, 2); // CREATE + batch
    assert_eq!(index(table).stats().insert_count, 0);
    assert_eq!(index(table).stats().entries_indexed, 1_000);
    sql(&mut ex, "BEGIN");
    sql(&mut ex, "INSERT INTO shapes VALUES (1000, ST_Point(1000,1), ''), (1001, NULL, ''), (1002, ST_GeomFromText('POLYGON EMPTY'), '')");
    assert_eq!(stats(&ex).bulk_load_count, 3);
    assert_eq!(stats(&ex).entries_indexed, 1_001);
    let query = QueryParser::parse(
        "SELECT id FROM shapes WHERE ST_Intersects(geom,ST_MakeEnvelope(990,0,1010,2)) ORDER BY id",
    )
    .unwrap();
    compare_scan(&mut ex, &query, &QueryParams::new());
    sql(
        &mut ex,
        "UPDATE shapes SET geom = ST_Point(-1,-1) WHERE id = 999",
    );
    sql(&mut ex, "DELETE FROM shapes WHERE id < 500");
    sql(&mut ex, "ROLLBACK");
    assert_eq!(stats(&ex).entries_indexed, 1_000);
    assert_eq!(stats(&ex).bulk_load_count, 2);
    compare_scan(&mut ex, &query, &QueryParams::new());
    sql(
        &mut ex,
        "CREATE INDEX shapes_geom ON shapes USING gist (geom)",
    );
    assert_eq!(stats(&ex).bulk_load_count, 1);
    assert_eq!(stats(&ex).insert_count, 0);
    compare_scan(&mut ex, &query, &QueryParams::new());
}

#[test]
fn removing_or_updating_the_last_mismatched_srid_restores_knn() {
    let mut ex = fixture();
    sql(&mut ex, "INSERT INTO shapes VALUES (1, ST_Point(1,1), ''), (2, ST_GeomFromText('POINT(2 2)',4326), ''), (3, ST_GeomFromText('POINT(3 3)',3857), '')");
    sql(
        &mut ex,
        "UPDATE shapes SET geom = ST_SetSRID(geom,0) WHERE id = 2",
    );
    sql(&mut ex, "DELETE FROM shapes WHERE id = 3");
    let query = QueryParser::parse("SELECT id FROM shapes ORDER BY geom <-> ST_Point(0,0) LIMIT 1")
        .unwrap();
    compare_scan(&mut ex, &query, &QueryParams::new());
    sql(&mut ex, "BEGIN");
    sql(
        &mut ex,
        "UPDATE shapes SET geom = ST_SetSRID(geom,4326) WHERE id = 1",
    );
    let searches = stats(&ex).search_count;
    ex.execute(&query, &QueryParams::new()).unwrap();
    assert_eq!(
        stats(&ex).search_count,
        searches,
        "mixed SRIDs must fall back"
    );
    sql(&mut ex, "ROLLBACK");
    compare_scan(&mut ex, &query, &QueryParams::new());
}

#[test]
fn range_gist_and_multiple_geometry_indexes_follow_row_changes() {
    let mut ex = fixture();
    sql(&mut ex, "CREATE INDEX shapes_id ON shapes USING gist (id)");
    sql(
        &mut ex,
        "CREATE INDEX shapes_geom_copy ON shapes USING gist (geom)",
    );
    sql(&mut ex, "INSERT INTO shapes VALUES (1, ST_Point(1,1), ''), (2, ST_Point(2,2), ''), (3, ST_Point(3,3), '')");
    sql(&mut ex, "DELETE FROM shapes WHERE id = 1");
    sql(
        &mut ex,
        "UPDATE shapes SET id = 8, geom = NULL WHERE id = 2",
    );
    let table = &ex.get_tables()["shapes"];
    let IndexType::GiST(range) = &table.typed_indexes["shapes_id"] else {
        panic!()
    };
    assert_eq!(
        range.search(&GiSTPredicate::IntRange { min: 1, max: 3 }),
        vec![1]
    );
    assert_eq!(
        range.search(&GiSTPredicate::IntRange { min: 8, max: 8 }),
        vec![0]
    );
    for name in ["shapes_geom", "shapes_geom_copy"] {
        let IndexType::GiST(geometry) = &table.typed_indexes[name] else {
            panic!()
        };
        assert_eq!(geometry.stats().entries_indexed, 1);
        assert_eq!(
            geometry.search(&GiSTPredicate::Point { x: 3.0, y: 3.0 }),
            vec![1]
        );
    }
}

#[tokio::test]
async fn direct_bulk_insert_and_reopen_use_packed_index() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("spatial");
    let db = NexusSQL::new(path.to_str().unwrap()).await.unwrap();
    db.execute("CREATE TABLE shapes (id INT, geom GEOMETRY, note TEXT)")
        .await
        .unwrap();
    db.execute("CREATE INDEX shapes_geom ON shapes USING gist (geom)")
        .await
        .unwrap();
    for batch in 0..2 {
        db.bulk_insert(
            "shapes",
            &["id".into(), "geom".into()],
            (batch * 500..(batch + 1) * 500)
                .flat_map(|i| [Value::Integer(i), point(i as f64, 1.0)])
                .collect(),
            500,
        )
        .await
        .unwrap();
    }
    let query =
        "SELECT id FROM shapes WHERE ST_Intersects(geom,ST_MakeEnvelope(498,0,502,2)) ORDER BY id";
    let expected: Vec<Vec<Value>> = (498..=502).map(|id| vec![id.into()]).collect();
    assert_eq!(values(db.execute(query).await.unwrap()), expected);
    db.shutdown().await.unwrap();
    let disk =
        nexusql::storage::disk::DiskStorage::new(path.join("default").to_str().unwrap()).unwrap();
    let table = disk.load_table("shapes").unwrap().unwrap();
    assert_eq!(index(&table).stats().bulk_load_count, 1);
    assert_eq!(index(&table).stats().insert_count, 0);
    assert_eq!(index(&table).stats().entries_indexed, 1_000);
    drop(disk);
    let db = NexusSQL::new(path.to_str().unwrap()).await.unwrap();
    assert_eq!(values(db.execute(query).await.unwrap()), expected);
    db.execute("UPDATE shapes SET geom = ST_Point(-1,-1) WHERE id = 500")
        .await
        .unwrap();
    db.execute("DELETE FROM shapes WHERE id = 0").await.unwrap();
    assert_eq!(
        values(db.execute(query).await.unwrap()),
        vec![
            vec![498.into()],
            vec![499.into()],
            vec![501.into()],
            vec![502.into()]
        ]
    );
    db.shutdown().await.unwrap();
}
