// Week 14: Joins & Subqueries
//
// Implements PostgreSQL-compatible join operations and subquery execution:
// - INNER JOIN, LEFT/RIGHT/FULL OUTER JOIN, CROSS JOIN, NATURAL JOIN
// - Hash Join, Merge Join, Nested Loop Join algorithms
// - Scalar, Row, and Table subqueries
// - EXISTS, NOT EXISTS, IN, NOT IN operators
// - Join optimization and cost-based algorithm selection

use crate::types::{Row, Value};
use crate::{NexusError, Result};
use std::collections::{HashMap, HashSet};

/// Join type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinType {
    /// INNER JOIN - only matching rows
    Inner,
    /// LEFT OUTER JOIN - all left rows + matching right
    LeftOuter,
    /// RIGHT OUTER JOIN - all right rows + matching left
    RightOuter,
    /// FULL OUTER JOIN - all rows from both sides
    FullOuter,
    /// CROSS JOIN - Cartesian product
    Cross,
    /// NATURAL JOIN - implicit join on common columns
    Natural,
}

/// Join algorithm strategy
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinAlgorithm {
    /// Hash Join - build hash table from smaller input
    Hash,
    /// Merge Join - requires sorted inputs
    Merge,
    /// Nested Loop Join - simple iteration
    NestedLoop,
}

/// Join condition
#[derive(Debug, Clone)]
pub enum JoinCondition {
    /// ON clause with expression
    On {
        left_column: String,
        right_column: String,
    },
    /// USING clause with column list
    Using(Vec<String>),
    /// Natural join (implicit)
    Natural,
    /// No condition (CROSS JOIN)
    None,
}

/// Join executor
pub struct JoinExecutor {
    join_type: JoinType,
    algorithm: JoinAlgorithm,
    condition: JoinCondition,
}

impl JoinExecutor {
    /// Create a new join executor
    pub fn new(join_type: JoinType, algorithm: JoinAlgorithm, condition: JoinCondition) -> Self {
        Self {
            join_type,
            algorithm,
            condition,
        }
    }

    /// Execute the join
    pub fn execute(&self, left: Vec<Row>, right: Vec<Row>) -> Result<Vec<Row>> {
        match self.join_type {
            JoinType::Cross => self.cross_join(left, right),
            JoinType::Inner => self.inner_join(left, right),
            JoinType::LeftOuter => self.left_outer_join(left, right),
            JoinType::RightOuter => self.right_outer_join(left, right),
            JoinType::FullOuter => self.full_outer_join(left, right),
            JoinType::Natural => self.natural_join(left, right),
        }
    }

    /// CROSS JOIN - Cartesian product
    fn cross_join(&self, left: Vec<Row>, right: Vec<Row>) -> Result<Vec<Row>> {
        let mut result = Vec::new();

        for left_row in &left {
            for right_row in &right {
                result.push(Self::combine_rows(left_row, right_row));
            }
        }

        Ok(result)
    }

    /// INNER JOIN - only matching rows
    fn inner_join(&self, left: Vec<Row>, right: Vec<Row>) -> Result<Vec<Row>> {
        match self.algorithm {
            JoinAlgorithm::Hash => self.hash_join_inner(left, right),
            JoinAlgorithm::Merge => self.merge_join_inner(left, right),
            JoinAlgorithm::NestedLoop => self.nested_loop_join_inner(left, right),
        }
    }

    /// Hash Join implementation for INNER JOIN
    fn hash_join_inner(&self, left: Vec<Row>, right: Vec<Row>) -> Result<Vec<Row>> {
        let (left_col, right_col) = self.get_join_columns()?;

        // Build phase: create hash table from left (smaller) side
        let mut hash_table: HashMap<Value, Vec<Row>> = HashMap::new();
        for row in left {
            if let Some(key) = Self::get_column_value(&row, &left_col) {
                hash_table.entry(key).or_insert_with(Vec::new).push(row);
            }
        }

        // Probe phase: match right side against hash table
        let mut result = Vec::new();
        for right_row in right {
            if let Some(key) = Self::get_column_value(&right_row, &right_col) {
                if let Some(left_rows) = hash_table.get(&key) {
                    for left_row in left_rows {
                        result.push(Self::combine_rows(left_row, &right_row));
                    }
                }
            }
        }

        Ok(result)
    }

    /// Merge Join implementation for INNER JOIN
    fn merge_join_inner(&self, mut left: Vec<Row>, mut right: Vec<Row>) -> Result<Vec<Row>> {
        let (left_col, right_col) = self.get_join_columns()?;

        // Sort both inputs
        left.sort_by(|a, b| {
            let a_val = Self::get_column_value(a, &left_col);
            let b_val = Self::get_column_value(b, &left_col);
            match (&a_val, &b_val) {
                (Some(a), Some(b)) => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
                (Some(_), None) => std::cmp::Ordering::Greater,
                (None, Some(_)) => std::cmp::Ordering::Less,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });

        right.sort_by(|a, b| {
            let a_val = Self::get_column_value(a, &right_col);
            let b_val = Self::get_column_value(b, &right_col);
            match (&a_val, &b_val) {
                (Some(a), Some(b)) => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
                (Some(_), None) => std::cmp::Ordering::Greater,
                (None, Some(_)) => std::cmp::Ordering::Less,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });

        // Merge
        let mut result = Vec::new();
        let mut left_idx = 0;
        let mut right_idx = 0;

        while left_idx < left.len() && right_idx < right.len() {
            let left_val = Self::get_column_value(&left[left_idx], &left_col);
            let right_val = Self::get_column_value(&right[right_idx], &right_col);

            let cmp = match (&left_val, &right_val) {
                (Some(l), Some(r)) => l.partial_cmp(r).unwrap_or(std::cmp::Ordering::Equal),
                (Some(_), None) => std::cmp::Ordering::Greater,
                (None, Some(_)) => std::cmp::Ordering::Less,
                (None, None) => std::cmp::Ordering::Equal,
            };

            match cmp {
                std::cmp::Ordering::Equal => {
                    // Found match - output all combinations
                    let mut right_match_idx = right_idx;
                    while right_match_idx < right.len() {
                        let rv = Self::get_column_value(&right[right_match_idx], &right_col);
                        if rv != left_val {
                            break;
                        }
                        result.push(Self::combine_rows(&left[left_idx], &right[right_match_idx]));
                        right_match_idx += 1;
                    }
                    left_idx += 1;
                }
                std::cmp::Ordering::Less => left_idx += 1,
                std::cmp::Ordering::Greater => right_idx += 1,
            }
        }

        Ok(result)
    }

    /// Nested Loop Join implementation for INNER JOIN
    fn nested_loop_join_inner(&self, left: Vec<Row>, right: Vec<Row>) -> Result<Vec<Row>> {
        let (left_col, right_col) = self.get_join_columns()?;
        let mut result = Vec::new();

        for left_row in &left {
            for right_row in &right {
                if Self::rows_match(left_row, &left_col, right_row, &right_col) {
                    result.push(Self::combine_rows(left_row, right_row));
                }
            }
        }

        Ok(result)
    }

    /// LEFT OUTER JOIN - all left rows + matching right
    fn left_outer_join(&self, left: Vec<Row>, right: Vec<Row>) -> Result<Vec<Row>> {
        let (left_col, right_col) = self.get_join_columns()?;
        let mut result = Vec::new();

        for left_row in &left {
            let mut found_match = false;

            for right_row in &right {
                if Self::rows_match(left_row, &left_col, right_row, &right_col) {
                    result.push(Self::combine_rows(left_row, right_row));
                    found_match = true;
                }
            }

            // If no match, add left row with NULLs for right columns
            if !found_match {
                result.push(Self::combine_row_with_nulls(
                    left_row,
                    right.first().map(|r| r.columns.len()).unwrap_or(0),
                ));
            }
        }

        Ok(result)
    }

    /// RIGHT OUTER JOIN - all right rows + matching left
    fn right_outer_join(&self, left: Vec<Row>, right: Vec<Row>) -> Result<Vec<Row>> {
        let (left_col, right_col) = self.get_join_columns()?;
        let mut result = Vec::new();

        for right_row in &right {
            let mut found_match = false;

            for left_row in &left {
                if Self::rows_match(left_row, &left_col, right_row, &right_col) {
                    result.push(Self::combine_rows(left_row, right_row));
                    found_match = true;
                }
            }

            // If no match, add right row with NULLs for left columns
            if !found_match {
                result.push(Self::combine_nulls_with_row(
                    left.first().map(|r| r.columns.len()).unwrap_or(0),
                    right_row,
                ));
            }
        }

        Ok(result)
    }

    /// FULL OUTER JOIN - all rows from both sides
    fn full_outer_join(&self, left: Vec<Row>, right: Vec<Row>) -> Result<Vec<Row>> {
        let (left_col, right_col) = self.get_join_columns()?;
        let mut result = Vec::new();
        let mut right_matched = HashSet::new();

        // Process left side
        for left_row in &left {
            let mut found_match = false;

            for (idx, right_row) in right.iter().enumerate() {
                if Self::rows_match(left_row, &left_col, right_row, &right_col) {
                    result.push(Self::combine_rows(left_row, right_row));
                    right_matched.insert(idx);
                    found_match = true;
                }
            }

            // If no match, add left row with NULLs
            if !found_match {
                result.push(Self::combine_row_with_nulls(
                    left_row,
                    right.first().map(|r| r.columns.len()).unwrap_or(0),
                ));
            }
        }

        // Add unmatched right rows
        for (idx, right_row) in right.iter().enumerate() {
            if !right_matched.contains(&idx) {
                result.push(Self::combine_nulls_with_row(
                    left.first().map(|r| r.columns.len()).unwrap_or(0),
                    right_row,
                ));
            }
        }

        Ok(result)
    }

    /// NATURAL JOIN - implicit join on common columns
    fn natural_join(&self, left: Vec<Row>, right: Vec<Row>) -> Result<Vec<Row>> {
        if left.is_empty() || right.is_empty() {
            return Ok(Vec::new());
        }

        // Find common columns
        let left_cols: HashSet<_> = left[0].columns.iter().cloned().collect();
        let right_cols: HashSet<_> = right[0].columns.iter().cloned().collect();
        let common_cols: Vec<_> = left_cols.intersection(&right_cols).cloned().collect();

        if common_cols.is_empty() {
            // No common columns - behaves like CROSS JOIN
            return self.cross_join(left, right);
        }

        // Perform join on all common columns
        let mut result = Vec::new();
        for left_row in &left {
            for right_row in &right {
                let mut matches = true;
                for col in &common_cols {
                    if Self::get_column_value(left_row, col)
                        != Self::get_column_value(right_row, col)
                    {
                        matches = false;
                        break;
                    }
                }

                if matches {
                    // Combine rows, including common columns only once
                    result.push(Self::combine_natural(left_row, right_row, &common_cols));
                }
            }
        }

        Ok(result)
    }

    /// Helper: Get join columns from condition
    fn get_join_columns(&self) -> Result<(String, String)> {
        match &self.condition {
            JoinCondition::On {
                left_column,
                right_column,
            } => Ok((left_column.clone(), right_column.clone())),
            JoinCondition::Using(cols) if !cols.is_empty() => {
                Ok((cols[0].clone(), cols[0].clone()))
            }
            _ => Err(NexusError::ExecutionError(
                "Join condition requires ON or USING clause".to_string(),
            )),
        }
    }

    /// Helper: Get column value from row
    fn get_column_value(row: &Row, column: &str) -> Option<Value> {
        row.columns
            .iter()
            .position(|c| c == column)
            .and_then(|idx| row.values.get(idx).cloned())
    }

    /// Helper: Check if two rows match on specified columns
    fn rows_match(left: &Row, left_col: &str, right: &Row, right_col: &str) -> bool {
        let left_val = Self::get_column_value(left, left_col);
        let right_val = Self::get_column_value(right, right_col);
        left_val.is_some() && left_val == right_val
    }

    /// Helper: Combine two rows
    fn combine_rows(left: &Row, right: &Row) -> Row {
        let mut columns = left.columns.clone();
        columns.extend(right.columns.clone());

        let mut values = left.values.clone();
        values.extend(right.values.clone());

        Row::new(columns, values)
    }

    /// Helper: Combine row with NULLs on right
    fn combine_row_with_nulls(left: &Row, right_col_count: usize) -> Row {
        let mut columns = left.columns.clone();
        let mut values = left.values.clone();

        for _ in 0..right_col_count {
            columns.push("?".to_string());
            values.push(Value::Null);
        }

        Row::new(columns, values)
    }

    /// Helper: Combine NULLs on left with row
    fn combine_nulls_with_row(left_col_count: usize, right: &Row) -> Row {
        let mut columns = Vec::new();
        let mut values = Vec::new();

        for _ in 0..left_col_count {
            columns.push("?".to_string());
            values.push(Value::Null);
        }

        columns.extend(right.columns.clone());
        values.extend(right.values.clone());

        Row::new(columns, values)
    }

    /// Helper: Combine rows for NATURAL JOIN (common columns once)
    fn combine_natural(left: &Row, right: &Row, common_cols: &[String]) -> Row {
        let mut columns = left.columns.clone();
        let mut values = left.values.clone();

        // Add only non-common columns from right
        for (idx, col) in right.columns.iter().enumerate() {
            if !common_cols.contains(col) {
                columns.push(col.clone());
                values.push(right.values[idx].clone());
            }
        }

        Row::new(columns, values)
    }

    /// Choose best join algorithm based on data size
    pub fn choose_algorithm(left_size: usize, right_size: usize) -> JoinAlgorithm {
        if left_size < 10 || right_size < 10 {
            // Small datasets - nested loop is fine
            JoinAlgorithm::NestedLoop
        } else if left_size > 1000 && right_size > 1000 {
            // Large datasets - hash join is usually best
            JoinAlgorithm::Hash
        } else {
            // Medium datasets - merge join if data can be sorted
            JoinAlgorithm::Merge
        }
    }
}

/// Subquery executor
pub struct SubqueryExecutor;

impl SubqueryExecutor {
    /// Execute scalar subquery (returns single value)
    pub fn execute_scalar(rows: Vec<Row>) -> Result<Value> {
        if rows.is_empty() {
            return Ok(Value::Null);
        }

        if rows.len() > 1 {
            return Err(NexusError::ExecutionError(
                "Scalar subquery returned more than one row".to_string(),
            ));
        }

        if rows[0].values.is_empty() {
            return Ok(Value::Null);
        }

        Ok(rows[0].values[0].clone())
    }

    /// Execute EXISTS subquery (returns true if any rows)
    pub fn execute_exists(rows: Vec<Row>) -> bool {
        !rows.is_empty()
    }

    /// Execute IN subquery (check if value in results)
    pub fn execute_in(value: &Value, rows: Vec<Row>) -> bool {
        for row in rows {
            if !row.values.is_empty() && &row.values[0] == value {
                return true;
            }
        }
        false
    }

    /// Execute NOT IN subquery
    pub fn execute_not_in(value: &Value, rows: Vec<Row>) -> bool {
        !Self::execute_in(value, rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_rows_left() -> Vec<Row> {
        vec![
            Row::new(
                vec!["id".to_string(), "name".to_string()],
                vec![Value::Integer(1), Value::Text("Alice".to_string())],
            ),
            Row::new(
                vec!["id".to_string(), "name".to_string()],
                vec![Value::Integer(2), Value::Text("Bob".to_string())],
            ),
            Row::new(
                vec!["id".to_string(), "name".to_string()],
                vec![Value::Integer(3), Value::Text("Charlie".to_string())],
            ),
        ]
    }

    fn create_test_rows_right() -> Vec<Row> {
        vec![
            Row::new(
                vec!["id".to_string(), "dept".to_string()],
                vec![Value::Integer(1), Value::Text("Engineering".to_string())],
            ),
            Row::new(
                vec!["id".to_string(), "dept".to_string()],
                vec![Value::Integer(2), Value::Text("Sales".to_string())],
            ),
            Row::new(
                vec!["id".to_string(), "dept".to_string()],
                vec![Value::Integer(4), Value::Text("Marketing".to_string())],
            ),
        ]
    }

    #[test]
    fn test_inner_join_hash() {
        let left = create_test_rows_left();
        let right = create_test_rows_right();

        let executor = JoinExecutor::new(
            JoinType::Inner,
            JoinAlgorithm::Hash,
            JoinCondition::On {
                left_column: "id".to_string(),
                right_column: "id".to_string(),
            },
        );

        let result = executor.execute(left, right).unwrap();
        assert_eq!(result.len(), 2); // Only ids 1 and 2 match
        assert_eq!(result[0].values[0], Value::Integer(1));
        assert_eq!(result[1].values[0], Value::Integer(2));
    }

    #[test]
    fn test_left_outer_join() {
        let left = create_test_rows_left();
        let right = create_test_rows_right();

        let executor = JoinExecutor::new(
            JoinType::LeftOuter,
            JoinAlgorithm::NestedLoop,
            JoinCondition::On {
                left_column: "id".to_string(),
                right_column: "id".to_string(),
            },
        );

        let result = executor.execute(left, right).unwrap();
        assert_eq!(result.len(), 3); // All 3 left rows

        // Check that Charlie (id=3) has NULL for dept
        let charlie_row = result
            .iter()
            .find(|r| r.values[1] == Value::Text("Charlie".to_string()))
            .unwrap();
        assert_eq!(charlie_row.values.last(), Some(&Value::Null));
    }

    #[test]
    fn test_cross_join() {
        let left = vec![
            Row::new(vec!["a".to_string()], vec![Value::Integer(1)]),
            Row::new(vec!["a".to_string()], vec![Value::Integer(2)]),
        ];
        let right = vec![
            Row::new(vec!["b".to_string()], vec![Value::Text("X".to_string())]),
            Row::new(vec!["b".to_string()], vec![Value::Text("Y".to_string())]),
        ];

        let executor = JoinExecutor::new(
            JoinType::Cross,
            JoinAlgorithm::NestedLoop,
            JoinCondition::None,
        );

        let result = executor.execute(left, right).unwrap();
        assert_eq!(result.len(), 4); // 2 * 2 = 4
    }

    #[test]
    fn test_full_outer_join() {
        let left = create_test_rows_left();
        let right = create_test_rows_right();

        let executor = JoinExecutor::new(
            JoinType::FullOuter,
            JoinAlgorithm::NestedLoop,
            JoinCondition::On {
                left_column: "id".to_string(),
                right_column: "id".to_string(),
            },
        );

        let result = executor.execute(left, right).unwrap();
        assert_eq!(result.len(), 4); // Alice, Bob, Charlie, Marketing
    }

    #[test]
    fn test_subquery_scalar() {
        let rows = vec![Row::new(vec!["max".to_string()], vec![Value::Integer(100)])];

        let result = SubqueryExecutor::execute_scalar(rows).unwrap();
        assert_eq!(result, Value::Integer(100));
    }

    #[test]
    fn test_subquery_exists() {
        let rows = vec![Row::new(vec!["id".to_string()], vec![Value::Integer(1)])];

        assert!(SubqueryExecutor::execute_exists(rows));
        assert!(!SubqueryExecutor::execute_exists(Vec::new()));
    }

    #[test]
    fn test_subquery_in() {
        let rows = vec![
            Row::new(vec!["id".to_string()], vec![Value::Integer(1)]),
            Row::new(vec!["id".to_string()], vec![Value::Integer(2)]),
            Row::new(vec!["id".to_string()], vec![Value::Integer(3)]),
        ];

        assert!(SubqueryExecutor::execute_in(
            &Value::Integer(2),
            rows.clone()
        ));
        assert!(!SubqueryExecutor::execute_in(&Value::Integer(5), rows));
    }
}
