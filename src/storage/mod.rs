//! Storage Layer - Persistence
//! PROPRIETARY

pub mod disk;
pub mod wal;

use crate::index::BTreeIndex;
use crate::types::{Row, Value};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Serializable table data (without indexes)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TableData {
    name: String,
    columns: Vec<String>,
    rows: Vec<Row>,
}

/// Table storage with index support
#[derive(Debug, Clone)]
pub struct Table {
    pub name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Row>,

    /// Indexes: column_name -> BTreeIndex
    /// Note: Indexes are not persisted, they must be rebuilt on load
    pub indexes: HashMap<String, BTreeIndex>,
}

impl Serialize for Table {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let data = TableData {
            name: self.name.clone(),
            columns: self.columns.clone(),
            rows: self.rows.clone(),
        };
        data.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Table {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = TableData::deserialize(deserializer)?;
        Ok(Table {
            name: data.name,
            columns: data.columns,
            rows: data.rows,
            indexes: HashMap::new(), // Indexes must be rebuilt
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
