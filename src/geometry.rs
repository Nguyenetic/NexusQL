//! Planar, two-dimensional SQL geometry. All geometry algorithms are georust's.

use crate::types::Value;
use geo::{Area, BoundingRect, Centroid, Contains, CoordsIter, Distance, Euclidean, Intersects};
use geo_types::{Geometry, LineString, Point, Rect};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use wkt::{ToWkt, TryFromWkt};

/// A geometry with optional coordinate-system metadata (no reprojection).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometryValue {
    shape: Geometry<f64>,
    pub srid: Option<i32>,
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
        Ok(Self { shape, srid })
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
        self.shape.bounding_rect()
    }

    pub fn estimated_size(&self) -> usize {
        self.shape.coords_count() * 16 + 16
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
            if args.len() == 1 =>
        {
            let g = geometry(&args[0])?;
            match name {
                "ST_ASTEXT" => Some(Value::Text(g.as_text())),
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
            if args.len() == if name == "ST_DWITHIN" { 3 } else { 2 } =>
        {
            let a = geometry(&args[0])?;
            let b = geometry(&args[1])?;
            if a.srid.unwrap_or(0) != b.srid.unwrap_or(0) {
                return None;
            }
            match name {
                "ST_CONTAINS" => Some(Value::Boolean(a.shape.contains(&b.shape))),
                "ST_WITHIN" => Some(Value::Boolean(b.shape.contains(&a.shape))),
                "ST_INTERSECTS" => Some(Value::Boolean(a.shape.intersects(&b.shape))),
                _ => {
                    if a.bounds().is_none() || b.bounds().is_none() {
                        return if name == "ST_DWITHIN" {
                            Some(Value::Boolean(false))
                        } else {
                            None
                        };
                    }
                    let distance = Euclidean.distance(&a.shape, &b.shape);
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
