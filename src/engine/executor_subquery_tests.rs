// Tests for Subquery execution (IN, EXISTS, Scalar)

#[cfg(test)]
mod subquery_tests {
    use crate::engine::executor::Executor;
    use crate::query::ast::{
        BinaryOperator, ColumnDef, DataType, Expr, FromClause, SelectItem, Statement,
        SubqueryExpr, TableRef,
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

    fn setup_subquery_tables(executor: &mut Executor) {
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
        // Users: Alice(1), Bob(2), Charlie(3), Diana(4)
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

        // Insert orders
        // Orders: user 1 has 2 orders, user 2 has 1 order, user 3 has none, user 4 has none
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

    // === IN Subquery Tests ===

    #[test]
    fn test_in_subquery() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_subquery_tables(&mut executor);

        // SELECT * FROM users WHERE id IN (SELECT user_id FROM orders)
        // Should return Alice (1) and Bob (2) who have orders
        let subquery = SubqueryExpr {
            columns: vec![SelectItem::Expr {
                expr: Expr::Column("user_id".to_string()),
                alias: None,
            }],
            from: TableRef::Table {
                name: "orders".to_string(),
                alias: None,
            },
            where_clause: None,
            group_by: vec![],
            having: None,
            order_by: None,
            limit: None,
        };

        let stmt = Statement::Select {
            with: vec![],
            columns: vec![SelectItem::Wildcard],
            from: from_table("users"),
            where_clause: Some(Expr::InSubquery {
                expr: Box::new(Expr::Column("id".to_string())),
                subquery: Box::new(subquery),
                negated: false,
            }),
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Should have 2 users who have orders: Alice (1) and Bob (2)
        assert_eq!(result.rows.len(), 2);

        // Verify user IDs
        let user_ids: Vec<i64> = result
            .rows
            .iter()
            .filter_map(|r| {
                if let Value::Integer(id) = &r.values[0] {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        assert!(user_ids.contains(&1));
        assert!(user_ids.contains(&2));
        assert!(!user_ids.contains(&3));
        assert!(!user_ids.contains(&4));
    }

    #[test]
    fn test_not_in_subquery() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_subquery_tables(&mut executor);

        // SELECT * FROM users WHERE id NOT IN (SELECT user_id FROM orders)
        // Should return Charlie (3) and Diana (4) who have no orders
        let subquery = SubqueryExpr {
            columns: vec![SelectItem::Expr {
                expr: Expr::Column("user_id".to_string()),
                alias: None,
            }],
            from: TableRef::Table {
                name: "orders".to_string(),
                alias: None,
            },
            where_clause: None,
            group_by: vec![],
            having: None,
            order_by: None,
            limit: None,
        };

        let stmt = Statement::Select {
            with: vec![],
            columns: vec![SelectItem::Wildcard],
            from: from_table("users"),
            where_clause: Some(Expr::InSubquery {
                expr: Box::new(Expr::Column("id".to_string())),
                subquery: Box::new(subquery),
                negated: true,
            }),
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Should have 2 users without orders: Charlie (3) and Diana (4)
        assert_eq!(result.rows.len(), 2);

        let user_ids: Vec<i64> = result
            .rows
            .iter()
            .filter_map(|r| {
                if let Value::Integer(id) = &r.values[0] {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        assert!(!user_ids.contains(&1));
        assert!(!user_ids.contains(&2));
        assert!(user_ids.contains(&3));
        assert!(user_ids.contains(&4));
    }

    // === EXISTS Subquery Tests ===

    #[test]
    fn test_exists_subquery() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_subquery_tables(&mut executor);

        // SELECT * FROM users WHERE EXISTS (SELECT 1 FROM orders WHERE orders.user_id = users.id)
        // Should return Alice (1) and Bob (2) who have orders
        let subquery = SubqueryExpr {
            columns: vec![SelectItem::Expr {
                expr: Expr::Literal(Value::Integer(1)),
                alias: None,
            }],
            from: TableRef::Table {
                name: "orders".to_string(),
                alias: None,
            },
            where_clause: Some(Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Column("user_id".to_string())),
                op: BinaryOperator::Eq,
                right: Box::new(Expr::Column("id".to_string())), // References outer users.id
            })),
            group_by: vec![],
            having: None,
            order_by: None,
            limit: None,
        };

        let stmt = Statement::Select {
            with: vec![],
            columns: vec![SelectItem::Wildcard],
            from: from_table("users"),
            where_clause: Some(Expr::ExistsSubquery {
                subquery: Box::new(subquery),
                negated: false,
            }),
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Should have 2 users who have orders
        assert_eq!(result.rows.len(), 2);

        let user_ids: Vec<i64> = result
            .rows
            .iter()
            .filter_map(|r| {
                if let Value::Integer(id) = &r.values[0] {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        assert!(user_ids.contains(&1));
        assert!(user_ids.contains(&2));
    }

    #[test]
    fn test_not_exists_subquery() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_subquery_tables(&mut executor);

        // SELECT * FROM users WHERE NOT EXISTS (SELECT 1 FROM orders WHERE orders.user_id = users.id)
        // Should return Charlie (3) and Diana (4) who have no orders
        let subquery = SubqueryExpr {
            columns: vec![SelectItem::Expr {
                expr: Expr::Literal(Value::Integer(1)),
                alias: None,
            }],
            from: TableRef::Table {
                name: "orders".to_string(),
                alias: None,
            },
            where_clause: Some(Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Column("user_id".to_string())),
                op: BinaryOperator::Eq,
                right: Box::new(Expr::Column("id".to_string())),
            })),
            group_by: vec![],
            having: None,
            order_by: None,
            limit: None,
        };

        let stmt = Statement::Select {
            with: vec![],
            columns: vec![SelectItem::Wildcard],
            from: from_table("users"),
            where_clause: Some(Expr::ExistsSubquery {
                subquery: Box::new(subquery),
                negated: true,
            }),
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Should have 2 users without orders
        assert_eq!(result.rows.len(), 2);

        let user_ids: Vec<i64> = result
            .rows
            .iter()
            .filter_map(|r| {
                if let Value::Integer(id) = &r.values[0] {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        assert!(user_ids.contains(&3));
        assert!(user_ids.contains(&4));
    }

    // === Combined Subquery Tests ===

    #[test]
    fn test_in_subquery_with_where() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_subquery_tables(&mut executor);

        // SELECT * FROM users WHERE id IN (SELECT user_id FROM orders WHERE amount > 100)
        // Should return only Alice (user 1) who has an order > 100
        let subquery = SubqueryExpr {
            columns: vec![SelectItem::Expr {
                expr: Expr::Column("user_id".to_string()),
                alias: None,
            }],
            from: TableRef::Table {
                name: "orders".to_string(),
                alias: None,
            },
            where_clause: Some(Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Column("amount".to_string())),
                op: BinaryOperator::Gt,
                right: Box::new(Expr::Literal(Value::Integer(100))),
            })),
            group_by: vec![],
            having: None,
            order_by: None,
            limit: None,
        };

        let stmt = Statement::Select {
            with: vec![],
            columns: vec![SelectItem::Wildcard],
            from: from_table("users"),
            where_clause: Some(Expr::InSubquery {
                expr: Box::new(Expr::Column("id".to_string())),
                subquery: Box::new(subquery),
                negated: false,
            }),
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Orders with amount > 100:
        // - Order 2: user_id=1, amount=200 > 100 ✓
        // - Order 3: user_id=2, amount=150 > 100 ✓
        // So users 1 and 2 should be in the result
        assert_eq!(result.rows.len(), 2);

        let user_ids: Vec<i64> = result
            .rows
            .iter()
            .filter_map(|r| {
                if let Value::Integer(id) = &r.values[0] {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        assert!(user_ids.contains(&1)); // Alice has order with amount=200
        assert!(user_ids.contains(&2)); // Bob has order with amount=150
    }
}
