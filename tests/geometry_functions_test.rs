use nexusql::engine::Executor;
use nexusql::query::evaluator::QueryParams;
use nexusql::{QueryParser, ResultSet, Value};

fn sql(ex: &mut Executor, query: &str) -> ResultSet {
    ex.execute(&QueryParser::parse(query).unwrap(), &QueryParams::new())
        .unwrap()
}

fn scalar(ex: &mut Executor, expression: &str) -> Value {
    sql(ex, &format!("SELECT {expression}")).rows[0].values[0].clone()
}

fn number(value: Value) -> f64 {
    value
        .as_f64()
        .unwrap_or_else(|| panic!("expected number, got {value:?}"))
}

fn point(value: Value) -> (f64, f64) {
    let Value::Geometry(g) = value else {
        panic!("expected geometry, got {value:?}")
    };
    let geo_types::Geometry::Point(p) = g.shape() else {
        panic!("expected point, got {g:?}")
    };
    (p.x(), p.y())
}

fn near(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() < tolerance,
        "{actual} != {expected} (tolerance {tolerance})"
    );
}

#[test]
fn utm_matches_independent_proj_reference_under_one_millimetre() {
    let mut ex = Executor::new();
    // Independent oracle: pyproj 3.8.0 / PROJ 9.8.1, always_xy=True.
    // Includes KFM, both hemispheres, zone edges, equator and UTM latitude limits.
    let reference = [
        (-86.507, 36.940, 32616, 543899.795566784, 4088329.915872085),
        (-180.0, 0.0, 32601, 166021.44308054057, 0.0),
        (180.0, 0.0, 32660, 833978.5569194623, 0.0),
        (-174.0, 84.0, 32601, 534994.6550611365, 9329005.182447437),
        (6.0, 60.0, 32632, 332705.1788755489, 6655205.483634564),
        (
            151.2093,
            -33.8688,
            32756,
            334368.633648097,
            6250948.345385009,
        ),
        (18.0, -80.0, 32734, 441867.78486720106, 1116915.0440516975),
        (0.0, 0.0, 32731, 166021.44308054057, 10000000.0),
    ];
    for (lon, lat, srid, east, north) in reference {
        let (x, y) = point(scalar(
            &mut ex,
            &format!("ST_Transform(ST_SetSRID(ST_Point({lon},{lat}),4326),{srid})"),
        ));
        assert!((x - east).hypot(y - north) < 0.001, "EPSG:{srid}: {x}, {y}");
        // Inverse uses the independent reference, not our own forward result.
        let (x, y) = point(scalar(
            &mut ex,
            &format!("ST_Transform(ST_SetSRID(ST_Point({east},{north}),{srid}),4326)"),
        ));
        near(x, lon, 1e-9);
        near(y, lat, 1e-9);
    }
}

#[test]
fn every_wgs84_utm_zone_in_both_hemispheres() {
    let mut ex = Executor::new();
    for zone in 1..=60 {
        let lon = zone as f64 * 6.0 - 183.0 + 2.999;
        for (base, lat, north) in [
            (32600, 45.0, 4987326.584440043),
            (32700, -45.0, 5012673.415559957),
        ] {
            let (x, y) = point(scalar(
                &mut ex,
                &format!(
                    "ST_Transform(ST_SetSRID(ST_Point({lon},{lat}),4326),{})",
                    base + zone
                ),
            ));
            assert!((x - 736367.210697864).hypot(y - north) < 0.001);
        }
    }
}

#[test]
fn web_mercator_and_projected_to_projected_transform() {
    let mut ex = Executor::new();
    let (x, y) = point(scalar(
        &mut ex,
        "ST_Transform(ST_GeomFromText('POINT(-86.507 36.940)',4326),3857)",
    ));
    near(x, -9629915.190053718, 0.00001);
    near(y, 4430746.857550766, 0.00001);
    let (x, y) = point(scalar(
        &mut ex,
        "ST_Transform(ST_GeomFromText('POINT(-9629915.190053718 4430746.857550766)',3857),32616)",
    ));
    near(x, 543899.795566784, 0.001);
    near(y, 4088329.915872085, 0.001);
    let (x, y) = point(scalar(
        &mut ex,
        "ST_Transform(ST_GeomFromText('POINT(-9629915.190053718 4430746.857550766)',3857),4326)",
    ));
    near(x, -86.507, 1e-10);
    near(y, 36.940, 1e-10);
}

#[test]
fn srid_metadata_empty_shapes_and_transform_validation() {
    let mut ex = Executor::new();
    assert_eq!(
        scalar(&mut ex, "ST_AsText(ST_SetSRID(ST_Point(1,2),32616))"),
        Value::from("POINT(1 2)")
    );
    assert_eq!(
        scalar(&mut ex, "ST_SRID(ST_SetSRID(ST_Point(1,2),0))"),
        Value::from(0)
    );
    assert_eq!(
        scalar(
            &mut ex,
            "ST_SRID(ST_Transform(ST_GeomFromText('GEOMETRYCOLLECTION EMPTY',4326),32616))"
        ),
        Value::from(32616)
    );
    for expression in [
        "ST_Transform(ST_Point(1,2),32616)",
        "ST_Transform(ST_SetSRID(ST_Point(1,2),0),32616)",
        "ST_Transform(ST_SetSRID(ST_Point(1,2),4326),32600)",
        "ST_Transform(ST_SetSRID(ST_Point(1,2),4326),32761)",
        "ST_Transform(ST_SetSRID(ST_Point(1,2),9999),4326)",
        "ST_Transform(ST_SetSRID(ST_Point(181,0),4326),3857)",
        "ST_Transform(ST_SetSRID(ST_Point(1,91),4326),32616)",
        "ST_Transform(ST_SetSRID(ST_Point(1,90),4326),3857)",
        "ST_SetSRID(ST_Point(1,2),2147483648)",
        "ST_SetSRID(NULL,4326)",
        "ST_Transform(NULL,4326)",
    ] {
        assert_eq!(scalar(&mut ex, expression), Value::Null, "{expression}");
    }
}

#[test]
fn transform_maps_every_vertex_in_nested_geometry() {
    let mut ex = Executor::new();
    let original = "GEOMETRYCOLLECTION(POINT(-86.507 36.940),LINESTRING(-86.508 36.940,-86.507 36.941),MULTIPOLYGON(((-86.508 36.940,-86.506 36.940,-86.506 36.942,-86.508 36.940))))";
    let transformed = scalar(
        &mut ex,
        &format!("ST_Transform(ST_GeomFromText('{original}',4326),32616)"),
    );
    let Value::Geometry(g) = transformed else {
        panic!("transform failed")
    };
    assert_eq!(g.srid, Some(32616));
    use geo::CoordsIter;
    assert_eq!(g.shape().coords_count(), 7);
    for c in g.shape().coords_iter() {
        assert!(c.x > 543000.0 && c.x < 545000.0);
        assert!(c.y > 4088000.0 && c.y < 4089000.0);
    }
}

#[test]
fn sphere_and_spheroid_distances_match_geodesic_references() {
    let mut ex = Executor::new();
    // WGS84 reference values from PROJ's independent geodesic implementation.
    for (a, b, expected) in [
        ("-86.507 36.940", "-86.506 36.941", 142.3067490163474),
        ("-0.1278 51.5074", "2.3522 48.8566", 343923.1200909896),
        ("0 0", "180 0", 20003931.458625447),
        ("0 0", "1 0", 111319.49079327357),
    ] {
        let args = format!("ST_GeomFromText('POINT({a})',4326),ST_GeomFromText('POINT({b})',4326)");
        let actual = number(scalar(&mut ex, &format!("ST_DistanceSpheroid({args})")));
        near(actual, expected, 0.00001);
        let explicit = number(scalar(
            &mut ex,
            &format!("ST_DistanceSpheroid({args},'SPHEROID[\"WGS 84\",6378137,298.257223563]')"),
        ));
        near(explicit, expected, 0.00001);
    }
    near(
        number(scalar(
            &mut ex,
            "ST_DistanceSphere(ST_Point(0,0),ST_Point(1,0))",
        )),
        111195.07973463158,
        0.00001,
    );
    near(
        number(scalar(
            &mut ex,
            "ST_DistanceSphere(ST_Point(179,0),ST_Point(-179,0),6371000)",
        )),
        222389.85328911748,
        0.00001,
    );
    assert_eq!(
        scalar(&mut ex, "ST_DistanceSpheroid(ST_Point(1,2),ST_Point(1,2))"),
        Value::from(0.0)
    );
    near(
        number(scalar(
            &mut ex,
            "ST_DistanceSpheroid(ST_GeomFromText('MULTIPOINT(0 0,10 0)'),ST_Point(1,0))",
        )),
        111319.49079327357,
        0.00001,
    );
}

#[test]
fn geographic_distance_rejects_invalid_units_and_coordinates() {
    let mut ex = Executor::new();
    for expression in [
        "ST_DistanceSphere(ST_Point(0,0),ST_Point(0,91))",
        "ST_DistanceSphere(ST_Point(181,0),ST_Point(0,0))",
        "ST_DistanceSphere(ST_Point(0,0),ST_Point(1,0),-1)",
        "ST_DistanceSphere(ST_Point(0,0),ST_Point(1,0),0)",
        "ST_DistanceSpheroid(ST_GeomFromText('POINT(0 0)',3857),ST_GeomFromText('POINT(1 0)',3857))",
        "ST_DistanceSpheroid(ST_Point(0,0),ST_SetSRID(ST_Point(1,0),4326))",
        "ST_DistanceSpheroid(ST_Point(0,0),ST_Point(1,0),'SPHEROID[\"bad\",6378137,1]')",
        "ST_DistanceSpheroid(ST_Point(0,0),ST_Point(1,0),'broken')",
        "ST_DistanceSphere(ST_GeomFromText('MULTIPOINT EMPTY'),ST_Point(0,0))",
        "ST_DistanceSpheroid(NULL,ST_Point(0,0))",
    ] {
        assert_eq!(scalar(&mut ex, expression), Value::Null, "{expression}");
    }
}

#[test]
fn covers_includes_shell_hole_edges_and_line_endpoints() {
    let mut ex = Executor::new();
    let polygon = "ST_GeomFromText('POLYGON((0 0,10 0,10 10,0 10,0 0),(3 3,7 3,7 7,3 7,3 3))')";
    for (other, expected) in [
        ("POINT(0 5)", true),
        ("POINT(3 5)", true),
        ("POINT(5 5)", false),
        ("POINT(1 1)", true),
        ("LINESTRING(0 0,10 0)", true),
        ("LINESTRING(1 5,9 5)", false),
        ("MULTIPOINT(0 5,1 1)", true),
        ("MULTIPOINT(0 5,5 5)", false),
        ("GEOMETRYCOLLECTION(POINT(0 5),LINESTRING(0 0,10 0))", true),
        ("GEOMETRYCOLLECTION EMPTY", false),
    ] {
        for expression in [
            format!("ST_Covers({polygon},ST_GeomFromText('{other}'))"),
            format!("ST_CoveredBy(ST_GeomFromText('{other}'),{polygon})"),
        ] {
            assert_eq!(
                scalar(&mut ex, &expression),
                Value::from(expected),
                "{expression}"
            );
        }
    }
    assert_eq!(
        scalar(
            &mut ex,
            "ST_Covers(ST_GeomFromText('LINESTRING(0 0,5 0)'),ST_Point(0,0))"
        ),
        Value::from(true)
    );
    assert_eq!(
        scalar(
            &mut ex,
            "ST_Covers(ST_GeomFromText('POLYGON EMPTY'),ST_Point(0,0))"
        ),
        Value::from(false)
    );
}

#[test]
fn length_and_perimeter_follow_geometry_dimension_and_include_holes() {
    let mut ex = Executor::new();
    for (wkt, length, perimeter) in [
        ("POINT(1 2)", 0.0, 0.0),
        ("LINESTRING(0 0,3 4,6 4)", 8.0, 0.0),
        ("MULTILINESTRING((0 0,3 4),(0 0,0 10))", 15.0, 0.0),
        (
            "POLYGON((0 0,10 0,10 10,0 10,0 0),(3 3,7 3,7 7,3 7,3 3))",
            0.0,
            56.0,
        ),
        (
            "GEOMETRYCOLLECTION(LINESTRING(0 0,3 4),POLYGON((0 0,2 0,2 2,0 2,0 0)))",
            5.0,
            8.0,
        ),
        ("LINESTRING EMPTY", 0.0, 0.0),
        ("POLYGON EMPTY", 0.0, 0.0),
    ] {
        assert_eq!(
            scalar(&mut ex, &format!("ST_Length('{wkt}')")),
            Value::from(length)
        );
        assert_eq!(
            scalar(&mut ex, &format!("ST_Perimeter('{wkt}')")),
            Value::from(perimeter)
        );
    }
}

#[test]
fn buffers_are_planar_rounded_and_respect_negative_distances() {
    let mut ex = Executor::new();
    let point_buffer = "ST_Buffer(ST_SetSRID(ST_Point(0,0),32616),10)";
    near(
        number(scalar(&mut ex, &format!("ST_Area({point_buffer})"))),
        312.1445152258052,
        0.00001,
    );
    assert_eq!(
        scalar(&mut ex, &format!("ST_SRID({point_buffer})")),
        Value::from(32616)
    );
    assert_eq!(
        scalar(&mut ex, &format!("ST_IsValid({point_buffer})")),
        Value::from(true)
    );
    for expression in [
        "ST_Buffer(ST_Point(0,0),0)",
        "ST_Buffer(ST_Point(0,0),-1)",
        "ST_Buffer('LINESTRING(0 0,10 0)',-1)",
        "ST_Buffer(ST_MakeEnvelope(0,0,10,10),-6)",
        "ST_Buffer('POLYGON EMPTY',2)",
    ] {
        assert_eq!(
            scalar(&mut ex, &format!("ST_AsText({expression})")),
            Value::from("POLYGON EMPTY"),
            "{expression}"
        );
    }
    near(
        number(scalar(
            &mut ex,
            "ST_Area(ST_Buffer(ST_MakeEnvelope(0,0,10,10),-1))",
        )),
        64.0,
        1e-6,
    );
    near(
        number(scalar(
            &mut ex,
            "ST_Area(ST_Buffer(ST_MakeEnvelope(0,0,10,10),0))",
        )),
        100.0,
        1e-6,
    );
    near(
        number(scalar(
            &mut ex,
            "ST_Area(ST_Buffer('LINESTRING(0 0,10 0)',1))",
        )),
        20.0 + 3.121445152258052,
        1e-6,
    );
    assert_eq!(
        scalar(
            &mut ex,
            "ST_IsValid(ST_Buffer('POLYGON((0 0,2 2,0 2,2 0,0 0))',0))"
        ),
        Value::from(true)
    );
}

#[test]
fn polygon_union_dissolves_overlap_and_respects_holes() {
    let mut ex = Executor::new();
    let union = "ST_Union(ST_MakeEnvelope(0,0,10,10,32616),ST_MakeEnvelope(5,0,15,10,32616))";
    near(
        number(scalar(&mut ex, &format!("ST_Area({union})"))),
        150.0,
        1e-6,
    );
    assert_eq!(
        scalar(&mut ex, &format!("ST_SRID({union})")),
        Value::from(32616)
    );
    assert_eq!(
        scalar(&mut ex, &format!("ST_IsValid({union})")),
        Value::from(true)
    );
    let donut = "'POLYGON((0 0,10 0,10 10,0 10,0 0),(3 3,7 3,7 7,3 7,3 3))'";
    near(
        number(scalar(
            &mut ex,
            &format!("ST_Area(ST_Union({donut},ST_MakeEnvelope(3,3,7,7)))"),
        )),
        100.0,
        1e-6,
    );
    near(
        number(scalar(
            &mut ex,
            &format!("ST_Area(ST_Union({donut},'POLYGON EMPTY'))"),
        )),
        84.0,
        1e-6,
    );
}

#[test]
fn union_nodes_lines_deduplicates_points_and_clips_covered_components() {
    let mut ex = Executor::new();
    assert_eq!(
        scalar(&mut ex, "ST_AsText(ST_Union(ST_Point(1,2),ST_Point(1,2)))"),
        Value::from("POINT(1 2)")
    );
    assert_eq!(
        scalar(&mut ex, "ST_AsText(ST_Union(ST_Point(1,2),ST_Point(3,4)))"),
        Value::from("MULTIPOINT((1 2),(3 4))")
    );
    let overlapping = "ST_Union('LINESTRING(0 0,10 0)','LINESTRING(5 0,15 0)')";
    near(
        number(scalar(&mut ex, &format!("ST_Length({overlapping})"))),
        15.0,
        1e-9,
    );
    let crossing = scalar(
        &mut ex,
        "ST_Union('LINESTRING(0 5,10 5)','LINESTRING(5 0,5 10)')",
    );
    let Value::Geometry(g) = crossing else {
        panic!("union failed")
    };
    let geo_types::Geometry::MultiLineString(lines) = g.shape() else {
        panic!("noded lines expected")
    };
    assert_eq!(lines.0.len(), 4);
    let mixed = "ST_Union(ST_MakeEnvelope(0,0,10,10),'GEOMETRYCOLLECTION(LINESTRING(-5 5,15 5),POINT(5 5),POINT(20 20))')";
    near(
        number(scalar(&mut ex, &format!("ST_Area({mixed})"))),
        100.0,
        1e-9,
    );
    near(
        number(scalar(&mut ex, &format!("ST_Length({mixed})"))),
        10.0,
        1e-9,
    );
    assert_eq!(
        scalar(&mut ex, &format!("ST_Covers({mixed},ST_Point(20,20))")),
        Value::from(true)
    );
    assert_eq!(
        scalar(
            &mut ex,
            "ST_AsText(ST_Union('POLYGON EMPTY','POLYGON EMPTY'))"
        ),
        Value::from("POLYGON EMPTY")
    );
}

#[test]
fn union_aggregate_supports_groups_nulls_empty_input_and_srid_checks() {
    let mut ex = Executor::new();
    sql(
        &mut ex,
        "CREATE TABLE regions (category INT, geom GEOMETRY)",
    );
    assert_eq!(
        sql(&mut ex, "SELECT ST_Union(geom) FROM regions").rows[0].values[0],
        Value::Null
    );
    sql(&mut ex, "INSERT INTO regions VALUES (1,ST_MakeEnvelope(0,0,10,10)),(1,ST_MakeEnvelope(5,0,15,10)),(1,NULL),(2,ST_Point(30,30)),(3,NULL)");
    let grouped = sql(
        &mut ex,
        "SELECT category, ST_Union(geom) FROM regions GROUP BY category ORDER BY category",
    );
    assert_eq!(grouped.rows.len(), 3);
    let Value::Geometry(g) = &grouped.rows[0].values[1] else {
        panic!("union expected")
    };
    use geo::Area;
    near(g.shape().unsigned_area(), 150.0, 1e-9);
    assert_eq!(grouped.rows[2].values[1], Value::Null);
    let all = sql(&mut ex, "SELECT ST_Union(geom) FROM regions");
    assert_eq!(all.rows.len(), 1);
    let Value::Geometry(g) = &all.rows[0].values[0] else {
        panic!("union expected")
    };
    assert!(matches!(
        g.shape(),
        geo_types::Geometry::GeometryCollection(_)
    ));
    // Two arguments must remain a scalar expression, producing one row per row.
    assert_eq!(
        sql(
            &mut ex,
            "SELECT ST_Union(geom,ST_Point(50,50)) FROM regions"
        )
        .rows
        .len(),
        5
    );
    sql(
        &mut ex,
        "INSERT INTO regions VALUES (4,ST_SetSRID(ST_Point(1,1),4326))",
    );
    assert_eq!(
        sql(&mut ex, "SELECT ST_Union(geom) FROM regions").rows[0].values[0],
        Value::Null
    );
}

#[test]
fn validity_detects_self_intersections_holes_overlaps_and_disconnected_interiors() {
    let mut ex = Executor::new();
    for (wkt, expected) in [
        ("POLYGON((0 0,2 2,0 2,2 0,0 0))", false),
        (
            "POLYGON((0 0,10 0,10 10,0 10,0 0),(20 20,21 20,21 21,20 20))",
            false,
        ),
        (
            "POLYGON((0 0,10 0,10 10,0 10,0 0),(0 5,5 0,10 5,5 10,0 5))",
            false,
        ),
        (
            "MULTIPOLYGON(((0 0,10 0,10 10,0 10,0 0)),((5 0,15 0,15 10,5 10,5 0)))",
            false,
        ),
        ("POLYGON((0 0,10 0,10 10,0 10,0 0),(0 5,2 4,2 6,0 5))", true),
        ("LINESTRING(0 0,0 0)", false),
        ("LINESTRING(0 0,2 2,0 2,2 0)", true),
        ("POLYGON EMPTY", true),
        ("GEOMETRYCOLLECTION EMPTY", true),
    ] {
        assert_eq!(
            scalar(&mut ex, &format!("ST_IsValid('{wkt}')")),
            Value::from(expected),
            "{wkt}"
        );
    }
}

#[test]
fn make_valid_repairs_rings_and_retains_collapsed_lines_and_points() {
    let mut ex = Executor::new();
    for wkt in [
        "POLYGON((0 0,2 2,0 2,2 0,0 0))",
        "POLYGON((0 0,10 0,10 10,0 10,0 0),(20 20,21 20,21 21,20 20))",
        "POLYGON((0 0,10 0,10 10,0 10,0 0),(0 5,5 0,10 5,5 10,0 5))",
        "MULTIPOLYGON(((0 0,10 0,10 10,0 10,0 0)),((5 0,15 0,15 10,5 10,5 0)))",
        "POLYGON((0 0,1 0,2 0,0 0))",
        "POLYGON((0 0,0 0,0 0,0 0))",
        "LINESTRING(0 0,0 0)",
        "GEOMETRYCOLLECTION(LINESTRING(1 1,1 1),POLYGON((0 0,2 2,0 2,2 0,0 0)))",
    ] {
        assert_eq!(
            scalar(&mut ex, &format!("ST_IsValid(ST_MakeValid('{wkt}'))")),
            Value::from(true),
            "{wkt}"
        );
    }
    near(
        number(scalar(
            &mut ex,
            "ST_Area(ST_MakeValid('POLYGON((0 0,2 2,0 2,2 0,0 0))'))",
        )),
        2.0,
        1e-9,
    );
    assert_eq!(
        scalar(&mut ex, "ST_AsText(ST_MakeValid('LINESTRING(1 2,1 2)'))"),
        Value::from("POINT(1 2)")
    );
    near(
        number(scalar(
            &mut ex,
            "ST_Length(ST_MakeValid('POLYGON((0 0,1 0,2 0,0 0))'))",
        )),
        2.0,
        1e-9,
    );
    assert_eq!(
        scalar(
            &mut ex,
            "ST_SRID(ST_MakeValid(ST_GeomFromText('POLYGON((0 0,2 2,0 2,2 0,0 0))',32616)))"
        ),
        Value::from(32616)
    );
    assert_eq!(
        scalar(&mut ex, "ST_AsText(ST_MakeValid('POLYGON EMPTY'))"),
        Value::from("POLYGON EMPTY")
    );
}

#[test]
fn closest_point_is_on_first_input_including_crossings_containment_and_holes() {
    let mut ex = Executor::new();
    for (a, b, x, y) in [
        ("LINESTRING(0 0,10 0)", "POINT(4 3)", 4.0, 0.0),
        ("POINT(4 3)", "LINESTRING(0 0,10 0)", 4.0, 3.0),
        ("LINESTRING(0 5,10 5)", "LINESTRING(5 0,5 10)", 5.0, 5.0),
        ("POLYGON((0 0,10 0,10 10,0 10,0 0))", "POINT(4 3)", 4.0, 3.0),
        ("LINESTRING EMPTY", "POINT(4 3)", f64::NAN, f64::NAN),
        ("LINESTRING(0 0,0 0)", "POINT(4 3)", 0.0, 0.0),
    ] {
        let actual = scalar(&mut ex, &format!("ST_ClosestPoint('{a}','{b}')"));
        if x.is_nan() {
            assert_eq!(actual, Value::Null);
            continue;
        }
        let (actual_x, actual_y) = point(actual);
        near(actual_x, x, 1e-9);
        near(actual_y, y, 1e-9);
    }
    let donut = "'POLYGON((0 0,10 0,10 10,0 10,0 0),(3 3,7 3,7 7,3 7,3 3))'";
    near(
        number(scalar(
            &mut ex,
            &format!("ST_Distance(ST_ClosestPoint({donut},ST_Point(5,5)),ST_Point(5,5))"),
        )),
        2.0,
        1e-9,
    );
    assert_eq!(scalar(&mut ex, "ST_SRID(ST_ClosestPoint(ST_SetSRID(ST_Point(0,0),32616),ST_SetSRID(ST_Point(1,1),32616)))"), Value::from(32616));
}

fn index_searches(ex: &Executor, table: &str, index: &str) -> u64 {
    let nexusql::storage::IndexType::GiST(index) = &ex.get_tables()[table].typed_indexes[index]
    else {
        panic!("GiST expected")
    };
    index.stats().search_count
}

#[test]
fn covers_filters_probe_either_index_argument_and_recheck_holes() {
    let mut ex = Executor::new();
    sql(&mut ex, "CREATE TABLE shapes (id INT, geom GEOMETRY)");
    sql(&mut ex, "INSERT INTO shapes VALUES (1,'POLYGON((0 0,10 0,10 10,0 10,0 0),(3 3,7 3,7 7,3 7,3 3))'),(2,ST_MakeEnvelope(30,30,40,40)),(3,'POLYGON EMPTY')");
    let filters = [
        ("ST_Covers(geom,ST_Point(0,5))", vec![1]),
        ("ST_Covers(geom,ST_Point(3,5))", vec![1]),
        ("ST_Covers(geom,ST_Point(5,5))", vec![]),
        ("ST_CoveredBy(ST_Point(0,5),geom)", vec![1]),
        ("ST_CoveredBy(ST_Point(5,5),geom)", vec![]),
        ("ST_Covers(ST_MakeEnvelope(0,0,10,10),geom)", vec![1]),
        ("ST_CoveredBy(geom,ST_MakeEnvelope(0,0,10,10))", vec![1]),
        ("ST_Covers(geom,ST_Point(0,5)) AND id = 1", vec![1]),
        (
            "ST_Covers(geom,ST_Point(0,5)) OR ST_CoveredBy(ST_Point(30,30),geom)",
            vec![1, 2],
        ),
    ];
    for indexed in [false, true] {
        if indexed {
            sql(
                &mut ex,
                "CREATE INDEX shapes_geom ON shapes USING gist (geom)",
            );
        }
        for (filter, ids) in &filters {
            let before = if indexed {
                index_searches(&ex, "shapes", "shapes_geom")
            } else {
                0
            };
            let rows = sql(
                &mut ex,
                &format!("SELECT id FROM shapes WHERE {filter} ORDER BY id"),
            )
            .rows;
            assert_eq!(
                rows.iter()
                    .map(|r| r.values[0].as_i64().unwrap())
                    .collect::<Vec<_>>(),
                *ids,
                "{filter}, indexed={indexed}"
            );
            if indexed {
                assert!(
                    index_searches(&ex, "shapes", "shapes_geom") > before,
                    "{filter}"
                );
            }
        }
    }
    let statement =
        QueryParser::parse("SELECT id FROM shapes s WHERE ST_CoveredBy(ST_Point($1,$2),s.geom)")
            .unwrap();
    let before = index_searches(&ex, "shapes", "shapes_geom");
    let result = ex
        .execute(
            &statement,
            &QueryParams::with_positional(vec![3.into(), 5.into()]),
        )
        .unwrap();
    assert_eq!(result.rows[0].values[0], Value::from(1));
    assert!(index_searches(&ex, "shapes", "shapes_geom") > before);
}

#[test]
fn covers_inner_and_left_joins_use_rtree_and_include_shared_edges() {
    let mut ex = Executor::new();
    sql(&mut ex, "CREATE TABLE rooms (id INT, geom GEOMETRY)");
    sql(
        &mut ex,
        "INSERT INTO rooms VALUES (1,ST_MakeEnvelope(0,0,10,10)),(2,ST_MakeEnvelope(10,0,20,10))",
    );
    sql(&mut ex, "CREATE TABLE devices (id INT, geom GEOMETRY)");
    sql(
        &mut ex,
        "INSERT INTO devices VALUES (1,ST_Point(10,5)),(2,ST_Point(30,30))",
    );
    let queries = [
        "SELECT d.id,r.id FROM devices d JOIN rooms r ON ST_Covers(r.geom,d.geom) ORDER BY d.id,r.id",
        "SELECT d.id,r.id FROM devices d LEFT JOIN rooms r ON ST_CoveredBy(d.geom,r.geom) ORDER BY d.id,r.id",
        "SELECT r.id,d.id FROM rooms r JOIN devices d ON ST_Covers(r.geom,d.geom) ORDER BY r.id,d.id",
        "SELECT r.id,d.id FROM rooms r JOIN devices d ON ST_CoveredBy(d.geom,r.geom) ORDER BY r.id,d.id",
    ];
    let expected: Vec<_> = queries
        .iter()
        .map(|query| sql(&mut ex, query).rows)
        .collect();
    assert_eq!(expected[0].len(), 2);
    assert_eq!(expected[1].len(), 3);
    sql(
        &mut ex,
        "CREATE INDEX rooms_geom ON rooms USING gist (geom)",
    );
    sql(
        &mut ex,
        "CREATE INDEX devices_geom ON devices USING gist (geom)",
    );
    for (i, query) in queries.iter().enumerate() {
        let (table, index) = if i < 2 {
            ("rooms", "rooms_geom")
        } else {
            ("devices", "devices_geom")
        };
        let before = index_searches(&ex, table, index);
        assert_eq!(sql(&mut ex, query).rows, expected[i]);
        assert_eq!(index_searches(&ex, table, index) - before, 2);
    }
}

#[test]
fn geojson_round_trips_all_geometry_types_and_nested_empty_collections() {
    let mut ex = Executor::new();
    for wkt in [
        "POINT(-86.507 36.940)",
        "LINESTRING(0 0,1 2)",
        "POLYGON((0 0,10 0,10 10,0 10,0 0),(3 3,7 3,7 7,3 7,3 3))",
        "MULTIPOINT(1 2,3 4)",
        "MULTILINESTRING((0 0,1 2),(3 4,5 6))",
        "MULTIPOLYGON(((0 0,1 0,1 1,0 0)),((2 2,3 2,3 3,2 2)))",
        "GEOMETRYCOLLECTION(POINT(1 2),GEOMETRYCOLLECTION(LINESTRING(0 0,1 2),POLYGON EMPTY))",
        "LINESTRING EMPTY",
        "POLYGON EMPTY",
        "MULTIPOINT EMPTY",
        "MULTILINESTRING EMPTY",
        "MULTIPOLYGON EMPTY",
        "GEOMETRYCOLLECTION EMPTY",
    ] {
        for srid in [4326, 32616] {
            let input = format!("ST_GeomFromText('{wkt}',{srid})");
            let expected = scalar(&mut ex, &input);
            let actual = scalar(
                &mut ex,
                &format!("ST_GeomFromGeoJSON(ST_AsGeoJSON({input}))"),
            );
            assert_eq!(actual, expected, "{wkt}, SRID {srid}");
        }
    }
    let encoded = scalar(
        &mut ex,
        "ST_AsGeoJSON(ST_SetSRID(ST_Point(-86.507,36.940),4326))",
    );
    let json: serde_json::Value = serde_json::from_str(encoded.as_str().unwrap()).unwrap();
    assert_eq!(json["type"], "Point");
    assert_eq!(json["coordinates"], serde_json::json!([-86.507, 36.940]));
    assert!(json.get("crs").is_none());
    assert_eq!(
        scalar(
            &mut ex,
            "ST_SRID(ST_GeomFromGeoJSON('{\"type\":\"Point\",\"coordinates\":[1,2]}'))"
        ),
        Value::from(4326)
    );
}

#[test]
fn geojson_rejects_non_geometry_malformed_or_non_2d_input() {
    let mut ex = Executor::new();
    for input in [
        "null",
        "broken",
        "{}",
        "[]",
        r#"{"type":"Feature","geometry":{"type":"Point","coordinates":[1,2]}}"#,
        r#"{"type":"FeatureCollection","features":[]}"#,
        r#"{"type":"Point","coordinates":[1,2,3]}"#,
        r#"{"type":"Point","coordinates":[]}"#,
        r#"{"type":"Point","coordinates":["1",2]}"#,
        r#"{"type":"Point","coordinates":[1e400,2]}"#,
        r#"{"type":"Point","coordinates":[NaN,2]}"#,
        r#"{"type":"LineString","coordinates":[[1,2]]}"#,
        r#"{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1]]]}"#,
        r#"{"type":"GeometryCollection","geometries":[{"type":"Point","coordinates":[1,2,3]}]}"#,
        r#"{"type":"Point","coordinates":[1,2],"crs":{"type":"name","properties":{"name":"unknown"}}}"#,
    ] {
        assert_eq!(
            scalar(&mut ex, &format!("ST_GeomFromGeoJSON('{input}')")),
            Value::Null,
            "{input}"
        );
    }
}

#[test]
fn new_functions_propagate_nulls_bad_arity_and_srid_mismatches() {
    let mut ex = Executor::new();
    for function in [
        "ST_AsGeoJSON",
        "ST_GeomFromGeoJSON",
        "ST_IsValid",
        "ST_MakeValid",
        "ST_Length",
        "ST_Perimeter",
    ] {
        assert_eq!(scalar(&mut ex, &format!("{function}(NULL)")), Value::Null);
        assert_eq!(scalar(&mut ex, &format!("{function}()")), Value::Null);
        assert_eq!(
            scalar(&mut ex, &format!("{function}('broken')")),
            Value::Null
        );
    }
    for function in ["ST_Covers", "ST_CoveredBy", "ST_Union", "ST_ClosestPoint"] {
        assert_eq!(
            scalar(&mut ex, &format!("{function}(NULL,ST_Point(0,0))")),
            Value::Null
        );
        assert_eq!(
            scalar(
                &mut ex,
                &format!("{function}(ST_Point(0,0),ST_SetSRID(ST_Point(0,0),4326))")
            ),
            Value::Null
        );
        assert_eq!(
            scalar(
                &mut ex,
                &format!("{function}(ST_Point(0,0),ST_Point(1,1),1)")
            ),
            Value::Null
        );
    }
    assert_eq!(
        scalar(&mut ex, "ST_Buffer(ST_Point(0,0),NULL)"),
        Value::Null
    );
    assert_eq!(
        scalar(&mut ex, "ST_Buffer(ST_Point(0,0),'bad')"),
        Value::Null
    );
}

#[test]
fn geographic_distances_measure_arc_interiors_and_antimeridian_crossings() {
    let mut ex = Executor::new();
    for (line, point) in [
        ("LINESTRING(-1 0,1 0)", "POINT(0 1)"),
        ("LINESTRING(179 0,-179 0)", "POINT(180 1)"),
        ("MULTILINESTRING EMPTY", "POINT(0 1)"),
    ] {
        if line.ends_with("EMPTY") {
            assert_eq!(
                scalar(&mut ex, &format!("ST_DistanceSpheroid('{line}','{point}')")),
                Value::Null
            );
            continue;
        }
        for (function, expected) in [
            ("ST_DistanceSphere", 111195.07973463158),
            ("ST_DistanceSpheroid", 110574.38855779878),
        ] {
            near(
                number(scalar(&mut ex, &format!("{function}('{line}','{point}')"))),
                expected,
                0.00001,
            );
            near(
                number(scalar(&mut ex, &format!("{function}('{point}','{line}')"))),
                expected,
                0.00001,
            );
        }
    }
    for (a, b) in [
        ("LINESTRING(-1 0,1 0)", "LINESTRING(0 -1,0 1)"),
        ("LINESTRING(179 0,-179 0)", "LINESTRING(180 -1,180 1)"),
        ("LINESTRING(0 0,2 0)", "LINESTRING(1 0,3 0)"),
    ] {
        assert_eq!(
            scalar(&mut ex, &format!("ST_DistanceSpheroid('{a}','{b}')")),
            Value::from(0.0)
        );
    }
    near(number(scalar(&mut ex, "ST_DistanceSpheroid('GEOMETRYCOLLECTION(POLYGON EMPTY,LINESTRING(-1 0,1 0),POINT(10 10))','POINT(0 1)')")), 110574.38855779878, 0.00001);
    assert_eq!(
        scalar(
            &mut ex,
            "ST_DistanceSphere('LINESTRING(0 0,180 0)','POINT(0 1)')"
        ),
        Value::Null,
        "an antipodal edge has no unique arc"
    );
    near(
        number(scalar(
            &mut ex,
            "ST_DistanceSphere(ST_Point(0,0),ST_Point(180,0))",
        )),
        std::f64::consts::PI * 6371008.771415059,
        0.00001,
    );
}

#[test]
fn geographic_polygon_interiors_holes_and_poles_are_spherical() {
    let mut ex = Executor::new();
    for polygon in [
        "POLYGON((179 -1,-179 -1,-179 1,179 1,179 -1))",
        "POLYGON((179 -1,179 1,-179 1,-179 -1,179 -1))",
    ] {
        assert_eq!(
            scalar(
                &mut ex,
                &format!("ST_DistanceSpheroid('{polygon}','POINT(180 0)')")
            ),
            Value::from(0.0)
        );
        assert!(
            number(scalar(
                &mut ex,
                &format!("ST_DistanceSpheroid('{polygon}','POINT(0 0)')")
            )) > 19_000_000.0
        );
    }
    let cap = "POLYGON((-135 80,-45 80,45 80,135 80,-135 80))";
    assert_eq!(
        scalar(
            &mut ex,
            &format!("ST_DistanceSphere('{cap}','POINT(0 90)')")
        ),
        Value::from(0.0)
    );
    assert!(
        number(scalar(
            &mut ex,
            &format!("ST_DistanceSphere('{cap}','POINT(0 -90)')")
        )) > 18_000_000.0
    );
    let donut = "POLYGON((179 -1,-179 -1,-179 1,179 1,179 -1),(179.5 -0.5,-179.5 -0.5,-179.5 0.5,179.5 0.5,179.5 -0.5))";
    let distance = number(scalar(
        &mut ex,
        &format!("ST_DistanceSpheroid('{donut}','POINT(180 0)')"),
    ));
    assert!(distance > 55_000.0 && distance < 56_000.0);
    assert_eq!(
        scalar(
            &mut ex,
            &format!("ST_DistanceSphere('{donut}','POINT(179.5 0)')")
        ),
        Value::from(0.0)
    );
    // Small local polygons must not lose their signed spherical area to rounding.
    let kfm = "POLYGON((-86.5071 36.9399,-86.5069 36.9399,-86.5069 36.9401,-86.5071 36.9401,-86.5071 36.9399))";
    assert_eq!(
        scalar(
            &mut ex,
            &format!("ST_DistanceSpheroid('{kfm}','POINT(-86.507 36.940)')")
        ),
        Value::from(0.0)
    );
}

#[test]
fn empty_buffer_members_and_collapsed_polygon_validity() {
    let mut ex = Executor::new();
    for geometry in [
        "GEOMETRYCOLLECTION(POLYGON EMPTY,POINT(0 0))",
        "GEOMETRYCOLLECTION(GEOMETRYCOLLECTION(POLYGON EMPTY),POINT(0 0))",
        "MULTIPOLYGON(EMPTY,((0 0,10 0,10 10,0 10,0 0)))",
    ] {
        assert_eq!(
            scalar(&mut ex, &format!("ST_IsValid(ST_Buffer('{geometry}',1))")),
            Value::from(true),
            "{geometry}"
        );
    }
    assert_eq!(
        scalar(&mut ex, "ST_IsValid('POLYGON((0 0,1 0,2 0,0 0))')"),
        Value::from(false)
    );
    let spike = "ST_MakeValid('POLYGON((0 0,10 0,10 10,5 10,5 12,5 10,0 10,0 0))')";
    near(
        number(scalar(&mut ex, &format!("ST_Area({spike})"))),
        100.0,
        1e-9,
    );
    near(
        number(scalar(&mut ex, &format!("ST_Length({spike})"))),
        2.0,
        1e-9,
    );
}

#[test]
fn make_valid_linework_matches_geos_reference_for_duplicate_and_overlapping_rings() {
    let mut ex = Executor::new();
    // Independent reference: Shapely 2.2.0 / GEOS 3.14.1 make_valid(linework).
    for (wkt, area, length) in [
        ("POLYGON((-0 0,2 2,0 2,2 0,0 0))", 2.0, 0.0),
        (
            "MULTIPOLYGON(((0 0,10 0,10 10,0 10,0 0)),((5 0,15 0,15 10,5 10,5 0)))",
            150.0,
            20.0,
        ),
        (
            "MULTIPOLYGON(((0 0,10 0,10 10,0 10,0 0)),((5 5,15 5,15 15,5 15,5 5)))",
            150.0,
            0.0,
        ),
        (
            "MULTIPOLYGON(((0 0,10 0,10 10,0 10,0 0)),((2 2,8 2,8 8,2 8,2 2)))",
            64.0,
            0.0,
        ),
        (
            "MULTIPOLYGON(((0 0,10 0,10 10,0 10,0 0)),((0 0,10 0,10 10,0 10,0 0)))",
            100.0,
            0.0,
        ),
        (
            "POLYGON((0 0,10 0,10 10,0 10,0 0),(3 3,7 3,7 7,3 7,3 3),(3 3,7 3,7 7,3 7,3 3))",
            84.0,
            0.0,
        ),
    ] {
        let repaired = format!("ST_MakeValid('{wkt}')");
        assert_eq!(
            scalar(&mut ex, &format!("ST_IsValid({repaired})")),
            Value::from(true),
            "{wkt}"
        );
        near(
            number(scalar(&mut ex, &format!("ST_Area({repaired})"))),
            area,
            1e-9,
        );
        near(
            number(scalar(&mut ex, &format!("ST_Length({repaired})"))),
            length,
            1e-9,
        );
    }
}
