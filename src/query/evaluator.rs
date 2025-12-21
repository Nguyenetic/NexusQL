//! WHERE Clause Expression Evaluator
//! PROPRIETARY

use crate::query::ast::{BinaryOperator, Expr, Parameter};
use crate::types::{Row, Value};
use std::collections::HashMap;

/// Parameter values for query execution
#[derive(Debug, Clone, Default)]
pub struct QueryParams {
    /// Positional parameters (1-indexed)
    pub positional: Vec<Value>,
    /// Named parameters
    pub named: HashMap<String, Value>,
}

impl QueryParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_positional(params: Vec<Value>) -> Self {
        Self {
            positional: params,
            named: HashMap::new(),
        }
    }

    pub fn with_named(params: HashMap<String, Value>) -> Self {
        Self {
            positional: Vec::new(),
            named: params,
        }
    }

    pub fn get_positional(&self, index: usize) -> Option<&Value> {
        // Convert from 1-based to 0-based index
        if index > 0 && index <= self.positional.len() {
            self.positional.get(index - 1)
        } else {
            None
        }
    }

    pub fn get_named(&self, name: &str) -> Option<&Value> {
        self.named.get(name)
    }
}

/// Evaluate WHERE clause expression against a row
pub fn evaluate_where(row: &Row, expr: &Expr, params: &QueryParams) -> bool {
    match evaluate_expr(row, expr, params) {
        Value::Boolean(b) => b,
        Value::Integer(i) => i != 0,
        Value::Null => false,
        _ => false,
    }
}

/// Evaluate any expression
pub fn evaluate_expr(row: &Row, expr: &Expr, params: &QueryParams) -> Value {
    match expr {
        Expr::Column(col_name) => row.get(col_name).cloned().unwrap_or(Value::Null),

        Expr::Literal(val) => val.clone(),

        Expr::Parameter(param) => match param {
            Parameter::Positional(index) => params
                .get_positional(*index)
                .cloned()
                .unwrap_or(Value::Null),
            Parameter::Named(name) => params.get_named(name).cloned().unwrap_or(Value::Null),
        },

        Expr::BinaryOp { left, op, right } => {
            let left_val = evaluate_expr(row, left, params);
            let right_val = evaluate_expr(row, right, params);
            evaluate_binary_op(&left_val, op, &right_val)
        }

        Expr::Function { name, args } => evaluate_function(row, name, args, params),

        Expr::Wildcard => {
            // Wildcard (*) in expressions - typically used inside COUNT(*)
            // Return a special marker value that aggregate handlers can recognize
            Value::Integer(1) // Each row counts as 1
        }

        Expr::IsNull(inner) => {
            let val = evaluate_expr(row, inner, params);
            Value::Boolean(matches!(val, Value::Null))
        }

        Expr::IsNotNull(inner) => {
            let val = evaluate_expr(row, inner, params);
            Value::Boolean(!matches!(val, Value::Null))
        }
    }
}

/// Evaluate binary operations
fn evaluate_binary_op(left: &Value, op: &BinaryOperator, right: &Value) -> Value {
    match op {
        BinaryOperator::Eq => Value::Boolean(values_equal(left, right)),
        BinaryOperator::NotEq => Value::Boolean(!values_equal(left, right)),
        BinaryOperator::Gt => {
            Value::Boolean(compare_values(left, right) == Some(std::cmp::Ordering::Greater))
        }
        BinaryOperator::GtEq => {
            let cmp = compare_values(left, right);
            Value::Boolean(
                cmp == Some(std::cmp::Ordering::Greater) || cmp == Some(std::cmp::Ordering::Equal),
            )
        }
        BinaryOperator::Lt => {
            Value::Boolean(compare_values(left, right) == Some(std::cmp::Ordering::Less))
        }
        BinaryOperator::LtEq => {
            let cmp = compare_values(left, right);
            Value::Boolean(
                cmp == Some(std::cmp::Ordering::Less) || cmp == Some(std::cmp::Ordering::Equal),
            )
        }
        BinaryOperator::And => {
            let left_bool = to_bool(left);
            let right_bool = to_bool(right);
            Value::Boolean(left_bool && right_bool)
        }
        BinaryOperator::Or => {
            let left_bool = to_bool(left);
            let right_bool = to_bool(right);
            Value::Boolean(left_bool || right_bool)
        }
        BinaryOperator::Like => {
            // Simple LIKE implementation
            if let (Value::Text(text), Value::Text(pattern)) = (left, right) {
                let pattern_simple = pattern.replace('%', ".*").replace('_', ".");
                let re = regex::Regex::new(&pattern_simple).ok();
                Value::Boolean(re.map(|r| r.is_match(text)).unwrap_or(false))
            } else {
                Value::Boolean(false)
            }
        }
        BinaryOperator::Matches => {
            // NexusSQL custom MATCHES operator
            if let (Value::Text(text), Value::Text(pattern)) = (left, right) {
                Value::Boolean(text.contains(pattern.as_str()))
            } else {
                Value::Boolean(false)
            }
        }

        // Vector distance operators (pgvector-compatible)
        BinaryOperator::L2Distance => {
            if let Some(distance) = left.l2_distance(right) {
                Value::Float(ordered_float::OrderedFloat(distance as f64))
            } else {
                Value::Null
            }
        }
        BinaryOperator::CosineDistance => {
            if let Some(distance) = left.cosine_distance(right) {
                Value::Float(ordered_float::OrderedFloat(distance as f64))
            } else {
                Value::Null
            }
        }
        BinaryOperator::InnerProduct => {
            if let Some(distance) = left.inner_product(right) {
                Value::Float(ordered_float::OrderedFloat(distance as f64))
            } else {
                Value::Null
            }
        }
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Integer(a), Value::Integer(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Text(a), Value::Text(b)) => a == b,
        (Value::Boolean(a), Value::Boolean(b)) => a == b,
        (Value::Null, Value::Null) => true,
        // Cross-type comparisons for SQL compatibility
        (Value::Integer(i), Value::Text(s)) | (Value::Text(s), Value::Integer(i)) => {
            // Try to parse the text as an integer for comparison
            s.parse::<i64>().map(|parsed| parsed == *i).unwrap_or(false)
        }
        (Value::Float(f), Value::Text(s)) | (Value::Text(s), Value::Float(f)) => {
            // Try to parse the text as a float for comparison
            s.parse::<f64>()
                .map(|parsed| ordered_float::OrderedFloat(parsed) == *f)
                .unwrap_or(false)
        }
        (Value::Integer(i), Value::Float(f)) | (Value::Float(f), Value::Integer(i)) => {
            // Compare integer to float
            ordered_float::OrderedFloat(*i as f64) == *f
        }
        _ => false,
    }
}

fn compare_values(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Integer(a), Value::Integer(b)) => Some(a.cmp(b)),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
        (Value::Text(a), Value::Text(b)) => Some(a.cmp(b)),
        // Cross-type comparisons for SQL compatibility
        (Value::Integer(i), Value::Text(s)) => s
            .parse::<i64>()
            .ok()
            .and_then(|parsed| Some(i.cmp(&parsed))),
        (Value::Text(s), Value::Integer(i)) => {
            s.parse::<i64>().ok().and_then(|parsed| Some(parsed.cmp(i)))
        }
        (Value::Integer(i), Value::Float(f)) => {
            ordered_float::OrderedFloat(*i as f64).partial_cmp(f)
        }
        (Value::Float(f), Value::Integer(i)) => {
            f.partial_cmp(&ordered_float::OrderedFloat(*i as f64))
        }
        _ => None,
    }
}

fn to_bool(val: &Value) -> bool {
    match val {
        Value::Boolean(b) => *b,
        Value::Integer(i) => *i != 0,
        Value::Float(f) => *f != 0.0,
        Value::Text(s) => !s.is_empty(),
        Value::Null => false,
        _ => false,
    }
}

/// Evaluate function calls (built-in functions)
fn evaluate_function(row: &Row, name: &str, args: &[Expr], params: &QueryParams) -> Value {
    let name_upper = name.to_uppercase();

    match name_upper.as_str() {
        // Vector distance functions (pgvector-compatible)
        "VECTOR_DISTANCE" | "L2_DISTANCE" => {
            if args.len() != 2 {
                return Value::Null;
            }
            let arg1 = evaluate_expr(row, &args[0], params);
            let arg2 = evaluate_expr(row, &args[1], params);

            if let Some(distance) = arg1.l2_distance(&arg2) {
                Value::Float(ordered_float::OrderedFloat(distance as f64))
            } else {
                Value::Null
            }
        }

        "COSINE_DISTANCE" => {
            if args.len() != 2 {
                return Value::Null;
            }
            let arg1 = evaluate_expr(row, &args[0], params);
            let arg2 = evaluate_expr(row, &args[1], params);

            if let Some(distance) = arg1.cosine_distance(&arg2) {
                Value::Float(ordered_float::OrderedFloat(distance as f64))
            } else {
                Value::Null
            }
        }

        "INNER_PRODUCT" => {
            if args.len() != 2 {
                return Value::Null;
            }
            let arg1 = evaluate_expr(row, &args[0], params);
            let arg2 = evaluate_expr(row, &args[1], params);

            if let Some(distance) = arg1.inner_product(&arg2) {
                Value::Float(ordered_float::OrderedFloat(distance as f64))
            } else {
                Value::Null
            }
        }

        // String functions
        "UPPER" => {
            if args.len() != 1 {
                return Value::Null;
            }
            let arg = evaluate_expr(row, &args[0], params);
            if let Value::Text(s) = arg {
                Value::Text(s.to_uppercase())
            } else {
                Value::Null
            }
        }

        "LOWER" => {
            if args.len() != 1 {
                return Value::Null;
            }
            let arg = evaluate_expr(row, &args[0], params);
            if let Value::Text(s) = arg {
                Value::Text(s.to_lowercase())
            } else {
                Value::Null
            }
        }

        "LENGTH" | "LEN" => {
            if args.len() != 1 {
                return Value::Null;
            }
            let arg = evaluate_expr(row, &args[0], params);
            match arg {
                Value::Text(s) => Value::Integer(s.len() as i64),
                Value::Vector(v) => Value::Integer(v.len() as i64),
                Value::Blob(b) => Value::Integer(b.len() as i64),
                _ => Value::Null,
            }
        }

        // Math functions
        "ABS" => {
            if args.len() != 1 {
                return Value::Null;
            }
            let arg = evaluate_expr(row, &args[0], params);
            match arg {
                Value::Integer(i) => Value::Integer(i.abs()),
                Value::Float(f) => Value::Float(ordered_float::OrderedFloat(f.abs())),
                _ => Value::Null,
            }
        }

        "ROUND" => {
            if args.len() != 1 {
                return Value::Null;
            }
            let arg = evaluate_expr(row, &args[0], params);
            if let Value::Float(f) = arg {
                Value::Integer(f.round() as i64)
            } else {
                Value::Null
            }
        }

        // Unknown function
        _ => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_eq() {
        let row = Row::new(vec!["age".to_string()], vec![Value::Integer(25)]);

        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Column("age".to_string())),
            op: BinaryOperator::Eq,
            right: Box::new(Expr::Literal(Value::Integer(25))),
        };

        let params = QueryParams::default();
        assert!(evaluate_where(&row, &expr, &params));
    }

    #[test]
    fn test_evaluate_gt() {
        let row = Row::new(vec!["age".to_string()], vec![Value::Integer(30)]);

        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Column("age".to_string())),
            op: BinaryOperator::Gt,
            right: Box::new(Expr::Literal(Value::Integer(18))),
        };

        let params = QueryParams::default();
        assert!(evaluate_where(&row, &expr, &params));
    }
}
