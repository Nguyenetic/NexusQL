use geo::{BoundingRect, CoordsIter};
use geo_types::{Coord, Geometry, LineString, MultiPolygon, Point, Polygon};
use nexusql::engine::Executor;
use nexusql::geometry::prepared::{Predicate, PreparedGeometryCache};
use nexusql::geometry::GeometryValue;
use nexusql::query::evaluator::QueryParams;
use nexusql::{QueryParser, Row, Value};
use rand::{rngs::StdRng, Rng, SeedableRng};

fn polygon(offset: f64, reverse: bool) -> Polygon<f64> {
    let mut exterior = (0..128)
        .map(|i| {
            let angle = std::f64::consts::TAU * i as f64 / 128.0;
            let radius = 300.0 + 40.0 * (angle * 7.0).cos();
            Coord {
                x: offset + (radius * angle.cos()).round(),
                y: (radius * angle.sin()).round(),
            }
        })
        .collect::<Vec<_>>();
    let mut holes = vec![
        vec![
            (-120.0, -120.0),
            (-60.0, -120.0),
            (-60.0, -60.0),
            (-120.0, -60.0),
        ],
        vec![(60.0, 60.0), (140.0, 60.0), (140.0, 140.0), (60.0, 140.0)],
    ];
    if reverse {
        exterior.reverse();
        holes[0].reverse();
    }
    Polygon::new(
        LineString::new(exterior),
        holes
            .into_iter()
            .map(|ring| {
                LineString::from(
                    ring.into_iter()
                        .map(|(x, y)| (x + offset, y))
                        .collect::<Vec<_>>(),
                )
            })
            .collect(),
    )
}

fn point(x: f64, y: f64) -> GeometryValue {
    GeometryValue::new(Point::new(x, y).into(), None).unwrap()
}

fn equivalent(cache: &mut PreparedGeometryCache, g: &GeometryValue, p: &GeometryValue) {
    for predicate in [
        Predicate::Contains,
        Predicate::Covers,
        Predicate::Intersects,
        Predicate::Within,
        Predicate::CoveredBy,
    ] {
        assert_eq!(
            cache.evaluate(predicate, g, p),
            predicate.unprepared(g, p),
            "{predicate:?}: {} / {}",
            g.as_text(),
            p.as_text()
        );
        assert_eq!(
            cache.evaluate(predicate, p, g),
            predicate.unprepared(p, g),
            "reversed {predicate:?}: {}",
            p.as_text()
        );
    }
}

#[test]
fn ten_thousand_random_points_and_exact_edges_match_original_predicates() {
    let polygons = [polygon(0.0, false), polygon(1000.0, true)];
    let shapes = [
        GeometryValue::new(polygons[0].clone().into(), None).unwrap(),
        GeometryValue::new(MultiPolygon(polygons.to_vec()).into(), None).unwrap(),
    ];
    let mut cache = PreparedGeometryCache::default();
    let mut rng = StdRng::seed_from_u64(0x5052455041524544);
    for _ in 0..10_000 {
        let p = point(rng.gen_range(-400.0..1400.0), rng.gen_range(-400.0..400.0));
        for g in &shapes {
            equivalent(&mut cache, g, &p);
        }
    }
    // Integer vertices and dyadic midpoints are exactly on the original edges.
    for polygon in &polygons {
        for ring in std::iter::once(polygon.exterior()).chain(polygon.interiors()) {
            for edge in ring.lines() {
                for p in [
                    point(edge.start.x, edge.start.y),
                    point(
                        (edge.start.x + edge.end.x) / 2.0,
                        (edge.start.y + edge.end.y) / 2.0,
                    ),
                ] {
                    for g in &shapes {
                        equivalent(&mut cache, g, &p);
                    }
                }
            }
        }
    }
    assert_eq!(
        cache.len(),
        2,
        "one preparation per reused polygon allocation"
    );
}

#[test]
fn boundaries_holes_empty_and_nonpoint_fallbacks() {
    let g = GeometryValue::from_wkt("POLYGON((0 0,10 0,10 10,0 10,0 0),(3 3,7 3,7 7,3 7,3 3))")
        .unwrap();
    let mut cache = PreparedGeometryCache::default();
    for (x, y, contains, covers) in [
        (1.0, 1.0, true, true),
        (0.0, 5.0, false, true),
        (0.0, 0.0, false, true),
        (3.0, 5.0, false, true),
        (3.0, 3.0, false, true),
        (5.0, 5.0, false, false),
        (11.0, 5.0, false, false),
    ] {
        let p = point(x, y);
        assert_eq!(cache.evaluate(Predicate::Contains, &g, &p), Some(contains));
        assert_eq!(cache.evaluate(Predicate::Covers, &g, &p), Some(covers));
        assert_eq!(cache.evaluate(Predicate::Intersects, &g, &p), Some(covers));
    }
    for text in [
        "POLYGON EMPTY",
        "MULTIPOLYGON EMPTY",
        "LINESTRING EMPTY",
        "GEOMETRYCOLLECTION EMPTY",
        "LINESTRING(0 0,10 0)",
        "LINESTRING(1 1,9 9)",
        "MULTIPOINT(1 1,3 3)",
        "POLYGON((1 1,2 1,2 2,1 2,1 1))",
        "GEOMETRYCOLLECTION(POINT(1 1),LINESTRING(0 0,2 2))",
    ] {
        equivalent(&mut cache, &g, &GeometryValue::from_wkt(text).unwrap());
    }
    let mut labelled = point(1.0, 1.0);
    labelled.srid = Some(4326);
    assert_eq!(cache.evaluate(Predicate::Contains, &g, &labelled), None);
    labelled.srid = Some(0);
    assert_eq!(
        cache.evaluate(Predicate::Contains, &g, &labelled),
        Some(true)
    );
}

#[test]
fn cache_bounds_and_clone_identity() {
    let mut cache = PreparedGeometryCache::new(2, 10);
    for x in 0..4 {
        let g =
            GeometryValue::from_wkt(&format!("POLYGON(({x} 0,10 0,10 10,{x} 10,{x} 0))")).unwrap();
        let p = point(5.0, 5.0);
        assert_eq!(cache.evaluate(Predicate::Contains, &g, &p), Some(true));
        assert_eq!(
            cache.evaluate(Predicate::Contains, &g.clone(), &p),
            Some(true)
        );
        assert!(cache.len() <= 2);
        assert!(cache.coordinate_count() <= 10);
    }
    assert_eq!(cache.len(), 2);
    let g = GeometryValue::new(polygon(0.0, false).into(), None).unwrap();
    equivalent(&mut cache, &g, &point(1.0, 1.0));
    let mut tiny = PreparedGeometryCache::new(10, 1);
    equivalent(&mut tiny, &g, &point(1.0, 1.0));
    assert!(tiny.is_empty());
}

#[test]
fn cached_bounds_preserve_legacy_serialization_and_equality() {
    #[derive(serde::Serialize)]
    struct Legacy<'a> {
        shape: &'a Geometry<f64>,
        srid: Option<i32>,
    }
    for text in [
        "POLYGON EMPTY",
        "GEOMETRYCOLLECTION(POINT(-3 8),LINESTRING(1 2,10 -5))",
        "SRID=4326;MULTIPOLYGON(((0 0,1 0,1 1,0 0)))",
    ] {
        let g = GeometryValue::from_wkt(text).unwrap();
        let legacy = Legacy {
            shape: g.shape(),
            srid: g.srid,
        };
        let bytes = bincode::serialize(&legacy).unwrap();
        assert_eq!(bytes, bincode::serialize(&g).unwrap());
        let loaded: GeometryValue = bincode::deserialize(&bytes).unwrap();
        assert_eq!(loaded, g);
        assert_eq!(loaded.bounds(), g.shape().bounding_rect());
        assert_eq!(loaded.clone().bounds(), loaded.bounds());
        assert_eq!(
            serde_json::to_value(&g).unwrap(),
            serde_json::to_value(&legacy).unwrap()
        );
        let json: GeometryValue =
            serde_json::from_str(&serde_json::to_string(&legacy).unwrap()).unwrap();
        assert_eq!(json.bounds(), g.bounds());
        assert_eq!(json.shape().coords_count(), g.shape().coords_count());
    }
}

#[test]
fn mixed_geometry_pairs_and_invalid_polygons_keep_original_results() {
    let shapes = [
        "POINT(0 0)",
        "POINT(1 1)",
        "POINT(5 5)",
        "POINT(3 5)",
        "LINESTRING(0 0,10 0)",
        "LINESTRING(1 1,9 9)",
        "MULTIPOINT(1 1,3 3)",
        "MULTIPOINT(5 5,6 6)",
        "MULTILINESTRING((0 0,10 0),(1 1,9 9))",
        "POLYGON((0 0,10 0,10 10,0 10,0 0),(3 3,7 3,7 7,3 7,3 3))",
        "MULTIPOLYGON(((0 0,10 0,10 10,0 10,0 0),(3 3,7 3,7 7,3 7,3 3)))",
        "POLYGON((0 0,10 10,0 10,10 0,0 0))",
        "POLYGON((0 0,10 0,20 0,0 0))",
        "MULTIPOLYGON(((0 0,10 0,10 10,0 10,0 0)),((5 0,15 0,15 10,5 10,5 0)))",
        "GEOMETRYCOLLECTION(POINT(1 1),LINESTRING(0 0,10 0))",
        "POINT(-0 -0)",
        "POINT(0 -0)",
        "POLYGON((-10 -10,10 -10,10 -0,20 -0,20 10,-10 10,-10 -10))",
    ]
    .map(|wkt| GeometryValue::from_wkt(wkt).unwrap());
    let mut cache = PreparedGeometryCache::default();
    for a in &shapes {
        for b in &shapes {
            equivalent(&mut cache, a, b);
        }
    }
}

fn sql(ex: &mut Executor, query: &str, params: &QueryParams) -> Vec<Vec<Value>> {
    ex.execute(&QueryParser::parse(query).unwrap(), params)
        .unwrap()
        .rows
        .into_iter()
        .map(|r| r.values)
        .collect()
}

#[test]
fn sql_constants_parameters_join_sides_and_statement_isolation() {
    let mut ex = Executor::new();
    let mut params = QueryParams::new();
    sql(
        &mut ex,
        "CREATE TABLE devices(id INT, geom GEOMETRY)",
        &params,
    );
    sql(
        &mut ex,
        "CREATE TABLE zones(id INT, geom GEOMETRY)",
        &params,
    );
    sql(
        &mut ex,
        "INSERT INTO zones VALUES(1, ST_MakeEnvelope(0,0,10,10)), (2, ST_MakeEnvelope(10,0,20,10))",
        &params,
    );
    for (id, (x, y)) in [(1.0, 1.0), (10.0, 5.0), (15.0, 5.0), (30.0, 30.0)]
        .into_iter()
        .enumerate()
    {
        ex.get_tables_mut()
            .get_mut("devices")
            .unwrap()
            .rows
            .push(Row::new(
                vec!["id".into(), "geom".into()],
                vec![Value::Integer(id as i64), Value::Geometry(point(x, y))],
            ));
    }
    let query = "SELECT id FROM devices WHERE ST_Contains($1,geom) ORDER BY id";
    for (text, expected) in [
        (
            "POLYGON((0 0,10 0,10 10,0 10,0 0))",
            vec![vec![Value::Integer(0)]],
        ),
        (
            "POLYGON((10 0,20 0,20 10,10 10,10 0))",
            vec![vec![Value::Integer(2)]],
        ),
    ] {
        for value in [
            Value::Text(text.into()),
            Value::Geometry(GeometryValue::from_wkt(text).unwrap()),
        ] {
            params.positional = vec![value];
            assert_eq!(sql(&mut ex, query, &params), expected);
        }
    }
    assert_eq!(
        sql(
            &mut ex,
            "SELECT id FROM devices WHERE ST_Contains(ST_MakeEnvelope(0,0,10,10),geom)",
            &params
        ),
        vec![vec![Value::Integer(0)]]
    );
    assert_eq!(sql(&mut ex, "SELECT ST_Contains(NULL,geom),ST_Contains('broken',geom),ST_Covers(ST_SetSRID(geom,4326),geom) FROM devices", &params), vec![vec![Value::Null; 3]; 4]);
    for indexed in [false, true] {
        if indexed {
            sql(
                &mut ex,
                "CREATE INDEX zones_geom ON zones USING gist(geom)",
                &params,
            );
            sql(
                &mut ex,
                "CREATE INDEX devices_geom ON devices USING gist(geom)",
                &params,
            );
        }
        for predicate in [
            "ST_Contains(z.geom,d.geom)",
            "ST_Within(d.geom,z.geom)",
            "ST_Covers(z.geom,d.geom)",
            "ST_CoveredBy(d.geom,z.geom)",
            "ST_Intersects(z.geom,d.geom)",
            "ST_Intersects(d.geom,z.geom)",
        ] {
            let expected = if predicate.contains("Contains") || predicate.contains("Within") {
                vec![vec![0.into(), 1.into()], vec![2.into(), 2.into()]]
            } else {
                vec![
                    vec![0.into(), 1.into()],
                    vec![1.into(), 1.into()],
                    vec![1.into(), 2.into()],
                    vec![2.into(), 2.into()],
                ]
            };
            for from in ["devices d JOIN zones z", "zones z JOIN devices d"] {
                assert_eq!(
                    sql(
                        &mut ex,
                        &format!("SELECT d.id,z.id FROM {from} ON {predicate} ORDER BY d.id,z.id"),
                        &params
                    ),
                    expected
                );
            }
        }
    }
}
