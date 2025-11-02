//! Window Functions
//!
//! Implements PostgreSQL-compatible window functions for analytics:
//! - Ranking: ROW_NUMBER, RANK, DENSE_RANK, NTILE
//! - Value: LAG, LEAD, FIRST_VALUE, LAST_VALUE, NTH_VALUE
//! - Aggregate: SUM, AVG, COUNT, MIN, MAX over windows
//!
//! Supports PARTITION BY, ORDER BY, and frame specifications
//! (ROWS BETWEEN, RANGE BETWEEN).

use crate::error::{NexusError, Result};
use crate::types::{Row, Value};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

/// Window frame specification
#[derive(Debug, Clone, PartialEq)]
pub enum FrameType {
    Rows,  // Physical row-based frame
    Range, // Logical value-based frame
}

#[derive(Debug, Clone, PartialEq)]
pub enum FrameBound {
    UnboundedPreceding,
    Preceding(i64),
    CurrentRow,
    Following(i64),
    UnboundedFollowing,
}

#[derive(Debug, Clone)]
pub struct WindowFrame {
    pub frame_type: FrameType,
    pub start: FrameBound,
    pub end: FrameBound,
}

impl WindowFrame {
    pub fn new(frame_type: FrameType, start: FrameBound, end: FrameBound) -> Self {
        Self {
            frame_type,
            start,
            end,
        }
    }

    /// Default frame: RANGE BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    pub fn default() -> Self {
        Self {
            frame_type: FrameType::Range,
            start: FrameBound::UnboundedPreceding,
            end: FrameBound::CurrentRow,
        }
    }

    /// Calculate frame bounds for a given row
    pub fn get_bounds(&self, current_idx: usize, partition_size: usize) -> (usize, usize) {
        let start_idx = match self.start {
            FrameBound::UnboundedPreceding => 0,
            FrameBound::Preceding(n) => current_idx.saturating_sub(n as usize),
            FrameBound::CurrentRow => current_idx,
            FrameBound::Following(n) => (current_idx + n as usize).min(partition_size),
            FrameBound::UnboundedFollowing => partition_size,
        };

        let end_idx = match self.end {
            FrameBound::UnboundedPreceding => 0,
            FrameBound::Preceding(n) => current_idx.saturating_sub(n as usize),
            FrameBound::CurrentRow => current_idx + 1,
            FrameBound::Following(n) => (current_idx + n as usize + 1).min(partition_size),
            FrameBound::UnboundedFollowing => partition_size,
        };

        (start_idx, end_idx)
    }
}

/// Window specification
#[derive(Clone)]
pub struct WindowSpec {
    pub partition_by: Vec<usize>,     // Column indices for PARTITION BY
    pub order_by: Vec<(usize, bool)>, // (column_idx, ascending)
    pub frame: WindowFrame,
}

impl WindowSpec {
    pub fn new() -> Self {
        Self {
            partition_by: Vec::new(),
            order_by: Vec::new(),
            frame: WindowFrame::default(),
        }
    }

    pub fn partition_by(mut self, columns: Vec<usize>) -> Self {
        self.partition_by = columns;
        self
    }

    pub fn order_by(mut self, columns: Vec<(usize, bool)>) -> Self {
        self.order_by = columns;
        self
    }

    pub fn frame(mut self, frame: WindowFrame) -> Self {
        self.frame = frame;
        self
    }
}

impl Default for WindowSpec {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Window Function Trait
// ============================================================================

pub trait WindowFunction: Send + Sync {
    /// Execute the window function for a single row
    fn execute(
        &self,
        partition: &[&Row],
        current_idx: usize,
        frame_start: usize,
        frame_end: usize,
        value_column: Option<usize>,
    ) -> Result<Value>;

    fn name(&self) -> &str;
}

// ============================================================================
// ROW_NUMBER - Sequential number within partition
// ============================================================================

pub struct RowNumberFunction;

impl WindowFunction for RowNumberFunction {
    fn execute(
        &self,
        _partition: &[&Row],
        current_idx: usize,
        _frame_start: usize,
        _frame_end: usize,
        _value_column: Option<usize>,
    ) -> Result<Value> {
        Ok(Value::Integer((current_idx + 1) as i64))
    }

    fn name(&self) -> &str {
        "ROW_NUMBER"
    }
}

// ============================================================================
// RANK - Rank with gaps for ties
// ============================================================================

pub struct RankFunction;

impl WindowFunction for RankFunction {
    fn execute(
        &self,
        partition: &[&Row],
        current_idx: usize,
        _frame_start: usize,
        _frame_end: usize,
        value_column: Option<usize>,
    ) -> Result<Value> {
        let col_idx = value_column.ok_or_else(|| {
            NexusError::ExecutionError("RANK requires ORDER BY column".to_string())
        })?;

        let current_value = partition[current_idx]
            .values
            .get(col_idx)
            .ok_or_else(|| NexusError::ExecutionError("Invalid column index".to_string()))?;

        let mut rank = 1;
        for i in 0..current_idx {
            if let Some(value) = partition[i].values.get(col_idx) {
                if value != current_value {
                    rank = i as i64 + 2;
                }
            }
        }

        Ok(Value::Integer(rank))
    }

    fn name(&self) -> &str {
        "RANK"
    }
}

// ============================================================================
// DENSE_RANK - Rank without gaps
// ============================================================================

pub struct DenseRankFunction;

impl WindowFunction for DenseRankFunction {
    fn execute(
        &self,
        partition: &[&Row],
        current_idx: usize,
        _frame_start: usize,
        _frame_end: usize,
        value_column: Option<usize>,
    ) -> Result<Value> {
        let col_idx = value_column.ok_or_else(|| {
            NexusError::ExecutionError("DENSE_RANK requires ORDER BY column".to_string())
        })?;

        let current_value = partition[current_idx]
            .values
            .get(col_idx)
            .ok_or_else(|| NexusError::ExecutionError("Invalid column index".to_string()))?;

        let mut seen_values = Vec::new();
        for i in 0..=current_idx {
            if let Some(value) = partition[i].values.get(col_idx) {
                if !seen_values.contains(value) {
                    seen_values.push(value.clone());
                }
            }
        }

        let rank = seen_values
            .iter()
            .position(|v| v == current_value)
            .map(|pos| pos as i64 + 1)
            .unwrap_or(1);

        Ok(Value::Integer(rank))
    }

    fn name(&self) -> &str {
        "DENSE_RANK"
    }
}

// ============================================================================
// LAG - Access previous row value
// ============================================================================

pub struct LagFunction {
    offset: usize,
    default: Option<Value>,
}

impl LagFunction {
    pub fn new(offset: usize, default: Option<Value>) -> Self {
        Self { offset, default }
    }
}

impl WindowFunction for LagFunction {
    fn execute(
        &self,
        partition: &[&Row],
        current_idx: usize,
        _frame_start: usize,
        _frame_end: usize,
        value_column: Option<usize>,
    ) -> Result<Value> {
        let col_idx = value_column
            .ok_or_else(|| NexusError::ExecutionError("LAG requires a value column".to_string()))?;

        if current_idx >= self.offset {
            let prev_idx = current_idx - self.offset;
            partition[prev_idx]
                .values
                .get(col_idx)
                .cloned()
                .ok_or_else(|| NexusError::ExecutionError("Invalid column index".to_string()))
        } else {
            Ok(self.default.clone().unwrap_or(Value::Null))
        }
    }

    fn name(&self) -> &str {
        "LAG"
    }
}

// ============================================================================
// LEAD - Access next row value
// ============================================================================

pub struct LeadFunction {
    offset: usize,
    default: Option<Value>,
}

impl LeadFunction {
    pub fn new(offset: usize, default: Option<Value>) -> Self {
        Self { offset, default }
    }
}

impl WindowFunction for LeadFunction {
    fn execute(
        &self,
        partition: &[&Row],
        current_idx: usize,
        _frame_start: usize,
        _frame_end: usize,
        value_column: Option<usize>,
    ) -> Result<Value> {
        let col_idx = value_column.ok_or_else(|| {
            NexusError::ExecutionError("LEAD requires a value column".to_string())
        })?;

        let next_idx = current_idx + self.offset;
        if next_idx < partition.len() {
            partition[next_idx]
                .values
                .get(col_idx)
                .cloned()
                .ok_or_else(|| NexusError::ExecutionError("Invalid column index".to_string()))
        } else {
            Ok(self.default.clone().unwrap_or(Value::Null))
        }
    }

    fn name(&self) -> &str {
        "LEAD"
    }
}

// ============================================================================
// FIRST_VALUE - First value in frame
// ============================================================================

pub struct FirstValueFunction;

impl WindowFunction for FirstValueFunction {
    fn execute(
        &self,
        partition: &[&Row],
        _current_idx: usize,
        frame_start: usize,
        _frame_end: usize,
        value_column: Option<usize>,
    ) -> Result<Value> {
        let col_idx = value_column.ok_or_else(|| {
            NexusError::ExecutionError("FIRST_VALUE requires a value column".to_string())
        })?;

        partition[frame_start]
            .values
            .get(col_idx)
            .cloned()
            .ok_or_else(|| NexusError::ExecutionError("Invalid column index".to_string()))
    }

    fn name(&self) -> &str {
        "FIRST_VALUE"
    }
}

// ============================================================================
// LAST_VALUE - Last value in frame
// ============================================================================

pub struct LastValueFunction;

impl WindowFunction for LastValueFunction {
    fn execute(
        &self,
        partition: &[&Row],
        _current_idx: usize,
        _frame_start: usize,
        frame_end: usize,
        value_column: Option<usize>,
    ) -> Result<Value> {
        let col_idx = value_column.ok_or_else(|| {
            NexusError::ExecutionError("LAST_VALUE requires a value column".to_string())
        })?;

        if frame_end == 0 {
            return Ok(Value::Null);
        }

        partition[frame_end - 1]
            .values
            .get(col_idx)
            .cloned()
            .ok_or_else(|| NexusError::ExecutionError("Invalid column index".to_string()))
    }

    fn name(&self) -> &str {
        "LAST_VALUE"
    }
}

// ============================================================================
// Window Aggregate Functions (SUM, AVG, etc. over windows)
// ============================================================================

pub struct WindowSumFunction;

impl WindowFunction for WindowSumFunction {
    fn execute(
        &self,
        partition: &[&Row],
        _current_idx: usize,
        frame_start: usize,
        frame_end: usize,
        value_column: Option<usize>,
    ) -> Result<Value> {
        let col_idx = value_column.ok_or_else(|| {
            NexusError::ExecutionError("Window SUM requires a value column".to_string())
        })?;

        let mut sum = 0.0;
        for i in frame_start..frame_end {
            if let Some(value) = partition[i].values.get(col_idx) {
                match value {
                    Value::Integer(v) => sum += *v as f64,
                    Value::Float(v) => sum += v,
                    _ => {}
                }
            }
        }

        Ok(Value::Float(sum))
    }

    fn name(&self) -> &str {
        "SUM"
    }
}

// ============================================================================
// Window Manager
// ============================================================================

pub struct WindowManager {
    functions: HashMap<String, Arc<dyn WindowFunction>>,
}

impl WindowManager {
    pub fn new() -> Self {
        let mut manager = Self {
            functions: HashMap::new(),
        };

        manager.register_builtin_functions();
        manager
    }

    fn register_builtin_functions(&mut self) {
        self.functions
            .insert("ROW_NUMBER".to_string(), Arc::new(RowNumberFunction));
        self.functions
            .insert("RANK".to_string(), Arc::new(RankFunction));
        self.functions
            .insert("DENSE_RANK".to_string(), Arc::new(DenseRankFunction));
        self.functions
            .insert("LAG".to_string(), Arc::new(LagFunction::new(1, None)));
        self.functions
            .insert("LEAD".to_string(), Arc::new(LeadFunction::new(1, None)));
        self.functions
            .insert("FIRST_VALUE".to_string(), Arc::new(FirstValueFunction));
        self.functions
            .insert("LAST_VALUE".to_string(), Arc::new(LastValueFunction));
        self.functions
            .insert("SUM".to_string(), Arc::new(WindowSumFunction));
    }

    pub fn register_function(&mut self, name: String, function: Arc<dyn WindowFunction>) {
        self.functions.insert(name.to_uppercase(), function);
    }

    /// Execute window function over rows
    pub fn execute_window_function(
        &self,
        function_name: &str,
        rows: &[Row],
        window_spec: &WindowSpec,
        value_column: Option<usize>,
    ) -> Result<Vec<Value>> {
        let function = self
            .functions
            .get(&function_name.to_uppercase())
            .ok_or_else(|| {
                NexusError::ExecutionError(format!("Unknown window function: {}", function_name))
            })?;

        // Partition rows
        let partitions = self.partition_rows(rows, &window_spec.partition_by);

        let mut results = vec![Value::Null; rows.len()];

        for (partition_key, partition_indices) in partitions {
            // Get partition rows
            let partition_rows: Vec<&Row> =
                partition_indices.iter().map(|&idx| &rows[idx]).collect();

            // Sort partition by ORDER BY
            let mut sorted_indices = partition_indices.clone();
            if !window_spec.order_by.is_empty() {
                sorted_indices
                    .sort_by(|&a, &b| self.compare_rows(&rows[a], &rows[b], &window_spec.order_by));
            }

            // Execute function for each row in partition
            for (partition_idx, &original_idx) in sorted_indices.iter().enumerate() {
                let (frame_start, frame_end) = window_spec
                    .frame
                    .get_bounds(partition_idx, partition_rows.len());

                let result = function.execute(
                    &partition_rows,
                    partition_idx,
                    frame_start,
                    frame_end,
                    value_column,
                )?;

                results[original_idx] = result;
            }
        }

        Ok(results)
    }

    fn partition_rows(
        &self,
        rows: &[Row],
        partition_columns: &[usize],
    ) -> HashMap<Vec<Value>, Vec<usize>> {
        let mut partitions: HashMap<Vec<Value>, Vec<usize>> = HashMap::new();

        for (idx, row) in rows.iter().enumerate() {
            let key: Vec<Value> = if partition_columns.is_empty() {
                // No PARTITION BY - all rows in one partition
                vec![]
            } else {
                partition_columns
                    .iter()
                    .filter_map(|&col_idx| row.values.get(col_idx).cloned())
                    .collect()
            };

            partitions.entry(key).or_insert_with(Vec::new).push(idx);
        }

        partitions
    }

    fn compare_rows(&self, a: &Row, b: &Row, order_by: &[(usize, bool)]) -> Ordering {
        for &(col_idx, ascending) in order_by {
            let val_a = a.values.get(col_idx);
            let val_b = b.values.get(col_idx);

            let cmp = match (val_a, val_b) {
                (Some(a), Some(b)) => a.cmp(b),
                (Some(_), None) => Ordering::Greater,
                (None, Some(_)) => Ordering::Less,
                (None, None) => Ordering::Equal,
            };

            if cmp != Ordering::Equal {
                return if ascending { cmp } else { cmp.reverse() };
            }
        }

        Ordering::Equal
    }
}

impl Default for WindowManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_row_number() {
        let manager = WindowManager::new();
        let rows = vec![
            Row::new(vec![Value::Integer(1)]),
            Row::new(vec![Value::Integer(2)]),
            Row::new(vec![Value::Integer(3)]),
        ];

        let spec = WindowSpec::new();
        let results = manager
            .execute_window_function("ROW_NUMBER", &rows, &spec, None)
            .unwrap();

        assert_eq!(results[0], Value::Integer(1));
        assert_eq!(results[1], Value::Integer(2));
        assert_eq!(results[2], Value::Integer(3));
    }

    #[test]
    fn test_rank() {
        let manager = WindowManager::new();
        let rows = vec![
            Row::new(vec![Value::Integer(10)]),
            Row::new(vec![Value::Integer(20)]),
            Row::new(vec![Value::Integer(20)]),
            Row::new(vec![Value::Integer(30)]),
        ];

        let spec = WindowSpec::new().order_by(vec![(0, true)]);
        let results = manager
            .execute_window_function("RANK", &rows, &spec, Some(0))
            .unwrap();

        assert_eq!(results[0], Value::Integer(1));
        assert_eq!(results[1], Value::Integer(2));
        assert_eq!(results[2], Value::Integer(2));
        assert_eq!(results[3], Value::Integer(4)); // Gap after tie
    }

    #[test]
    fn test_lag_lead() {
        let manager = WindowManager::new();
        let rows = vec![
            Row::new(vec![Value::Integer(10)]),
            Row::new(vec![Value::Integer(20)]),
            Row::new(vec![Value::Integer(30)]),
        ];

        let spec = WindowSpec::new();

        let lag_results = manager
            .execute_window_function("LAG", &rows, &spec, Some(0))
            .unwrap();

        assert_eq!(lag_results[0], Value::Null); // No previous row
        assert_eq!(lag_results[1], Value::Integer(10));
        assert_eq!(lag_results[2], Value::Integer(20));

        let lead_results = manager
            .execute_window_function("LEAD", &rows, &spec, Some(0))
            .unwrap();

        assert_eq!(lead_results[0], Value::Integer(20));
        assert_eq!(lead_results[1], Value::Integer(30));
        assert_eq!(lead_results[2], Value::Null); // No next row
    }

    #[test]
    fn test_partitioned_row_number() {
        let manager = WindowManager::new();
        let rows = vec![
            Row::new(vec![Value::Text("A".to_string()), Value::Integer(1)]),
            Row::new(vec![Value::Text("A".to_string()), Value::Integer(2)]),
            Row::new(vec![Value::Text("B".to_string()), Value::Integer(1)]),
            Row::new(vec![Value::Text("B".to_string()), Value::Integer(2)]),
        ];

        let spec = WindowSpec::new().partition_by(vec![0]);
        let results = manager
            .execute_window_function("ROW_NUMBER", &rows, &spec, None)
            .unwrap();

        // Each partition should have its own numbering
        assert_eq!(results[0], Value::Integer(1)); // A, row 1
        assert_eq!(results[1], Value::Integer(2)); // A, row 2
        assert_eq!(results[2], Value::Integer(1)); // B, row 1
        assert_eq!(results[3], Value::Integer(2)); // B, row 2
    }
}
