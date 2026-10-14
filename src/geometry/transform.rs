//! WGS84 transformations, with no system PROJ dependency. proj4rs uses the
//! sixth-order Poder/Engsager series for UTM (not the approximate Snyder series).

use super::GeometryValue;
use geo::MapCoords;
use geo_types::Coord;
use proj4rs::{transform::transform, Proj};

fn projection(srid: i32) -> Option<Proj> {
    let definition = match srid {
        4326 => "+proj=longlat +datum=WGS84".to_owned(),
        // Web Mercator is spherical, while its geographic coordinates are WGS84.
        3857 => "+proj=merc +a=6378137 +b=6378137 +units=m".to_owned(),
        32601..=32660 => format!("+proj=utm +zone={} +datum=WGS84 +units=m", srid - 32600),
        32701..=32760 => format!(
            "+proj=utm +zone={} +south +datum=WGS84 +units=m",
            srid - 32700
        ),
        _ => return None,
    };
    Proj::from_proj_string(&definition).ok()
}

pub(super) fn reproject(g: &GeometryValue, target_srid: i32) -> Option<GeometryValue> {
    let source_srid = g.srid?;
    let source = projection(source_srid)?;
    let target = projection(target_srid)?;
    let shape = g
        .shape()
        .try_map_coords(|c: Coord<f64>| -> Result<Coord<f64>, ()> {
            if source_srid == 4326 && (c.x.abs() > 180.0 || c.y.abs() > 90.0) {
                return Err(());
            }
            let mut point = if source_srid == 4326 {
                (c.x.to_radians(), c.y.to_radians(), 0.0)
            } else {
                (c.x, c.y, 0.0)
            };
            // Mercator has no finite value at the poles. Validate even an identity
            // transform through the projection engine rather than copying bad data.
            transform(&source, &target, &mut point).map_err(|_| ())?;
            let (x, y) = if target_srid == 4326 {
                (point.0.to_degrees(), point.1.to_degrees())
            } else {
                (point.0, point.1)
            };
            if !x.is_finite() || !y.is_finite() {
                return Err(());
            }
            Ok(Coord { x, y })
        })
        .ok()?;
    GeometryValue::new(shape, Some(target_srid)).ok()
}
