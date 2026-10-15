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

#[tokio::test]
async fn test_where_in_list_and_between_through_sql() {
    let db = nexusql::NexusSQL::new(":memory:").await.unwrap();
    db.execute("CREATE TABLE t (id INT, provider TEXT, created TEXT)")
        .await
        .unwrap();
    db.execute(
        "INSERT INTO t VALUES (1, 'claude', '2026-10-01'), (2, 'codex', '2026-10-05'), \
         (3, 'kimi', '2026-10-09'), (4, 'claude', '2026-10-12')",
    )
    .await
    .unwrap();
    let ids = |sql: &'static str| {
        let db = &db;
        async move {
            db.execute(sql)
                .await
                .unwrap()
                .rows
                .iter()
                .map(|r| r.get("id").and_then(Value::as_i64).unwrap())
                .collect::<Vec<_>>()
        }
    };
    assert_eq!(
        ids("SELECT id FROM t WHERE provider IN ('codex', 'kimi')").await,
        [2, 3]
    );
    assert_eq!(
        ids("SELECT id FROM t WHERE provider NOT IN ('claude')").await,
        [2, 3]
    );
    assert_eq!(
        ids("SELECT id FROM t WHERE created BETWEEN '2026-10-02' AND '2026-10-09'").await,
        [2, 3]
    );
    assert_eq!(
        ids("SELECT id FROM t WHERE id NOT BETWEEN 2 AND 3 AND provider IN ('claude')").await,
        [1, 4]
    );
}
