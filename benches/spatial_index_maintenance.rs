//! Run on the base and changed revisions with Criterion's --save-baseline/--baseline.
//! Setup and SQL parsing are excluded; UPDATE includes predicate evaluation.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use geo_types::Point;
use nexusql::engine::Executor;
use nexusql::geometry::GeometryValue;
use nexusql::query::evaluator::QueryParams;
use nexusql::{QueryParser, Row, Value};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::time::Duration;

fn fixture() -> Executor {
    let mut ex = Executor::new();
    ex.execute(
        &QueryParser::parse("CREATE TABLE devices (id INT, geom GEOMETRY)").unwrap(),
        &QueryParams::new(),
    )
    .unwrap();
    let table = ex.get_tables_mut().get_mut("devices").unwrap();
    let mut rng = StdRng::seed_from_u64(0x0052_5452_4545);
    for id in 0..100_000 {
        let point = Point::new(rng.gen_range(0.0..10_000.0), rng.gen_range(0.0..10_000.0));
        table.insert(Row::new(
            table.columns.clone(),
            vec![
                Value::Integer(id),
                Value::Geometry(GeometryValue::new(point.into(), None).unwrap()),
            ],
        ));
    }
    ex
}

fn maintenance(c: &mut Criterion) {
    let create =
        QueryParser::parse("CREATE INDEX devices_geom ON devices USING gist (geom)").unwrap();
    let mut group = c.benchmark_group("spatial_maintenance");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    let mut ex = fixture();
    ex.execute(&create, &QueryParams::new()).unwrap();
    let update =
        QueryParser::parse("UPDATE devices SET geom = ST_Point($1, $2) WHERE id = 50000").unwrap();
    let positions = [
        QueryParams::with_positional(vec![1.0.into(), 2.0.into()]),
        QueryParams::with_positional(vec![3.0.into(), 4.0.into()]),
    ];
    let mut turn = 0;
    group.bench_function("update_sql_100k", |b| {
        b.iter(|| {
            turn ^= 1;
            black_box(ex.execute(&update, &positions[turn]).unwrap());
        });
    });

    let mut ex = fixture();
    group.bench_function("create_index_sql_100k", |b| {
        b.iter(|| black_box(ex.execute(&create, &QueryParams::new()).unwrap()));
    });
    group.finish();
}

criterion_group!(benches, maintenance);
criterion_main!(benches);
