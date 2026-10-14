//! Planar editing and measurements. Overlay is pure Rust (geo / i_overlay).

use geo::buffer::{BufferStyle, LineCap, LineJoin};
use geo::line_intersection::{line_intersection, LineIntersection};
use geo::orient::Direction;
use geo::{
    Area, BooleanOps, Buffer, Closest, ClosestPoint, CoordsIter, Distance, Euclidean, Intersects,
    Length, LinesIter, Orient, Validation,
};
use geo_types::{
    Coord, Geometry, GeometryCollection, Line, LineString, MultiLineString, MultiPoint,
    MultiPolygon, Point, Polygon,
};
use std::cmp::Ordering;

pub(super) fn length(g: &Geometry<f64>) -> f64 {
    match g {
        Geometry::Line(l) => Euclidean.length(l),
        Geometry::LineString(l) => Euclidean.length(l),
        Geometry::MultiLineString(m) => Euclidean.length(m),
        Geometry::GeometryCollection(c) => c.iter().map(length).sum(),
        _ => 0.0,
    }
}

pub(super) fn perimeter(g: &Geometry<f64>) -> f64 {
    match g {
        Geometry::Polygon(p) => std::iter::once(p.exterior())
            .chain(p.interiors())
            .map(|r| Euclidean.length(r))
            .sum(),
        Geometry::MultiPolygon(m) => m.iter().map(|p| perimeter(&p.clone().into())).sum(),
        Geometry::GeometryCollection(c) => c.iter().map(perimeter).sum(),
        Geometry::Rect(r) => perimeter(&r.to_polygon().into()),
        Geometry::Triangle(t) => perimeter(&t.to_polygon().into()),
        _ => 0.0,
    }
}

pub(super) fn is_valid(g: &Geometry<f64>) -> bool {
    if !g.is_valid() {
        return false;
    }
    // geo's Validation checks ring simplicity and containment, but currently
    // omits disconnected interiors. Polygonizing the even-odd interior detects
    // holes touching a shell (or each other) so as to split it into pieces.
    match g {
        Geometry::Polygon(p) if !p.exterior().0.is_empty() && p.unsigned_area() == 0.0 => false,
        Geometry::Polygon(p) if !p.exterior().0.is_empty() && !p.interiors().is_empty() => {
            p.union(&Polygon::empty()).0.len() == 1
        }
        Geometry::MultiPolygon(m) => m.iter().all(|p| is_valid(&p.clone().into())),
        Geometry::GeometryCollection(c) => c.iter().all(is_valid),
        _ => true,
    }
}

pub(super) fn polygonal(mut polygons: MultiPolygon<f64>) -> Geometry<f64> {
    match polygons.0.len() {
        0 => Polygon::empty().into(),
        1 => polygons.0.remove(0).into(),
        _ => polygons.into(),
    }
}

pub(super) fn buffer(g: &Geometry<f64>, distance: f64) -> Option<Geometry<f64>> {
    if g.coords_count() == 0 {
        return Some(Polygon::empty().into());
    }
    // Guard overflow before passing coordinates to the floating overlay adapter.
    if !distance.is_finite()
        || g.coords_iter().any(|c| {
            ![
                c.x - distance,
                c.x + distance,
                c.y - distance,
                c.y + distance,
                distance * 2.0,
            ]
            .iter()
            .all(|v| v.is_finite())
        })
    {
        return None;
    }
    let style = BufferStyle::new(distance)
        .line_cap(LineCap::Round(std::f64::consts::PI / 16.0))
        .line_join(LineJoin::Round(std::f64::consts::PI / 16.0));
    let repaired;
    let g = if is_valid(g) {
        g
    } else {
        repaired = make_valid(g)?;
        &repaired
    };
    let polygons = match g {
        // i_overlay's outline routine assumes nonempty rings. Strip empty
        // members before buffering, including empties nested in collections.
        Geometry::Polygon(p) => Polygon::new(
            p.exterior().clone(),
            p.interiors()
                .iter()
                .filter(|r| !r.0.is_empty())
                .cloned()
                .collect(),
        )
        .buffer_with_style(style),
        Geometry::MultiPolygon(m) => {
            let buffered = m
                .iter()
                .map(|p| buffer(&p.clone().into(), distance))
                .collect::<Option<Vec<_>>>()?;
            let mut parts = Parts::default();
            for child in buffered {
                parts.collect(&child);
            }
            geo::algorithm::unary_union(&parts.polygons)
        }
        Geometry::GeometryCollection(c) => {
            let buffered = c
                .iter()
                .map(|g| buffer(g, distance))
                .collect::<Option<Vec<_>>>()?;
            let mut parts = Parts::default();
            for child in buffered {
                parts.collect(&child);
            }
            geo::algorithm::unary_union(&parts.polygons)
        }
        _ => g.buffer_with_style(style),
    };
    Some(polygonal(polygons))
}

#[derive(Default)]
struct Parts {
    polygons: Vec<Polygon<f64>>,
    lines: Vec<Line<f64>>,
    points: Vec<Point<f64>>,
}

impl Parts {
    fn line_string(&mut self, l: &LineString<f64>) {
        self.points.extend(l.0.iter().copied().map(Point));
        self.lines.extend(l.lines().filter(|l| l.start != l.end));
    }

    fn collect(&mut self, g: &Geometry<f64>) {
        match g {
            Geometry::Point(p) => self.points.push(*p),
            Geometry::LineString(l) => self.line_string(l),
            Geometry::Polygon(p) => self.polygons.push(p.clone()),
            Geometry::MultiPoint(m) => self.points.extend(&m.0),
            Geometry::MultiLineString(m) => m.iter().for_each(|l| self.line_string(l)),
            Geometry::MultiPolygon(m) => self.polygons.extend(m.0.iter().cloned()),
            Geometry::GeometryCollection(c) => c.iter().for_each(|g| self.collect(g)),
            Geometry::Line(l) => self.line_string(&LineString::new(vec![l.start, l.end])),
            Geometry::Rect(r) => self.polygons.push(r.to_polygon()),
            Geometry::Triangle(t) => self.polygons.push(t.to_polygon()),
        }
    }

    fn boundary_lines(&self) -> Vec<Line<f64>> {
        self.lines
            .iter()
            .copied()
            .chain(self.polygons.iter().flat_map(|p| p.lines_iter()))
            .collect()
    }
}

fn coord_cmp(a: Coord<f64>, b: Coord<f64>) -> Ordering {
    a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y))
}

fn intersection_points(a: Line<f64>, b: Line<f64>) -> Vec<Coord<f64>> {
    match line_intersection(a, b) {
        Some(LineIntersection::SinglePoint { intersection, .. }) => vec![intersection],
        Some(LineIntersection::Collinear { intersection }) => {
            vec![intersection.start, intersection.end]
        }
        None => vec![],
    }
}

// Node every segment at crossings and overlap endpoints. Polygon boundaries are
// cutters as well, so midpoint classification can remove only the covered part.
fn node(lines: &[Line<f64>], polygons: &MultiPolygon<f64>) -> Vec<Line<f64>> {
    let cutters: Vec<_> = lines.iter().copied().chain(polygons.lines_iter()).collect();
    let mut result = Vec::new();
    for line in lines {
        let mut coordinates = vec![line.start, line.end];
        for cutter in &cutters {
            coordinates.extend(intersection_points(*line, *cutter));
        }
        // Lexicographic order is monotonic along a straight segment and avoids
        // dividing by zero or squaring large coordinates to find parameters.
        coordinates.sort_by(|a, b| coord_cmp(*a, *b));
        coordinates.dedup();
        for pair in coordinates.windows(2) {
            let middle = Point::new(
                pair[0].x / 2.0 + pair[1].x / 2.0,
                pair[0].y / 2.0 + pair[1].y / 2.0,
            );
            if !polygons.intersects(&middle) {
                result.push(Line::new(pair[0], pair[1]));
            }
        }
    }
    result.sort_by(|a, b| coord_cmp(a.start, b.start).then(coord_cmp(a.end, b.end)));
    result.dedup();
    result
}

fn assemble(
    polygons: MultiPolygon<f64>,
    lines: Vec<Line<f64>>,
    mut points: Vec<Point<f64>>,
    empty: Geometry<f64>,
) -> Geometry<f64> {
    points.retain(|p| !polygons.intersects(p) && !lines.iter().any(|l| l.intersects(p)));
    points.sort_by(|a, b| coord_cmp(a.0, b.0));
    points.dedup();
    let mut geometries = Vec::new();
    if !polygons.0.is_empty() {
        geometries.push(polygonal(polygons));
    }
    let mut lines: Vec<_> = lines
        .into_iter()
        .map(|l| LineString::new(vec![l.start, l.end]))
        .collect();
    match lines.len() {
        0 => (),
        1 => geometries.push(lines.remove(0).into()),
        _ => geometries.push(MultiLineString(lines).into()),
    }
    match points.len() {
        0 => (),
        1 => geometries.push(points[0].into()),
        _ => geometries.push(MultiPoint(points).into()),
    }
    match geometries.len() {
        0 => empty,
        1 => geometries.remove(0),
        _ => Geometry::GeometryCollection(GeometryCollection(geometries)),
    }
}

pub(super) fn union(a: &Geometry<f64>, b: &Geometry<f64>) -> Option<Geometry<f64>> {
    if !is_valid(a) || !is_valid(b) {
        return None;
    }
    let mut parts = Parts::default();
    parts.collect(a);
    parts.collect(b);
    let oriented: Vec<_> = parts
        .polygons
        .iter()
        .map(|p| p.orient(Direction::Default))
        .collect();
    let polygons = geo::algorithm::unary_union(&oriented);
    let lines = node(&parts.lines, &polygons);
    let empty = if !parts.polygons.is_empty() {
        Polygon::empty().into()
    } else if matches!(a, Geometry::LineString(_) | Geometry::MultiLineString(_))
        || matches!(b, Geometry::LineString(_) | Geometry::MultiLineString(_))
    {
        LineString::new(vec![]).into()
    } else if matches!(a, Geometry::MultiPoint(_)) || matches!(b, Geometry::MultiPoint(_)) {
        MultiPoint::new(vec![]).into()
    } else {
        Geometry::GeometryCollection(GeometryCollection::empty())
    };
    Some(assemble(polygons, lines, parts.points, empty))
}

pub(super) fn make_valid(g: &Geometry<f64>) -> Option<Geometry<f64>> {
    if is_valid(g) {
        return Some(g.clone());
    }
    if let Geometry::GeometryCollection(c) = g {
        return Some(Geometry::GeometryCollection(GeometryCollection(
            c.iter().map(make_valid).collect::<Option<Vec<_>>>()?,
        )));
    }
    let mut parts = Parts::default();
    parts.collect(g);
    // Even-odd polygonization of all original rings also promotes holes outside
    // the shell, splits bowties, and resolves overlapping MultiPolygon members.
    let polygons = MultiPolygon(parts.polygons.clone()).union(&MultiPolygon::empty());
    // Keep collapsed polygon edges and isolated vertices, rather than losing
    // them through a buffer(0). The result may have a lower dimension.
    let boundaries = parts.boundary_lines();
    parts.points.extend(g.coords_iter().map(Point));
    let lines = node(&boundaries, &polygons);
    let result = assemble(
        polygons,
        lines,
        parts.points,
        Geometry::GeometryCollection(GeometryCollection::empty()),
    );
    is_valid(&result).then_some(result)
}

pub(super) fn closest_point(a: &Geometry<f64>, b: &Geometry<f64>) -> Option<Point<f64>> {
    let mut left = Parts::default();
    let mut right = Parts::default();
    left.collect(a);
    right.collect(b);
    let left_lines = left.boundary_lines();
    let right_lines = right.boundary_lines();
    // Intersecting segments can have no vertex in the intersection (e.g. a +).
    for l in &left_lines {
        for r in &right_lines {
            if let Some(p) = intersection_points(*l, *r).first() {
                return Some(Point(*p));
            }
        }
    }
    let mut best = None;
    let mut best_distance = f64::INFINITY;
    let mut consider = |p: Point<f64>, q: Point<f64>| {
        let distance = Euclidean.distance(p, q);
        if distance < best_distance {
            best_distance = distance;
            best = Some(p);
        }
    };
    // A non-crossing minimum lies at a vertex of at least one input. Always
    // return the coordinate on a, even when the nearest vertex belongs to b.
    for p in a.coords_iter().map(Point) {
        match b.closest_point(&p) {
            Closest::Intersection(q) | Closest::SinglePoint(q) => consider(p, q),
            Closest::Indeterminate => (),
        }
        for q in b.coords_iter().map(Point) {
            consider(p, q);
        }
    }
    for q in b.coords_iter().map(Point) {
        match a.closest_point(&q) {
            Closest::Intersection(p) | Closest::SinglePoint(p) => consider(p, q),
            Closest::Indeterminate => (),
        }
    }
    best
}
