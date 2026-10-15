//! SQL HNSW index: `CREATE INDEX name ON t USING hnsw (col) [WITH (...)]`.
//!
//! The definition is stored in the table schema and committed through the WAL;
//! the graph is derived data. It is built from the table rows the first time a
//! nearest-neighbor query needs it (not at open, so opening a database and
//! keyword-only work never pay for it), then maintained by INSERT, UPDATE and
//! DELETE. Graph nodes are keyed by row offset, like GiST and FULLTEXT entries.

use super::hnsw::{HNSWConfig, HNSWIndex};
use super::index::{VectorIndex, VectorSearchResult};
use crate::types::{Row, Value};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// Distance the graph is built for. Cosine graphs store unit vectors, so their
/// L2 order is the cosine order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorMetric {
    L2,
    Cosine,
}

impl VectorMetric {
    pub fn parse(text: &str) -> Option<Self> {
        match text.to_ascii_lowercase().as_str() {
            "l2" | "euclidean" | "vector_l2_ops" => Some(Self::L2),
            "cosine" | "vector_cosine_ops" => Some(Self::Cosine),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::L2 => "l2",
            Self::Cosine => "cosine",
        }
    }
}

/// Persisted definition of an HNSW index; the graph is rebuilt on demand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorIndexDefinition {
    pub name: String,
    pub column: String,
    pub metric: VectorMetric,
    pub m: usize,
    pub ef_construction: usize,
    pub ef_search: usize,
}

/// A built graph plus how many nodes deletions removed since it was built.
#[derive(Clone)]
struct Graph {
    hnsw: HNSWIndex,
    removed: usize,
}

/// The HNSW index of one VECTOR column.
pub struct VectorColumnIndex {
    pub def: VectorIndexDefinition,
    /// `None` until a query needs the graph. Clones (transaction snapshots)
    /// share the graph; the first write after a clone copies it.
    graph: Mutex<Option<Arc<Graph>>>,
}

impl Clone for VectorColumnIndex {
    fn clone(&self) -> Self {
        Self {
            def: self.def.clone(),
            graph: Mutex::new(self.graph.lock().unwrap().clone()),
        }
    }
}

impl std::fmt::Debug for VectorColumnIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let built = self.graph.lock().unwrap().as_ref().map(|g| g.hnsw.len());
        f.debug_struct("VectorColumnIndex")
            .field("def", &self.def)
            .field("built_nodes", &built)
            .finish()
    }
}

/// The vector a row contributes to a graph of `metric`, or None when the value
/// cannot be indexed (NULL, not a vector, non-finite, or a zero vector under
/// cosine). Such rows make the index incomplete and queries fall back to a scan.
pub fn indexable(value: Option<&Value>, metric: VectorMetric) -> Option<Vec<f32>> {
    let Some(Value::Vector(v)) = value else {
        return None;
    };
    if v.is_empty() || v.iter().any(|x| !x.is_finite()) {
        return None;
    }
    match metric {
        VectorMetric::L2 => Some(v.clone()),
        VectorMetric::Cosine => {
            let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            (norm > 0.0 && norm.is_finite()).then(|| v.iter().map(|x| x / norm).collect())
        }
    }
}

fn config(def: &VectorIndexDefinition) -> HNSWConfig {
    let m = def.m.max(2);
    HNSWConfig {
        m,
        m_max0: m * 2,
        ef_construction: def.ef_construction.max(m),
        ef_search: def.ef_search.max(1),
        ml: 1.0 / (m as f64).ln(),
    }
}

fn insert_into(
    def: &VectorIndexDefinition,
    hnsw: &mut HNSWIndex,
    row_id: usize,
    value: Option<&Value>,
) {
    let Some(vector) = indexable(value, def.metric) else {
        return;
    };
    if hnsw.is_empty() && hnsw.dimension() != vector.len() {
        *hnsw = HNSWIndex::new(vector.len(), config(def));
    }
    // A vector of another dimension is left out, like a NULL.
    let _ = hnsw.insert(row_id as u64, &vector);
}

impl VectorColumnIndex {
    pub fn new(def: VectorIndexDefinition) -> Self {
        Self {
            def,
            graph: Mutex::new(None),
        }
    }

    /// Whether the graph has been built (for tests and diagnostics).
    pub fn is_built(&self) -> bool {
        self.graph.lock().unwrap().is_some()
    }

    /// Drop the graph; the next query rebuilds it from the rows.
    pub fn invalidate(&mut self) {
        *self.graph.get_mut().unwrap() = None;
    }

    /// Build a graph over `rows` (row offset = node id).
    fn build(&self, rows: &[Row]) -> Graph {
        let mut hnsw = HNSWIndex::new(0, config(&self.def));
        for (row_id, row) in rows.iter().enumerate() {
            insert_into(&self.def, &mut hnsw, row_id, row.get(&self.def.column));
        }
        Graph { hnsw, removed: 0 }
    }

    fn built_mut(&mut self) -> Option<&mut Graph> {
        self.graph.get_mut().unwrap().as_mut().map(Arc::make_mut)
    }

    /// A row was appended at `row_id`.
    pub fn push_row(&mut self, row_id: usize, value: Option<&Value>) {
        let def = self.def.clone();
        if let Some(graph) = self.built_mut() {
            insert_into(&def, &mut graph.hnsw, row_id, value);
        }
    }

    /// The row at `row_id` changed from `old` to `new`.
    pub fn replace_row(&mut self, row_id: usize, old: Option<&Value>, new: Option<&Value>) {
        if old == new {
            return;
        }
        let def = self.def.clone();
        if let Some(graph) = self.built_mut() {
            graph.hnsw.remove_node(row_id as u64);
            insert_into(&def, &mut graph.hnsw, row_id, new);
        }
    }

    /// Rows were deleted: `offsets[old]` is the new offset or `usize::MAX`.
    /// A graph that lost more than a quarter of its nodes since it was built
    /// is dropped and rebuilt on the next query, as deletions without repair
    /// weaken its connectivity.
    pub fn remap(&mut self, offsets: &[usize]) {
        let rebuild = match self.built_mut() {
            Some(graph) => {
                graph.removed += graph.hnsw.retain_remap(offsets);
                graph.removed * 4 > graph.hnsw.len() + graph.removed
            }
            None => false,
        };
        if rebuild {
            self.invalidate();
        }
    }

    /// Approximate nearest neighbors of `query` among `rows`, nearest first,
    /// or None when some row is not indexed (the caller must scan instead).
    /// `query` must already be normalized for cosine (see `indexable`).
    pub fn search(
        &self,
        rows: &[Row],
        query: &[f32],
        k: usize,
        ef: usize,
    ) -> Option<Vec<VectorSearchResult>> {
        let graph = {
            let mut slot = self.graph.lock().unwrap();
            slot.get_or_insert_with(|| Arc::new(self.build(rows)))
                .clone()
        };
        if graph.hnsw.len() != rows.len() {
            return None;
        }
        Some(graph.hnsw.search_with_ef(query, k, ef))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(metric: VectorMetric) -> VectorIndexDefinition {
        VectorIndexDefinition {
            name: "v".into(),
            column: "e".into(),
            metric,
            m: 8,
            ef_construction: 32,
            ef_search: 32,
        }
    }

    fn row(v: Vec<f32>) -> Row {
        Row::new(vec!["e".into()], vec![Value::Vector(v)])
    }

    #[test]
    fn builds_lazily_and_finds_exact_match() {
        let rows: Vec<Row> = (0..200)
            .map(|i| row(vec![i as f32, (i % 7) as f32, 1.0]))
            .collect();
        let index = VectorColumnIndex::new(def(VectorMetric::L2));
        assert!(!index.is_built());
        let hits = index.search(&rows, &[50.0, 1.0, 1.0], 1, 32).unwrap();
        assert!(index.is_built());
        assert_eq!(hits[0].row_id, 50);
    }

    #[test]
    fn incomplete_index_refuses_to_answer() {
        let mut rows = vec![row(vec![1.0, 0.0]), row(vec![0.0, 1.0])];
        rows.push(Row::new(vec!["e".into()], vec![Value::Null]));
        let index = VectorColumnIndex::new(def(VectorMetric::Cosine));
        assert!(index.search(&rows, &[1.0, 0.0], 1, 8).is_none());
    }

    #[test]
    fn remap_drops_deleted_rows_and_renumbers() {
        let rows: Vec<Row> = (0..10).map(|i| row(vec![i as f32, 0.0])).collect();
        let mut index = VectorColumnIndex::new(def(VectorMetric::L2));
        index.search(&rows, &[0.0, 0.0], 1, 8).unwrap();
        // Delete row 0: every other row moves down one offset.
        let offsets: Vec<usize> = (0..10)
            .map(|i| if i == 0 { usize::MAX } else { i - 1 })
            .collect();
        index.remap(&offsets);
        let rest: Vec<Row> = rows[1..].to_vec();
        let hits = index.search(&rest, &[5.0, 0.0], 1, 8).unwrap();
        assert_eq!(hits[0].row_id, 4); // old row 5
    }

    #[test]
    fn clones_share_until_written() {
        let rows: Vec<Row> = (0..20).map(|i| row(vec![i as f32, 1.0])).collect();
        let mut index = VectorColumnIndex::new(def(VectorMetric::L2));
        index.search(&rows, &[0.0, 1.0], 1, 8).unwrap();
        let snapshot = index.clone();
        index.push_row(20, Some(&Value::Vector(vec![100.0, 1.0])));
        let mut grown = rows.clone();
        grown.push(row(vec![100.0, 1.0]));
        assert_eq!(
            index.search(&grown, &[99.0, 1.0], 1, 8).unwrap()[0].row_id,
            20
        );
        // The snapshot still describes the 20 original rows.
        assert_eq!(
            snapshot.search(&rows, &[99.0, 1.0], 1, 8).unwrap()[0].row_id,
            19
        );
    }
}
