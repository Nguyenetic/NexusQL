//! GeoJSON geometry fragments. Keep the engine's strict, finite, 2D contract.

use super::GeometryValue;
use geo_types::{
    Coord, Geometry, GeometryCollection, LineString, MultiLineString, MultiPoint, MultiPolygon,
    Point, Polygon,
};
use serde_json::{json, Value};

fn coord(c: Coord<f64>) -> Value {
    json!([c.x, c.y])
}

fn line(l: &LineString<f64>) -> Value {
    Value::Array(l.0.iter().copied().map(coord).collect())
}

fn polygon(p: &Polygon<f64>) -> Value {
    if p.exterior().0.is_empty() {
        json!([])
    } else {
        Value::Array(
            std::iter::once(p.exterior())
                .chain(p.interiors())
                .map(line)
                .collect(),
        )
    }
}

fn encode(g: &Geometry<f64>) -> Value {
    let (kind, coordinates) = match g {
        Geometry::Point(p) => ("Point", coord(p.0)),
        Geometry::LineString(l) => ("LineString", line(l)),
        Geometry::Polygon(p) => ("Polygon", polygon(p)),
        Geometry::MultiPoint(m) => (
            "MultiPoint",
            Value::Array(m.0.iter().map(|p| coord(p.0)).collect()),
        ),
        Geometry::MultiLineString(m) => (
            "MultiLineString",
            Value::Array(m.0.iter().map(line).collect()),
        ),
        Geometry::MultiPolygon(m) => (
            "MultiPolygon",
            Value::Array(m.0.iter().map(polygon).collect()),
        ),
        Geometry::GeometryCollection(c) => {
            return json!({"type": "GeometryCollection", "geometries": c.0.iter().map(encode).collect::<Vec<_>>()})
        }
        Geometry::Line(l) => ("LineString", json!([coord(l.start), coord(l.end)])),
        Geometry::Rect(r) => ("Polygon", polygon(&r.to_polygon())),
        Geometry::Triangle(t) => ("Polygon", polygon(&t.to_polygon())),
    };
    json!({"type": kind, "coordinates": coordinates})
}

pub(super) fn as_json(g: &GeometryValue) -> String {
    let mut object = encode(g.shape());
    // Match ST_AsGeoJSON's default short-CRS behavior for non-WGS84 data.
    if let Some(srid) = g.srid.filter(|s| !matches!(s, 0 | 4326)) {
        object["crs"] = json!({"type": "name", "properties": {"name": format!("EPSG:{srid}")}});
    }
    object.to_string()
}

fn parse_coord(value: &Value) -> Option<Coord<f64>> {
    let coordinates = value.as_array()?;
    if coordinates.len() != 2 {
        return None;
    }
    let (x, y) = (coordinates[0].as_f64()?, coordinates[1].as_f64()?);
    (x.is_finite() && y.is_finite()).then_some(Coord { x, y })
}

fn parse_line(value: &Value) -> Option<LineString<f64>> {
    let coords = value
        .as_array()?
        .iter()
        .map(parse_coord)
        .collect::<Option<Vec<_>>>()?;
    if coords.len() == 1 {
        return None;
    }
    Some(LineString::new(coords))
}

fn parse_polygon(value: &Value) -> Option<Polygon<f64>> {
    let rings = value
        .as_array()?
        .iter()
        .map(parse_line)
        .collect::<Option<Vec<_>>>()?;
    if rings.is_empty() {
        return Some(Polygon::empty());
    }
    // geo-types closes rings automatically. GeoJSON requires explicit closure;
    // reject malformed rings instead of quietly adding a coordinate.
    if rings
        .iter()
        .any(|r| r.0.len() < 4 || r.0.first() != r.0.last())
    {
        return None;
    }
    let mut rings = rings.into_iter();
    Some(Polygon::new(rings.next()?, rings.collect()))
}

fn decode(object: &Value) -> Option<Geometry<f64>> {
    let coordinates = &object["coordinates"];
    Some(match object["type"].as_str()? {
        "Point" => Point(parse_coord(coordinates)?).into(),
        "LineString" => parse_line(coordinates)?.into(),
        "Polygon" => parse_polygon(coordinates)?.into(),
        "MultiPoint" => MultiPoint(
            coordinates
                .as_array()?
                .iter()
                .map(|v| parse_coord(v).map(Point))
                .collect::<Option<Vec<_>>>()?,
        )
        .into(),
        "MultiLineString" => MultiLineString(
            coordinates
                .as_array()?
                .iter()
                .map(parse_line)
                .collect::<Option<Vec<_>>>()?,
        )
        .into(),
        "MultiPolygon" => MultiPolygon(
            coordinates
                .as_array()?
                .iter()
                .map(parse_polygon)
                .collect::<Option<Vec<_>>>()?,
        )
        .into(),
        "GeometryCollection" => Geometry::GeometryCollection(GeometryCollection(
            object["geometries"]
                .as_array()?
                .iter()
                .map(decode)
                .collect::<Option<Vec<_>>>()?,
        )),
        // Features and FeatureCollections are not geometry fragments.
        _ => return None,
    })
}

pub(super) fn from_json(text: &str) -> Option<GeometryValue> {
    let object: Value = serde_json::from_str(text).ok()?;
    let srid = match object.get("crs") {
        None | Some(Value::Null) => 4326,
        Some(crs) => {
            if crs["type"].as_str()? != "name" {
                return None;
            }
            let name = crs["properties"]["name"].as_str()?;
            let code = name
                .strip_prefix("EPSG:")
                .or_else(|| name.strip_prefix("urn:ogc:def:crs:EPSG::"))?;
            code.parse::<i32>().ok()?
        }
    };
    GeometryValue::new(decode(&object)?, Some(srid)).ok()
}
