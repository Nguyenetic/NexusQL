use nexusql::engine::executor::Executor;
use nexusql::geometry::GeometryValue;
use nexusql::index::GiSTStats;
use nexusql::query::evaluator::QueryParams;
use nexusql::query::QueryParser;
use nexusql::storage::IndexType;
use nexusql::{ResultSet, Row, Value};
use rand::{rngs::StdRng, Rng, SeedableRng};

fn sql(ex: &mut Executor, text: &str) -> ResultSet {
    ex.execute(&QueryParser::parse(text).unwrap(), &QueryParams::new())
        .unwrap()
}

fn values(rs: ResultSet) -> Vec<Vec<Value>> {
    rs.rows.into_iter().map(|row| row.values).collect()
}

fn stats(ex: &Executor) -> GiSTStats {
    let IndexType::GiST(index) = &ex.get_tables()["shapes"].typed_indexes["shapes_geom"] else {
        panic!("GiST expected")
    };
    index.stats()
}

fn create_index(ex: &mut Executor) {
    sql(ex, "CREATE INDEX shapes_geom ON shapes USING gist (geom)");
}

fn point_fixture() -> Executor {
    let mut ex = Executor::new();
    sql(&mut ex, "CREATE TABLE shapes (id INT, geom GEOMETRY)");
    sql(&mut ex, "INSERT INTO shapes VALUES (1, ST_Point(20,0)), (2, ST_Point(2,0)), (3, ST_Point(3,0)), (4, ST_Point(1,0))");
    ex
}

#[test]
fn distance_operator_matches_st_distance_as_a_scalar() {
    let mut ex = Executor::new();
    for (left, right) in [
        ("ST_Point(0,0)", "ST_Point(3,4)"),
        ("'POINT(0 0)'", "'POINT(3 4)'"),
        ("ST_MakeEnvelope(0,0,10,10)", "ST_Point(3,4)"),
        (
            "ST_GeomFromText('POLYGON((0 0,10 0,10 10,0 10,0 0),(3 3,7 3,7 7,3 7,3 3))')",
            "ST_Point(5,5)",
        ),
        ("NULL", "ST_Point(3,4)"),
        ("ST_GeomFromText('POLYGON EMPTY')", "ST_Point(3,4)"),
        ("ST_Point(0,0)", "ST_GeomFromText('POINT(3 4)',4326)"),
        ("'invalid'", "ST_Point(3,4)"),
    ] {
        let result = sql(
            &mut ex,
            &format!("SELECT ({left} <-> {right}), ST_Distance({left},{right})"),
        );
        assert_eq!(result.rows[0].values[0], result.rows[0].values[1]);
    }
    assert_eq!(
        values(sql(&mut ex, "SELECT ST_Point(0,0)<->ST_Point(3,4)")),
        vec![vec![Value::from(5.0)]]
    );
}

#[test]
fn distance_operator_precedence_parameters_and_vector_compatibility() {
    let mut ex = Executor::new();
    assert_eq!(
        values(sql(&mut ex, "SELECT '[0,0]' <-> '[3,4]'")),
        vec![vec![Value::from(5.0)]]
    );
    sql(&mut ex, "CREATE TABLE points (id INT, geom GEOMETRY)");
    sql(
        &mut ex,
        "INSERT INTO points VALUES (-2, ST_Point(3,4)), (1, ST_Point(10,0))",
    );
    assert_eq!(
        values(sql(
            &mut ex,
            "SELECT id FROM points WHERE geom <-> ST_Point(0,0) <= 5 AND id<-1"
        )),
        vec![vec![Value::from(-2)]]
    );
    let stmt =
        QueryParser::parse("SELECT geom <-> ST_Point($1,$2) FROM points WHERE id = -2").unwrap();
    assert_eq!(
        values(
            ex.execute(
                &stmt,
                &QueryParams::with_positional(vec![0.into(), 0.into()])
            )
            .unwrap()
        ),
        vec![vec![Value::from(5.0)]]
    );
    assert_eq!(
        values(sql(
            &mut ex,
            "SELECT '<->', ST_Point(0,0) /* <-> */ <-> ST_Point(3,4)"
        )),
        vec![vec![Value::from("<->"), Value::from(5.0)]]
    );
}

#[test]
fn indexed_knn_rechecks_holes_and_pulls_past_rejected_rows() {
    let mut ex = Executor::new();
    sql(&mut ex, "CREATE TABLE shapes (id INT, geom GEOMETRY)");
    sql(&mut ex, "INSERT INTO shapes VALUES (1, ST_GeomFromText('POLYGON((-10 -10,10 -10,10 10,-10 10,-10 -10),(-9 -9,-9 9,9 9,9 -9,-9 -9))')), (2, ST_Point(2,0)), (3, ST_Point(3,0)), (4, ST_Point(200,0))");
    let queries = [
        "SELECT id FROM shapes ORDER BY geom <-> ST_Point(0,0) LIMIT 1",
        "SELECT id FROM shapes WHERE id != 2 ORDER BY geom <-> ST_Point(0,0) LIMIT 1",
        "SELECT id FROM shapes WHERE id = 4 ORDER BY geom <-> ST_Point(0,0) LIMIT 1",
        "SELECT id FROM shapes WHERE ST_DWithin(geom,ST_Point(0,0),5) OR id = 4 ORDER BY geom <-> ST_Point(0,0) LIMIT 3",
    ];
    let expected: Vec<_> = queries.iter().map(|q| values(sql(&mut ex, q))).collect();
    assert_eq!(expected[0], vec![vec![Value::from(2)]]);
    assert_eq!(expected[1], vec![vec![Value::from(3)]]);
    assert_eq!(expected[2], vec![vec![Value::from(4)]]);
    create_index(&mut ex);
    for (query, expected) in queries.iter().zip(expected) {
        let before = stats(&ex);
        assert_eq!(values(sql(&mut ex, query)), expected, "{query}");
        assert_eq!(stats(&ex).search_count, before.search_count + 1);
    }
}

#[test]
fn exact_ties_preserve_scan_order_at_the_limit() {
    let mut ex = Executor::new();
    sql(&mut ex, "CREATE TABLE shapes (id INT, geom GEOMETRY)");
    let inserts = (0..96)
        .map(|i| {
            let shape = match i % 4 {
                0 => "ST_Point(1,0)",
                1 => "ST_Point(-1,0)",
                2 => "ST_MakeEnvelope(-2,1,2,3)",
                _ => "ST_MakeEnvelope(-2,-3,2,-1)",
            };
            format!("({}, {shape})", 96 - i)
        })
        .collect::<Vec<_>>()
        .join(",");
    sql(&mut ex, &format!("INSERT INTO shapes VALUES {inserts}"));
    let queries: Vec<_> = [1, 5, 50]
        .into_iter()
        .flat_map(|k| {
            ["", "WHERE id < 80"].map(|filter| {
                format!("SELECT id FROM shapes {filter} ORDER BY geom <-> ST_Point(0,0) LIMIT {k}")
            })
        })
        .collect();
    let expected: Vec<_> = queries.iter().map(|q| values(sql(&mut ex, q))).collect();
    assert_eq!(expected[0][0], vec![Value::from(96)]);
    create_index(&mut ex);
    for (query, expected) in queries.iter().zip(expected) {
        let before = stats(&ex);
        assert_eq!(values(sql(&mut ex, query)), expected, "{query}");
        assert_eq!(stats(&ex).search_count, before.search_count + 1);
        assert_eq!(
            stats(&ex).candidates_returned - before.candidates_returned,
            96
        );
    }
}

#[test]
fn non_point_query_geometries_do_not_use_center_distance_as_exact_distance() {
    let mut ex = Executor::new();
    sql(&mut ex, "CREATE TABLE shapes (id INT, geom GEOMETRY)");
    sql(&mut ex, "INSERT INTO shapes VALUES (1, ST_Point(500,10)), (2, ST_Point(0,1)), (3, ST_Point(98,0)), (4, ST_Point(0,0))");
    let queries = [
        "SELECT id FROM shapes WHERE id <= 2 ORDER BY geom <-> ST_GeomFromText('LINESTRING(0 0,1000 0)') LIMIT 1",
        "SELECT id FROM shapes WHERE id >= 3 ORDER BY geom <-> ST_GeomFromText('POLYGON((-100 -100,100 -100,100 100,-100 100,-100 -100),(-99 -99,-99 99,99 99,99 -99,-99 -99))') LIMIT 1",
    ];
    let expected: Vec<_> = queries.iter().map(|q| values(sql(&mut ex, q))).collect();
    assert_eq!(expected[0], vec![vec![Value::from(2)]]);
    assert_eq!(expected[1], vec![vec![Value::from(3)]]);
    create_index(&mut ex);
    for (query, expected) in queries.iter().zip(expected) {
        assert_eq!(values(sql(&mut ex, query)), expected);
    }
    assert_eq!(stats(&ex).search_count, 2);
}

#[test]
fn knn_resolves_aliases_parameters_casts_and_reversed_operands() {
    let mut ex = point_fixture();
    let queries = [
        "SELECT s.id, s.geom <-> ST_Point($1,$2) AS distance FROM shapes s WHERE s.id >= 2 ORDER BY distance LIMIT 2",
        "SELECT id FROM shapes WHERE id >= 2 ORDER BY shapes.geom <-> ST_Point($1,$2) LIMIT 2",
        "SELECT id FROM shapes ORDER BY ST_Point($1,$2) <-> geom LIMIT 2",
        "SELECT id FROM shapes ORDER BY geom <-> CAST($3 AS GEOMETRY) LIMIT 2",
        "SELECT id FROM shapes ORDER BY geom <-> ST_Centroid(ST_MakeEnvelope($1,$2,2,2)) LIMIT 2",
    ];
    let params = QueryParams::with_positional(vec![0.into(), 0.into(), "POINT(0 0)".into()]);
    let statements: Vec<_> = queries
        .iter()
        .map(|q| QueryParser::parse(q).unwrap())
        .collect();
    let expected: Vec<_> = statements
        .iter()
        .map(|stmt| values(ex.execute(stmt, &params).unwrap()))
        .collect();
    create_index(&mut ex);
    for (stmt, expected) in statements.iter().zip(expected) {
        let before = stats(&ex).search_count;
        assert_eq!(values(ex.execute(stmt, &params).unwrap()), expected);
        assert_eq!(stats(&ex).search_count, before + 1);
    }
}

#[test]
fn unsupported_plans_retain_full_sort_results() {
    let mut ex = point_fixture();
    sql(&mut ex, "CREATE TABLE other (id INT)");
    sql(&mut ex, "INSERT INTO other VALUES (1),(2),(3),(4)");
    let queries = [
        "SELECT id FROM shapes ORDER BY geom <-> ST_Point(0,0)",
        "SELECT id FROM shapes ORDER BY geom <-> ST_Point(0,0) DESC LIMIT 2",
        "SELECT id FROM shapes ORDER BY geom <-> ST_Point(id,0) LIMIT 2",
        "SELECT id FROM shapes ORDER BY ST_Distance(geom,ST_Point(0,0)) LIMIT 2",
        "SELECT id FROM shapes WHERE id IN (SELECT id FROM other) ORDER BY geom <-> ST_Point(0,0) LIMIT 2",
        "SELECT s.id FROM shapes s JOIN other o ON s.id = o.id ORDER BY s.geom <-> ST_Point(0,0) LIMIT 2",
        "SELECT COUNT(*) FROM shapes ORDER BY geom <-> ST_Point(0,0) LIMIT 1",
        "SELECT geom, COUNT(*) FROM shapes GROUP BY geom ORDER BY geom <-> ST_Point(0,0) LIMIT 2",
        "SELECT id, ROW_NUMBER() OVER (ORDER BY id) FROM shapes ORDER BY geom <-> ST_Point(0,0) LIMIT 2",
    ];
    let expected: Vec<_> = queries.iter().map(|q| values(sql(&mut ex, q))).collect();
    create_index(&mut ex);
    for (query, expected) in queries.iter().zip(expected) {
        assert_eq!(values(sql(&mut ex, query)), expected, "{query}");
        assert_eq!(stats(&ex).search_count, 0, "{query} must fall back");
    }
}

#[test]
fn null_empty_and_srid_mismatches_preserve_full_sort_behavior() {
    for insert in [
        "(1, ST_Point(20,0)), (2, NULL), (3, ST_Point(2,0))",
        "(1, ST_Point(20,0)), (2, ST_GeomFromText('POLYGON EMPTY')), (3, ST_Point(2,0))",
        "(1, ST_Point(20,0)), (2, ST_GeomFromText('POINT(1 0)',4326)), (3, ST_Point(2,0))",
    ] {
        let mut ex = Executor::new();
        sql(&mut ex, "CREATE TABLE shapes (id INT, geom GEOMETRY)");
        sql(&mut ex, &format!("INSERT INTO shapes VALUES {insert}"));
        let query = "SELECT id FROM shapes ORDER BY geom <-> ST_Point(0,0) LIMIT 2";
        let expected = values(sql(&mut ex, query));
        create_index(&mut ex);
        assert_eq!(values(sql(&mut ex, query)), expected);
        assert_eq!(stats(&ex).search_count, 0);
    }
    let mut ex = point_fixture();
    let queries: Vec<_> = [
        "NULL",
        "'broken'",
        "ST_GeomFromText('POLYGON EMPTY')",
        "ST_GeomFromText('POINT(0 0)',4326)",
    ]
    .map(|target| format!("SELECT id FROM shapes ORDER BY geom <-> {target} LIMIT 2"))
    .into();
    let expected: Vec<_> = queries.iter().map(|q| values(sql(&mut ex, q))).collect();
    create_index(&mut ex);
    for (query, expected) in queries.iter().zip(expected) {
        assert_eq!(values(sql(&mut ex, query)), expected);
        assert_eq!(stats(&ex).search_count, 0);
    }
}

#[test]
fn zero_large_limits_and_exhausted_filters() {
    let mut ex = point_fixture();
    let queries = [
        "SELECT id FROM shapes ORDER BY geom <-> ST_Point(0,0) LIMIT 0",
        "SELECT id FROM shapes ORDER BY geom <-> ST_Point(0,0) LIMIT 50",
        "SELECT id FROM shapes WHERE id = 3 ORDER BY geom <-> ST_Point(0,0) LIMIT 5",
        "SELECT id FROM shapes WHERE id > 99 ORDER BY geom <-> ST_Point(0,0) LIMIT 5",
    ];
    let expected: Vec<_> = queries.iter().map(|q| values(sql(&mut ex, q))).collect();
    create_index(&mut ex);
    for (query, expected) in queries.iter().zip(expected) {
        let before = stats(&ex);
        assert_eq!(values(sql(&mut ex, query)), expected);
        assert_eq!(stats(&ex).search_count, before.search_count + 1);
        if query.ends_with("LIMIT 0") {
            assert_eq!(stats(&ex).candidates_returned, before.candidates_returned);
        }
    }
    sql(&mut ex, "DELETE FROM shapes");
    let before = stats(&ex).search_count;
    assert!(sql(&mut ex, queries[1]).rows.is_empty());
    assert_eq!(stats(&ex).search_count, before + 1);
}

#[test]
fn knn_metadata_tracks_insert_update_delete_and_rollback() {
    let mut ex = Executor::new();
    sql(&mut ex, "CREATE TABLE shapes (id INT, geom GEOMETRY)");
    create_index(&mut ex);
    let query =
        "SELECT id FROM shapes ORDER BY geom <-> ST_GeomFromText('POINT(0 0)',3857) LIMIT 2";
    for (change, uses_index) in [
        ("INSERT INTO shapes VALUES (1, ST_GeomFromText('POINT(9 0)',3857)), (2, ST_GeomFromText('POINT(2 0)',3857))", true),
        ("INSERT INTO shapes VALUES (3, NULL)", false),
        ("UPDATE shapes SET geom = ST_GeomFromText('POINT(1 0)',3857) WHERE id = 3", true),
        ("UPDATE shapes SET geom = ST_Point(1,0) WHERE id = 3", false),
        ("DELETE FROM shapes WHERE id = 3", true),
        ("BEGIN", true),
        ("UPDATE shapes SET geom = ST_Point(1,0) WHERE id = 1", false),
        ("ROLLBACK", true),
    ] {
        sql(&mut ex, change);
        let before = stats(&ex).search_count;
        let indexed = values(sql(&mut ex, query));
        assert_eq!(stats(&ex).search_count, before + u64::from(uses_index), "after {change}");
        let index = ex.get_tables_mut().get_mut("shapes").unwrap().typed_indexes.remove("shapes_geom").unwrap();
        assert_eq!(indexed, values(sql(&mut ex, query)), "after {change}");
        ex.get_tables_mut().get_mut("shapes").unwrap().typed_indexes.insert("shapes_geom".into(), index);
    }
}

#[test]
fn ten_thousand_random_geometries_match_scan_for_k_1_5_50() {
    let mut ex = Executor::new();
    sql(
        &mut ex,
        "CREATE TABLE shapes (id INT, bucket INT, geom GEOMETRY)",
    );
    let mut rng = StdRng::seed_from_u64(0x4b4e4e);
    let table = ex.get_tables_mut().get_mut("shapes").unwrap();
    for id in 0..10_000 {
        let x: f64 = rng.gen_range(-2000.0..2000.0);
        let y: f64 = rng.gen_range(-2000.0..2000.0);
        let size: f64 = rng.gen_range(1.0..60.0);
        let (x1, y1) = (x + size, y + size);
        let wkt = match id % 4 {
            0 => format!("POINT({x} {y})"),
            1 => format!("POLYGON(({x} {y},{x1} {y},{x1} {y1},{x} {y1},{x} {y}))"),
            2 => format!("POLYGON(({x} {y},{x1} {y},{x} {y1},{x} {y}))"),
            _ => {
                let (a, b, c, d) = (
                    x + size * 0.1,
                    y + size * 0.1,
                    x + size * 0.9,
                    y + size * 0.9,
                );
                format!("POLYGON(({x} {y},{x1} {y},{x1} {y1},{x} {y1},{x} {y}),({a} {b},{a} {d},{c} {d},{c} {b},{a} {b}))")
            }
        };
        table.insert(Row::new(
            vec!["id".into(), "bucket".into(), "geom".into()],
            vec![
                Value::from(id),
                Value::from(rng.gen_range(0..7)),
                Value::Geometry(GeometryValue::from_wkt(&wkt).unwrap()),
            ],
        ));
    }
    let mut comparisons = Vec::new();
    for target in [
        "ST_Point(13.5,-27.25)",
        "ST_MakeEnvelope(-20,-20,20,20)",
        "ST_GeomFromText('LINESTRING(-400 20,400 20)')",
        "ST_GeomFromText('POLYGON((-60 -60,60 -60,60 60,-60 60,-60 -60),(-50 -50,-50 50,50 50,50 -50,-50 -50))')",
    ] {
        for k in [1, 5, 50] {
            for filter in ["", "WHERE bucket = 3"] {
                let query = format!("SELECT id, geom <-> {target} AS distance FROM shapes {filter} ORDER BY distance LIMIT {k}");
                let expected = values(sql(&mut ex, &query));
                assert_eq!(expected.len(), k);
                let reference = format!("SELECT id, ST_Distance(geom,{target}) AS distance FROM shapes {filter} ORDER BY distance LIMIT {k}");
                assert_eq!(expected, values(sql(&mut ex, &reference)));
                comparisons.push((query, expected));
            }
        }
    }
    create_index(&mut ex);
    assert!(stats(&ex).tree_height > 1);
    for (query, expected) in &comparisons {
        let before = stats(&ex);
        assert_eq!(values(sql(&mut ex, query)), *expected, "{query}");
        let after = stats(&ex);
        assert_eq!(after.search_count, before.search_count + 1, "{query}");
        assert!(
            after.candidates_returned - before.candidates_returned < 2500,
            "must prune most of the 10,000 rows: {query}"
        );
    }
    assert_eq!(stats(&ex).search_count, 24);
}
