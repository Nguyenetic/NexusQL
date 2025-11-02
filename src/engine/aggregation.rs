//! Aggregation Functions
//! PROPRIETARY

use crate::types::{Row, Value};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum AggregateFunction {
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

pub struct AggregationExecutor;

impl AggregationExecutor {
    pub fn execute_aggregate(
        rows: Vec<Row>,
        agg_functions: Vec<(AggregateFunction, String)>,
        group_by_columns: Vec<String>,
    ) -> Vec<Row> {
        if group_by_columns.is_empty() {
            Self::aggregate_all(rows, agg_functions)
        } else {
            Self::aggregate_with_grouping(rows, agg_functions, group_by_columns)
        }
    }

    fn aggregate_all(rows: Vec<Row>, agg_functions: Vec<(AggregateFunction, String)>) -> Vec<Row> {
        let mut result_columns = Vec::new();
        let mut result_values = Vec::new();

        for (func, column) in agg_functions {
            let value = Self::compute_aggregate(&func, &rows, &column);
            result_columns.push(format!("{}({})", Self::func_name(&func), column));
            result_values.push(value);
        }

        vec![Row::new(result_columns, result_values)]
    }

    fn aggregate_with_grouping(
        rows: Vec<Row>,
        agg_functions: Vec<(AggregateFunction, String)>,
        group_by_columns: Vec<String>,
    ) -> Vec<Row> {
        // Group rows - use String keys instead of Vec<Value>
        let mut groups: HashMap<String, Vec<Row>> = HashMap::new();

        for row in rows {
            let group_key: Vec<Value> = group_by_columns
                .iter()
                .filter_map(|col| row.get(col).cloned())
                .collect();

            // Convert to string key for HashMap
            let key_str = group_key
                .iter()
                .map(|v| format!("{:?}", v))
                .collect::<Vec<_>>()
                .join("|");

            groups.entry(key_str).or_insert_with(Vec::new).push(row);
        }

        let mut results = Vec::new();

        for (_, group_rows) in groups {
            let group_key: Vec<Value> = group_by_columns
                .iter()
                .filter_map(|col| group_rows[0].get(col).cloned())
                .collect();

            let mut columns = group_by_columns.clone();
            let mut values = group_key;

            for (func, column) in &agg_functions {
                let agg_value = Self::compute_aggregate(func, &group_rows, column);
                columns.push(format!("{}({})", Self::func_name(func), column));
                values.push(agg_value);
            }

            results.push(Row::new(columns, values));
        }

        results
    }

    fn compute_aggregate(func: &AggregateFunction, rows: &[Row], column: &str) -> Value {
        match func {
            AggregateFunction::Count => Value::Integer(rows.len() as i64),
            AggregateFunction::Sum => {
                let sum: f64 = rows
                    .iter()
                    .filter_map(|row| row.get(column))
                    .filter_map(|val| Self::to_number(val))
                    .sum();
                Value::Float(ordered_float::OrderedFloat(sum))
            }
            AggregateFunction::Avg => {
                let values: Vec<f64> = rows
                    .iter()
                    .filter_map(|row| row.get(column))
                    .filter_map(|val| Self::to_number(val))
                    .collect();

                if values.is_empty() {
                    Value::Null
                } else {
                    Value::Float(ordered_float::OrderedFloat(
                        values.iter().sum::<f64>() / values.len() as f64,
                    ))
                }
            }
            AggregateFunction::Min => rows
                .iter()
                .filter_map(|row| row.get(column))
                .min_by(|a, b| Self::compare_values(a, b))
                .cloned()
                .unwrap_or(Value::Null),
            AggregateFunction::Max => rows
                .iter()
                .filter_map(|row| row.get(column))
                .max_by(|a, b| Self::compare_values(a, b))
                .cloned()
                .unwrap_or(Value::Null),
        }
    }

    fn to_number(val: &Value) -> Option<f64> {
        match val {
            Value::Integer(i) => Some(*i as f64),
            Value::Float(f) => Some(f.into_inner()),
            _ => None,
        }
    }

    fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
        match (a, b) {
            (Value::Integer(x), Value::Integer(y)) => x.cmp(y),
            (Value::Float(x), Value::Float(y)) => {
                x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
            }
            (Value::Text(x), Value::Text(y)) => x.cmp(y),
            _ => std::cmp::Ordering::Equal,
        }
    }

    fn func_name(func: &AggregateFunction) -> &'static str {
        match func {
            AggregateFunction::Count => "COUNT",
            AggregateFunction::Sum => "SUM",
            AggregateFunction::Avg => "AVG",
            AggregateFunction::Min => "MIN",
            AggregateFunction::Max => "MAX",
        }
    }
}
