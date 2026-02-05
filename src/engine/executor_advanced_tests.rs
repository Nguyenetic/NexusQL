// Tests for Subqueries, CTEs, and Window Functions

#[cfg(test)]
mod advanced_query_tests {
    use crate::engine::executor::Executor;
    use crate::query::ast::{
        BinaryOperator, ColumnDef, CommonTableExpr, DataType, Expr, FromClause, OrderBy,
        SelectItem, Statement, SubqueryExpr, TableRef, WindowFrameBound, WindowFrameUnits,
        WindowOrderBy, WindowSpec,
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

    fn from_table(name: &str) -> FromClause {
        FromClause {
            table: name.to_string(),
            alias: None,
            joins: vec![],
        }
    }

    fn setup_test_tables(executor: &mut Executor) {
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
                        ColumnDef {
                            name: "department".to_string(),
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
                            name: "amount".to_string(),
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

        // Insert users
        for (id, name, dept) in [
            (1, "Alice", "Engineering"),
            (2, "Bob", "Sales"),
            (3, "Charlie", "Engineering"),
            (4, "Diana", "HR"),
        ] {
            executor
                .execute(
                    &Statement::Insert {
                        table: "users".to_string(),
                        columns: None,
                        values: vec![vec![
                            Expr::Literal(Value::Integer(id)),
                            Expr::Literal(Value::Text(name.to_string())),
                            Expr::Literal(Value::Text(dept.to_string())),
                        ]],
                        on_conflict: None,
                    },
                    &params,
                )
                .unwrap();
        }

        // Insert orders (user 1 has 2 orders, user 2 has 1, user 3 has none)
        for (id, user_id, amount) in [(1, 1, 100), (2, 1, 200), (3, 2, 150)] {
            executor
                .execute(
                    &Statement::Insert {
                        table: "orders".to_string(),
                        columns: None,
                        values: vec![vec![
                            Expr::Literal(Value::Integer(id)),
                            Expr::Literal(Value::Integer(user_id)),
                            Expr::Literal(Value::Integer(amount)),
                        ]],
                        on_conflict: None,
                    },
                    &params,
                )
                .unwrap();
        }
    }

    // === CTE Tests ===

    #[test]
    fn test_simple_cte() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_test_tables(&mut executor);

        // WITH eng_users AS (SELECT * FROM users WHERE department = 'Engineering')
        // SELECT * FROM eng_users
        let cte = CommonTableExpr {
            name: "eng_users".to_string(),
            columns: None,
            query: Box::new(SubqueryExpr {
                columns: vec![SelectItem::Wildcard],
                from: TableRef::Table {
                    name: "users".to_string(),
                    alias: None,
                },
                where_clause: Some(Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Column("department".to_string())),
                    op: BinaryOperator::Eq,
                    right: Box::new(Expr::Literal(Value::Text("Engineering".to_string()))),
                })),
                group_by: vec![],
                having: None,
                order_by: None,
                limit: None,
            }),
            recursive: false,
        };

        let stmt = Statement::Select {
            with: vec![cte],
            columns: vec![SelectItem::Wildcard],
            from: from_table("eng_users"),
            where_clause: None,
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Should have 2 engineering users (Alice and Charlie)
        assert_eq!(result.rows.len(), 2);

        // CTE should be cleaned up - eng_users should not exist as a table
        let check_stmt = Statement::Select {
            with: vec![],
            columns: vec![SelectItem::Wildcard],
            from: from_table("eng_users"),
            where_clause: None,
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };
        // This should fail because eng_users table doesn't exist after CTE cleanup
        assert!(executor.execute(&check_stmt, &params).is_err());
    }

    #[test]
    fn test_cte_with_column_names() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_test_tables(&mut executor);

        // WITH user_names(user_id, user_name) AS (SELECT id, name FROM users)
        // SELECT * FROM user_names
        let cte = CommonTableExpr {
            name: "user_names".to_string(),
            columns: Some(vec!["user_id".to_string(), "user_name".to_string()]),
            query: Box::new(SubqueryExpr {
                columns: vec![
                    SelectItem::Expr {
                        expr: Expr::Column("id".to_string()),
                        alias: None,
                    },
                    SelectItem::Expr {
                        expr: Expr::Column("name".to_string()),
                        alias: None,
                    },
                ],
                from: TableRef::Table {
                    name: "users".to_string(),
                    alias: None,
                },
                where_clause: None,
                group_by: vec![],
                having: None,
                order_by: None,
                limit: None,
            }),
            recursive: false,
        };

        let stmt = Statement::Select {
            with: vec![cte],
            columns: vec![SelectItem::Wildcard],
            from: from_table("user_names"),
            where_clause: None,
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Should have 4 users
        assert_eq!(result.rows.len(), 4);

        // Check that column names are user_id and user_name
        if !result.rows.is_empty() {
            assert!(result.rows[0].columns.contains(&"user_id".to_string()));
            assert!(result.rows[0].columns.contains(&"user_name".to_string()));
        }
    }

    #[test]
    fn test_cte_with_where_clause() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_test_tables(&mut executor);

        // WITH ordered_users AS (SELECT * FROM users ORDER BY id)
        // SELECT * FROM ordered_users WHERE id > 2
        let cte = CommonTableExpr {
            name: "ordered_users".to_string(),
            columns: None,
            query: Box::new(SubqueryExpr {
                columns: vec![SelectItem::Wildcard],
                from: TableRef::Table {
                    name: "users".to_string(),
                    alias: None,
                },
                where_clause: None,
                group_by: vec![],
                having: None,
                order_by: Some(OrderBy {
                    expr: Expr::Column("id".to_string()),
                    ascending: true,
                }),
                limit: None,
            }),
            recursive: false,
        };

        let stmt = Statement::Select {
            with: vec![cte],
            columns: vec![SelectItem::Wildcard],
            from: from_table("ordered_users"),
            where_clause: Some(Expr::BinaryOp {
                left: Box::new(Expr::Column("id".to_string())),
                op: BinaryOperator::Gt,
                right: Box::new(Expr::Literal(Value::Integer(2))),
            }),
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Should have users with id > 2: Charlie (3) and Diana (4)
        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn test_multiple_ctes() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_test_tables(&mut executor);

        // WITH
        //   eng AS (SELECT * FROM users WHERE department = 'Engineering'),
        //   sales AS (SELECT * FROM users WHERE department = 'Sales')
        // SELECT * FROM eng
        let cte1 = CommonTableExpr {
            name: "eng".to_string(),
            columns: None,
            query: Box::new(SubqueryExpr {
                columns: vec![SelectItem::Wildcard],
                from: TableRef::Table {
                    name: "users".to_string(),
                    alias: None,
                },
                where_clause: Some(Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Column("department".to_string())),
                    op: BinaryOperator::Eq,
                    right: Box::new(Expr::Literal(Value::Text("Engineering".to_string()))),
                })),
                group_by: vec![],
                having: None,
                order_by: None,
                limit: None,
            }),
            recursive: false,
        };

        let cte2 = CommonTableExpr {
            name: "sales".to_string(),
            columns: None,
            query: Box::new(SubqueryExpr {
                columns: vec![SelectItem::Wildcard],
                from: TableRef::Table {
                    name: "users".to_string(),
                    alias: None,
                },
                where_clause: Some(Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Column("department".to_string())),
                    op: BinaryOperator::Eq,
                    right: Box::new(Expr::Literal(Value::Text("Sales".to_string()))),
                })),
                group_by: vec![],
                having: None,
                order_by: None,
                limit: None,
            }),
            recursive: false,
        };

        // First query from eng CTE
        let stmt1 = Statement::Select {
            with: vec![cte1.clone(), cte2.clone()],
            columns: vec![SelectItem::Wildcard],
            from: from_table("eng"),
            where_clause: None,
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result1 = executor.execute(&stmt1, &params).unwrap();
        assert_eq!(result1.rows.len(), 2); // Alice and Charlie

        // Second query from sales CTE
        let stmt2 = Statement::Select {
            with: vec![cte1, cte2],
            columns: vec![SelectItem::Wildcard],
            from: from_table("sales"),
            where_clause: None,
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result2 = executor.execute(&stmt2, &params).unwrap();
        assert_eq!(result2.rows.len(), 1); // Bob only
    }

    #[test]
    fn test_cte_with_limit() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_test_tables(&mut executor);

        // WITH limited_users AS (SELECT * FROM users LIMIT 2)
        // SELECT * FROM limited_users
        let cte = CommonTableExpr {
            name: "limited_users".to_string(),
            columns: None,
            query: Box::new(SubqueryExpr {
                columns: vec![SelectItem::Wildcard],
                from: TableRef::Table {
                    name: "users".to_string(),
                    alias: None,
                },
                where_clause: None,
                group_by: vec![],
                having: None,
                order_by: None,
                limit: Some(2),
            }),
            recursive: false,
        };

        let stmt = Statement::Select {
            with: vec![cte],
            columns: vec![SelectItem::Wildcard],
            from: from_table("limited_users"),
            where_clause: None,
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Should have only 2 users due to LIMIT in CTE
        assert_eq!(result.rows.len(), 2);
    }
}
