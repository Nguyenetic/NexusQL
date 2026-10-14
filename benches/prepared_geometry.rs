//! KFM-sized, deterministic SQL join: 500 zones (20..=200 vertices), 50,000 devices.
//! Run with `cargo bench --bench prepared_geometry -- --runs 3` on both revisions.
//! Setup and parsing are excluded; every timed sample executes a fresh statement.
//!
//! Reference run, Windows x64, release profile, three samples (2026-10-14):
//!                      base 57068ef       prepared cache       speedup
//! scan_join median     24,214.716 ms      11,297.150 ms         2.14x
//! gist_join median        377.785 ms         269.560 ms         1.40x
//! Both paths/revisions returned 32,872 rows, checksum 822932692973692.
//! These SQL timings include shared geometry clones and cached bounds as well
//! as prepared predicates; they do not isolate the edge index's contribution.

use geo_types::{Coord, LineString, Point, Polygon};
use nexusql::engine::Executor;
use nexusql::geometry::GeometryValue;
use nexusql::query::evaluator::QueryParams;
use nexusql::{QueryParser, Row, Value};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::hint::black_box;
use std::time::Instant;

fn fixture() -> Executor {
    let mut ex = Executor::new();
    for sql in [
        "CREATE TABLE zones (id INT, geom GEOMETRY)",
        "CREATE TABLE devices (id INT, geom GEOMETRY)",
    ] {
        ex.execute(&QueryParser::parse(sql).unwrap(), &QueryParams::new())
            .unwrap();
    }
    for id in 0..500 {
        let n = 20 + id * 37 % 181;
        let (x, y) = ((id % 25) as f64 * 100.0, (id / 25) as f64 * 100.0);
        let ring = (0..n)
            .map(|v| {
                let angle = std::f64::consts::TAU * v as f64 / n as f64;
                let radius = 45.0 + 12.0 * (angle * 5.0).cos();
                Coord {
                    x: x + 50.0 + radius * angle.cos(),
                    y: y + 50.0 + radius * angle.sin(),
                }
            })
            .collect::<Vec<_>>();
        let shape = Polygon::new(LineString::new(ring), vec![]);
        ex.get_tables_mut()
            .get_mut("zones")
            .unwrap()
            .rows
            .push(Row::new(
                vec!["id".into(), "geom".into()],
                vec![
                    Value::Integer(id as i64),
                    Value::Geometry(GeometryValue::new(shape.into(), None).unwrap()),
                ],
            ));
    }
    let mut rng = StdRng::seed_from_u64(0x4b464d);
    for id in 0..50_000 {
        let point = Point::new(rng.gen_range(0.0..2500.0), rng.gen_range(0.0..2000.0));
        ex.get_tables_mut()
            .get_mut("devices")
            .unwrap()
            .rows
            .push(Row::new(
                vec!["id".into(), "geom".into()],
                vec![
                    Value::Integer(id),
                    Value::Geometry(GeometryValue::new(point.into(), None).unwrap()),
                ],
            ));
    }
    ex
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--test") || cfg!(debug_assertions) {
        println!("prepared_geometry: run cargo bench --bench prepared_geometry for timings");
        return;
    }
    let runs = args
        .windows(2)
        .find(|w| w[0] == "--runs")
        .map(|w| w[1].parse::<usize>().expect("positive --runs integer"))
        .unwrap_or(3)
        .max(1);
    let mut ex = fixture();
    let query = QueryParser::parse(
        "SELECT d.id, z.id FROM devices d JOIN zones z ON ST_Contains(z.geom, d.geom)",
    )
    .unwrap();
    let params = QueryParams::new();
    let mut expected = None;
    for indexed in [false, true] {
        if indexed {
            ex.execute(
                &QueryParser::parse("CREATE INDEX zones_geom ON zones USING gist (geom)").unwrap(),
                &params,
            )
            .unwrap();
        }
        let mut times = Vec::new();
        for _ in 0..runs {
            let start = Instant::now();
            let result = black_box(ex.execute(black_box(&query), &params).unwrap());
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            // Include both IDs so identical row counts alone cannot hide wrong assignments.
            let fingerprint = (
                result.rows.len(),
                result
                    .rows
                    .iter()
                    .map(|row| {
                        let d = row.values[0].as_i64().unwrap() as u64;
                        let z = row.values[1].as_i64().unwrap() as u64;
                        d.wrapping_mul(1_000_003).wrapping_add(z)
                    })
                    .sum::<u64>(),
            );
            assert_eq!(*expected.get_or_insert(fingerprint), fingerprint);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "{}: median_ms={:.3}, min_ms={:.3}, samples_ms={times:?}, rows={}, checksum={}",
            if indexed { "gist_join" } else { "scan_join" },
            times[times.len() / 2],
            times[0],
            expected.unwrap().0,
            expected.unwrap().1
        );
    }
}
