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

        Expr::Cast { expr, data_type } => {
            let val = evaluate_expr(row, expr, params);
            cast_value(val, data_type)
        }

        // Subquery expressions require executor context and are evaluated there
        // These placeholders should not be reached during normal row-level evaluation
        Expr::Subquery(_) => {
            // Scalar subqueries are evaluated by the executor before row evaluation
            Value::Null
        }

        Expr::InSubquery { .. } => {
            // IN subqueries are evaluated by the executor
            Value::Boolean(false)
        }

        Expr::ExistsSubquery { .. } => {
            // EXISTS subqueries are evaluated by the executor
            Value::Boolean(false)
        }

        Expr::WindowFunction { .. } => {
            // Window functions are evaluated by the executor with partition context
            Value::Null
        }
    }
}

/// Cast a value to the specified data type
fn cast_value(value: Value, target_type: &crate::query::ast::DataType) -> Value {
    use crate::query::ast::DataType;

    match (value, target_type) {
        // NULL stays NULL
        (Value::Null, _) => Value::Null,

        // Cast to Integer
        (Value::Integer(i), DataType::Integer) => Value::Integer(i),
        (Value::Float(f), DataType::Integer) => Value::Integer(f.into_inner() as i64),
        (Value::Text(s), DataType::Integer) => s
            .parse::<i64>()
            .map(Value::Integer)
            .unwrap_or(Value::Null),
        (Value::Boolean(b), DataType::Integer) => Value::Integer(if b { 1 } else { 0 }),

        // Cast to Float
        (Value::Integer(i), DataType::Float) => {
            Value::Float(ordered_float::OrderedFloat(i as f64))
        }
        (Value::Float(f), DataType::Float) => Value::Float(f),
        (Value::Text(s), DataType::Float) => s
            .parse::<f64>()
            .map(|f| Value::Float(ordered_float::OrderedFloat(f)))
            .unwrap_or(Value::Null),
        (Value::Boolean(b), DataType::Float) => {
            Value::Float(ordered_float::OrderedFloat(if b { 1.0 } else { 0.0 }))
        }

        // Cast to Text
        (Value::Integer(i), DataType::Text) => Value::Text(i.to_string()),
        (Value::Float(f), DataType::Text) => Value::Text(f.to_string()),
        (Value::Text(s), DataType::Text) => Value::Text(s),
        (Value::Boolean(b), DataType::Text) => {
            Value::Text(if b { "true".to_string() } else { "false".to_string() })
        }
        (Value::Vector(v), DataType::Text) => {
            let formatted = v
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(",");
            Value::Text(format!("[{}]", formatted))
        }

        // Cast to Boolean
        (Value::Integer(i), DataType::Boolean) => Value::Boolean(i != 0),
        (Value::Float(f), DataType::Boolean) => Value::Boolean(f.into_inner() != 0.0),
        (Value::Text(s), DataType::Boolean) => {
            let lower = s.to_lowercase();
            Value::Boolean(lower == "true" || lower == "1" || lower == "yes" || lower == "on")
        }
        (Value::Boolean(b), DataType::Boolean) => Value::Boolean(b),

        // Cast to Vector
        (Value::Text(s), DataType::Vector(_)) => {
            // Parse "[0.1, 0.2, 0.3]" format
            let trimmed = s.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let inner = &trimmed[1..trimmed.len() - 1];
                let result: Result<Vec<f32>, _> = inner
                    .split(',')
                    .map(|s| s.trim().parse::<f32>())
                    .collect();
                match result {
                    Ok(v) => Value::Vector(v),
                    Err(_) => Value::Null,
                }
            } else {
                Value::Null
            }
        }
        (Value::Vector(v), DataType::Vector(_)) => Value::Vector(v),

        // Unsupported casts return NULL
        _ => Value::Null,
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

        // Null handling functions
        "NULLIF" => {
            // NULLIF(x, y) returns NULL if x = y, otherwise returns x
            if args.len() != 2 {
                return Value::Null;
            }
            let arg1 = evaluate_expr(row, &args[0], params);
            let arg2 = evaluate_expr(row, &args[1], params);

            if arg1 == arg2 {
                Value::Null
            } else {
                arg1
            }
        }

        "COALESCE" => {
            // COALESCE(x1, x2, ...) returns the first non-NULL argument
            for arg in args {
                let value = evaluate_expr(row, arg, params);
                if value != Value::Null {
                    return value;
                }
            }
            Value::Null
        }

        "IFNULL" | "NVL" => {
            // IFNULL(x, y) / NVL(x, y) returns x if x is not NULL, otherwise y
            // This is equivalent to COALESCE(x, y)
            if args.len() != 2 {
                return Value::Null;
            }
            let arg1 = evaluate_expr(row, &args[0], params);
            if arg1 != Value::Null {
                arg1
            } else {
                evaluate_expr(row, &args[1], params)
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

    #[test]
    fn test_cast_text_to_integer() {
        use crate::query::ast::DataType;

        let row = Row::new(vec!["val".to_string()], vec![Value::Text("42".to_string())]);
        let expr = Expr::Cast {
            expr: Box::new(Expr::Column("val".to_string())),
            data_type: DataType::Integer,
        };
        let params = QueryParams::default();
        let result = evaluate_expr(&row, &expr, &params);
        assert_eq!(result, Value::Integer(42));
    }

    #[test]
    fn test_cast_integer_to_text() {
        use crate::query::ast::DataType;

        let row = Row::new(vec!["val".to_string()], vec![Value::Integer(123)]);
        let expr = Expr::Cast {
            expr: Box::new(Expr::Column("val".to_string())),
            data_type: DataType::Text,
        };
        let params = QueryParams::default();
        let result = evaluate_expr(&row, &expr, &params);
        assert_eq!(result, Value::Text("123".to_string()));
    }

    #[test]
    fn test_cast_float_to_integer() {
        use crate::query::ast::DataType;

        let row = Row::new(
            vec!["val".to_string()],
            vec![Value::Float(ordered_float::OrderedFloat(3.7))],
        );
        let expr = Expr::Cast {
            expr: Box::new(Expr::Column("val".to_string())),
            data_type: DataType::Integer,
        };
        let params = QueryParams::default();
        let result = evaluate_expr(&row, &expr, &params);
        assert_eq!(result, Value::Integer(3));
    }

    #[test]
    fn test_cast_bool_to_integer() {
        use crate::query::ast::DataType;

        let row = Row::new(vec!["val".to_string()], vec![Value::Boolean(true)]);
        let expr = Expr::Cast {
            expr: Box::new(Expr::Column("val".to_string())),
            data_type: DataType::Integer,
        };
        let params = QueryParams::default();
        let result = evaluate_expr(&row, &expr, &params);
        assert_eq!(result, Value::Integer(1));
    }

    #[test]
    fn test_cast_null_remains_null() {
        use crate::query::ast::DataType;

        let row = Row::new(vec!["val".to_string()], vec![Value::Null]);
        let expr = Expr::Cast {
            expr: Box::new(Expr::Column("val".to_string())),
            data_type: DataType::Integer,
        };
        let params = QueryParams::default();
        let result = evaluate_expr(&row, &expr, &params);
        assert_eq!(result, Value::Null);
    }
}
