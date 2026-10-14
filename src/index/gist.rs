//! GiST (Generalized Search Tree)
//!
//! PostgreSQL-compatible balanced search tree for:
//! - Geometric data (points, boxes, polygons)
//! - Range types (int4range, daterange, etc.)
//! - IP addresses (inet, cidr)
//! - Custom types with user-defined predicates
//!
//! Based on PostgreSQL GiST implementation:
//! - R-tree for spatial data
//! - Predicate-based searching
//! - Lossy compression via bounding boxes

use crate::types::Value;
use rstar::{PointDistance, RTree, RTreeObject, AABB};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

#[derive(Debug, Clone, PartialEq)]
struct SpatialEntry {
    row_id: usize,
    bounds: AABB<[f64; 2]>,
}

impl RTreeObject for SpatialEntry {
    type Envelope = AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        self.bounds
    }
}

impl PointDistance for SpatialEntry {
    fn distance_2(&self, point: &[f64; 2]) -> f64 {
        // A lower bound only: the executor rechecks the original geometry.
        self.bounds.distance_2(point)
    }
}

/// GiST Index - Generalized Search Tree
#[derive(Debug, Clone)]
pub struct GiSTIndex {
    /// Index name
    pub name: String,

    /// Column being indexed
    pub column_name: String,

    /// Root node of the tree
    root: Option<Box<GiSTNode>>,
    spatial: RTree<SpatialEntry>,
    spatial_srid: Option<i32>,
    mixed_spatial_srids: bool,
    searches: Arc<AtomicU64>,
    candidates: Arc<AtomicU64>,

    /// Index type (determines predicates and keys)
    index_type: GiSTIndexType,

    /// Total entries indexed
    pub entry_count: usize,
}

/// Type of GiST index
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GiSTIndexType {
    /// R-tree for 2D points and rectangles
    RTree,

    /// Range tree for integer/float ranges
    RangeTree,

    /// IP address tree (prefix matching)
    InetTree,
}

/// GiST Node (internal or leaf)
#[derive(Debug, Clone)]
struct GiSTNode {
    /// Node predicate (bounding box, range, etc.)
    predicate: GiSTPredicate,

    /// Child nodes (if internal) or row IDs (if leaf)
    children: Vec<GiSTEntry>,

    /// Is this a leaf node?
    is_leaf: bool,
}

/// Entry in a GiST node
#[derive(Debug, Clone)]
struct GiSTEntry {
    /// Key/predicate for this entry
    key: GiSTPredicate,

    /// Either child node or row ID
    pointer: GiSTPointer,
}

/// Pointer to either a child node or a row
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum GiSTPointer {
    /// Pointer to child node
    Node(Box<GiSTNode>),

    /// Pointer to row ID
    RowId(usize),
}

/// GiST Predicate - used for searching and bounding
#[derive(Debug, Clone, PartialEq)]
pub enum GiSTPredicate {
    /// 2D bounding box (for points and rectangles)
    BoundingBox {
        min_x: f64,
        max_x: f64,
        min_y: f64,
        max_y: f64,
    },

    /// Integer range [min, max]
    IntRange { min: i64, max: i64 },

    /// Float range [min, max]
    FloatRange { min: f64, max: f64 },

    /// Single point (2D)
    Point { x: f64, y: f64 },

    /// IP address prefix
    IpPrefix { addr: u32, mask: u8 },
}

impl GiSTPredicate {
    fn envelope(&self) -> Option<AABB<[f64; 2]>> {
        let (min, max) = match self {
            Self::BoundingBox {
                min_x,
                max_x,
                min_y,
                max_y,
            } => ([*min_x, *min_y], [*max_x, *max_y]),
            Self::Point { x, y } => ([*x, *y], [*x, *y]),
            _ => return None,
        };
        if min.iter().chain(max.iter()).any(|n| !n.is_finite()) {
            return None;
        }
        Some(AABB::from_corners(min, max))
    }
    /// Check if this predicate contains another
    fn contains(&self, other: &GiSTPredicate) -> bool {
        match (self, other) {
            (
                GiSTPredicate::BoundingBox {
                    min_x: sx1,
                    max_x: sx2,
                    min_y: sy1,
                    max_y: sy2,
                },
                GiSTPredicate::BoundingBox {
                    min_x: ox1,
                    max_x: ox2,
                    min_y: oy1,
                    max_y: oy2,
                },
            ) => sx1 <= ox1 && sx2 >= ox2 && sy1 <= oy1 && sy2 >= oy2,

            (
                GiSTPredicate::BoundingBox {
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                },
                GiSTPredicate::Point { x, y },
            ) => min_x <= x && x <= max_x && min_y <= y && y <= max_y,

            (
                GiSTPredicate::IntRange {
                    min: smin,
                    max: smax,
                },
                GiSTPredicate::IntRange {
                    min: omin,
                    max: omax,
                },
            ) => smin <= omin && smax >= omax,

            (
                GiSTPredicate::FloatRange {
                    min: smin,
                    max: smax,
                },
                GiSTPredicate::FloatRange {
                    min: omin,
                    max: omax,
                },
            ) => smin <= omin && smax >= omax,

            _ => false,
        }
    }

    /// Check if this predicate overlaps another
    fn overlaps(&self, other: &GiSTPredicate) -> bool {
        match (self, other) {
            (
                GiSTPredicate::BoundingBox {
                    min_x: sx1,
                    max_x: sx2,
                    min_y: sy1,
                    max_y: sy2,
                },
                GiSTPredicate::BoundingBox {
                    min_x: ox1,
                    max_x: ox2,
                    min_y: oy1,
                    max_y: oy2,
                },
            ) => !(sx2 < ox1 || sx1 > ox2 || sy2 < oy1 || sy1 > oy2),

            (
                GiSTPredicate::IntRange {
                    min: smin,
                    max: smax,
                },
                GiSTPredicate::IntRange {
                    min: omin,
                    max: omax,
                },
            ) => !(smax < omin || smin > omax),

            (
                GiSTPredicate::FloatRange {
                    min: smin,
                    max: smax,
                },
                GiSTPredicate::FloatRange {
                    min: omin,
                    max: omax,
                },
            ) => !(smax < omin || smin > omax),

            _ => false,
        }
    }

    /// Compute union (bounding predicate) of two predicates
    fn union(&self, other: &GiSTPredicate) -> GiSTPredicate {
        match (self, other) {
            (
                GiSTPredicate::BoundingBox {
                    min_x: sx1,
                    max_x: sx2,
                    min_y: sy1,
                    max_y: sy2,
                },
                GiSTPredicate::BoundingBox {
                    min_x: ox1,
                    max_x: ox2,
                    min_y: oy1,
                    max_y: oy2,
                },
            ) => GiSTPredicate::BoundingBox {
                min_x: sx1.min(*ox1),
                max_x: sx2.max(*ox2),
                min_y: sy1.min(*oy1),
                max_y: sy2.max(*oy2),
            },

            (
                GiSTPredicate::IntRange {
                    min: smin,
                    max: smax,
                },
                GiSTPredicate::IntRange {
                    min: omin,
                    max: omax,
                },
            ) => GiSTPredicate::IntRange {
                min: (*smin).min(*omin),
                max: (*smax).max(*omax),
            },

            (
                GiSTPredicate::FloatRange {
                    min: smin,
                    max: smax,
                },
                GiSTPredicate::FloatRange {
                    min: omin,
                    max: omax,
                },
            ) => GiSTPredicate::FloatRange {
                min: smin.min(*omin),
                max: smax.max(*omax),
            },

            _ => self.clone(),
        }
    }
}

impl GiSTIndex {
    /// Create a new GiST index
    pub fn new(name: String, column_name: String, index_type: GiSTIndexType) -> Self {
        Self {
            name,
            column_name,
            root: None,
            spatial: RTree::new(),
            spatial_srid: None,
            mixed_spatial_srids: false,
            searches: Arc::new(AtomicU64::new(0)),
            candidates: Arc::new(AtomicU64::new(0)),
            index_type,
            entry_count: 0,
        }
    }

    /// Insert a value into the index
    pub fn insert(&mut self, row_id: usize, value: &Value) {
        if let Some(predicate) = self.value_to_predicate(value) {
            if self.index_type == GiSTIndexType::RTree {
                if let Some(bounds) = predicate.envelope() {
                    if let Value::Geometry(g) = value {
                        let srid = g.srid.unwrap_or(0);
                        if self.spatial_srid.is_some_and(|previous| previous != srid) {
                            self.mixed_spatial_srids = true;
                        }
                        self.spatial_srid = Some(srid);
                    }
                    self.spatial.insert(SpatialEntry { row_id, bounds });
                    self.entry_count += 1;
                }
                return;
            }
            if self.root.is_some() {
                if let Some(root) = &mut self.root {
                    Self::insert_into_node_static(root, predicate, row_id);
                }
            } else {
                // Create root as leaf node
                self.root = Some(Box::new(GiSTNode {
                    predicate: predicate.clone(),
                    children: vec![GiSTEntry {
                        key: predicate,
                        pointer: GiSTPointer::RowId(row_id),
                    }],
                    is_leaf: true,
                }));
            }
            self.entry_count += 1;
        }
    }

    /// Convert value to predicate based on index type
    fn value_to_predicate(&self, value: &Value) -> Option<GiSTPredicate> {
        match (self.index_type, value) {
            (GiSTIndexType::RTree, Value::Geometry(g)) => {
                g.bounds().map(|b| GiSTPredicate::BoundingBox {
                    min_x: b.min().x,
                    min_y: b.min().y,
                    max_x: b.max().x,
                    max_y: b.max().y,
                })
            }
            (GiSTIndexType::RangeTree, Value::Integer(i)) => {
                Some(GiSTPredicate::IntRange { min: *i, max: *i })
            }
            (GiSTIndexType::RangeTree, Value::Float(f)) => {
                let val = f.into_inner();
                Some(GiSTPredicate::FloatRange { min: val, max: val })
            }
            (GiSTIndexType::RTree, Value::Text(s)) => {
                // Parse "x,y" as point or "x1,y1,x2,y2" as box
                let parts: Vec<f64> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();

                if parts.len() == 2 {
                    Some(GiSTPredicate::Point {
                        x: parts[0],
                        y: parts[1],
                    })
                } else if parts.len() == 4 {
                    Some(GiSTPredicate::BoundingBox {
                        min_x: parts[0].min(parts[2]),
                        max_x: parts[0].max(parts[2]),
                        min_y: parts[1].min(parts[3]),
                        max_y: parts[1].max(parts[3]),
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Insert into a node (static method to avoid borrowing issues)
    fn insert_into_node_static(node: &mut GiSTNode, predicate: GiSTPredicate, row_id: usize) {
        // Update node's bounding predicate
        node.predicate = node.predicate.union(&predicate);

        if node.is_leaf {
            // Insert into leaf
            node.children.push(GiSTEntry {
                key: predicate,
                pointer: GiSTPointer::RowId(row_id),
            });
        } else {
            // Find best child to insert into (not implemented for simplicity)
            // In a full implementation, choose child with minimum enlargement
        }
    }

    /// Search for rows matching a predicate
    pub fn search(&self, query: &GiSTPredicate) -> Vec<usize> {
        self.searches.fetch_add(1, Ordering::Relaxed);
        let mut results = Vec::new();

        if self.index_type == GiSTIndexType::RTree {
            if let Some(bounds) = query.envelope() {
                results.extend(
                    self.spatial
                        .locate_in_envelope_intersecting(bounds)
                        .map(|entry| entry.row_id),
                );
            }
        } else if let Some(root) = &self.root {
            self.search_node(root, query, &mut results);
        }

        self.candidates
            .fetch_add(results.len() as u64, Ordering::Relaxed);
        results.sort_unstable();
        results
    }

    pub fn index_type(&self) -> GiSTIndexType {
        self.index_type
    }

    /// KNN pruning requires a numeric distance for every row. NULL/empty values
    /// and mismatched SRIDs must retain the executor's existing full-sort behavior.
    pub(crate) fn supports_knn(&self, srid: Option<i32>, row_count: usize) -> bool {
        self.index_type == GiSTIndexType::RTree
            && self.entry_count == row_count
            && !self.mixed_spatial_srids
            && (row_count == 0 || self.spatial_srid == Some(srid.unwrap_or(0)))
    }

    /// Lazily visit bounding boxes in increasing squared distance from a point.
    /// Count only candidates actually pulled, including the stopping lookahead.
    pub(crate) fn nearest_neighbors(
        &self,
        point: [f64; 2],
    ) -> impl Iterator<Item = (usize, f64)> + '_ {
        self.searches.fetch_add(1, Ordering::Relaxed);
        self.spatial
            .nearest_neighbor_iter_with_distance_2(point)
            .map(|(entry, distance)| {
                self.candidates.fetch_add(1, Ordering::Relaxed);
                (entry.row_id, distance)
            })
    }

    /// Row IDs are vector offsets. Rebuild after a delete or update so offsets
    /// and bounds cannot become stale; retain diagnostic counters.
    pub fn clear(&mut self) {
        self.root = None;
        self.spatial = RTree::new();
        self.spatial_srid = None;
        self.mixed_spatial_srids = false;
        self.entry_count = 0;
    }

    /// Search within a node
    fn search_node(&self, node: &GiSTNode, query: &GiSTPredicate, results: &mut Vec<usize>) {
        // Check if node's predicate overlaps query
        if !node.predicate.overlaps(query) && !query.contains(&node.predicate) {
            return;
        }

        for entry in &node.children {
            if entry.key.overlaps(query) || query.contains(&entry.key) {
                match &entry.pointer {
                    GiSTPointer::RowId(row_id) => {
                        results.push(*row_id);
                    }
                    GiSTPointer::Node(child) => {
                        self.search_node(child, query, results);
                    }
                }
            }
        }
    }

    /// Get index statistics
    pub fn stats(&self) -> GiSTStats {
        GiSTStats {
            entries_indexed: self.entry_count,
            tree_height: self.tree_height(),
            search_count: self.searches.load(Ordering::Relaxed),
            candidates_returned: self.candidates.load(Ordering::Relaxed),
        }
    }

    fn tree_height(&self) -> usize {
        if self.index_type == GiSTIndexType::RTree {
            fn height(parent: &rstar::ParentNode<SpatialEntry>) -> usize {
                parent
                    .children()
                    .iter()
                    .map(|node| match node {
                        rstar::RTreeNode::Leaf(_) => 1,
                        rstar::RTreeNode::Parent(child) => 1 + height(child),
                    })
                    .max()
                    .unwrap_or(0)
            }
            return height(self.spatial.root());
        }
        self.root.as_ref().map_or(0, |root| self.node_height(root))
    }

    fn node_height(&self, node: &GiSTNode) -> usize {
        if node.is_leaf {
            1
        } else {
            1 + node
                .children
                .iter()
                .filter_map(|e| {
                    if let GiSTPointer::Node(child) = &e.pointer {
                        Some(self.node_height(child))
                    } else {
                        None
                    }
                })
                .max()
                .unwrap_or(0)
        }
    }
}

/// GiST Index Statistics
#[derive(Debug, Clone)]
pub struct GiSTStats {
    pub entries_indexed: usize,
    pub tree_height: usize,
    /// Actual search calls, including SQL scans and parameterized join probes.
    pub search_count: u64,
    pub candidates_returned: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gist_range_containment() {
        let mut index = GiSTIndex::new(
            "age_range_idx".to_string(),
            "age".to_string(),
            GiSTIndexType::RangeTree,
        );

        // Insert ages
        index.insert(0, &Value::Integer(25));
        index.insert(1, &Value::Integer(30));
        index.insert(2, &Value::Integer(35));
        index.insert(3, &Value::Integer(40));

        // Search for ages in range [28, 36]
        let query = GiSTPredicate::IntRange { min: 28, max: 36 };
        let results = index.search(&query);

        // Should find rows 1, 2 (ages 30, 35)
        assert!(results.contains(&1));
        assert!(results.contains(&2));
    }

    #[test]
    fn test_gist_point_search() {
        let mut index = GiSTIndex::new(
            "location_idx".to_string(),
            "location".to_string(),
            GiSTIndexType::RTree,
        );

        // Insert points
        index.insert(0, &Value::Text("10.0,20.0".to_string()));
        index.insert(1, &Value::Text("15.0,25.0".to_string()));
        index.insert(2, &Value::Text("30.0,40.0".to_string()));

        // Search for points in bounding box [0,0] to [20,30]
        let query = GiSTPredicate::BoundingBox {
            min_x: 0.0,
            max_x: 20.0,
            min_y: 0.0,
            max_y: 30.0,
        };
        let results = index.search(&query);

        // Should find rows 0, 1
        assert!(results.contains(&0));
        assert!(results.contains(&1));
        assert!(!results.contains(&2));
    }

    #[test]
    fn test_gist_stats() {
        let mut index = GiSTIndex::new(
            "test_idx".to_string(),
            "value".to_string(),
            GiSTIndexType::RangeTree,
        );

        for i in 0..10 {
            index.insert(i, &Value::Integer(i as i64));
        }

        let stats = index.stats();
        assert_eq!(stats.entries_indexed, 10);
        assert!(stats.tree_height > 0);
    }
}
