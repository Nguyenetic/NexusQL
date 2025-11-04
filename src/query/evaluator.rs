//! WHERE Clause Expression Evaluator
//! PROPRIETARY

use crate::query::ast::{BinaryOperator, Expr};
use crate::types::{Row, Value};

/// Evaluate WHERE clause expression against a row
pub fn evaluate_where(row: &Row, expr: &Expr) -> bool {
    match evaluate_expr(row, expr) {
        Value::Boolean(b) => b,
        Value::Integer(i) => i != 0,
        Value::Null => false,
        _ => false,
    }
}

/// Evaluate any expression
pub fn evaluate_expr(row: &Row, expr: &Expr) -> Value {
    match expr {
        Expr::Column(col_name) => row.get(col_name).cloned().unwrap_or(Value::Null),

        Expr::Literal(val) => val.clone(),

        Expr::BinaryOp { left, op, right } => {
            let left_val = evaluate_expr(row, left);
            let right_val = evaluate_expr(row, right);
            evaluate_binary_op(&left_val, op, &right_val)
        }

        Expr::Function { name: _, args: _ } => {
            // TODO: Implement function calls
            Value::Null
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
        _ => false,
    }
}

fn compare_values(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Integer(a), Value::Integer(b)) => Some(a.cmp(b)),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
        (Value::Text(a), Value::Text(b)) => Some(a.cmp(b)),
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

        assert!(evaluate_where(&row, &expr));
    }

    #[test]
    fn test_evaluate_gt() {
        let row = Row::new(vec!["age".to_string()], vec![Value::Integer(30)]);

        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Column("age".to_string())),
            op: BinaryOperator::Gt,
            right: Box::new(Expr::Literal(Value::Integer(18))),
        };

        assert!(evaluate_where(&row, &expr));
    }
}
