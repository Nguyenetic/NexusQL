//! Geographic distances. Edges are minor great-circle arcs; closest approaches
//! are measured with a spherical radius or Karney's ellipsoidal inverse solution.

use super::{geometry, number};
use crate::types::Value;
use geo::{CoordsIter, Distance, Geodesic, GeodesicMeasure};
use geo_types::{Geometry, LineString, Point, Polygon};

// Angular tolerance is below a tenth of a micrometre on Earth. Working in 3D
// avoids longitude discontinuities at the antimeridian and singularities at poles.
const ANGULAR_EPSILON: f64 = 1e-14;

#[derive(Clone, Copy)]
struct Vector([f64; 3]);

impl Vector {
    fn from_point(p: Point<f64>) -> Self {
        let (sin_lon, cos_lon) = p.x().to_radians().sin_cos();
        let (sin_lat, cos_lat) = p.y().to_radians().sin_cos();
        Self([cos_lat * cos_lon, cos_lat * sin_lon, sin_lat])
    }

    fn point(self) -> Point<f64> {
        Point::new(
            self.0[1].atan2(self.0[0]).to_degrees(),
            self.0[2].atan2(self.0[0].hypot(self.0[1])).to_degrees(),
        )
    }

    fn dot(self, b: Self) -> f64 {
        self.0.iter().zip(b.0).map(|(x, y)| x * y).sum()
    }

    fn cross(self, b: Self) -> Self {
        Self([
            self.0[1] * b.0[2] - self.0[2] * b.0[1],
            self.0[2] * b.0[0] - self.0[0] * b.0[2],
            self.0[0] * b.0[1] - self.0[1] * b.0[0],
        ])
    }

    fn norm(self) -> f64 {
        self.dot(self).sqrt()
    }

    fn scale(self, k: f64) -> Self {
        Self(self.0.map(|x| x * k))
    }

    fn subtract(self, b: Self) -> Self {
        Self([self.0[0] - b.0[0], self.0[1] - b.0[1], self.0[2] - b.0[2]])
    }

    fn unit(self) -> Option<Self> {
        let norm = self.norm();
        (norm > ANGULAR_EPSILON).then(|| self.scale(1.0 / norm))
    }

    fn angle(self, b: Self) -> f64 {
        self.cross(b).norm().atan2(self.dot(b))
    }
}

#[derive(Clone, Copy)]
struct Arc {
    start: Vector,
    end: Vector,
    normal: Option<Vector>,
}

impl Arc {
    fn new(start: Vector, end: Vector) -> Option<Self> {
        let normal = start.cross(end).unit();
        // Antipodal endpoints have no unique minor arc. Coincident endpoints
        // remain valid and are handled as a point.
        if normal.is_none() && start.dot(end) < 0.0 {
            return None;
        }
        Some(Self { start, end, normal })
    }

    fn contains(self, point: Vector) -> bool {
        let Some(normal) = self.normal else {
            return self.start.angle(point) <= ANGULAR_EPSILON;
        };
        normal.dot(point).abs() <= ANGULAR_EPSILON
            && self.start.cross(point).dot(normal) >= -ANGULAR_EPSILON
            && point.cross(self.end).dot(normal) >= -ANGULAR_EPSILON
    }

    fn closest(self, point: Vector) -> Vector {
        if let Some(normal) = self.normal {
            if let Some(projected) = point.subtract(normal.scale(point.dot(normal))).unit() {
                if self.contains(projected) {
                    return projected;
                }
            }
        }
        if point.angle(self.start) <= point.angle(self.end) {
            self.start
        } else {
            self.end
        }
    }

    fn intersects(self, other: Self) -> bool {
        if self.contains(other.start)
            || self.contains(other.end)
            || other.contains(self.start)
            || other.contains(self.end)
        {
            return true;
        }
        let Some(crossing) = self
            .normal
            .and_then(|n| other.normal.and_then(|m| n.cross(m).unit()))
        else {
            return false;
        };
        [crossing, crossing.scale(-1.0)]
            .into_iter()
            .any(|p| self.contains(p) && other.contains(p))
    }
}

struct Ring {
    vertices: Vec<Vector>,
    // The sign of the smaller region's spherical area disambiguates the
    // antipodal winding; taking abs(winding) alone would include both poles.
    area: f64,
}

impl Ring {
    fn new(line: &LineString<f64>) -> Self {
        let vertices: Vec<_> = line
            .0
            .iter()
            .map(|c| Vector::from_point(Point(*c)))
            .collect();
        let mut area = 0.0;
        if let Some(origin) = vertices.first() {
            for edge in vertices[1..].windows(2) {
                area += 2.0
                    * origin.dot(edge[0].cross(edge[1])).atan2(
                        1.0 + origin.dot(edge[0]) + edge[0].dot(edge[1]) + edge[1].dot(*origin),
                    );
            }
        }
        let half_sphere = std::f64::consts::TAU;
        if area.abs() > half_sphere {
            area = (area + half_sphere).rem_euclid(2.0 * half_sphere) - half_sphere;
        }
        Self { vertices, area }
    }

    // 0 outside, 1 inside, 2 on the boundary.
    fn position(&self, point: Vector) -> u8 {
        let mut winding = 0.0;
        for edge in self.vertices.windows(2) {
            let Some(arc) = Arc::new(edge[0], edge[1]) else {
                return 0;
            };
            if arc.contains(point) {
                return 2;
            }
            winding += point
                .dot(edge[0].cross(edge[1]))
                .atan2(edge[0].dot(edge[1]) - point.dot(edge[0]) * point.dot(edge[1]));
        }
        u8::from(winding.abs() > std::f64::consts::PI && winding * self.area > 0.0)
    }
}

#[derive(Default)]
struct Geography {
    vertices: Vec<Vector>,
    edges: Vec<Arc>,
    polygons: Vec<Vec<Ring>>,
}

impl Geography {
    fn line(&mut self, line: &LineString<f64>) -> Option<()> {
        let vertices: Vec<_> = line
            .0
            .iter()
            .map(|c| Vector::from_point(Point(*c)))
            .collect();
        for pair in vertices.windows(2) {
            self.edges.push(Arc::new(pair[0], pair[1])?);
        }
        self.vertices.extend(vertices);
        Some(())
    }

    fn polygon(&mut self, polygon: &Polygon<f64>) -> Option<()> {
        let mut rings = Vec::new();
        for ring in std::iter::once(polygon.exterior()).chain(polygon.interiors()) {
            self.line(ring)?;
            rings.push(Ring::new(ring));
        }
        self.polygons.push(rings);
        Some(())
    }

    fn collect(&mut self, g: &Geometry<f64>) -> Option<()> {
        match g {
            Geometry::Point(p) => self.vertices.push(Vector::from_point(*p)),
            Geometry::LineString(l) => self.line(l)?,
            Geometry::Polygon(p) => self.polygon(p)?,
            Geometry::MultiPoint(m) => self
                .vertices
                .extend(m.iter().map(|p| Vector::from_point(*p))),
            Geometry::MultiLineString(m) => {
                for line in m {
                    self.line(line)?;
                }
            }
            Geometry::MultiPolygon(m) => {
                for polygon in m {
                    self.polygon(polygon)?;
                }
            }
            Geometry::GeometryCollection(c) => {
                for child in c {
                    self.collect(child)?;
                }
            }
            Geometry::Line(l) => self.line(&LineString::new(vec![l.start, l.end]))?,
            Geometry::Rect(r) => self.polygon(&r.to_polygon())?,
            Geometry::Triangle(t) => self.polygon(&t.to_polygon())?,
        }
        Some(())
    }

    fn covers(&self, point: Vector) -> bool {
        self.polygons
            .iter()
            .any(|rings| match rings[0].position(point) {
                0 => false,
                2 => true,
                _ => !rings[1..].iter().any(|ring| ring.position(point) == 1),
            })
    }
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
    if a.shape()
        .coords_iter()
        .chain(b.shape().coords_iter())
        .any(|p| p.x.abs() > 180.0 || p.y.abs() > 90.0)
    {
        return None;
    }
    let (mut left, mut right) = (Geography::default(), Geography::default());
    left.collect(a.shape())?;
    right.collect(b.shape())?;
    if left.vertices.is_empty() || right.vertices.is_empty() {
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
        Some(radius)
    } else {
        None
    };
    let custom = if name == "ST_DISTANCESPHEROID" && args.len() == 3 {
        let (radius, flattening) = spheroid(args[2].as_str()?)?;
        Some(GeodesicMeasure::new(radius, flattening))
    } else {
        None
    };
    if left
        .vertices
        .iter()
        .any(|p| right.covers(*p) || right.edges.iter().any(|edge| edge.contains(*p)))
        || right
            .vertices
            .iter()
            .any(|p| left.covers(*p) || left.edges.iter().any(|edge| edge.contains(*p)))
        || left
            .edges
            .iter()
            .any(|a| right.edges.iter().any(|b| a.intersects(*b)))
    {
        return Some(0.0);
    }
    let mut minimum = f64::INFINITY;
    let mut consider = |p: Vector, q: Vector| -> Option<()> {
        let distance = if let Some(radius) = sphere {
            radius * p.angle(q)
        } else if let Some(custom) = &custom {
            custom.distance(p.point(), q.point())
        } else {
            Geodesic.distance(p.point(), q.point())
        };
        if !distance.is_finite() {
            return None;
        }
        minimum = minimum.min(distance);
        Some(())
    };
    for p in &left.vertices {
        for q in &right.vertices {
            consider(*p, *q)?;
        }
        for arc in &right.edges {
            consider(*p, arc.closest(*p))?;
        }
    }
    for q in &right.vertices {
        for arc in &left.edges {
            consider(arc.closest(*q), *q)?;
        }
    }
    Some(minimum)
}
