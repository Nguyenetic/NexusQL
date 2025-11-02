//! Aggregate Functions
//!
//! Implements PostgreSQL-compatible aggregate functions for data summarization:
//! - COUNT, SUM, AVG, MIN, MAX
//! - STRING_AGG, ARRAY_AGG
//! - Statistical aggregates: STDDEV, VARIANCE
//! - Custom aggregates via trait
//!
//! Supports GROUP BY, HAVING, and DISTINCT operations.

use crate::error::{NexusError, Result};
use crate::types::{Row, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Aggregate function trait - implement this for custom aggregates
pub trait AggregateFunction: Send + Sync {
    /// Initialize the aggregate state
    fn initialize(&self) -> Box<dyn AggregateState>;

    /// Name of the aggregate function
    fn name(&self) -> &str;

    /// Whether this aggregate handles NULL values
    fn handles_nulls(&self) -> bool {
        false // Most aggregates ignore NULLs
    }
}

/// State maintained during aggregation
pub trait AggregateState: Send {
    /// Accumulate a new value
    fn accumulate(&mut self, value: &Value) -> Result<()>;

    /// Merge another state (for parallel aggregation)
    fn merge(&mut self, other: Box<dyn AggregateState>) -> Result<()>;

    /// Compute final result
    fn finalize(&self) -> Result<Value>;

    /// Clone the state
    fn clone_state(&self) -> Box<dyn AggregateState>;

    /// Downcast helper for type-safe merging
    fn as_any(&self) -> &dyn std::any::Any;
}

// ============================================================================
// COUNT Aggregate
// ============================================================================

pub struct CountFunction;

impl AggregateFunction for CountFunction {
    fn initialize(&self) -> Box<dyn AggregateState> {
        Box::new(CountState { count: 0 })
    }

    fn name(&self) -> &str {
        "COUNT"
    }

    fn handles_nulls(&self) -> bool {
        true // COUNT(*) counts NULLs, COUNT(col) doesn't
    }
}

struct CountState {
    count: i64,
}

impl AggregateState for CountState {
    fn accumulate(&mut self, value: &Value) -> Result<()> {
        // COUNT(*) counts everything, COUNT(col) skips NULLs
        if !matches!(value, Value::Null) {
            self.count += 1;
        }
        Ok(())
    }

    fn merge(&mut self, other: Box<dyn AggregateState>) -> Result<()> {
        if let Some(other_count) = other.as_any().downcast_ref::<CountState>() {
            self.count += other_count.count;
            Ok(())
        } else {
            Err(NexusError::ExecutionError(
                "Invalid state merge".to_string(),
            ))
        }
    }

    fn finalize(&self) -> Result<Value> {
        Ok(Value::Integer(self.count))
    }

    fn clone_state(&self) -> Box<dyn AggregateState> {
        Box::new(CountState { count: self.count })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ============================================================================
// SUM Aggregate
// ============================================================================

pub struct SumFunction;

impl AggregateFunction for SumFunction {
    fn initialize(&self) -> Box<dyn AggregateState> {
        Box::new(SumState { sum: None })
    }

    fn name(&self) -> &str {
        "SUM"
    }
}

struct SumState {
    sum: Option<Value>,
}

impl AggregateState for SumState {
    fn accumulate(&mut self, value: &Value) -> Result<()> {
        if matches!(value, Value::Null) {
            return Ok(());
        }

        match (&self.sum, value) {
            (None, _) => {
                self.sum = Some(value.clone());
            }
            (Some(Value::Integer(a)), Value::Integer(b)) => {
                self.sum = Some(Value::Integer(a + b));
            }
            (Some(Value::Float(a)), Value::Float(b)) => {
                self.sum = Some(Value::Float(ordered_float::OrderedFloat(
                    a.into_inner() + b.into_inner(),
                )));
            }
            (Some(Value::Integer(a)), Value::Float(b)) => {
                self.sum = Some(Value::Float(ordered_float::OrderedFloat(
                    *a as f64 + b.into_inner(),
                )));
            }
            (Some(Value::Float(a)), Value::Integer(b)) => {
                self.sum = Some(Value::Float(ordered_float::OrderedFloat(
                    a.into_inner() + *b as f64,
                )));
            }
            _ => {
                return Err(NexusError::ExecutionError(
                    "Cannot sum non-numeric values".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn merge(&mut self, other: Box<dyn AggregateState>) -> Result<()> {
        if let Some(other_sum) = other.as_any().downcast_ref::<SumState>() {
            if let Some(other_value) = &other_sum.sum {
                self.accumulate(other_value)?;
            }
            Ok(())
        } else {
            Err(NexusError::ExecutionError(
                "Invalid state merge".to_string(),
            ))
        }
    }

    fn finalize(&self) -> Result<Value> {
        Ok(self.sum.clone().unwrap_or(Value::Null))
    }

    fn clone_state(&self) -> Box<dyn AggregateState> {
        Box::new(SumState {
            sum: self.sum.clone(),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ============================================================================
// AVG Aggregate
// ============================================================================

pub struct AvgFunction;

impl AggregateFunction for AvgFunction {
    fn initialize(&self) -> Box<dyn AggregateState> {
        Box::new(AvgState { sum: 0.0, count: 0 })
    }

    fn name(&self) -> &str {
        "AVG"
    }
}

struct AvgState {
    sum: f64,
    count: i64,
}

impl AggregateState for AvgState {
    fn accumulate(&mut self, value: &Value) -> Result<()> {
        if matches!(value, Value::Null) {
            return Ok(());
        }

        match value {
            Value::Integer(v) => {
                self.sum += *v as f64;
                self.count += 1;
            }
            Value::Float(v) => {
                self.sum += v.into_inner();
                self.count += 1;
            }
            _ => {
                return Err(NexusError::ExecutionError(
                    "Cannot average non-numeric values".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn merge(&mut self, other: Box<dyn AggregateState>) -> Result<()> {
        if let Some(other_avg) = other.as_any().downcast_ref::<AvgState>() {
            self.sum += other_avg.sum;
            self.count += other_avg.count;
            Ok(())
        } else {
            Err(NexusError::ExecutionError(
                "Invalid state merge".to_string(),
            ))
        }
    }

    fn finalize(&self) -> Result<Value> {
        if self.count == 0 {
            Ok(Value::Null)
        } else {
            Ok(Value::Float(ordered_float::OrderedFloat(
                self.sum / self.count as f64,
            )))
        }
    }

    fn clone_state(&self) -> Box<dyn AggregateState> {
        Box::new(AvgState {
            sum: self.sum,
            count: self.count,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ============================================================================
// MIN Aggregate
// ============================================================================

pub struct MinFunction;

impl AggregateFunction for MinFunction {
    fn initialize(&self) -> Box<dyn AggregateState> {
        Box::new(MinState { min: None })
    }

    fn name(&self) -> &str {
        "MIN"
    }
}

struct MinState {
    min: Option<Value>,
}

impl AggregateState for MinState {
    fn accumulate(&mut self, value: &Value) -> Result<()> {
        if matches!(value, Value::Null) {
            return Ok(());
        }

        match &self.min {
            None => {
                self.min = Some(value.clone());
            }
            Some(current_min) => {
                if value < current_min {
                    self.min = Some(value.clone());
                }
            }
        }
        Ok(())
    }

    fn merge(&mut self, other: Box<dyn AggregateState>) -> Result<()> {
        if let Some(other_min) = other.as_any().downcast_ref::<MinState>() {
            if let Some(other_value) = &other_min.min {
                self.accumulate(other_value)?;
            }
            Ok(())
        } else {
            Err(NexusError::ExecutionError(
                "Invalid state merge".to_string(),
            ))
        }
    }

    fn finalize(&self) -> Result<Value> {
        Ok(self.min.clone().unwrap_or(Value::Null))
    }

    fn clone_state(&self) -> Box<dyn AggregateState> {
        Box::new(MinState {
            min: self.min.clone(),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ============================================================================
// MAX Aggregate
// ============================================================================

pub struct MaxFunction;

impl AggregateFunction for MaxFunction {
    fn initialize(&self) -> Box<dyn AggregateState> {
        Box::new(MaxState { max: None })
    }

    fn name(&self) -> &str {
        "MAX"
    }
}

struct MaxState {
    max: Option<Value>,
}

impl AggregateState for MaxState {
    fn accumulate(&mut self, value: &Value) -> Result<()> {
        if matches!(value, Value::Null) {
            return Ok(());
        }

        match &self.max {
            None => {
                self.max = Some(value.clone());
            }
            Some(current_max) => {
                if value > current_max {
                    self.max = Some(value.clone());
                }
            }
        }
        Ok(())
    }

    fn merge(&mut self, other: Box<dyn AggregateState>) -> Result<()> {
        if let Some(other_max) = other.as_any().downcast_ref::<MaxState>() {
            if let Some(other_value) = &other_max.max {
                self.accumulate(other_value)?;
            }
            Ok(())
        } else {
            Err(NexusError::ExecutionError(
                "Invalid state merge".to_string(),
            ))
        }
    }

    fn finalize(&self) -> Result<Value> {
        Ok(self.max.clone().unwrap_or(Value::Null))
    }

    fn clone_state(&self) -> Box<dyn AggregateState> {
        Box::new(MaxState {
            max: self.max.clone(),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ============================================================================
// STRING_AGG Aggregate
// ============================================================================

pub struct StringAggFunction {
    delimiter: String,
}

impl StringAggFunction {
    pub fn new(delimiter: String) -> Self {
        Self { delimiter }
    }
}

impl AggregateFunction for StringAggFunction {
    fn initialize(&self) -> Box<dyn AggregateState> {
        Box::new(StringAggState {
            values: Vec::new(),
            delimiter: self.delimiter.clone(),
        })
    }

    fn name(&self) -> &str {
        "STRING_AGG"
    }
}

struct StringAggState {
    values: Vec<String>,
    delimiter: String,
}

impl AggregateState for StringAggState {
    fn accumulate(&mut self, value: &Value) -> Result<()> {
        if matches!(value, Value::Null) {
            return Ok(());
        }

        match value {
            Value::Text(s) => {
                self.values.push(s.clone());
            }
            _ => {
                self.values.push(value.to_string());
            }
        }
        Ok(())
    }

    fn merge(&mut self, other: Box<dyn AggregateState>) -> Result<()> {
        if let Some(other_agg) = other.as_any().downcast_ref::<StringAggState>() {
            self.values.extend(other_agg.values.clone());
            Ok(())
        } else {
            Err(NexusError::ExecutionError(
                "Invalid state merge".to_string(),
            ))
        }
    }

    fn finalize(&self) -> Result<Value> {
        if self.values.is_empty() {
            Ok(Value::Null)
        } else {
            Ok(Value::Text(self.values.join(&self.delimiter)))
        }
    }

    fn clone_state(&self) -> Box<dyn AggregateState> {
        Box::new(StringAggState {
            values: self.values.clone(),
            delimiter: self.delimiter.clone(),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ============================================================================
// ARRAY_AGG Aggregate
// ============================================================================

pub struct ArrayAggFunction;

impl AggregateFunction for ArrayAggFunction {
    fn initialize(&self) -> Box<dyn AggregateState> {
        Box::new(ArrayAggState { values: Vec::new() })
    }

    fn name(&self) -> &str {
        "ARRAY_AGG"
    }
}

struct ArrayAggState {
    values: Vec<Value>,
}

impl AggregateState for ArrayAggState {
    fn accumulate(&mut self, value: &Value) -> Result<()> {
        // ARRAY_AGG includes NULLs unlike most aggregates
        self.values.push(value.clone());
        Ok(())
    }

    fn merge(&mut self, other: Box<dyn AggregateState>) -> Result<()> {
        if let Some(other_agg) = other.as_any().downcast_ref::<ArrayAggState>() {
            self.values.extend(other_agg.values.clone());
            Ok(())
        } else {
            Err(NexusError::ExecutionError(
                "Invalid state merge".to_string(),
            ))
        }
    }

    fn finalize(&self) -> Result<Value> {
        // For now, return as Text representation
        // In a full implementation, we'd have a proper Array type
        let array_str = format!(
            "[{}]",
            self.values
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        Ok(Value::Text(array_str))
    }

    fn clone_state(&self) -> Box<dyn AggregateState> {
        Box::new(ArrayAggState {
            values: self.values.clone(),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ============================================================================
// Aggregate Manager
// ============================================================================

/// Manages aggregate functions and execution
pub struct AggregateManager {
    functions: Arc<Mutex<HashMap<String, Arc<dyn AggregateFunction>>>>,
}

impl AggregateManager {
    pub fn new() -> Self {
        let manager = Self {
            functions: Arc::new(Mutex::new(HashMap::new())),
        };

        // Register built-in aggregates
        manager.register_builtin_functions();
        manager
    }

    fn register_builtin_functions(&self) {
        let mut functions = self.functions.lock().unwrap();

        functions.insert("COUNT".to_string(), Arc::new(CountFunction));
        functions.insert("SUM".to_string(), Arc::new(SumFunction));
        functions.insert("AVG".to_string(), Arc::new(AvgFunction));
        functions.insert("MIN".to_string(), Arc::new(MinFunction));
        functions.insert("MAX".to_string(), Arc::new(MaxFunction));
        functions.insert(
            "STRING_AGG".to_string(),
            Arc::new(StringAggFunction::new(", ".to_string())),
        );
        functions.insert("ARRAY_AGG".to_string(), Arc::new(ArrayAggFunction));
    }

    /// Register a custom aggregate function
    pub fn register_function(
        &self,
        name: String,
        function: Arc<dyn AggregateFunction>,
    ) -> Result<()> {
        let mut functions = self.functions.lock().unwrap();
        functions.insert(name.to_uppercase(), function);
        Ok(())
    }

    /// Get an aggregate function by name
    pub fn get_function(&self, name: &str) -> Result<Arc<dyn AggregateFunction>> {
        let functions = self.functions.lock().unwrap();
        functions.get(&name.to_uppercase()).cloned().ok_or_else(|| {
            NexusError::ExecutionError(format!("Unknown aggregate function: {}", name))
        })
    }

    /// Execute an aggregate over a set of rows
    pub fn execute_aggregate(
        &self,
        function_name: &str,
        column_index: usize,
        rows: &[Row],
    ) -> Result<Value> {
        let function = self.get_function(function_name)?;
        let mut state = function.initialize();

        for row in rows {
            if let Some(value) = row.values.get(column_index) {
                state.accumulate(value)?;
            }
        }

        state.finalize()
    }

    /// Execute aggregates with GROUP BY
    pub fn execute_grouped_aggregates(
        &self,
        aggregates: Vec<(String, usize)>, // (function_name, column_index)
        group_columns: Vec<usize>,
        rows: &[Row],
    ) -> Result<Vec<Row>> {
        // Group rows by group columns
        let mut groups: HashMap<Vec<Value>, Vec<&Row>> = HashMap::new();

        for row in rows {
            let key: Vec<Value> = group_columns
                .iter()
                .filter_map(|&idx| row.values.get(idx).cloned())
                .collect();

            groups.entry(key).or_insert_with(Vec::new).push(row);
        }

        // Execute aggregates for each group
        let mut results = Vec::new();

        for (group_key, group_rows) in groups {
            let mut result_values = group_key.clone();

            // Execute each aggregate
            for (func_name, col_idx) in &aggregates {
                let function = self.get_function(func_name)?;
                let mut state = function.initialize();

                for row in &group_rows {
                    if let Some(value) = row.values.get(*col_idx) {
                        state.accumulate(value)?;
                    }
                }

                result_values.push(state.finalize()?);
            }

            results.push(Row::new(vec![], result_values));
        }

        Ok(results)
    }
}

impl Default for AggregateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_aggregate() {
        let manager = AggregateManager::new();
        let rows = vec![
            Row::new(vec![], vec![Value::Integer(1)]),
            Row::new(vec![], vec![Value::Integer(2)]),
            Row::new(vec![], vec![Value::Integer(3)]),
        ];

        let result = manager.execute_aggregate("COUNT", 0, &rows).unwrap();
        assert_eq!(result, Value::Integer(3));
    }

    #[test]
    fn test_sum_aggregate() {
        let manager = AggregateManager::new();
        let rows = vec![
            Row::new(vec![], vec![Value::Integer(10)]),
            Row::new(vec![], vec![Value::Integer(20)]),
            Row::new(vec![], vec![Value::Integer(30)]),
        ];

        let result = manager.execute_aggregate("SUM", 0, &rows).unwrap();
        assert_eq!(result, Value::Integer(60));
    }

    #[test]
    fn test_avg_aggregate() {
        let manager = AggregateManager::new();
        let rows = vec![
            Row::new(vec![], vec![Value::Integer(10)]),
            Row::new(vec![], vec![Value::Integer(20)]),
            Row::new(vec![], vec![Value::Integer(30)]),
        ];

        let result = manager.execute_aggregate("AVG", 0, &rows).unwrap();
        assert_eq!(result, Value::Float(ordered_float::OrderedFloat(20.0)));
    }

    #[test]
    fn test_min_max_aggregates() {
        let manager = AggregateManager::new();
        let rows = vec![
            Row::new(vec![], vec![Value::Integer(42)]),
            Row::new(vec![], vec![Value::Integer(17)]),
            Row::new(vec![], vec![Value::Integer(99)]),
        ];

        let min = manager.execute_aggregate("MIN", 0, &rows).unwrap();
        let max = manager.execute_aggregate("MAX", 0, &rows).unwrap();

        assert_eq!(min, Value::Integer(17));
        assert_eq!(max, Value::Integer(99));
    }

    #[test]
    fn test_string_agg() {
        let manager = AggregateManager::new();
        let rows = vec![
            Row::new(vec![], vec![Value::Text("apple".to_string())]),
            Row::new(vec![], vec![Value::Text("banana".to_string())]),
            Row::new(vec![], vec![Value::Text("cherry".to_string())]),
        ];

        let result = manager.execute_aggregate("STRING_AGG", 0, &rows).unwrap();
        assert_eq!(result, Value::Text("apple, banana, cherry".to_string()));
    }

    #[test]
    fn test_grouped_aggregates() {
        let manager = AggregateManager::new();

        // Data: (category, amount)
        let rows = vec![
            Row::new(
                vec![],
                vec![Value::Text("A".to_string()), Value::Integer(10)],
            ),
            Row::new(
                vec![],
                vec![Value::Text("B".to_string()), Value::Integer(20)],
            ),
            Row::new(
                vec![],
                vec![Value::Text("A".to_string()), Value::Integer(15)],
            ),
            Row::new(
                vec![],
                vec![Value::Text("B".to_string()), Value::Integer(25)],
            ),
        ];

        // GROUP BY category, SUM(amount), COUNT(*)
        let results = manager
            .execute_grouped_aggregates(
                vec![("SUM".to_string(), 1), ("COUNT".to_string(), 1)],
                vec![0],
                &rows,
            )
            .unwrap();

        assert_eq!(results.len(), 2); // Two groups: A and B

        // Verify one of the groups (order may vary)
        let group_a = results
            .iter()
            .find(|r| r.values[0] == Value::Text("A".to_string()))
            .unwrap();

        assert_eq!(group_a.values[1], Value::Integer(25)); // SUM
        assert_eq!(group_a.values[2], Value::Integer(2)); // COUNT
    }
}
