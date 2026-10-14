//! Storage Layer - Persistence
//! PROPRIETARY

pub mod disk;
pub mod wal;

use crate::index::{BRINIndex, BTreeIndex, GINIndex, GiSTIndex, HNSWIndex};
use crate::query::ast::DataType;
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
    #[serde(default)]
    schema: TableSchema,
}

/// Kept in a separate disk key so pre-geometry table metadata remains readable.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TableSchema {
    pub column_types: HashMap<String, DataType>,
    pub gist_indexes: Vec<GiSTDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiSTDefinition {
    pub name: String,
    pub column: String,
    pub index_type: crate::index::GiSTIndexType,
}

/// Metadata written to `table:<name>:meta`. Rows live in separate
/// `table:<name>:chunk:<nnnn>` keys so individual values stay under
/// sled's inlined-value threshold and the blob heap stays empty.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TableMeta {
    pub name: String,
    pub columns: Vec<String>,
    pub chunk_count: u32,
    #[serde(default)]
    pub auto_increment_column: Option<String>,
    #[serde(default)]
    pub next_auto_increment_id: i64,
}

/// Borrowed serialization view of `TableMeta` — same wire format, no clones.
#[derive(Serialize)]
pub(crate) struct TableMetaSer<'a> {
    pub name: &'a str,
    pub columns: &'a Vec<String>,
    pub chunk_count: u32,
    pub auto_increment_column: &'a Option<String>,
    pub next_auto_increment_id: i64,
}

/// Table storage with index support.
#[derive(Debug, Clone)]
pub struct Table {
    pub name: String,
    pub schema: TableSchema,
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
            schema: &self.schema,
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
    schema: &'a TableSchema,
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
        let mut table = Table {
            name: data.name,
            schema: data.schema,
            columns: cols,
            rows,
            // Indexes + typed_indexes must be rebuilt after load
            indexes: HashMap::new(),
            typed_indexes: HashMap::new(),
            auto_increment_column: data.auto_increment_column,
            next_auto_increment_id: data.next_auto_increment_id.max(1),
        };
        table.rebuild_gist_indexes();
        Ok(table)
    }
}

impl Table {
    pub fn new(name: String, columns: Vec<String>) -> Self {
        Self {
            name,
            schema: TableSchema::default(),
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
        for index in self.typed_indexes.values_mut() {
            if let IndexType::GiST(index) = index {
                if let Some(value) = row.get(&index.column_name) {
                    index.insert(row_idx, value);
                }
            }
        }

        self.rows.push(row);
    }

    /// Append a batch, packing GiST entries once and reusing existing cached bounds.
    pub fn insert_batch(&mut self, rows: Vec<Row>) {
        if rows.len() <= 1 {
            for row in rows {
                self.insert(row);
            }
            return;
        }
        let start = self.rows.len();
        for index in self.indexes.values_mut() {
            for (offset, row) in rows.iter().enumerate() {
                index.insert(row, start + offset);
            }
        }
        for index in self.typed_indexes.values_mut() {
            if let IndexType::GiST(index) = index {
                let column = index.column_name.clone();
                index.insert_batch(rows.iter().enumerate().filter_map(|(offset, row)| {
                    row.get(&column).map(|value| (start + offset, value))
                }));
            }
        }
        self.rows.extend(rows);
    }

    /// Replace a validated row, maintaining only changed GiST column values.
    pub fn replace_row(&mut self, row_id: usize, row: Row) {
        let old = &self.rows[row_id];
        for index in self.typed_indexes.values_mut() {
            if let IndexType::GiST(index) = index {
                let previous = old.get(&index.column_name).unwrap_or(&Value::Null);
                let next = row.get(&index.column_name).unwrap_or(&Value::Null);
                if previous != next {
                    index.remove(row_id, previous);
                    index.insert(row_id, next);
                }
            }
        }
        self.rows[row_id] = row;
    }

    /// Retain row order, remove deleted entries, then remap surviving vector offsets.
    /// Only row IDs change during remapping; R-tree envelopes remain valid.
    pub fn retain_rows(&mut self, mut keep: impl FnMut(&Row) -> bool) {
        let mut offsets = Vec::with_capacity(self.rows.len());
        let mut next_id = 0;
        self.rows.retain(|row| {
            let row_id = offsets.len();
            if keep(row) {
                offsets.push(next_id);
                next_id += 1;
                true
            } else {
                for index in self.typed_indexes.values_mut() {
                    if let IndexType::GiST(index) = index {
                        let value = row.get(&index.column_name).unwrap_or(&Value::Null);
                        index.remove(row_id, value);
                    }
                }
                offsets.push(usize::MAX);
                false
            }
        });
        if self.rows.len() != offsets.len() {
            for index in self.typed_indexes.values_mut() {
                if let IndexType::GiST(index) = index {
                    index.remap_row_ids(&offsets);
                }
            }
        }
    }

    pub fn rebuild_gist_indexes(&mut self) {
        for def in &self.schema.gist_indexes {
            let index = self
                .typed_indexes
                .entry(def.name.clone())
                .or_insert_with(|| {
                    IndexType::GiST(GiSTIndex::new(
                        def.name.clone(),
                        def.column.clone(),
                        def.index_type,
                    ))
                });
            if let IndexType::GiST(index) = index {
                index.bulk_load(
                    self.rows
                        .iter()
                        .enumerate()
                        .filter_map(|(id, row)| row.get(&def.column).map(|value| (id, value))),
                );
            }
        }
    }

    /// Geometry accepts either a typed value or WKT. Other existing column
    /// coercion behavior is unchanged.
    pub fn coerce_values(&self, values: &mut [Value]) -> Result<()> {
        for (column, value) in self.columns.iter().zip(values) {
            if self.schema.column_types.get(column) == Some(&DataType::Geometry) {
                Self::coerce_geometry(value)?;
            }
        }
        Ok(())
    }

    pub fn coerce_geometry(value: &mut Value) -> Result<()> {
        match value {
            Value::Null | Value::Geometry(_) => Ok(()),
            Value::Text(text) => {
                *value = Value::Geometry(
                    crate::geometry::GeometryValue::from_wkt(text)
                        .map_err(crate::NexusError::QueryError)?,
                );
                Ok(())
            }
            _ => Err(crate::NexusError::QueryError(
                "GEOMETRY requires WKT or a geometry value".into(),
            )),
        }
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
