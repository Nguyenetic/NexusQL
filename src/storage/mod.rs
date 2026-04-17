//! Storage Layer - Persistence
//! PROPRIETARY

pub mod disk;
pub mod wal;

use crate::index::{BRINIndex, BTreeIndex, GINIndex, GiSTIndex, HNSWIndex};
use crate::types::{Row, Value};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Typed index variants beyond the plain column B-Tree.
/// Stored under `Table::typed_indexes` keyed by the user-supplied index name
/// from `CREATE INDEX <idx_name> ON <table> (<col>) USING <type>`.
#[derive(Debug, Clone)]
pub enum IndexType {
    /// Hierarchical Navigable Small World — for high-dimensional vector search.
    HNSW(HNSWIndex),
    /// Generalized Inverted Index — for arrays / JSON / full-text tokens.
    GIN(GINIndex),
    /// Generalized Search Tree — for geometric / spatial / range types.
    GiST(GiSTIndex),
    /// Block Range Index — compact summary index for time-series.
    BRIN(BRINIndex),
}

/// Serializable table data. On-disk format stores just the row *values*
/// (not the per-row Vec<String> column-name copies that `Row` carries in
/// memory). For a 150k-row table with ~10 columns, this cuts serialized
/// size by roughly (10 * avg_name_len / (10 * avg_value_size)) — 3-10x
/// on realistic text-heavy data, and a proportional speedup on flush.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TableData {
    name: String,
    columns: Vec<String>,
    /// Values only — Row's column-name vec is NOT stored. On load we
    /// reconstruct rows using the shared `columns` above.
    row_values: Vec<Vec<Value>>,
    #[serde(default)]
    auto_increment_column: Option<String>,
    #[serde(default)]
    next_auto_increment_id: i64,
}

/// Table storage with index support.
#[derive(Debug, Clone)]
pub struct Table {
    pub name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Row>,

    /// Plain B-Tree indexes: column_name -> BTreeIndex.
    /// Not persisted — rebuilt on load from row data.
    pub indexes: HashMap<String, BTreeIndex>,

    /// Named typed indexes (HNSW / GIN / GiST / BRIN) created via
    /// `CREATE INDEX ... USING <type>`. Keyed by index name.
    /// Not persisted — rebuilt on load.
    pub typed_indexes: HashMap<String, IndexType>,

    /// Name of the AUTO_INCREMENT column on this table, if any.
    pub auto_increment_column: Option<String>,

    /// Next auto-increment value to hand out when a NULL is inserted into
    /// `auto_increment_column`. Starts at 1.
    pub next_auto_increment_id: i64,
}

impl Serialize for Table {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Borrow view: no clone per row, just reference the values vecs.
        let row_values: Vec<&Vec<Value>> = self.rows.iter().map(|r| &r.values).collect();
        let data = TableDataSer {
            name: &self.name,
            columns: &self.columns,
            row_values: &row_values,
            auto_increment_column: &self.auto_increment_column,
            next_auto_increment_id: self.next_auto_increment_id,
        };
        data.serialize(serializer)
    }
}

/// Zero-copy serialization view of `Table` — same wire format as
/// `TableData`, but borrowing instead of cloning.
#[derive(Serialize)]
struct TableDataSer<'a> {
    name: &'a str,
    columns: &'a Vec<String>,
    row_values: &'a Vec<&'a Vec<Value>>,
    auto_increment_column: &'a Option<String>,
    next_auto_increment_id: i64,
}

impl<'de> Deserialize<'de> for Table {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = TableData::deserialize(deserializer)?;
        let cols = data.columns;
        let rows = data
            .row_values
            .into_iter()
            .map(|values| Row {
                columns: cols.clone(),
                values,
            })
            .collect();
        Ok(Table {
            name: data.name,
            columns: cols,
            rows,
            // Indexes + typed_indexes must be rebuilt after load
            indexes: HashMap::new(),
            typed_indexes: HashMap::new(),
            auto_increment_column: data.auto_increment_column,
            next_auto_increment_id: data.next_auto_increment_id.max(1),
        })
    }
}

impl Table {
    pub fn new(name: String, columns: Vec<String>) -> Self {
        Self {
            name,
            columns,
            rows: Vec::new(),
            indexes: HashMap::new(),
            typed_indexes: HashMap::new(),
            auto_increment_column: None,
            next_auto_increment_id: 1,
        }
    }

    /// Mark a column as AUTO_INCREMENT. Called when a CREATE TABLE definition
    /// specifies `AUTO_INCREMENT` / `SERIAL` on a column. Only one column per
    /// table may be auto-increment.
    pub fn set_auto_increment_column(&mut self, column_name: String) {
        self.auto_increment_column = Some(column_name);
        if self.next_auto_increment_id < 1 {
            self.next_auto_increment_id = 1;
        }
    }

    /// Create an index on a column
    pub fn create_index(&mut self, column_name: &str) -> Result<()> {
        let column_idx = self
            .columns
            .iter()
            .position(|c| c == column_name)
            .ok_or_else(|| {
                crate::NexusError::QueryError(format!("Column {} not found", column_name))
            })?;

        let mut index = BTreeIndex::new(column_name.to_string(), column_idx);
        index.build_from_rows(&self.rows);

        self.indexes.insert(column_name.to_string(), index);
        Ok(())
    }

    pub fn insert(&mut self, row: Row) {
        let row_idx = self.rows.len();

        // Update indexes
        for (_col_name, index) in &mut self.indexes {
            index.insert(&row, row_idx);
        }

        self.rows.push(row);
    }

    /// Point query using index if available
    pub fn lookup(&self, column_name: &str, value: &Value) -> Vec<Row> {
        if let Some(index) = self.indexes.get(column_name) {
            // Use index for O(log n) lookup
            if let Some(row_indices) = index.lookup(value) {
                return row_indices
                    .iter()
                    .filter_map(|&idx| self.rows.get(idx).cloned())
                    .collect();
            }
            return Vec::new();
        }

        // Fallback to O(n) scan
        let column_idx = self.columns.iter().position(|c| c == column_name);
        if let Some(idx) = column_idx {
            self.rows
                .iter()
                .filter(|row| row.values.get(idx) == Some(value))
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Range query using index if available
    pub fn range_lookup(&self, column_name: &str, min: &Value, max: &Value) -> Vec<Row> {
        if let Some(index) = self.indexes.get(column_name) {
            let row_indices = index.range_lookup(min, max);
            return row_indices
                .iter()
                .filter_map(|&idx| self.rows.get(idx).cloned())
                .collect();
        }

        // Fallback to scan
        let column_idx = self.columns.iter().position(|c| c == column_name);
        if let Some(idx) = column_idx {
            self.rows
                .iter()
                .filter(|row| {
                    if let Some(val) = row.values.get(idx) {
                        val >= min && val <= max
                    } else {
                        false
                    }
                })
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn select_all(&self) -> Vec<Row> {
        self.rows.clone()
    }
}
pub mod lsm;
pub use lsm::LSMStorage;
