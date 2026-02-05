// Tests for JOIN operations

#[cfg(test)]
mod join_tests {
    use crate::engine::executor::Executor;
    use crate::query::ast::{
        BinaryOperator, ColumnDef, DataType, Expr, FromClause, JoinClause, JoinType, SelectItem,
        Statement,
    };
    use crate::query::evaluator::QueryParams;
    use crate::types::Value;
    use std::collections::HashMap;

    fn empty_params() -> QueryParams {
        QueryParams {
            positional: Vec::new(),
            named: HashMap::new(),
        }
    }

    /// Helper to create a simple FromClause for a single table
    fn from_table(name: &str) -> FromClause {
        FromClause {
            table: name.to_string(),
            alias: None,
            joins: vec![],
        }
    }

    /// Helper to create a FromClause with a JOIN
    fn from_with_join(
        table: &str,
        join_table: &str,
        join_type: JoinType,
        left_col: &str,
        right_col: &str,
    ) -> FromClause {
        FromClause {
            table: table.to_string(),
            alias: None,
            joins: vec![JoinClause {
                table: join_table.to_string(),
                alias: None,
                join_type,
                on_condition: Some(Expr::BinaryOp {
                    left: Box::new(Expr::Column(left_col.to_string())),
                    op: BinaryOperator::Eq,
                    right: Box::new(Expr::Column(right_col.to_string())),
                }),
            }],
        }
    }

    fn setup_join_tables(executor: &mut Executor) {
        let params = empty_params();

        // Create 'users' table
        executor
            .execute(
                &Statement::CreateTable {
                    name: "users".to_string(),
                    columns: vec![
                        ColumnDef {
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                            is_primary_key: true,
                            is_autoincrement: false,
                        },
                        ColumnDef {
                            name: "name".to_string(),
                            data_type: DataType::Text,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                    ],
                    if_not_exists: false,
                },
                &params,
            )
            .unwrap();

        // Create 'orders' table
        executor
            .execute(
                &Statement::CreateTable {
                    name: "orders".to_string(),
                    columns: vec![
                        ColumnDef {
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                            is_primary_key: true,
                            is_autoincrement: false,
                        },
                        ColumnDef {
                            name: "user_id".to_string(),
                            data_type: DataType::Integer,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                        ColumnDef {
                            name: "total".to_string(),
                            data_type: DataType::Float,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                    ],
                    if_not_exists: false,
                },
                &params,
            )
            .unwrap();

        // Insert users: Alice(1), Bob(2), Charlie(3)
        for (id, name) in [(1, "Alice"), (2, "Bob"), (3, "Charlie")] {
            executor
                .execute(
                    &Statement::Insert {
                        table: "users".to_string(),
                        columns: None,
                        values: vec![vec![
                            Expr::Literal(Value::Integer(id)),
                            Expr::Literal(Value::Text(name.to_string())),
                        ]],
                        on_conflict: None,
                    },
                    &params,
                )
                .unwrap();
        }

        // Insert orders: user_id 1 has 2 orders, user_id 2 has 1 order, user_id 3 has none
        for (id, user_id, total) in [(1, 1, 100.0), (2, 1, 200.0), (3, 2, 150.0)] {
            executor
                .execute(
                    &Statement::Insert {
                        table: "orders".to_string(),
                        columns: None,
                        values: vec![vec![
                            Expr::Literal(Value::Integer(id)),
                            Expr::Literal(Value::Integer(user_id)),
                            Expr::Literal(Value::Float(total.into())),
                        ]],
                        on_conflict: None,
                    },
                    &params,
                )
                .unwrap();
        }
    }

    #[test]
    fn test_inner_join() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_join_tables(&mut executor);

        // SELECT * FROM users INNER JOIN orders ON users.id = orders.user_id
        let stmt = Statement::Select {
            with: vec![],
            columns: vec![SelectItem::Wildcard],
            from: from_with_join("users", "orders", JoinType::Inner, "id", "user_id"),
            where_clause: None,
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Alice has 2 orders, Bob has 1 order, Charlie has none
        // INNER JOIN should return 3 rows (only matching)
        assert_eq!(result.rows.len(), 3);
    }

    #[test]
    fn test_left_join() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_join_tables(&mut executor);

        // SELECT * FROM users LEFT JOIN orders ON users.id = orders.user_id
        let stmt = Statement::Select {
            with: vec![],
            columns: vec![SelectItem::Wildcard],
            from: from_with_join("users", "orders", JoinType::Left, "id", "user_id"),
            where_clause: None,
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // LEFT JOIN should return 4 rows:
        // - Alice with order 1
        // - Alice with order 2
        // - Bob with order 3
        // - Charlie with NULL orders
        assert_eq!(result.rows.len(), 4);

        // Verify Charlie has NULL for order columns
        let charlie_row = result.rows.iter().find(|r| {
            r.values.iter().any(|v| *v == Value::Text("Charlie".to_string()))
        }).expect("Charlie should be in results");

        // The order columns should have NULL values for Charlie
        let order_id_idx = charlie_row.columns.iter().position(|c| c == "id").unwrap() + 2; // After users.id, users.name
        assert!(
            charlie_row.values.get(order_id_idx) == Some(&Value::Null)
                || charlie_row.values.iter().any(|v| *v == Value::Null),
            "Charlie should have NULL values for order columns"
        );
    }

    #[test]
    fn test_cross_join() {
        let mut executor = Executor::new();
        let params = empty_params();

        // Create small tables for cross join test
        executor
            .execute(
                &Statement::CreateTable {
                    name: "colors".to_string(),
                    columns: vec![ColumnDef {
                        name: "color".to_string(),
                        data_type: DataType::Text,
                        is_primary_key: false,
                        is_autoincrement: false,
                    }],
                    if_not_exists: false,
                },
                &params,
            )
            .unwrap();

        executor
            .execute(
                &Statement::CreateTable {
                    name: "sizes".to_string(),
                    columns: vec![ColumnDef {
                        name: "size".to_string(),
                        data_type: DataType::Text,
                        is_primary_key: false,
                        is_autoincrement: false,
                    }],
                    if_not_exists: false,
                },
                &params,
            )
            .unwrap();

        // Insert colors: Red, Blue
        for color in ["Red", "Blue"] {
            executor
                .execute(
                    &Statement::Insert {
                        table: "colors".to_string(),
                        columns: None,
                        values: vec![vec![Expr::Literal(Value::Text(color.to_string()))]],
                        on_conflict: None,
                    },
                    &params,
                )
                .unwrap();
        }

        // Insert sizes: S, M, L
        for size in ["S", "M", "L"] {
            executor
                .execute(
                    &Statement::Insert {
                        table: "sizes".to_string(),
                        columns: None,
                        values: vec![vec![Expr::Literal(Value::Text(size.to_string()))]],
                        on_conflict: None,
                    },
                    &params,
                )
                .unwrap();
        }

        // SELECT * FROM colors CROSS JOIN sizes
        let stmt = Statement::Select {
            with: vec![],
            columns: vec![SelectItem::Wildcard],
            from: FromClause {
                table: "colors".to_string(),
                alias: None,
                joins: vec![JoinClause {
                    table: "sizes".to_string(),
                    alias: None,
                    join_type: JoinType::Cross,
                    on_condition: None,
                }],
            },
            where_clause: None,
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // CROSS JOIN: 2 colors × 3 sizes = 6 rows
        assert_eq!(result.rows.len(), 6);
    }

    #[test]
    fn test_join_with_where_clause() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_join_tables(&mut executor);

        // SELECT * FROM users INNER JOIN orders ON users.id = orders.user_id WHERE total > 100
        let stmt = Statement::Select {
            with: vec![],
            columns: vec![SelectItem::Wildcard],
            from: from_with_join("users", "orders", JoinType::Inner, "id", "user_id"),
            where_clause: Some(Expr::BinaryOp {
                left: Box::new(Expr::Column("total".to_string())),
                op: BinaryOperator::Gt,
                right: Box::new(Expr::Literal(Value::Float(100.0.into()))),
            }),
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Only orders with total > 100: order 2 (200.0) and order 3 (150.0)
        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn test_multiple_joins() {
        let mut executor = Executor::new();
        let params = empty_params();

        // Create tables
        executor
            .execute(
                &Statement::CreateTable {
                    name: "customers".to_string(),
                    columns: vec![
                        ColumnDef {
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                            is_primary_key: true,
                            is_autoincrement: false,
                        },
                        ColumnDef {
                            name: "name".to_string(),
                            data_type: DataType::Text,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                    ],
                    if_not_exists: false,
                },
                &params,
            )
            .unwrap();

        executor
            .execute(
                &Statement::CreateTable {
                    name: "purchases".to_string(),
                    columns: vec![
                        ColumnDef {
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                            is_primary_key: true,
                            is_autoincrement: false,
                        },
                        ColumnDef {
                            name: "customer_id".to_string(),
                            data_type: DataType::Integer,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                        ColumnDef {
                            name: "product_id".to_string(),
                            data_type: DataType::Integer,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                    ],
                    if_not_exists: false,
                },
                &params,
            )
            .unwrap();

        executor
            .execute(
                &Statement::CreateTable {
                    name: "products".to_string(),
                    columns: vec![
                        ColumnDef {
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                            is_primary_key: true,
                            is_autoincrement: false,
                        },
                        ColumnDef {
                            name: "name".to_string(),
                            data_type: DataType::Text,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                    ],
                    if_not_exists: false,
                },
                &params,
            )
            .unwrap();

        // Insert data
        executor
            .execute(
                &Statement::Insert {
                    table: "customers".to_string(),
                    columns: None,
                    values: vec![vec![
                        Expr::Literal(Value::Integer(1)),
                        Expr::Literal(Value::Text("John".to_string())),
                    ]],
                    on_conflict: None,
                },
                &params,
            )
            .unwrap();

        executor
            .execute(
                &Statement::Insert {
                    table: "products".to_string(),
                    columns: None,
                    values: vec![vec![
                        Expr::Literal(Value::Integer(1)),
                        Expr::Literal(Value::Text("Widget".to_string())),
                    ]],
                    on_conflict: None,
                },
                &params,
            )
            .unwrap();

        executor
            .execute(
                &Statement::Insert {
                    table: "purchases".to_string(),
                    columns: None,
                    values: vec![vec![
                        Expr::Literal(Value::Integer(1)),
                        Expr::Literal(Value::Integer(1)),
                        Expr::Literal(Value::Integer(1)),
                    ]],
                    on_conflict: None,
                },
                &params,
            )
            .unwrap();

        // SELECT * FROM customers
        // INNER JOIN purchases ON customers.id = purchases.customer_id
        // INNER JOIN products ON purchases.product_id = products.id
        let stmt = Statement::Select {
            with: vec![],
            columns: vec![SelectItem::Wildcard],
            from: FromClause {
                table: "customers".to_string(),
                alias: None,
                joins: vec![
                    JoinClause {
                        table: "purchases".to_string(),
                        alias: None,
                        join_type: JoinType::Inner,
                        on_condition: Some(Expr::BinaryOp {
                            left: Box::new(Expr::Column("id".to_string())),
                            op: BinaryOperator::Eq,
                            right: Box::new(Expr::Column("customer_id".to_string())),
                        }),
                    },
                    JoinClause {
                        table: "products".to_string(),
                        alias: None,
                        join_type: JoinType::Inner,
                        on_condition: Some(Expr::BinaryOp {
                            left: Box::new(Expr::Column("product_id".to_string())),
                            op: BinaryOperator::Eq,
                            right: Box::new(Expr::Column("id".to_string())),
                        }),
                    },
                ],
            },
            where_clause: None,
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Should return 1 row: John purchased Widget
        assert_eq!(result.rows.len(), 1);
    }
}
