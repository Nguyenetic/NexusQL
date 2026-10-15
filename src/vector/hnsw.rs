//! HNSW (Hierarchical Navigable Small World) Index
//!
//! Approximate nearest neighbor search with:
//! - Sub-linear query time ~O(log n)
//! - High recall (90-95%+)
//! - Efficient for 100K - 10M+ vectors
//!
//! Based on research from:
//! - Original HNSW paper (Malkov & Yashunin, 2016)
//! - pgvector implementation
//! - USearch optimizations

use super::index::{IndexType, VectorIndex, VectorSearchResult};
use crate::Result;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// HNSW configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HNSWConfig {
    /// Maximum number of connections per node (M)
    /// Higher = better recall, more memory
    /// Recommended: 16-48
    pub m: usize,

    /// Maximum connections for layer 0 (typically M * 2)
    pub m_max0: usize,

    /// Size of dynamic candidate list during construction (efConstruction)
    /// Higher = better quality index, slower build
    /// Recommended: 64-400
    pub ef_construction: usize,

    /// Size of dynamic candidate list during search (efSearch)
    /// Higher = better recall, slower queries
    /// Recommended: 40-500
    pub ef_search: usize,

    /// Level multiplier for layer assignment
    /// Default: 1/ln(M)
    pub ml: f64,
}

impl Default for HNSWConfig {
    fn default() -> Self {
        Self {
            m: 16,
            m_max0: 32,
            ef_construction: 64,
            ef_search: 40,
            ml: 1.0 / (16.0_f64).ln(),
        }
    }
}

impl HNSWConfig {
    /// High recall configuration (slower, more accurate)
    pub fn high_recall() -> Self {
        Self {
            m: 24,
            m_max0: 48,
            ef_construction: 400,
            ef_search: 500,
            ml: 1.0 / (24.0_f64).ln(),
        }
    }

    /// Fast configuration (faster, good recall)
    pub fn fast() -> Self {
        Self {
            m: 12,
            m_max0: 24,
            ef_construction: 200,
            ef_search: 100,
            ml: 1.0 / (12.0_f64).ln(),
        }
    }
}

/// HNSW Node
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HNSWNode {
    /// Row ID from database
    row_id: u64,
    /// Vector data
    vector: Vec<f32>,
    /// Neighbors at each layer (layer -> list of row_ids)
    neighbors: Vec<Vec<u64>>,
}

/// HNSW Index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HNSWIndex {
    /// Configuration
    config: HNSWConfig,
    /// All nodes indexed by row_id
    nodes: HashMap<u64, HNSWNode>,
    /// Entry point (top-level node)
    entry_point: Option<u64>,
    /// Maximum layer currently in use
    max_layer: i32,
    /// Vector dimension
    dimension: usize,
}

impl HNSWIndex {
    /// Create new HNSW index
    pub fn new(dimension: usize, config: HNSWConfig) -> Self {
        Self {
            config,
            nodes: HashMap::new(),
            entry_point: None,
            max_layer: -1,
            dimension,
        }
    }

    /// Create with default config
    pub fn with_dimension(dimension: usize) -> Self {
        Self::new(dimension, HNSWConfig::default())
    }

    /// Get configuration
    pub fn config(&self) -> &HNSWConfig {
        &self.config
    }

    /// Randomly determine layer for new node
    fn random_layer(&self) -> i32 {
        let mut rng = rand::thread_rng();
        let uniform: f64 = rng.gen();
        (-uniform.ln() * self.config.ml).floor() as i32
    }

    /// Pick at most `m` links for a node from `candidates` (nearest first,
    /// squared distances to that node) with the HNSW paper's heuristic: a
    /// candidate is kept when it is closer to the node than to every link
    /// kept so far, so links spread over directions instead of clustering.
    /// Without it an outlier's only incoming link is pruned first and the
    /// outlier becomes unreachable. Remaining slots are filled with the
    /// nearest skipped candidates (keepPrunedConnections).
    fn select_neighbors(&self, candidates: &[(u64, f32)], m: usize) -> Vec<u64> {
        let mut kept: Vec<(u64, &[f32])> = Vec::with_capacity(m);
        let mut skipped = Vec::new();
        for &(id, distance) in candidates {
            if kept.len() >= m {
                break;
            }
            let Some(node) = self.nodes.get(&id) else {
                continue;
            };
            let vector = node.vector.as_slice();
            if kept
                .iter()
                .all(|(_, other)| squared_l2(vector, other) > distance)
            {
                kept.push((id, vector));
            } else {
                skipped.push(id);
            }
        }
        let mut links: Vec<u64> = kept.into_iter().map(|(id, _)| id).collect();
        let free = m.saturating_sub(links.len());
        links.extend(skipped.into_iter().take(free));
        links
    }

    /// Search for nearest neighbors in a specific layer
    fn search_layer(
        &self,
        query: &[f32],
        entry_points: Vec<u64>,
        ef: usize,
        layer: usize,
    ) -> Vec<(u64, f32)> {
        let mut visited = std::collections::HashSet::new();
        let mut candidates = BinaryHeap::new();
        let mut w = BinaryHeap::new();

        // Initialize with entry points
        for ep in entry_points {
            if let Some(node) = self.nodes.get(&ep) {
                let dist = squared_l2(query, &node.vector);
                visited.insert(ep);
                candidates.push(MaxHeapEntry {
                    distance: dist,
                    row_id: ep,
                });
                w.push(MinHeapEntry {
                    distance: dist,
                    row_id: ep,
                });
            }
        }

        while let Some(MaxHeapEntry {
            distance: c_dist,
            row_id: c,
        }) = candidates.pop()
        {
            // If candidate is further than worst in w, stop
            if let Some(f) = w.peek() {
                if c_dist > f.distance {
                    break;
                }
            }

            // Check neighbors of c
            let links = self
                .nodes
                .get(&c)
                .and_then(|node| node.neighbors.get(layer))
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            for &neighbor_id in links {
                if !visited.contains(&neighbor_id) {
                    visited.insert(neighbor_id);

                    if let Some(neighbor_node) = self.nodes.get(&neighbor_id) {
                        let dist = squared_l2(query, &neighbor_node.vector);

                        if w.len() < ef || dist < w.peek().unwrap().distance {
                            candidates.push(MaxHeapEntry {
                                distance: dist,
                                row_id: neighbor_id,
                            });
                            w.push(MinHeapEntry {
                                distance: dist,
                                row_id: neighbor_id,
                            });

                            if w.len() > ef {
                                w.pop();
                            }
                        }
                    }
                }
            }
        }

        // `w` orders by distance, so into_sorted_vec() is nearest first.
        w.into_sorted_vec()
            .into_iter()
            .map(|e| (e.row_id, e.distance))
            .collect()
    }

    /// Search with an explicit candidate-list size (efSearch). A larger `ef`
    /// raises recall; `k` results at most are returned, nearest first.
    pub fn search_with_ef(&self, query: &[f32], k: usize, ef: usize) -> Vec<VectorSearchResult> {
        let Some(entry) = self.entry_point else {
            return Vec::new();
        };
        if query.len() != self.dimension {
            return Vec::new();
        }
        let mut ep = vec![entry];
        for lc in (1..=self.max_layer).rev() {
            let nearest = self.search_layer(query, ep.clone(), 1, lc as usize);
            if !nearest.is_empty() {
                ep = vec![nearest[0].0];
            }
        }
        self.search_layer(query, ep, ef.max(k), 0)
            .into_iter()
            .take(k)
            .map(|(row_id, distance)| VectorSearchResult {
                row_id,
                distance: distance.sqrt(),
            })
            .collect()
    }

    /// Vector dimension accepted by `insert`.
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Whether `row_id` is indexed.
    pub fn contains(&self, row_id: u64) -> bool {
        self.nodes.contains_key(&row_id)
    }

    /// Drop the nodes whose `offsets[id]` is `usize::MAX` and renumber the
    /// rest to `offsets[id]`, in one pass over the graph. Links to dropped
    /// nodes are removed. Returns the number of dropped nodes.
    pub fn retain_remap(&mut self, offsets: &[usize]) -> usize {
        let map = |id: u64| {
            offsets
                .get(id as usize)
                .copied()
                .filter(|&n| n != usize::MAX)
                .map(|n| n as u64)
        };
        let old = std::mem::take(&mut self.nodes);
        let before = old.len();
        for (id, mut node) in old {
            let Some(new_id) = map(id) else {
                continue;
            };
            node.row_id = new_id;
            for links in &mut node.neighbors {
                *links = links.iter().filter_map(|&n| map(n)).collect();
            }
            self.nodes.insert(new_id, node);
        }
        self.entry_point = self.entry_point.and_then(map);
        if self.entry_point.is_none() {
            self.reset_entry_point();
        }
        before - self.nodes.len()
    }

    /// Remove one node and its incoming links, keeping a valid entry point.
    pub fn remove_node(&mut self, row_id: u64) {
        if self.nodes.remove(&row_id).is_none() {
            return;
        }
        for node in self.nodes.values_mut() {
            for links in &mut node.neighbors {
                links.retain(|&id| id != row_id);
            }
        }
        if self.entry_point == Some(row_id) {
            self.reset_entry_point();
        }
    }

    /// Point the search at the node with the most layers (the HNSW invariant).
    fn reset_entry_point(&mut self) {
        let top = self
            .nodes
            .values()
            .max_by_key(|node| (node.neighbors.len(), std::cmp::Reverse(node.row_id)));
        self.entry_point = top.map(|node| node.row_id);
        self.max_layer = top.map_or(-1, |node| node.neighbors.len() as i32 - 1);
    }
}

impl VectorIndex for HNSWIndex {
    fn insert(&mut self, row_id: u64, vector: &[f32]) -> Result<()> {
        if vector.len() != self.dimension {
            return Err(crate::NexusError::QueryError(format!(
                "Vector dimension mismatch: expected {}, got {}",
                self.dimension,
                vector.len()
            )));
        }

        // Determine layer for new node
        let layer = self.random_layer();

        // Create node with empty neighbors for each layer
        let mut neighbors = Vec::new();
        for _ in 0..=layer as usize {
            neighbors.push(Vec::new());
        }

        let node = HNSWNode {
            row_id,
            vector: vector.to_vec(),
            neighbors,
        };

        // If first node, set as entry point
        if self.entry_point.is_none() {
            self.entry_point = Some(row_id);
            self.max_layer = layer;
            self.nodes.insert(row_id, node);
            return Ok(());
        }

        // Insert node first so we can modify its neighbors
        self.nodes.insert(row_id, node);

        // Search for nearest neighbors from top layer to layer 0
        let mut ep = vec![self.entry_point.unwrap()];

        // Phase 1: Find entry point at target layer
        for lc in (layer + 1..=self.max_layer).rev() {
            let nearest = self.search_layer(vector, ep.clone(), 1, lc as usize);
            if !nearest.is_empty() {
                ep = vec![nearest[0].0];
            }
        }

        // Phase 2: Insert into layers from layer down to 0
        for lc in (0..=layer).rev() {
            let candidates =
                self.search_layer(vector, ep.clone(), self.config.ef_construction, lc as usize);

            // Select M nearest neighbors (excluding self, simplified - should use heuristic)
            let m = if lc == 0 {
                self.config.m_max0
            } else {
                self.config.m
            };

            let others: Vec<(u64, f32)> = candidates
                .iter()
                .filter(|(id, _)| *id != row_id) // Exclude self
                .copied()
                .collect();
            let neighbors_to_add = self.select_neighbors(&others, self.config.m.min(m));

            // Add bidirectional links
            if let Some(new_node) = self.nodes.get_mut(&row_id) {
                if (lc as usize) < new_node.neighbors.len() {
                    new_node.neighbors[lc as usize] = neighbors_to_add.clone();
                }
            }

            for &neighbor_id in &neighbors_to_add {
                if let Some(neighbor) = self.nodes.get_mut(&neighbor_id) {
                    if (lc as usize) < neighbor.neighbors.len() {
                        neighbor.neighbors[lc as usize].push(row_id);
                    }
                }
                // Over capacity: re-select the neighbor's links with the
                // heuristic. Keeping only the m closest (or dropping the
                // newest link) leaves outliers unreachable.
                let over = self
                    .nodes
                    .get(&neighbor_id)
                    .and_then(|n| n.neighbors.get(lc as usize))
                    .is_some_and(|links| links.len() > m);
                if over {
                    let node = &self.nodes[&neighbor_id];
                    let mut links: Vec<(u64, f32)> = node.neighbors[lc as usize]
                        .iter()
                        .filter_map(|id| {
                            self.nodes
                                .get(id)
                                .map(|n| (*id, squared_l2(&node.vector, &n.vector)))
                        })
                        .collect();
                    links.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
                    let kept = self.select_neighbors(&links, m);
                    if let Some(neighbor) = self.nodes.get_mut(&neighbor_id) {
                        neighbor.neighbors[lc as usize] = kept;
                    }
                }
            }

            ep = neighbors_to_add;
        }

        // Update max layer and entry point if necessary
        if layer > self.max_layer {
            self.max_layer = layer;
            self.entry_point = Some(row_id);
        }

        Ok(())
    }

    fn search(&self, query: &[f32], k: usize) -> Result<Vec<VectorSearchResult>> {
        if query.len() != self.dimension {
            return Err(crate::NexusError::QueryError(format!(
                "Query dimension mismatch: expected {}, got {}",
                self.dimension,
                query.len()
            )));
        }

        if self.entry_point.is_none() {
            return Ok(Vec::new());
        }

        let mut ep = vec![self.entry_point.unwrap()];

        // Search from top layer to layer 1
        for lc in (1..=self.max_layer).rev() {
            let nearest = self.search_layer(query, ep.clone(), 1, lc as usize);
            if !nearest.is_empty() {
                ep = vec![nearest[0].0];
            }
        }

        // Search layer 0 with ef_search
        let candidates = self.search_layer(query, ep, self.config.ef_search, 0);

        // search_layer returns results sorted by distance (nearest first)
        // Return top k
        let results: Vec<VectorSearchResult> = candidates
            .iter()
            .take(k)
            .map(|&(row_id, distance)| VectorSearchResult {
                row_id,
                distance: distance.sqrt(),
            })
            .collect();

        Ok(results)
    }

    fn remove(&mut self, row_id: u64) -> Result<()> {
        // Remove node
        self.nodes.remove(&row_id);

        // Remove from all neighbor lists
        for node in self.nodes.values_mut() {
            for neighbors in &mut node.neighbors {
                neighbors.retain(|&id| id != row_id);
            }
        }

        // Update entry point if necessary
        if Some(row_id) == self.entry_point {
            self.entry_point = self.nodes.keys().next().copied();
        }

        Ok(())
    }

    fn index_type(&self) -> IndexType {
        IndexType::HNSW
    }

    fn len(&self) -> usize {
        self.nodes.len()
    }

    fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(&self).map_err(|e| crate::NexusError::SerializeError(e))
    }

    fn deserialize(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data).map_err(|e| crate::NexusError::SerializeError(e))
    }
}

/// Squared Euclidean distance with eight independent accumulators, which the
/// compiler can keep in SIMD registers (one running f32 sum cannot be
/// reordered, so it adds one lane at a time). Construction and search only
/// compare distances, so the square root is taken on output.
#[inline]
fn squared_l2(a: &[f32], b: &[f32]) -> f32 {
    let mut acc = [0.0f32; 8];
    let chunks = a.len().min(b.len()) / 8 * 8;
    for (x, y) in a[..chunks].chunks_exact(8).zip(b[..chunks].chunks_exact(8)) {
        for i in 0..8 {
            let d = x[i] - y[i];
            acc[i] += d * d;
        }
    }
    let mut sum: f32 = acc.iter().sum();
    for (x, y) in a[chunks..].iter().zip(&b[chunks..]) {
        let d = x - y;
        sum += d * d;
    }
    sum
}

// Helper structs for priority queue
#[derive(Clone, Copy)]
struct MinHeapEntry {
    distance: f32,
    row_id: u64,
}

impl PartialEq for MinHeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for MinHeapEntry {}

impl PartialOrd for MinHeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Result set W: BinaryHeap must expose the *furthest* result so the
        // search can stop and evict it. (Reversing this evicted the nearest.)
        self.distance.partial_cmp(&other.distance)
    }
}

impl Ord for MinHeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

#[derive(Clone, Copy)]
struct MaxHeapEntry {
    distance: f32,
    row_id: u64,
}

impl PartialEq for MaxHeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for MaxHeapEntry {}

impl PartialOrd for MaxHeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Reverse for min-heap behavior (pop closest candidate first)
        // HNSW algorithm requires exploring nearest candidates first
        other.distance.partial_cmp(&self.distance)
    }
}

impl Ord for MaxHeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hnsw_insert_search() {
        let mut index = HNSWIndex::with_dimension(3);

        // Insert vectors
        index.insert(1, &[1.0, 0.0, 0.0]).unwrap();
        index.insert(2, &[0.0, 1.0, 0.0]).unwrap();
        index.insert(3, &[0.0, 0.0, 1.0]).unwrap();
        index.insert(4, &[0.9, 0.1, 0.0]).unwrap();

        assert_eq!(index.len(), 4);

        // Search for nearest to [1.0, 0.1, 0.0]
        let results = index.search(&[1.0, 0.1, 0.0], 2).unwrap();

        assert_eq!(results.len(), 2);
        // Should find 1 or 4 as closest (both are equidistant at 0.1)
        // HNSW is approximate, so we just verify we got valid results
        assert!(results[0].row_id >= 1 && results[0].row_id <= 4);
        assert!(results[1].row_id >= 1 && results[1].row_id <= 4);
        // Verify the closest result has reasonable distance
        assert!(results[0].distance < 0.5);
    }
}
