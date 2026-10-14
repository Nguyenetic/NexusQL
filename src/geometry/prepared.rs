//! Bounded, statement-owned prepared geometry. No GEOS/PostGIS code is used.

use super::GeometryValue;
use geo::indexed::IntervalTreeMultiPolygon;
use geo::orient::Direction;
use geo::{
    Contains, CoordsIter, Intersects, LinesIter, Orient, PreparedGeometry, Relate, Validation,
};
use geo_types::{Geometry, MultiPolygon, Point};
use rstar::{primitives::Line, RTree, AABB};
use std::cell::OnceCell;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub enum Predicate {
    Contains,
    Within,
    Intersects,
    Covers,
    CoveredBy,
}

impl Predicate {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "ST_CONTAINS" => Some(Self::Contains),
            "ST_WITHIN" => Some(Self::Within),
            "ST_INTERSECTS" => Some(Self::Intersects),
            "ST_COVERS" => Some(Self::Covers),
            "ST_COVEREDBY" => Some(Self::CoveredBy),
            _ => None,
        }
    }

    /// Original exact algorithms, also used when preparation is inapplicable.
    pub fn unprepared(self, a: &GeometryValue, b: &GeometryValue) -> Option<bool> {
        if a.srid.unwrap_or(0) != b.srid.unwrap_or(0) {
            return None;
        }
        Some(match self {
            Self::Contains => a.shape().contains(b.shape()),
            Self::Within => b.shape().contains(a.shape()),
            Self::Intersects => a.shape().intersects(b.shape()),
            Self::Covers => a.shape().relate(b.shape()).is_covers(),
            Self::CoveredBy => a.shape().relate(b.shape()).is_coveredby(),
        })
    }
}

struct Entry {
    // Retaining the source makes its shared allocation a collision-free key:
    // the address cannot be reused while this entry is alive.
    _source: GeometryValue,
    valid: bool,
    prepared: OnceCell<PreparedGeometry<'static, Geometry<f64>>>,
    points: OnceCell<PointIndex>,
}

// geo's general PreparedGeometry reuses its topology graph, but a point query
// still labels that graph. Its interval index avoids that work. A separate edge
// R-tree supplies the inclusive boundary test that the interval API omits.
struct PointIndex {
    interior: IntervalTreeMultiPolygon<f64>,
    boundary: RTree<Line<[f64; 2]>>,
}

impl PointIndex {
    fn new(g: &GeometryValue) -> Self {
        let polygons = match g.shape() {
            Geometry::Polygon(p) => MultiPolygon(vec![p.clone()]),
            Geometry::MultiPolygon(p) => p.clone(),
            _ => unreachable!(),
        };
        // The original point predicates ignore ring winding. The combined
        // interval index needs opposite shell/hole windings to subtract holes.
        let polygons = polygons.orient(Direction::Default);
        let boundary = RTree::bulk_load(
            polygons
                .lines_iter()
                .map(|line| Line::new([line.start.x, line.start.y], [line.end.x, line.end.y]))
                .collect(),
        );
        Self {
            interior: IntervalTreeMultiPolygon::new(&polygons),
            boundary,
        }
    }

    fn evaluate(&self, predicate: Predicate, point: &Point<f64>) -> bool {
        let envelope = AABB::from_point([point.x(), point.y()]);
        let boundary = self
            .boundary
            .locate_in_envelope_intersecting(envelope)
            .any(|edge| geo_types::Line::new(edge.from, edge.to).intersects(point));
        if boundary {
            return !matches!(predicate, Predicate::Contains);
        }
        self.interior.contains(point)
    }
}

impl Entry {
    fn evaluate(&self, predicate: Predicate, other: &GeometryValue) -> Option<bool> {
        if !self.valid
            || (matches!(predicate, Predicate::Contains)
                && matches!(other.shape(), Geometry::MultiPoint(_)))
        {
            // geo uses a specialized MultiPoint containment algorithm rather
            // than Relate. Preserve that path's existing semantics as well.
            return None;
        }
        if let Geometry::Point(point) = other.shape() {
            return Some(
                self.points
                    .get_or_init(|| PointIndex::new(&self._source))
                    .evaluate(predicate, point),
            );
        }
        let relation = self
            .prepared
            .get_or_init(|| PreparedGeometry::from(self._source.shape().clone()))
            .relate(other.shape());
        Some(match predicate {
            Predicate::Contains => relation.is_contains(),
            Predicate::Covers => relation.is_covers(),
            Predicate::Intersects => relation.is_intersects(),
            _ => unreachable!(),
        })
    }
}

/// Admission is bounded by both entry count and total coordinates. Once full,
/// new shapes use the original algorithms rather than repeatedly evicting and
/// rebuilding the same join side. Dropping the cache releases all preparation.
pub struct PreparedGeometryCache {
    entries: HashMap<usize, Entry>,
    coordinates: usize,
    max_entries: usize,
    max_coordinates: usize,
}

impl Default for PreparedGeometryCache {
    fn default() -> Self {
        Self::new(1024, 262_144)
    }
}

impl PreparedGeometryCache {
    pub fn new(max_entries: usize, max_coordinates: usize) -> Self {
        Self {
            entries: HashMap::new(),
            coordinates: 0,
            max_entries,
            max_coordinates,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn coordinate_count(&self) -> usize {
        self.coordinates
    }

    fn prepare(&mut self, g: &GeometryValue) -> Option<&Entry> {
        if !matches!(g.shape(), Geometry::Polygon(_) | Geometry::MultiPolygon(_)) {
            return None;
        }
        let key = g.identity();
        if !self.entries.contains_key(&key) {
            if self.entries.len() >= self.max_entries {
                return None;
            }
            let count = g.shape().coords_count();
            if count > self.max_coordinates.saturating_sub(self.coordinates) {
                return None;
            }
            self.entries.insert(
                key,
                Entry {
                    _source: g.clone(),
                    // Invalid inputs retain the original algorithms, whose
                    // winding/boundary behavior may differ from DE-9IM.
                    valid: g.shape().is_valid(),
                    prepared: OnceCell::new(),
                    points: OnceCell::new(),
                },
            );
            self.coordinates += count;
        }
        self.entries.get(&key)
    }

    pub fn evaluate(
        &mut self,
        predicate: Predicate,
        a: &GeometryValue,
        b: &GeometryValue,
    ) -> Option<bool> {
        if a.srid.unwrap_or(0) != b.srid.unwrap_or(0) {
            return None;
        }
        // Only reject disjoint, nonempty bounds. Empty/degenerate geometry
        // behavior stays with the original algorithms.
        if let (Some(a), Some(b)) = (a.bounds(), b.bounds()) {
            if !a.intersects(&b) {
                return Some(false);
            }
        } else {
            return predicate.unprepared(a, b);
        }
        let (predicate, a, b) = match predicate {
            Predicate::Within => (Predicate::Contains, b, a),
            Predicate::CoveredBy => (Predicate::Covers, b, a),
            predicate => (predicate, a, b),
        };
        if let Some(entry) = self.prepare(a) {
            if let Some(result) = entry.evaluate(predicate, b) {
                return Some(result);
            }
        }
        if matches!(predicate, Predicate::Intersects) {
            if let Some(entry) = self.prepare(b) {
                if let Some(result) = entry.evaluate(predicate, a) {
                    return Some(result);
                }
            }
        }
        predicate.unprepared(a, b)
    }
}
