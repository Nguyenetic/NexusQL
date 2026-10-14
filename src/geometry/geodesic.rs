//! Geographic point-set distances. Geodesic uses Karney's inverse solution,
//! including coincident and antipodal points, through geo's pure Rust backend.

use super::{geometry, number};
use crate::types::Value;
use geo::{Distance, Geodesic, GeodesicMeasure, HaversineMeasure};
use geo_types::{Geometry, Point};

fn points(g: &Geometry<f64>, out: &mut Vec<Point<f64>>) -> Option<()> {
    match g {
        Geometry::Point(p) => out.push(*p),
        Geometry::MultiPoint(m) => out.extend(&m.0),
        Geometry::GeometryCollection(c) => {
            for child in c {
                points(child, out)?;
            }
        }
        _ => return None,
    }
    Some(())
}

// Accept PostGIS's SPHEROID["name", semi_major_axis, inverse_flattening].
fn spheroid(text: &str) -> Option<(f64, f64)> {
    let (kind, fields) = text.trim().split_once('[')?;
    if !kind.trim().eq_ignore_ascii_case("SPHEROID") {
        return None;
    }
    let fields = fields.strip_suffix(']')?.trim().strip_prefix('"')?;
    let (_, fields) = fields.split_once('"')?;
    let mut fields = fields.trim().strip_prefix(',')?.split(',');
    let radius: f64 = fields.next()?.trim().parse().ok()?;
    let inverse: f64 = fields.next()?.trim().parse().ok()?;
    if fields.next().is_some()
        || !radius.is_finite()
        || radius <= 0.0
        || !inverse.is_finite()
        || (inverse != 0.0 && inverse <= 1.0)
    {
        return None;
    }
    Some((radius, if inverse == 0.0 { 0.0 } else { 1.0 / inverse }))
}

pub(super) fn evaluate(name: &str, args: &[Value]) -> Option<f64> {
    let a = geometry(&args[0])?;
    let b = geometry(&args[1])?;
    if a.srid.unwrap_or(0) != b.srid.unwrap_or(0) || !matches!(a.srid.unwrap_or(0), 0 | 4326) {
        return None;
    }
    let (mut left, mut right) = (Vec::new(), Vec::new());
    points(a.shape(), &mut left)?;
    points(b.shape(), &mut right)?;
    if left.is_empty()
        || right.is_empty()
        || left.iter().chain(&right).any(|p| {
            !p.x().is_finite() || !p.y().is_finite() || p.x().abs() > 180.0 || p.y().abs() > 90.0
        })
    {
        return None;
    }
    let sphere = if name == "ST_DISTANCESPHERE" {
        let radius = if args.len() == 3 {
            number(&args[2])?
        } else {
            // IUGG mean radius (2a+b)/3 for WGS84, also PostGIS's default.
            6_371_008.771_415_059
        };
        if radius <= 0.0 {
            return None;
        }
        Some(HaversineMeasure::new(radius))
    } else {
        None
    };
    let custom = if name == "ST_DISTANCESPHEROID" && args.len() == 3 {
        let (radius, flattening) = spheroid(args[2].as_str()?)?;
        Some(GeodesicMeasure::new(radius, flattening))
    } else {
        None
    };
    let mut minimum = f64::INFINITY;
    for p in left {
        for q in &right {
            let distance = if let Some(sphere) = &sphere {
                sphere.distance(p, *q)
            } else if let Some(custom) = &custom {
                custom.distance(p, *q)
            } else {
                Geodesic.distance(p, *q)
            };
            if !distance.is_finite() {
                return None;
            }
            minimum = minimum.min(distance);
        }
    }
    Some(minimum)
}
