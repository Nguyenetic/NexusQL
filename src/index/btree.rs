//! B-Tree Index for Fast Point Queries
//!
//! Implements a B-Tree index using Rust's BTreeMap for O(log n) lookups.
//! This is simpler and more efficient than implementing a custom B-Tree from scratch.

use crate::types::{Row, Value};
use std::collections::BTreeMap;

/// B-Tree index for a single column
#[derive(Debug, Clone)]
pub struct BTreeIndex {
    /// Column name being indexed
    pub column_name: String,

    /// Column index in the row
    column_idx: usize,

    /// Index mapping values to row indices
    /// Key: indexed value, Value: list of row indices
    index: BTreeMap<IndexKey, Vec<usize>>,

    /// Total entries in index
    pub entry_count: usize,
}

/// Key type for B-Tree index that supports common SQL types
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum IndexKey {
    Integer(i64),
    Float(OrderedFloat),
    Text(String),
    Boolean(bool),
    Null,
}

/// Wrapper for f64 that implements Ord (needed for BTreeMap)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrderedFloat(f64);

impl Eq for OrderedFloat {}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .partial_cmp(&other.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl From<&Value> for IndexKey {
    fn from(value: &Value) -> Self {
        match value {
            Value::Integer(i) => IndexKey::Integer(*i),
            Value::Float(f) => IndexKey::Float(OrderedFloat(*f)),
            Value::Text(s) => IndexKey::Text(s.clone()),
            Value::Boolean(b) => IndexKey::Boolean(*b),
            Value::Null => IndexKey::Null,
            Value::Blob(_) => IndexKey::Null, // Blobs are not indexable, treat as NULL
        }
    }
}

impl BTreeIndex {
    /// Create a new B-Tree index for a column
    pub fn new(column_name: String, column_idx: usize) -> Self {
        Self {
            column_name,
            column_idx,
            index: BTreeMap::new(),
            entry_count: 0,
        }
    }

    /// Build index from a set of rows
    pub fn build_from_rows(&mut self, rows: &[Row]) {
        self.index.clear();
        self.entry_count = 0;

        for (row_idx, row) in rows.iter().enumerate() {
            if let Some(value) = row.values.get(self.column_idx) {
                let key = IndexKey::from(value);
                self.index.entry(key).or_insert_with(Vec::new).push(row_idx);
                self.entry_count += 1;
            }
        }
    }

    /// Insert a single row into the index
    pub fn insert(&mut self, row: &Row, row_idx: usize) {
        if let Some(value) = row.values.get(self.column_idx) {
            let key = IndexKey::from(value);
            self.index.entry(key).or_insert_with(Vec::new).push(row_idx);
            self.entry_count += 1;
        }
    }

    /// Look up row indices by exact value
    pub fn lookup(&self, value: &Value) -> Option<&Vec<usize>> {
        let key = IndexKey::from(value);
        self.index.get(&key)
    }

    /// Range query: find all values between min and max (inclusive)
    pub fn range_lookup(&self, min: &Value, max: &Value) -> Vec<usize> {
        let min_key = IndexKey::from(min);
        let max_key = IndexKey::from(max);

        let mut result = Vec::new();
        for (_, row_indices) in self.index.range(min_key..=max_key) {
            result.extend_from_slice(row_indices);
        }
        result
    }

    /// Greater than query
    pub fn greater_than(&self, value: &Value) -> Vec<usize> {
        let key = IndexKey::from(value);

        let mut result = Vec::new();
        for (k, row_indices) in self.index.iter() {
            if k > &key {
                result.extend_from_slice(row_indices);
            }
        }
        result
    }

    /// Less than query
    pub fn less_than(&self, value: &Value) -> Vec<usize> {
        let key = IndexKey::from(value);

        let mut result = Vec::new();
        for (k, row_indices) in self.index.iter() {
            if k < &key {
                result.extend_from_slice(row_indices);
            }
        }
        result
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Get statistics about the index
    pub fn stats(&self) -> IndexStats {
        let unique_values = self.index.len();
        let total_entries = self.entry_count;
        let avg_entries_per_value = if unique_values > 0 {
            total_entries as f64 / unique_values as f64
        } else {
            0.0
        };

        IndexStats {
            column_name: self.column_name.clone(),
            unique_values,
            total_entries,
            avg_entries_per_value,
        }
    }
}

/// Statistics about an index
#[derive(Debug, Clone)]
pub struct IndexStats {
    pub column_name: String,
    pub unique_values: usize,
    pub total_entries: usize,
    pub avg_entries_per_value: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_row(id: i64, age: i64) -> Row {
        Row::new(
            vec!["id".to_string(), "age".to_string()],
            vec![Value::Integer(id), Value::Integer(age)],
        )
    }

    #[test]
    fn test_btree_insert_and_lookup() {
        let mut index = BTreeIndex::new("id".to_string(), 0);

        let row1 = create_test_row(1, 25);
        let row2 = create_test_row(2, 30);
        let row3 = create_test_row(1, 25); // Duplicate id

        index.insert(&row1, 0);
        index.insert(&row2, 1);
        index.insert(&row3, 2);

        // Lookup id=1 should return rows 0 and 2
        let result = index.lookup(&Value::Integer(1)).unwrap();
        assert_eq!(result, &vec![0, 2]);

        // Lookup id=2 should return row 1
        let result = index.lookup(&Value::Integer(2)).unwrap();
        assert_eq!(result, &vec![1]);

        // Lookup non-existent id=3
        assert!(index.lookup(&Value::Integer(3)).is_none());
    }

    #[test]
    fn test_range_lookup() {
        let mut index = BTreeIndex::new("age".to_string(), 1);

        let rows = vec![
            create_test_row(1, 20),
            create_test_row(2, 30),
            create_test_row(3, 40),
            create_test_row(4, 50),
        ];

        index.build_from_rows(&rows);

        // Range 25-45 should return rows with age 30 and 40 (indices 1 and 2)
        let result = index.range_lookup(&Value::Integer(25), &Value::Integer(45));
        assert_eq!(result.len(), 2);
        assert!(result.contains(&1));
        assert!(result.contains(&2));
    }

    #[test]
    fn test_greater_than() {
        let mut index = BTreeIndex::new("age".to_string(), 1);

        let rows = vec![
            create_test_row(1, 20),
            create_test_row(2, 30),
            create_test_row(3, 40),
        ];

        index.build_from_rows(&rows);

        // Greater than 25 should return rows 1 and 2 (age 30 and 40)
        let result = index.greater_than(&Value::Integer(25));
        assert_eq!(result.len(), 2);
    }
}
