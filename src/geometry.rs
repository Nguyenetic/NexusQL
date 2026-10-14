//! Two-dimensional SQL geometry, WGS84 projections and geographic measurements.

mod geodesic;
mod geojson;
pub mod prepared;
mod topology;
mod transform;

use crate::types::Value;
use geo::{
    Area, BoundingRect, Centroid, Contains, CoordsIter, Distance, Euclidean, Intersects, Relate,
};
use geo_types::{Geometry, LineString, Point, Rect};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use wkt::{ToWkt, TryFromWkt};

/// A geometry with optional coordinate-system metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct GeometryValue {
    shape: Arc<Geometry<f64>>,
    pub srid: Option<i32>,
    bounds: Option<Rect<f64>>,
    coordinate_count: usize,
}

// Keep the existing two-field wire format. Derived caches and shared allocation
// identity are rebuilt on deserialization, including when reopening old tables.
impl Serialize for GeometryValue {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut value = serializer.serialize_struct("GeometryValue", 2)?;
        value.serialize_field("shape", self.shape())?;
        value.serialize_field("srid", &self.srid)?;
        value.end()
    }
}

impl<'de> Deserialize<'de> for GeometryValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename = "GeometryValue")]
        struct Stored {
            shape: Geometry<f64>,
            srid: Option<i32>,
        }
        let stored = Stored::deserialize(deserializer)?;
        Self::new(stored.shape, stored.srid).map_err(serde::de::Error::custom)
    }
}

impl GeometryValue {
    pub fn from_wkt(text: &str) -> Result<Self, String> {
        let text = text.trim();
        let (text, srid) = if text
            .get(..5)
            .is_some_and(|s| s.eq_ignore_ascii_case("SRID="))
        {
            let (prefix, wkt) = text.split_once(';').ok_or("Expected SRID=n;WKT")?;
            (
                wkt,
                Some(prefix[5..].parse::<i32>().map_err(|_| "Invalid SRID")?),
            )
        } else {
            (text, None)
        };
        // geo-types is 2D. Reject higher dimensions rather than silently losing them.
        let header = text.split('(').next().unwrap_or(text).to_ascii_uppercase();
        if header
            .split_whitespace()
            .any(|s| matches!(s, "Z" | "M" | "ZM"))
        {
            return Err("Only two-dimensional WKT is supported".into());
        }
        let shape = Geometry::try_from_wkt_str(text).map_err(|e| e.to_string())?;
        Self::new(shape, srid)
    }

    pub fn new(shape: Geometry<f64>, srid: Option<i32>) -> Result<Self, String> {
        if shape
            .coords_iter()
            .any(|c| !c.x.is_finite() || !c.y.is_finite())
        {
            return Err("Geometry coordinates must be finite".into());
        }
        let bounds = shape.bounding_rect();
        let coordinate_count = shape.coords_count();
        Ok(Self {
            shape: Arc::new(shape),
            srid,
            bounds,
            coordinate_count,
        })
    }

    pub fn shape(&self) -> &Geometry<f64> {
        &self.shape
    }

    pub fn as_text(&self) -> String {
        self.shape.wkt_string()
    }

    pub fn as_sql(&self) -> String {
        format!(
            "ST_GeomFromText('{}', {})",
            self.as_text(),
            self.srid.unwrap_or(0)
        )
    }

    pub fn bounds(&self) -> Option<Rect<f64>> {
        self.bounds
    }

    pub fn estimated_size(&self) -> usize {
        self.coordinate_count * 16 + std::mem::size_of::<Self>()
    }

    pub(crate) fn identity(&self) -> usize {
        Arc::as_ptr(&self.shape) as usize
    }
}

impl Hash for GeometryValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.srid.hash(state);
        for c in self.shape.coords_iter() {
            // Floating-point equality treats -0 and +0 as equal.
            for value in [c.x, c.y] {
                (if value == 0.0 { 0 } else { value.to_bits() }).hash(state);
            }
        }
    }
}

pub(crate) fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Integer(i) => Some(*i as f64),
        Value::Float(f) if f.is_finite() => Some(f.0),
        _ => None,
    }
}

pub(crate) fn geometry(value: &Value) -> Option<std::borrow::Cow<'_, GeometryValue>> {
    match value {
        Value::Geometry(g) => Some(std::borrow::Cow::Borrowed(g)),
        Value::Text(s) => GeometryValue::from_wkt(s).ok().map(std::borrow::Cow::Owned),
        _ => None,
    }
}

fn value(shape: Geometry<f64>, srid: Option<i32>) -> Option<Value> {
    GeometryValue::new(shape, srid).ok().map(Value::Geometry)
}

/// SQL scalar evaluation follows the evaluator's existing NULL-on-invalid-input
/// convention. Assignment of malformed WKT to a GEOMETRY column is an error.
pub(crate) fn evaluate(name: &str, args: &[Value]) -> Value {
    evaluate_inner(name, args).unwrap_or(Value::Null)
}

/// SQL aggregate union ignores NULL rows, but an invalid non-NULL input or SRID
/// mismatch invalidates the whole result. An empty/all-NULL group returns NULL.
pub(crate) fn union_aggregate(values: impl Iterator<Item = Value>) -> Value {
    let mut result: Option<GeometryValue> = None;
    for input in values.filter(|v| !v.is_null()) {
        let Some(g) = geometry(&input) else {
            return Value::Null;
        };
        let shape = if let Some(previous) = &result {
            if previous.srid.unwrap_or(0) != g.srid.unwrap_or(0) {
                return Value::Null;
            }
            topology::union(previous.shape(), g.shape())
        } else {
            topology::union(
                g.shape(),
                &Geometry::GeometryCollection(geo_types::GeometryCollection::empty()),
            )
        };
        let Some(shape) = shape else {
            return Value::Null;
        };
        let Ok(merged) = GeometryValue::new(shape, g.srid) else {
            return Value::Null;
        };
        result = Some(merged);
    }
    result.map(Value::Geometry).unwrap_or(Value::Null)
}

fn evaluate_inner(name: &str, args: &[Value]) -> Option<Value> {
    if args.iter().any(Value::is_null) {
        return None;
    }
    match name {
        "ST_GEOMFROMTEXT" if matches!(args.len(), 1 | 2) => {
            let mut g = GeometryValue::from_wkt(args[0].as_str()?).ok()?;
            if args.len() == 2 {
                g.srid = Some(i32::try_from(args[1].as_i64()?).ok()?);
            }
            Some(Value::Geometry(g))
        }
        "ST_POINT" if args.len() == 2 => value(
            Point::new(number(&args[0])?, number(&args[1])?).into(),
            None,
        ),
        "ST_SETSRID" | "ST_TRANSFORM" if args.len() == 2 => {
            let mut g = geometry(&args[0])?.into_owned();
            let srid = i32::try_from(args[1].as_i64()?).ok()?;
            if name == "ST_TRANSFORM" {
                g = transform::reproject(&g, srid)?;
            } else {
                g.srid = Some(srid);
            }
            Some(Value::Geometry(g))
        }
        "ST_DISTANCESPHERE" | "ST_DISTANCESPHEROID" if matches!(args.len(), 2 | 3) => {
            geodesic::evaluate(name, args).map(Value::from)
        }
        "ST_GEOMFROMGEOJSON" if args.len() == 1 => {
            geojson::from_json(args[0].as_str()?).map(Value::Geometry)
        }
        "ST_BUFFER" if args.len() == 2 => {
            let g = geometry(&args[0])?;
            value(topology::buffer(g.shape(), number(&args[1])?)?, g.srid)
        }
        "ST_MAKEENVELOPE" if matches!(args.len(), 4 | 5) => {
            let (xmin, ymin, xmax, ymax) = (
                number(&args[0])?,
                number(&args[1])?,
                number(&args[2])?,
                number(&args[3])?,
            );
            if xmin > xmax || ymin > ymax {
                return None;
            }
            let srid = if args.len() == 5 {
                Some(i32::try_from(args[4].as_i64()?).ok()?)
            } else {
                None
            };
            value(
                Rect::new((xmin, ymin), (xmax, ymax)).to_polygon().into(),
                srid,
            )
        }
        "ST_ASTEXT" | "ST_SRID" | "ST_X" | "ST_Y" | "ST_AREA" | "ST_CENTROID" | "ST_ENVELOPE"
        | "ST_ASGEOJSON" | "ST_ISVALID" | "ST_MAKEVALID" | "ST_LENGTH" | "ST_PERIMETER"
            if args.len() == 1 =>
        {
            let g = geometry(&args[0])?;
            match name {
                "ST_ASTEXT" => Some(Value::Text(g.as_text())),
                "ST_ASGEOJSON" => Some(Value::Text(geojson::as_json(&g))),
                "ST_ISVALID" => Some(Value::Boolean(topology::is_valid(g.shape()))),
                "ST_MAKEVALID" => value(topology::make_valid(g.shape())?, g.srid),
                "ST_LENGTH" => Some(Value::from(topology::length(g.shape()))),
                "ST_PERIMETER" => Some(Value::from(topology::perimeter(g.shape()))),
                "ST_SRID" => Some(Value::Integer(g.srid.unwrap_or(0) as i64)),
                "ST_X" | "ST_Y" => match g.shape() {
                    Geometry::Point(p) => {
                        Some(Value::from(if name == "ST_X" { p.x() } else { p.y() }))
                    }
                    _ => None,
                },
                "ST_AREA" => Some(Value::from(g.shape.unsigned_area())),
                "ST_CENTROID" => value(g.shape.centroid()?.into(), g.srid),
                "ST_ENVELOPE" => {
                    let Some(b) = g.bounds() else {
                        return Some(Value::Geometry(g.into_owned()));
                    };
                    let shape = if b.min() == b.max() {
                        Point(b.min()).into()
                    } else if b.width() == 0.0 || b.height() == 0.0 {
                        LineString::new(vec![b.min(), b.max()]).into()
                    } else {
                        b.to_polygon().into()
                    };
                    value(shape, g.srid)
                }
                _ => None,
            }
        }
        "ST_CONTAINS" | "ST_WITHIN" | "ST_INTERSECTS" | "ST_DISTANCE" | "ST_DWITHIN"
        | "ST_COVERS" | "ST_COVEREDBY" | "ST_UNION" | "ST_CLOSESTPOINT"
            if args.len() == if name == "ST_DWITHIN" { 3 } else { 2 } =>
        {
            let a = geometry(&args[0])?;
            let b = geometry(&args[1])?;
            if a.srid.unwrap_or(0) != b.srid.unwrap_or(0) {
                return None;
            }
            match name {
                "ST_CONTAINS" => Some(Value::Boolean(a.shape().contains(b.shape()))),
                "ST_WITHIN" => Some(Value::Boolean(b.shape().contains(a.shape()))),
                "ST_INTERSECTS" => Some(Value::Boolean(a.shape().intersects(b.shape()))),
                "ST_COVERS" => Some(Value::Boolean(a.shape().relate(b.shape()).is_covers())),
                "ST_COVEREDBY" => Some(Value::Boolean(a.shape().relate(b.shape()).is_coveredby())),
                "ST_UNION" => value(topology::union(a.shape(), b.shape())?, a.srid),
                "ST_CLOSESTPOINT" => value(
                    topology::closest_point(a.shape(), b.shape())?.into(),
                    a.srid,
                ),
                _ => {
                    if a.bounds().is_none() || b.bounds().is_none() {
                        return if name == "ST_DWITHIN" {
                            Some(Value::Boolean(false))
                        } else {
                            None
                        };
                    }
                    let distance = Euclidean.distance(a.shape(), b.shape());
                    if name == "ST_DWITHIN" {
                        let radius = number(&args[2])?;
                        Some(Value::Boolean(radius >= 0.0 && distance <= radius))
                    } else {
                        Some(Value::from(distance))
                    }
                }
            }
        }
        _ => None,
    }
}
