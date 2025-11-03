// Advanced SQL Features
// Deferred features from v1.0.0: LATERAL joins, correlated subqueries,
// expression indexes, composite partitioning, ALL/ANY/SOME operators

use crate::types::{Row, Value};
use crate::Result;

/// LATERAL join executor
pub struct LateralJoin;

impl LateralJoin {
    /// Execute LATERAL subquery with reference to outer row
    pub fn execute_lateral<F>(outer_rows: Vec<Row>, lateral_fn: F) -> Result<Vec<Row>>
    where
        F: Fn(&Row) -> Result<Vec<Row>>,
    {
        let mut results = Vec::new();

        for outer_row in &outer_rows {
            let inner_rows = lateral_fn(outer_row)?;

            for inner_row in inner_rows {
                let mut combined_cols = outer_row.columns.clone();
                combined_cols.extend(inner_row.columns.clone());

                let mut combined_vals = outer_row.values.clone();
                combined_vals.extend(inner_row.values.clone());

                results.push(Row::new(combined_cols, combined_vals));
            }
        }

        Ok(results)
    }
}

/// Correlated subquery executor
pub struct CorrelatedSubquery;

impl CorrelatedSubquery {
    /// Execute subquery that references outer query
    pub fn execute<F>(outer_rows: Vec<Row>, subquery_fn: F) -> Result<Vec<Row>>
    where
        F: Fn(&Row) -> Result<Value>,
    {
        let mut results = Vec::new();

        for outer_row in outer_rows {
            let subquery_result = subquery_fn(&outer_row)?;

            let mut new_cols = outer_row.columns.clone();
            new_cols.push("subquery_result".to_string());

            let mut new_vals = outer_row.values.clone();
            new_vals.push(subquery_result);

            results.push(Row::new(new_cols, new_vals));
        }

        Ok(results)
    }
}

/// ALL/ANY/SOME operators for subqueries
pub struct SubqueryOps;

impl SubqueryOps {
    pub fn all_greater(value: &Value, results: &[Value]) -> bool {
        results.iter().all(|v| value > v)
    }

    pub fn any_equals(value: &Value, results: &[Value]) -> bool {
        results.iter().any(|v| v == value)
    }

    pub fn some_equals(value: &Value, results: &[Value]) -> bool {
        Self::any_equals(value, results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lateral_join() {
        let outer = vec![
            Row::new(vec!["id".to_string()], vec![Value::Integer(1)]),
            Row::new(vec!["id".to_string()], vec![Value::Integer(2)]),
        ];

        let result = LateralJoin::execute_lateral(outer, |outer_row| {
            let id = match outer_row.values[0] {
                Value::Integer(i) => i,
                _ => 0,
            };
            Ok(vec![Row::new(
                vec!["val".to_string()],
                vec![Value::Integer(id * 10)],
            )])
        })
        .unwrap();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_correlated_subquery() {
        let outer = vec![Row::new(vec!["id".to_string()], vec![Value::Integer(1)])];

        let result =
            CorrelatedSubquery::execute(outer, |_outer_row| Ok(Value::Integer(99))).unwrap();

        assert_eq!(result[0].values.last(), Some(&Value::Integer(99)));
    }

    #[test]
    fn test_all_operator() {
        let vals = vec![Value::Integer(10), Value::Integer(20)];
        assert!(SubqueryOps::all_greater(&Value::Integer(30), &vals));
        assert!(!SubqueryOps::all_greater(&Value::Integer(15), &vals));
    }

    #[test]
    fn test_any_operator() {
        let vals = vec![Value::Integer(10), Value::Integer(20)];
        assert!(SubqueryOps::any_equals(&Value::Integer(10), &vals));
        assert!(!SubqueryOps::any_equals(&Value::Integer(99), &vals));
    }
}
