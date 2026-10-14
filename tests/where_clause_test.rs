//! WHERE Clause Tests

use nexusql::query::ast::{BinaryOperator, Expr};
use nexusql::query::evaluator::{evaluate_where, QueryParams};
use nexusql::{Row, Value};

#[test]
fn test_where_equals() {
    let row = Row::new(
        vec!["name".to_string(), "age".to_string()],
        vec![Value::Text("Alice".to_string()), Value::Integer(25)],
    );

    // age = 25
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::Column("age".to_string())),
        op: BinaryOperator::Eq,
        right: Box::new(Expr::Literal(Value::Integer(25))),
    };

    assert!(evaluate_where(&row, &expr, &QueryParams::new()));
}

#[test]
fn test_where_greater_than() {
    let row = Row::new(vec!["age".to_string()], vec![Value::Integer(30)]);

    // age > 18
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::Column("age".to_string())),
        op: BinaryOperator::Gt,
        right: Box::new(Expr::Literal(Value::Integer(18))),
    };

    assert!(evaluate_where(&row, &expr, &QueryParams::new()));
}

#[test]
fn test_where_and() {
    let row = Row::new(
        vec!["age".to_string(), "active".to_string()],
        vec![Value::Integer(25), Value::Boolean(true)],
    );

    // age > 18 AND active = true
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Column("age".to_string())),
            op: BinaryOperator::Gt,
            right: Box::new(Expr::Literal(Value::Integer(18))),
        }),
        op: BinaryOperator::And,
        right: Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Column("active".to_string())),
            op: BinaryOperator::Eq,
            right: Box::new(Expr::Literal(Value::Boolean(true))),
        }),
    };

    assert!(evaluate_where(&row, &expr, &QueryParams::new()));
}
