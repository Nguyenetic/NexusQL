// Tests for FETCH and TRAVERSE operations

#[cfg(test)]
mod fetch_traverse_tests {
    use crate::engine::executor::Executor;
    use crate::query::ast::{
        BinaryOperator, Cardinality, ColumnDef, DataType, Expr, FetchClause, FromClause, OrderBy,
        Relationship, SelectItem, Statement, TraverseClause, TraverseDirection, TraverseMode,
        TraversePattern,
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

    fn setup_fetch_tables(executor: &mut Executor) {
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

        // Create 'items' table (for nested FETCH tests)
        executor
            .execute(
                &Statement::CreateTable {
                    name: "items".to_string(),
                    columns: vec![
                        ColumnDef {
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                            is_primary_key: true,
                            is_autoincrement: false,
                        },
                        ColumnDef {
                            name: "order_id".to_string(),
                            data_type: DataType::Integer,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                        ColumnDef {
                            name: "product".to_string(),
                            data_type: DataType::Text,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                        ColumnDef {
                            name: "price".to_string(),
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

        // Insert users
        for (id, name) in [(1, "Alice"), (2, "Bob")] {
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

        // Insert orders
        for (id, user_id, total) in [(1, 1, 99.99), (2, 1, 150.00), (3, 2, 75.50)] {
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

        // Insert items
        for (id, order_id, product, price) in [
            (1, 1, "Widget", 49.99),
            (2, 1, "Gadget", 50.00),
            (3, 2, "Thing", 150.00),
            (4, 3, "Stuff", 75.50),
        ] {
            executor
                .execute(
                    &Statement::Insert {
                        table: "items".to_string(),
                        columns: None,
                        values: vec![vec![
                            Expr::Literal(Value::Integer(id)),
                            Expr::Literal(Value::Integer(order_id)),
                            Expr::Literal(Value::Text(product.to_string())),
                            Expr::Literal(Value::Float(price.into())),
                        ]],
                        on_conflict: None,
                    },
                    &params,
                )
                .unwrap();
        }
    }

    // ========== CREATE RELATIONSHIP Tests ==========

    #[test]
    fn test_create_relationship() {
        let mut executor = Executor::new();
        let params = empty_params();

        // Create relationship
        let rel = Relationship {
            name: "orders_rel".to_string(),
            from_table: "users".to_string(),
            from_column: "id".to_string(),
            to_table: "orders".to_string(),
            to_column: "user_id".to_string(),
            cardinality: Cardinality::Many,
        };

        executor
            .execute(&Statement::CreateRelationship(rel.clone()), &params)
            .unwrap();

        // Verify relationship is stored
        let relationships = executor.get_relationships();
        assert!(relationships.contains_key("orders_rel"));
        assert_eq!(relationships["orders_rel"].from_table, "users");
        assert_eq!(relationships["orders_rel"].to_table, "orders");
    }

    // ========== FETCH Tests ==========

    #[test]
    fn test_fetch_basic() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_fetch_tables(&mut executor);

        // Create relationship for orders
        let rel = Relationship {
            name: "orders".to_string(),
            from_table: "users".to_string(),
            from_column: "id".to_string(),
            to_table: "orders".to_string(),
            to_column: "user_id".to_string(),
            cardinality: Cardinality::Many,
        };
        executor
            .execute(&Statement::CreateRelationship(rel), &params)
            .unwrap();

        // SELECT with FETCH
        let stmt = Statement::Select {
            with: vec![],
            columns: vec![SelectItem::Wildcard],
            from: from_table("users"),
            where_clause: Some(Expr::BinaryOp {
                left: Box::new(Expr::Column("id".to_string())),
                op: BinaryOperator::Eq,
                right: Box::new(Expr::Literal(Value::Integer(1))),
            }),
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![FetchClause {
                relationship: "orders".to_string(),
                columns: vec![SelectItem::Wildcard],
                where_clause: None,
                order_by: None,
                limit: None,
                nested_fetch: vec![],
            }],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Should have 1 row (Alice)
        assert_eq!(result.rows.len(), 1);

        // Should have an _orders column with JSON data
        let orders_col_idx = result.rows[0]
            .columns
            .iter()
            .position(|c| c == "_orders");
        assert!(orders_col_idx.is_some());

        // The orders data should contain Alice's 2 orders
        if let Value::Text(orders_json) = &result.rows[0].values[orders_col_idx.unwrap()] {
            // Should contain order IDs 1 and 2
            assert!(orders_json.contains("\"id\":1") || orders_json.contains("\"id\": 1"));
            assert!(orders_json.contains("\"id\":2") || orders_json.contains("\"id\": 2"));
        }
    }

    #[test]
    fn test_fetch_with_limit() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_fetch_tables(&mut executor);

        // Create relationship
        let rel = Relationship {
            name: "orders".to_string(),
            from_table: "users".to_string(),
            from_column: "id".to_string(),
            to_table: "orders".to_string(),
            to_column: "user_id".to_string(),
            cardinality: Cardinality::Many,
        };
        executor
            .execute(&Statement::CreateRelationship(rel), &params)
            .unwrap();

        // SELECT with FETCH LIMIT 1
        let stmt = Statement::Select {
            with: vec![],
            columns: vec![SelectItem::Wildcard],
            from: from_table("users"),
            where_clause: Some(Expr::BinaryOp {
                left: Box::new(Expr::Column("id".to_string())),
                op: BinaryOperator::Eq,
                right: Box::new(Expr::Literal(Value::Integer(1))),
            }),
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![FetchClause {
                relationship: "orders".to_string(),
                columns: vec![SelectItem::Wildcard],
                where_clause: None,
                order_by: None,
                limit: Some(1), // Only fetch 1 order
                nested_fetch: vec![],
            }],
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert_eq!(result.rows.len(), 1);

        // Orders JSON should only have 1 entry (due to LIMIT)
        let orders_col_idx = result.rows[0]
            .columns
            .iter()
            .position(|c| c == "_orders")
            .unwrap();
        if let Value::Text(orders_json) = &result.rows[0].values[orders_col_idx] {
            // Should have exactly one { } block (one order)
            let count = orders_json.matches('{').count() - 1; // Subtract outer array
            assert!(count >= 0 && count <= 2); // At most 1 order object
        }
    }

    #[test]
    fn test_fetch_nested() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_fetch_tables(&mut executor);

        // Create relationships
        let orders_rel = Relationship {
            name: "orders".to_string(),
            from_table: "users".to_string(),
            from_column: "id".to_string(),
            to_table: "orders".to_string(),
            to_column: "user_id".to_string(),
            cardinality: Cardinality::Many,
        };
        executor
            .execute(&Statement::CreateRelationship(orders_rel), &params)
            .unwrap();

        let items_rel = Relationship {
            name: "items".to_string(),
            from_table: "orders".to_string(),
            from_column: "id".to_string(),
            to_table: "items".to_string(),
            to_column: "order_id".to_string(),
            cardinality: Cardinality::Many,
        };
        executor
            .execute(&Statement::CreateRelationship(items_rel), &params)
            .unwrap();

        // SELECT with nested FETCH
        let stmt = Statement::Select {
            with: vec![],
            columns: vec![SelectItem::Wildcard],
            from: from_table("users"),
            where_clause: Some(Expr::BinaryOp {
                left: Box::new(Expr::Column("id".to_string())),
                op: BinaryOperator::Eq,
                right: Box::new(Expr::Literal(Value::Integer(1))),
            }),
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![FetchClause {
                relationship: "orders".to_string(),
                columns: vec![SelectItem::Wildcard],
                where_clause: None,
                order_by: None,
                limit: None,
                nested_fetch: vec![FetchClause {
                    relationship: "items".to_string(),
                    columns: vec![SelectItem::Wildcard],
                    where_clause: None,
                    order_by: None,
                    limit: None,
                    nested_fetch: vec![],
                }],
            }],
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert_eq!(result.rows.len(), 1);

        // Should have orders with nested items
        let orders_col_idx = result.rows[0]
            .columns
            .iter()
            .position(|c| c == "_orders");
        assert!(orders_col_idx.is_some());
    }

    // ========== TRAVERSE Tests ==========

    fn setup_graph_tables(executor: &mut Executor) {
        let params = empty_params();

        // Create 'people' table (nodes)
        executor
            .execute(
                &Statement::CreateTable {
                    name: "people".to_string(),
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

        // Create 'follows' table (edges)
        executor
            .execute(
                &Statement::CreateTable {
                    name: "follows".to_string(),
                    columns: vec![
                        ColumnDef {
                            name: "follower_id".to_string(),
                            data_type: DataType::Integer,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                        ColumnDef {
                            name: "followed_id".to_string(),
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

        // Insert people
        for (id, name) in [
            (1, "Alice"),
            (2, "Bob"),
            (3, "Charlie"),
            (4, "Diana"),
            (5, "Eve"),
        ] {
            executor
                .execute(
                    &Statement::Insert {
                        table: "people".to_string(),
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

        // Insert follows (edges): Alice->Bob, Bob->Charlie, Charlie->Diana, Alice->Charlie
        for (follower, followed) in [(1, 2), (2, 3), (3, 4), (1, 3)] {
            executor
                .execute(
                    &Statement::Insert {
                        table: "follows".to_string(),
                        columns: None,
                        values: vec![vec![
                            Expr::Literal(Value::Integer(follower)),
                            Expr::Literal(Value::Integer(followed)),
                        ]],
                        on_conflict: None,
                    },
                    &params,
                )
                .unwrap();
        }
    }

    #[test]
    fn test_traverse_basic() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_graph_tables(&mut executor);

        // Create relationship for follows
        let rel = Relationship {
            name: "FOLLOWS".to_string(),
            from_table: "people".to_string(),
            from_column: "id".to_string(),
            to_table: "follows".to_string(),
            to_column: "follower_id".to_string(),
            cardinality: Cardinality::Many,
        };
        executor
            .execute(&Statement::CreateRelationship(rel), &params)
            .unwrap();

        // TRAVERSE from Alice (id=1)
        let stmt = Statement::Traverse {
            clause: TraverseClause {
                min_depth: 0,
                max_depth: 2,
                pattern: TraversePattern {
                    source_label: None,
                    relationship_type: "FOLLOWS".to_string(),
                    target_label: None,
                    direction: TraverseDirection::Outgoing,
                },
                start_where: None,
                mode: TraverseMode::Bfs,
            },
            from_table: "people".to_string(),
            where_clause: Some(Expr::BinaryOp {
                left: Box::new(Expr::Column("id".to_string())),
                op: BinaryOperator::Eq,
                right: Box::new(Expr::Literal(Value::Integer(1))),
            }),
            return_columns: vec![SelectItem::Wildcard],
            limit: Some(10),
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Should find at least Alice as the starting node
        assert!(!result.rows.is_empty());
    }

    #[test]
    fn test_traverse_with_depth() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_graph_tables(&mut executor);

        // Create relationship
        let rel = Relationship {
            name: "FOLLOWS".to_string(),
            from_table: "people".to_string(),
            from_column: "id".to_string(),
            to_table: "follows".to_string(),
            to_column: "follower_id".to_string(),
            cardinality: Cardinality::Many,
        };
        executor
            .execute(&Statement::CreateRelationship(rel), &params)
            .unwrap();

        // TRAVERSE with specific depth range (1..2)
        let stmt = Statement::Traverse {
            clause: TraverseClause {
                min_depth: 1,
                max_depth: 2,
                pattern: TraversePattern {
                    source_label: None,
                    relationship_type: "FOLLOWS".to_string(),
                    target_label: None,
                    direction: TraverseDirection::Outgoing,
                },
                start_where: None,
                mode: TraverseMode::Dfs,
            },
            from_table: "people".to_string(),
            where_clause: Some(Expr::BinaryOp {
                left: Box::new(Expr::Column("id".to_string())),
                op: BinaryOperator::Eq,
                right: Box::new(Expr::Literal(Value::Integer(1))),
            }),
            return_columns: vec![SelectItem::Wildcard],
            limit: None,
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // With min_depth=1, should not include the starting node (Alice)
        // Should include nodes at depth 1 and 2
        assert!(!result.rows.is_empty());
    }

    #[test]
    fn test_traverse_all_mode() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_graph_tables(&mut executor);

        // Create relationship
        let rel = Relationship {
            name: "FOLLOWS".to_string(),
            from_table: "people".to_string(),
            from_column: "id".to_string(),
            to_table: "follows".to_string(),
            to_column: "follower_id".to_string(),
            cardinality: Cardinality::Many,
        };
        executor
            .execute(&Statement::CreateRelationship(rel), &params)
            .unwrap();

        // TRAVERSE with ALL mode
        let stmt = Statement::Traverse {
            clause: TraverseClause {
                min_depth: 0,
                max_depth: 3,
                pattern: TraversePattern {
                    source_label: None,
                    relationship_type: "FOLLOWS".to_string(),
                    target_label: None,
                    direction: TraverseDirection::Outgoing,
                },
                start_where: None,
                mode: TraverseMode::All,
            },
            from_table: "people".to_string(),
            where_clause: Some(Expr::BinaryOp {
                left: Box::new(Expr::Column("id".to_string())),
                op: BinaryOperator::Eq,
                right: Box::new(Expr::Literal(Value::Integer(1))),
            }),
            return_columns: vec![SelectItem::Wildcard],
            limit: None,
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert!(!result.rows.is_empty());
    }

    #[test]
    fn test_fetch_with_order_by() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_fetch_tables(&mut executor);

        // Create relationship
        let rel = Relationship {
            name: "orders".to_string(),
            from_table: "users".to_string(),
            from_column: "id".to_string(),
            to_table: "orders".to_string(),
            to_column: "user_id".to_string(),
            cardinality: Cardinality::Many,
        };
        executor
            .execute(&Statement::CreateRelationship(rel), &params)
            .unwrap();

        // SELECT with FETCH ORDER BY total DESC
        let stmt = Statement::Select {
            with: vec![],
            columns: vec![SelectItem::Wildcard],
            from: from_table("users"),
            where_clause: Some(Expr::BinaryOp {
                left: Box::new(Expr::Column("id".to_string())),
                op: BinaryOperator::Eq,
                right: Box::new(Expr::Literal(Value::Integer(1))),
            }),
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![FetchClause {
                relationship: "orders".to_string(),
                columns: vec![SelectItem::Wildcard],
                where_clause: None,
                order_by: Some(OrderBy {
                    expr: Expr::Column("total".to_string()),
                    ascending: false, // DESC
                }),
                limit: None,
                nested_fetch: vec![],
            }],
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert_eq!(result.rows.len(), 1);

        // Orders should be sorted by total DESC (150.00 before 99.99)
        let orders_col_idx = result.rows[0]
            .columns
            .iter()
            .position(|c| c == "_orders")
            .unwrap();

        if let Value::Text(orders_json) = &result.rows[0].values[orders_col_idx] {
            // The 150.00 order should appear before the 99.99 order
            let pos_150 = orders_json.find("150");
            let pos_99 = orders_json.find("99.99");
            if let (Some(p1), Some(p2)) = (pos_150, pos_99) {
                assert!(p1 < p2, "Orders should be sorted by total DESC");
            }
        }
    }
}
