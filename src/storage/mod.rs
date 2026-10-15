//! Storage Layer - Persistence
//! PROPRIETARY

pub mod disk;
pub mod wal;

use crate::fulltext::FullTextIndex;
use crate::index::{BRINIndex, BTreeIndex, GINIndex, GiSTIndex};
use crate::vector::sql::{VectorColumnIndex, VectorIndexDefinition};
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
    /// Hierarchical Navigable Small World — for high-dimensional vector search
    /// (`USING hnsw`). The graph is built on first use; see `vector::sql`.
    HNSW(VectorColumnIndex),
    /// Generalized Inverted Index — for arrays / JSON / full-text tokens.
    GIN(GINIndex),
    /// Generalized Search Tree — for geometric / spatial / range types.
    GiST(GiSTIndex),
    /// Block Range Index — compact summary index for time-series.
    BRIN(BRINIndex),
    /// Inverted text index with BM25 statistics (`USING FULLTEXT`).
    FullText(FullTextIndex),
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
    /// Appended field: `decode` still reads schemas written before it existed.
    #[serde(default)]
    pub fulltext_indexes: Vec<FullTextDefinition>,
    /// Appended field: HNSW definitions; graphs are rebuilt on first use.
    #[serde(default)]
    pub vector_indexes: Vec<VectorIndexDefinition>,
}

/// Schema layout before full-text indexes, for reading older databases.
#[derive(Deserialize)]
struct TableSchemaV1 {
    column_types: HashMap<String, DataType>,
    gist_indexes: Vec<GiSTDefinition>,
}

/// Schema layout before HNSW definitions were persisted.
#[derive(Deserialize)]
struct TableSchemaV2 {
    column_types: HashMap<String, DataType>,
    gist_indexes: Vec<GiSTDefinition>,
    fulltext_indexes: Vec<FullTextDefinition>,
}

impl TableSchema {
    /// Decode a bincode `table:<name>:schema` value of any known layout.
    /// bincode is not self-describing, so a missing trailing field fails
    /// the current layout and is retried as the previous ones.
    pub fn decode(bytes: &[u8]) -> bincode::Result<Self> {
        bincode::deserialize::<TableSchema>(bytes).or_else(|error| {
            bincode::deserialize::<TableSchemaV2>(bytes)
                .map(|old| TableSchema {
                    column_types: old.column_types,
                    gist_indexes: old.gist_indexes,
                    fulltext_indexes: old.fulltext_indexes,
                    vector_indexes: Vec::new(),
                })
                .or_else(|_| {
                    bincode::deserialize::<TableSchemaV1>(bytes).map(|old| TableSchema {
                        column_types: old.column_types,
                        gist_indexes: old.gist_indexes,
                        fulltext_indexes: Vec::new(),
                        vector_indexes: Vec::new(),
                    })
                })
                .map_err(|_| error)
        })
    }

    /// Forget every persisted definition named `name` (it is being replaced).
    pub fn forget_index(&mut self, name: &str) {
        self.gist_indexes.retain(|def| def.name != name);
        self.fulltext_indexes.retain(|def| def.name != name);
        self.vector_indexes.retain(|def| def.name != name);
    }
}

/// Persisted definition of a full-text index; its postings are rebuilt on load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullTextDefinition {
    pub name: String,
    pub column: String,
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
        table.rebuild_indexes();
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

    /// Give a NULL AUTO_INCREMENT value the next id. Explicit ids are kept and
    /// do not move the counter (PostgreSQL sequence semantics).
    pub fn assign_auto_increment(&mut self, values: &mut [Value]) {
        let Some(column) = &self.auto_increment_column else {
            return;
        };
        if let Some(i) = self.columns.iter().position(|c| c == column) {
            if values.get(i) == Some(&Value::Null) {
                values[i] = Value::Integer(self.next_auto_increment_id);
                self.next_auto_increment_id += 1;
            }
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
        for index in self.indexes.values_mut() {
            index.insert(&row, row_idx);
        }
        for index in self.typed_indexes.values_mut() {
            match index {
                IndexType::GiST(index) => {
                    if let Some(value) = row.get(&index.column_name) {
                        index.insert(row_idx, value);
                    }
                }
                IndexType::FullText(index) => index.push_row(row.get(&index.column)),
                IndexType::HNSW(index) => {
                    let column = index.def.column.clone();
                    index.push_row(row_idx, row.get(&column));
                }
                _ => {}
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
            match index {
                IndexType::GiST(index) => {
                    let column = index.column_name.clone();
                    index.insert_batch(rows.iter().enumerate().filter_map(|(offset, row)| {
                        row.get(&column).map(|value| (start + offset, value))
                    }));
                }
                IndexType::FullText(index) => {
                    for row in &rows {
                        index.push_row(row.get(&index.column));
                    }
                }
                IndexType::HNSW(index) => {
                    let column = index.def.column.clone();
                    for (offset, row) in rows.iter().enumerate() {
                        index.push_row(start + offset, row.get(&column));
                    }
                }
                _ => {}
            }
        }
        self.rows.extend(rows);
    }

    /// Replace a validated row, maintaining only changed GiST column values.
    pub fn replace_row(&mut self, row_id: usize, row: Row) {
        let old = &self.rows[row_id];
        for index in self.typed_indexes.values_mut() {
            match index {
                IndexType::GiST(index) => {
                    let previous = old.get(&index.column_name).unwrap_or(&Value::Null);
                    let next = row.get(&index.column_name).unwrap_or(&Value::Null);
                    if previous != next {
                        index.remove(row_id, previous);
                        index.insert(row_id, next);
                    }
                }
                IndexType::FullText(index) => {
                    index.replace_row(row_id, old.get(&index.column), row.get(&index.column))
                }
                IndexType::HNSW(index) => {
                    let column = index.def.column.clone();
                    index.replace_row(row_id, old.get(&column), row.get(&column))
                }
                _ => {}
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
                    match index {
                        IndexType::GiST(index) => {
                            let value = row.get(&index.column_name).unwrap_or(&Value::Null);
                            index.remove(row_id, value);
                        }
                        IndexType::FullText(index) => {
                            index.remove_row(row_id, row.get(&index.column))
                        }
                        _ => {}
                    }
                }
                offsets.push(usize::MAX);
                false
            }
        });
        if self.rows.len() != offsets.len() {
            for index in self.typed_indexes.values_mut() {
                match index {
                    IndexType::GiST(index) => index.remap_row_ids(&offsets),
                    IndexType::FullText(index) => index.remap_row_ids(&offsets),
                    IndexType::HNSW(index) => index.remap(&offsets),
                    _ => {}
                }
            }
        }
    }

    /// Drop rows from offset `len` onwards, maintaining indexes. Undoes an
    /// append-only statement without snapshotting the table.
    pub fn truncate_rows(&mut self, len: usize) {
        if self.rows.len() <= len {
            return;
        }
        let mut seen = 0;
        self.retain_rows(|_| {
            seen += 1;
            seen <= len
        });
        for index in self.indexes.values_mut() {
            index.build_from_rows(&self.rows);
        }
    }

    /// Put deleted rows back at their original offsets (ascending), then
    /// rebuild the indexes. Only used to undo a failed statement.
    pub fn restore_deleted(&mut self, rows: Vec<(usize, Row)>) {
        for (offset, row) in rows {
            let at = offset.min(self.rows.len());
            self.rows.insert(at, row);
        }
        for index in self.indexes.values_mut() {
            index.build_from_rows(&self.rows);
        }
        self.rebuild_indexes();
    }

    /// Rebuild every index whose definition is persisted in the schema.
    pub fn rebuild_indexes(&mut self) {
        self.rebuild_gist_indexes();
        self.rebuild_fulltext_indexes();
        self.rebuild_vector_indexes();
    }

    /// Recreate HNSW indexes from their definitions. Graphs stay unbuilt until
    /// a nearest-neighbor query needs one.
    pub fn rebuild_vector_indexes(&mut self) {
        for def in &self.schema.vector_indexes {
            self.typed_indexes.insert(
                def.name.clone(),
                IndexType::HNSW(VectorColumnIndex::new(def.clone())),
            );
        }
    }

    pub fn rebuild_fulltext_indexes(&mut self) {
        for def in &self.schema.fulltext_indexes {
            let mut index = FullTextIndex::new(def.name.clone(), def.column.clone());
            index.build(self.rows.iter().map(|row| row.get(&def.column)));
            self.typed_indexes
                .insert(def.name.clone(), IndexType::FullText(index));
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

#[cfg(test)]
mod schema_tests {
    use super::*;

    #[derive(Serialize)]
    struct TableSchemaV1Ser {
        column_types: HashMap<String, DataType>,
        gist_indexes: Vec<GiSTDefinition>,
    }

    #[test]
    fn schema_written_before_fulltext_indexes_still_decodes() {
        let old = TableSchemaV1Ser {
            column_types: [("geom".to_string(), DataType::Geometry)].into(),
            gist_indexes: vec![GiSTDefinition {
                name: "g".into(),
                column: "geom".into(),
                index_type: crate::index::GiSTIndexType::RTree,
            }],
        };
        let decoded = TableSchema::decode(&bincode::serialize(&old).unwrap()).unwrap();
        assert_eq!(decoded.gist_indexes.len(), 1);
        assert_eq!(decoded.column_types["geom"], DataType::Geometry);
        assert!(decoded.fulltext_indexes.is_empty());

        let current = TableSchema {
            fulltext_indexes: vec![FullTextDefinition {
                name: "f".into(),
                column: "body".into(),
            }],
            ..decoded
        };
        let round = TableSchema::decode(&bincode::serialize(&current).unwrap()).unwrap();
        assert_eq!(round.fulltext_indexes[0].column, "body");
        assert_eq!(round.gist_indexes[0].name, "g");
        assert!(TableSchema::decode(b"\x01").is_err());
    }

    #[derive(Serialize)]
    struct TableSchemaV2Ser {
        column_types: HashMap<String, DataType>,
        gist_indexes: Vec<GiSTDefinition>,
        fulltext_indexes: Vec<FullTextDefinition>,
    }

    #[test]
    fn schema_written_before_vector_indexes_still_decodes() {
        let old = TableSchemaV2Ser {
            column_types: HashMap::new(),
            gist_indexes: Vec::new(),
            fulltext_indexes: vec![FullTextDefinition {
                name: "f".into(),
                column: "body".into(),
            }],
        };
        let decoded = TableSchema::decode(&bincode::serialize(&old).unwrap()).unwrap();
        assert_eq!(decoded.fulltext_indexes[0].name, "f");
        assert!(decoded.vector_indexes.is_empty());
        let current = TableSchema {
            vector_indexes: vec![VectorIndexDefinition {
                name: "h".into(),
                column: "e".into(),
                metric: crate::vector::sql::VectorMetric::Cosine,
                m: 16,
                ef_construction: 64,
                ef_search: 40,
            }],
            ..decoded
        };
        let round = TableSchema::decode(&bincode::serialize(&current).unwrap()).unwrap();
        assert_eq!(round.vector_indexes[0].column, "e");
        assert_eq!(round.fulltext_indexes[0].column, "body");
    }

    #[test]
    fn restore_deleted_puts_rows_and_index_entries_back() {
        let mut table = Table::new("t".into(), vec!["body".into()]);
        table.schema.fulltext_indexes.push(FullTextDefinition {
            name: "f".into(),
            column: "body".into(),
        });
        table.rebuild_indexes();
        for text in ["alpha one", "beta two", "alpha three"] {
            table.insert(Row::new(vec!["body".into()], vec![Value::Text(text.into())]));
        }
        let removed: Vec<(usize, Row)> = vec![(0, table.rows[0].clone()), (2, table.rows[2].clone())];
        table.retain_rows(|row| row.get("body") == Some(&Value::Text("beta two".into())));
        table.restore_deleted(removed);
        let bodies: Vec<_> = table.rows.iter().map(|r| r.get("body").cloned()).collect();
        assert_eq!(
            bodies,
            ["alpha one", "beta two", "alpha three"].map(|t| Some(Value::Text(t.into())))
        );
        let Some(IndexType::FullText(index)) = table.typed_indexes.get("f") else {
            panic!("index rebuilt");
        };
        let query = crate::fulltext::Query::parse("alpha").unwrap();
        assert_eq!(index.search(&query), vec![0, 2]);
    }

    #[test]
    fn table_rebuilds_fulltext_index_from_schema() {
        let mut table = Table::new("t".into(), vec!["body".into()]);
        table.schema.fulltext_indexes.push(FullTextDefinition {
            name: "f".into(),
            column: "body".into(),
        });
        table.insert(Row::new(vec!["body".into()], vec![Value::Text("hello world".into())]));
        table.rebuild_indexes();
        let Some(IndexType::FullText(index)) = table.typed_indexes.get("f") else {
            panic!("index rebuilt");
        };
        let query = crate::fulltext::Query::parse("hello").unwrap();
        assert_eq!(index.search(&query), vec![0]);
    }
}
