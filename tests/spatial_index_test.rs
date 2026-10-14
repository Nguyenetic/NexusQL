use nexusql::engine::Executor;
use nexusql::index::{GiSTIndexType, GiSTPredicate};
use nexusql::query::evaluator::QueryParams;
use nexusql::storage::IndexType;
use nexusql::{NexusSQL, QueryParser, ResultSet, Value};

fn sql(ex: &mut Executor, text: &str) -> ResultSet {
    ex.execute(&QueryParser::parse(text).unwrap(), &QueryParams::new())
        .unwrap()
}

fn hits(ex: &Executor, table: &str, index: &str) -> u64 {
    let IndexType::GiST(index) = &ex.get_tables()[table].typed_indexes[index] else {
        panic!("GiST expected")
    };
    index.stats().search_count
}

// Hand-computed fixture: two adjacent rooms and a separate room above them.
// The shared edge belongs to neither room's interior; OUT is outside all three.
fn fixture() -> Executor {
    let mut ex = Executor::new();
    sql(&mut ex, "CREATE TABLE zones (code TEXT, geom GEOMETRY)");
    sql(&mut ex, "CREATE TABLE devices (name TEXT, geom GEOMETRY)");
    sql(&mut ex, "INSERT INTO zones VALUES ('A', ST_MakeEnvelope(0,0,10,10)), ('B', ST_MakeEnvelope(10,0,20,10)), ('C', ST_MakeEnvelope(0,20,10,30))");
    sql(&mut ex, "INSERT INTO devices VALUES ('CAM', ST_Point(2,2)), ('PC', ST_Point(8,8)), ('PRINTER', ST_Point(12,2)), ('SWITCH', ST_Point(5,25)), ('EDGE', ST_Point(10,5)), ('OUT', ST_Point(30,30))");
    ex
}

fn values(rs: ResultSet) -> Vec<Vec<Value>> {
    rs.rows.into_iter().map(|r| r.values).collect()
}

#[test]
fn three_zones_six_devices_and_shared_edge() {
    let mut ex = fixture();
    let query = "SELECT d.name, z.code FROM devices d JOIN zones z ON ST_Contains(z.geom, d.geom) ORDER BY d.name";
    let expected = vec![
        vec!["CAM".into(), "A".into()],
        vec!["PC".into(), "A".into()],
        vec!["PRINTER".into(), "B".into()],
        vec!["SWITCH".into(), "C".into()],
    ];
    assert_eq!(values(sql(&mut ex, query)), expected);
    sql(
        &mut ex,
        "CREATE INDEX zones_geom ON zones USING gist (geom)",
    );
    assert_eq!(values(sql(&mut ex, query)), expected);
    assert_eq!(
        hits(&ex, "zones", "zones_geom"),
        6,
        "each outer device must probe the R-tree"
    );
    let edge = sql(&mut ex, "SELECT z.code FROM devices d JOIN zones z ON ST_Intersects(z.geom,d.geom) WHERE d.name = 'EDGE' ORDER BY z.code");
    assert_eq!(
        values(edge),
        vec![vec![Value::from("A")], vec![Value::from("B")]]
    );
    sql(
        &mut ex,
        "CREATE INDEX devices_geom ON devices USING gist (geom)",
    );
    let nearby = sql(
        &mut ex,
        "SELECT name FROM devices WHERE ST_DWithin(geom,ST_Point(2,2),10) ORDER BY name",
    );
    assert_eq!(
        values(nearby),
        vec![
            vec![Value::from("CAM")],
            vec![Value::from("EDGE")],
            vec![Value::from("PC")],
            vec![Value::from("PRINTER")]
        ]
    );
    assert!(hits(&ex, "devices", "devices_geom") > 0);
    let nearest = sql(&mut ex, "SELECT name, ST_Distance(geom, ST_Point(2,2)) AS distance FROM devices WHERE name != 'CAM' ORDER BY distance LIMIT 1");
    assert_eq!(nearest.rows[0].values[0], Value::from("PC"));
}

#[test]
fn all_spatial_filters_probe_index_and_recheck_exact_geometry() {
    let mut ex = Executor::new();
    sql(&mut ex, "CREATE TABLE shapes (id INT, geom GEOMETRY)");
    sql(&mut ex, "INSERT INTO shapes VALUES (1, ST_GeomFromText('POLYGON((0 0,10 0,10 10,0 10,0 0),(3 3,7 3,7 7,3 7,3 3))')), (2, ST_MakeEnvelope(30,30,40,40))");
    let filters = [
        "ST_Contains(geom, ST_Point(1,1))",
        "ST_Contains(geom, ST_Point(5,5))",
        "ST_Within(ST_Point(1,1), geom)",
        "ST_Intersects(ST_Point(3,5), geom)",
        "ST_DWithin(geom, ST_Point(5,5), 1)",
        "ST_DWithin(ST_Point(5,5), geom, 2)",
        "ST_Contains(ST_MakeEnvelope(0,0,20,20), geom)",
        "ST_Within(geom, ST_MakeEnvelope(0,0,20,20))",
        "ST_Contains(geom, ST_Point(1,1)) AND id = 1",
        "ST_Contains(geom, ST_Point(1,1)) OR id = 2",
        "ST_Contains(geom, ST_Point(1,1)) OR ST_Intersects(geom, ST_Point(35,35))",
        "ST_DWithin(geom, ST_Point(1,1), id)",
    ];
    let expected: Vec<_> = filters
        .iter()
        .map(|f| {
            values(sql(
                &mut ex,
                &format!("SELECT id FROM shapes WHERE {f} ORDER BY id"),
            ))
        })
        .collect();
    sql(
        &mut ex,
        "CREATE INDEX shapes_geom ON shapes USING gist (geom)",
    );
    for (i, filter) in filters.iter().enumerate() {
        let before = hits(&ex, "shapes", "shapes_geom");
        let actual = values(sql(
            &mut ex,
            &format!("SELECT id FROM shapes WHERE {filter} ORDER BY id"),
        ));
        assert_eq!(actual, expected[i], "{filter}");
        if i < 9 {
            assert!(
                hits(&ex, "shapes", "shapes_geom") > before,
                "{filter} must use the index"
            );
        }
    }
    let stmt =
        QueryParser::parse("SELECT id FROM shapes s WHERE ST_DWithin(s.geom, ST_Point($1,$2), $3)")
            .unwrap();
    let params = QueryParams::with_positional(vec![5.into(), 5.into(), 2.into()]);
    assert_eq!(
        values(ex.execute(&stmt, &params).unwrap()),
        vec![vec![Value::from(1)]]
    );
}

#[test]
fn index_tracks_insert_update_delete_replace_and_rollback() {
    let mut ex = Executor::new();
    sql(&mut ex, "CREATE TABLE points (id INT, geom GEOMETRY)");
    sql(
        &mut ex,
        "CREATE INDEX points_geom ON points USING gist (geom)",
    );
    for text in [
        "INSERT INTO points VALUES (1, ST_Point(1,1)), (2, ST_Point(2,2)), (3, ST_Point(50,50))",
        "UPDATE points SET geom = ST_Point(3,3) WHERE id = 3",
        "UPDATE points SET geom = ST_Point(50,50) WHERE id = 1",
        "DELETE FROM points WHERE id = 2",
        "INSERT INTO points VALUES (4, NULL)",
        "UPDATE points SET geom = 'POINT(4 4)' WHERE id = 4",
    ] {
        sql(&mut ex, text);
        let indexed = values(sql(
            &mut ex,
            "SELECT id FROM points WHERE ST_DWithin(geom, ST_Point(0,0), 10) ORDER BY id",
        ));
        let plain = ex
            .get_tables_mut()
            .get_mut("points")
            .unwrap()
            .typed_indexes
            .remove("points_geom")
            .unwrap();
        let scan = values(sql(
            &mut ex,
            "SELECT id FROM points WHERE ST_DWithin(geom, ST_Point(0,0), 10) ORDER BY id",
        ));
        ex.get_tables_mut()
            .get_mut("points")
            .unwrap()
            .typed_indexes
            .insert("points_geom".into(), plain);
        assert_eq!(indexed, scan, "after {text}");
    }
    // The executor supports Replace, though the pre-existing PostgreSQL parser
    // does not accept SQLite's INSERT OR REPLACE spelling.
    let mut replace =
        QueryParser::parse("INSERT INTO points VALUES (3, ST_Point(100,100))").unwrap();
    if let nexusql::query::ast::Statement::Insert { on_conflict, .. } = &mut replace {
        *on_conflict = Some(nexusql::query::ast::OnConflict::Replace);
    }
    ex.execute(&replace, &QueryParams::new()).unwrap();
    let before = values(sql(
        &mut ex,
        "SELECT id FROM points WHERE ST_DWithin(geom, ST_Point(0,0), 10)",
    ));
    sql(&mut ex, "BEGIN");
    sql(&mut ex, "DELETE FROM points");
    sql(&mut ex, "ROLLBACK");
    assert_eq!(
        values(sql(
            &mut ex,
            "SELECT id FROM points WHERE ST_DWithin(geom, ST_Point(0,0), 10)"
        )),
        before
    );
    assert!(ex
        .execute(
            &QueryParser::parse("UPDATE points SET geom = 'broken'").unwrap(),
            &QueryParams::new()
        )
        .is_err());
    assert_eq!(
        values(sql(
            &mut ex,
            "SELECT id FROM points WHERE ST_DWithin(geom, ST_Point(0,0), 10)"
        )),
        before
    );
}

#[test]
fn range_gist_is_preserved() {
    let mut ex = Executor::new();
    sql(&mut ex, "CREATE TABLE ranges (n INT)");
    sql(&mut ex, "INSERT INTO ranges VALUES (1), (5), (9)");
    sql(&mut ex, "CREATE INDEX ranges_n ON ranges USING gist (n)");
    let IndexType::GiST(index) = &ex.get_tables()["ranges"].typed_indexes["ranges_n"] else {
        panic!()
    };
    assert_eq!(index.index_type(), GiSTIndexType::RangeTree);
    assert_eq!(
        index.search(&GiSTPredicate::IntRange { min: 4, max: 6 }),
        vec![1]
    );
}

#[test]
fn spatial_left_join_and_qualified_unaliased_join() {
    let mut ex = fixture();
    sql(
        &mut ex,
        "CREATE INDEX zones_geom ON zones USING gist (geom)",
    );
    let rows = values(sql(&mut ex, "SELECT devices.name, zones.code FROM devices LEFT JOIN zones ON ST_Within(devices.geom, zones.geom) ORDER BY devices.name"));
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[1], vec![Value::from("EDGE"), Value::Null]);
    assert_eq!(rows[2], vec![Value::from("OUT"), Value::Null]);
    assert_eq!(hits(&ex, "zones", "zones_geom"), 6);
}

#[test]
fn ten_thousand_points_five_hundred_polygons_indexed_join_matches_scan() {
    let mut ex = Executor::new();
    sql(&mut ex, "CREATE TABLE zones (id INT, geom GEOMETRY)");
    sql(&mut ex, "CREATE TABLE devices (id INT, geom GEOMETRY)");
    let zones = (0..500)
        .map(|i| {
            let (x, y) = ((i % 25) * 20, (i / 25) * 20);
            format!("({i}, ST_MakeEnvelope({x},{y},{},{}))", x + 20, y + 20)
        })
        .collect::<Vec<_>>()
        .join(",");
    sql(&mut ex, &format!("INSERT INTO zones VALUES {zones}"));
    for batch in 0..10 {
        let points = (batch * 1000..(batch + 1) * 1000)
            .map(|i| format!("({i}, ST_Point({},{}))", (i % 100) * 5, (i / 100) * 4))
            .collect::<Vec<_>>()
            .join(",");
        sql(&mut ex, &format!("INSERT INTO devices VALUES {points}"));
    }
    let query = "SELECT d.id AS device, z.id AS zone FROM devices d JOIN zones z ON ST_Contains(z.geom,d.geom)";
    let mut scan = values(sql(&mut ex, query));
    // 75 non-edge X positions times 80 non-edge Y positions.
    assert_eq!(scan.len(), 6000);
    sql(
        &mut ex,
        "CREATE INDEX zones_geom ON zones USING gist (geom)",
    );
    let mut indexed = values(sql(&mut ex, query));
    scan.sort();
    indexed.sort();
    assert_eq!(indexed, scan);
    let IndexType::GiST(index) = &ex.get_tables()["zones"].typed_indexes["zones_geom"] else {
        panic!()
    };
    let stats = index.stats();
    assert_eq!(stats.search_count, 10_000);
    assert!(stats.tree_height > 1, "must be a real balanced R-tree");
    assert!(
        stats.candidates_returned < 25_000,
        "must prune the 5,000,000 possible pairs"
    );
}

#[tokio::test]
async fn index_survives_bulk_insert_and_disk_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("spatial");
    let db = NexusSQL::new(path.to_str().unwrap()).await.unwrap();
    db.execute("CREATE TABLE points (id INT, geom GEOMETRY)")
        .await
        .unwrap();
    db.execute("CREATE INDEX points_geom ON points USING gist (geom)")
        .await
        .unwrap();
    db.bulk_insert(
        "points",
        &["id".into(), "geom".into()],
        vec![
            1.into(),
            "POINT(1 1)".into(),
            2.into(),
            "POINT(50 50)".into(),
        ],
        2,
    )
    .await
    .unwrap();
    let query = "SELECT id FROM points WHERE ST_Contains(ST_MakeEnvelope(0,0,10,10), geom)";
    assert_eq!(
        values(db.execute(query).await.unwrap()),
        vec![vec![Value::from(1)]]
    );
    db.shutdown().await.unwrap();
    // Inspect the persisted definition and actual lookup, not just query results.
    let disk =
        nexusql::storage::disk::DiskStorage::new(path.join("default").to_str().unwrap()).unwrap();
    let table = disk.load_table("points").unwrap().unwrap();
    let IndexType::GiST(index) = &table.typed_indexes["points_geom"] else {
        panic!()
    };
    assert_eq!(index.index_type(), GiSTIndexType::RTree);
    assert_eq!(
        index.search(&GiSTPredicate::Point { x: 1.0, y: 1.0 }),
        vec![0]
    );
    drop(disk);
    let db = NexusSQL::new(path.to_str().unwrap()).await.unwrap();
    assert_eq!(
        values(db.execute(query).await.unwrap()),
        vec![vec![Value::from(1)]]
    );
}
