//! BRIN (Block Range Index)
//!
//! PostgreSQL-compatible block range index for very large tables.
//! Extremely space-efficient for naturally sorted data (time-series, auto-increment IDs).
//!
//! Key characteristics:
//! - Stores min/max values per block range instead of per row
//! - Tiny index size (100-1000x smaller than B-Tree)
//! - Best for correlated data (insertion order matches sort order)
//! - Lossy: may return false positives, requires heap access verification
//!
//! Based on PostgreSQL BRIN implementation

use crate::types::Value;
use ordered_float::OrderedFloat;

/// BRIN Index - Block Range Index
#[derive(Debug, Clone)]
pub struct BRINIndex {
    /// Index name
    pub name: String,

    /// Column being indexed
    pub column_name: String,

    /// Block range summaries
    ranges: Vec<BlockRange>,

    /// Pages per range (default: 128 pages = ~1MB per range)
    pages_per_range: usize,

    /// Total entries indexed
    pub entry_count: usize,
}

/// Summary of a block range
#[derive(Debug, Clone)]
struct BlockRange {
    /// Starting row ID of this range
    start_row_id: usize,

    /// Ending row ID of this range (exclusive)
    end_row_id: usize,

    /// Minimum value in this range
    min_value: Option<BRINValue>,

    /// Maximum value in this range
    max_value: Option<BRINValue>,

    /// Has NULL values?
    has_nulls: bool,

    /// Number of values in this range
    value_count: usize,
}

/// Value types supported by BRIN
#[derive(Debug, Clone, PartialEq, PartialOrd)]
enum BRINValue {
    Integer(i64),
    Float(OrderedFloat<f64>),
    Text(String),
    Boolean(bool),
}

impl From<&Value> for Option<BRINValue> {
    fn from(value: &Value) -> Self {
        match value {
            Value::Integer(i) => Some(BRINValue::Integer(*i)),
            Value::Float(f) => Some(BRINValue::Float(*f)),
            Value::Text(s) => Some(BRINValue::Text(s.clone())),
            Value::Boolean(b) => Some(BRINValue::Boolean(*b)),
            Value::Null => None,
            _ => None,
        }
    }
}

impl BlockRange {
    fn new(start_row_id: usize, range_size: usize) -> Self {
        Self {
            start_row_id,
            end_row_id: start_row_id + range_size,
            min_value: None,
            max_value: None,
            has_nulls: false,
            value_count: 0,
        }
    }

    /// Update range with a new value
    fn update(&mut self, value: &Value) {
        if matches!(value, Value::Null) {
            self.has_nulls = true;
            return;
        }

        if let Some(brin_value) = Option::<BRINValue>::from(value) {
            // Update min
            self.min_value = Some(match &self.min_value {
                None => brin_value.clone(),
                Some(current_min) => {
                    if brin_value < *current_min {
                        brin_value.clone()
                    } else {
                        current_min.clone()
                    }
                }
            });

            // Update max
            self.max_value = Some(match &self.max_value {
                None => brin_value,
                Some(current_max) => {
                    if brin_value > *current_max {
                        brin_value
                    } else {
                        current_max.clone()
                    }
                }
            });

            self.value_count += 1;
        }
    }

    /// Check if value might be in this range
    fn might_contain(&self, value: &Value) -> bool {
        if matches!(value, Value::Null) {
            return self.has_nulls;
        }

        if let Some(brin_value) = Option::<BRINValue>::from(value) {
            match (&self.min_value, &self.max_value) {
                (Some(min), Some(max)) => brin_value >= *min && brin_value <= *max,
                _ => true, // If no min/max, assume it might be there
            }
        } else {
            false
        }
    }

    /// Check if value is less than range minimum
    fn is_less_than_min(&self, value: &BRINValue) -> bool {
        self.min_value.as_ref().map_or(false, |min| value < min)
    }

    /// Check if value is greater than range maximum
    fn is_greater_than_max(&self, value: &BRINValue) -> bool {
        self.max_value.as_ref().map_or(false, |max| value > max)
    }

    /// Check if range overlaps with [min, max]
    fn overlaps_range(&self, query_min: &BRINValue, query_max: &BRINValue) -> bool {
        match (&self.min_value, &self.max_value) {
            (Some(range_min), Some(range_max)) => {
                // Ranges overlap if: range_min <= query_max AND range_max >= query_min
                range_min <= query_max && range_max >= query_min
            }
            _ => true, // If no bounds, assume overlap
        }
    }
}

impl BRINIndex {
    /// Create a new BRIN index
    ///
    /// # Arguments
    /// * `name` - Index name
    /// * `column_name` - Column being indexed
    /// * `pages_per_range` - Number of pages per range (default: 128)
    ///   Each page is ~8KB, so 128 pages = ~1MB per range
    pub fn new(name: String, column_name: String, pages_per_range: Option<usize>) -> Self {
        Self {
            name,
            column_name,
            ranges: Vec::new(),
            pages_per_range: pages_per_range.unwrap_or(128),
            entry_count: 0,
        }
    }

    /// Insert a value into the index
    /// Assumes sequential row IDs (0, 1, 2, ...)
    pub fn insert(&mut self, row_id: usize, value: &Value) {
        // Determine which range this row belongs to
        let range_index = row_id / self.rows_per_range();

        // Ensure we have enough ranges
        while self.ranges.len() <= range_index {
            let start_row_id = self.ranges.len() * self.rows_per_range();
            self.ranges
                .push(BlockRange::new(start_row_id, self.rows_per_range()));
        }

        // Update the range
        self.ranges[range_index].update(value);
        self.entry_count += 1;
    }

    /// Rows per range (assuming 100 rows per page as heuristic)
    fn rows_per_range(&self) -> usize {
        self.pages_per_range * 100
    }

    /// Search for rows with exact value match
    /// Returns ranges that might contain the value (may have false positives)
    pub fn search_equals(&self, value: &Value) -> Vec<usize> {
        let mut candidate_rows = Vec::new();

        for range in &self.ranges {
            if range.might_contain(value) {
                // Return all row IDs in this range
                for row_id in range.start_row_id..range.end_row_id.min(self.entry_count) {
                    candidate_rows.push(row_id);
                }
            }
        }

        candidate_rows
    }

    /// Search for rows with value < target
    pub fn search_less_than(&self, value: &Value) -> Vec<usize> {
        if let Some(brin_value) = Option::<BRINValue>::from(value) {
            let mut candidate_rows = Vec::new();

            for range in &self.ranges {
                // Include range if its minimum is less than target
                // (entire range might be less than target, or might overlap)
                if !range.is_greater_than_max(&brin_value) {
                    for row_id in range.start_row_id..range.end_row_id.min(self.entry_count) {
                        candidate_rows.push(row_id);
                    }
                }
            }

            candidate_rows
        } else {
            vec![]
        }
    }

    /// Search for rows with value > target
    pub fn search_greater_than(&self, value: &Value) -> Vec<usize> {
        if let Some(brin_value) = Option::<BRINValue>::from(value) {
            let mut candidate_rows = Vec::new();

            for range in &self.ranges {
                // Include range if its maximum is greater than target
                if !range.is_less_than_min(&brin_value) {
                    for row_id in range.start_row_id..range.end_row_id.min(self.entry_count) {
                        candidate_rows.push(row_id);
                    }
                }
            }

            candidate_rows
        } else {
            vec![]
        }
    }

    /// Search for rows in range [min, max]
    pub fn search_range(&self, min_value: &Value, max_value: &Value) -> Vec<usize> {
        if let (Some(brin_min), Some(brin_max)) = (
            Option::<BRINValue>::from(min_value),
            Option::<BRINValue>::from(max_value),
        ) {
            let mut candidate_rows = Vec::new();

            for range in &self.ranges {
                if range.overlaps_range(&brin_min, &brin_max) {
                    for row_id in range.start_row_id..range.end_row_id.min(self.entry_count) {
                        candidate_rows.push(row_id);
                    }
                }
            }

            candidate_rows
        } else {
            vec![]
        }
    }

    /// Get index statistics
    pub fn stats(&self) -> BRINStats {
        let total_ranges = self.ranges.len();
        let index_size_bytes = total_ranges * std::mem::size_of::<BlockRange>();

        // Calculate compression ratio
        // If we used B-Tree, we'd need ~24 bytes per entry (key + pointer)
        let btree_size_estimate = self.entry_count * 24;
        let compression_ratio = if index_size_bytes > 0 {
            btree_size_estimate as f64 / index_size_bytes as f64
        } else {
            0.0
        };

        BRINStats {
            total_ranges,
            pages_per_range: self.pages_per_range,
            entries_indexed: self.entry_count,
            index_size_bytes,
            compression_ratio,
        }
    }
}

/// BRIN Index Statistics
#[derive(Debug, Clone)]
pub struct BRINStats {
    pub total_ranges: usize,
    pub pages_per_range: usize,
    pub entries_indexed: usize,
    pub index_size_bytes: usize,
    pub compression_ratio: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brin_sequential_data() {
        let mut index = BRINIndex::new(
            "timestamp_idx".to_string(),
            "created_at".to_string(),
            Some(128),
        );

        // Insert sequential timestamps (naturally sorted)
        for i in 0..1000 {
            index.insert(i, &Value::Integer(i as i64));
        }

        // Search for value 500
        let results = index.search_equals(&Value::Integer(500));
        assert!(results.contains(&500));

        // BRIN returns a range, not exact match
        // So results will include other values in the same block
        assert!(results.len() > 1);
    }

    #[test]
    fn test_brin_range_query() {
        let mut index = BRINIndex::new(
            "id_idx".to_string(),
            "id".to_string(),
            Some(10), // Small range for testing
        );

        // Insert 100 sequential values
        for i in 0..100 {
            index.insert(i, &Value::Integer(i as i64));
        }

        // Search for values in range [25, 35]
        let results = index.search_range(&Value::Integer(25), &Value::Integer(35));

        // Should include rows 25-35 and possibly more (due to block granularity)
        assert!(results.contains(&25));
        assert!(results.contains(&35));
    }

    #[test]
    fn test_brin_greater_than() {
        let mut index = BRINIndex::new("value_idx".to_string(), "value".to_string(), Some(10));

        for i in 0..50 {
            index.insert(i, &Value::Integer(i as i64));
        }

        // Search for values > 40
        let results = index.search_greater_than(&Value::Integer(40));

        // Should include rows 41-49
        assert!(results.contains(&41));
        assert!(results.contains(&49));
    }

    #[test]
    fn test_brin_compression() {
        let mut index = BRINIndex::new("test_idx".to_string(), "value".to_string(), Some(100));

        // Insert 10,000 entries
        for i in 0..10_000 {
            index.insert(i, &Value::Integer(i as i64));
        }

        let stats = index.stats();

        // BRIN should be much smaller than B-Tree
        assert!(stats.compression_ratio > 10.0);
        assert_eq!(stats.entries_indexed, 10_000);
    }

    #[test]
    fn test_brin_nulls() {
        let mut index = BRINIndex::new("nullable_idx".to_string(), "data".to_string(), Some(10));

        index.insert(0, &Value::Integer(10));
        index.insert(1, &Value::Null);
        index.insert(2, &Value::Integer(20));

        let results = index.search_equals(&Value::Null);
        assert!(results.contains(&1));
    }
}
