use nexusql::engine::executor::Executor;
use nexusql::query::evaluator::QueryParams;
use nexusql::query::QueryParser;
use nexusql::{ResultSet, Value};

fn sql(ex: &mut Executor, text: &str) -> ResultSet {
    ex.execute(&QueryParser::parse(text).unwrap(), &QueryParams::new())
        .unwrap()
}

fn values(rs: ResultSet) -> Vec<Vec<Value>> {
    rs.rows.into_iter().map(|row| row.values).collect()
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
