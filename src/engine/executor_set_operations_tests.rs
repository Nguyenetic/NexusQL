// Set operations tests for UNION, INTERSECT, EXCEPT

#[cfg(test)]
mod set_operations_tests {
    use crate::engine::executor::Executor;
    use crate::query::ast::{ColumnDef, DataType, Expr, FromClause, SelectItem, SetOperator, Statement};
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
                    ],
                    if_not_exists: false,
                },
                &params,
            )
            .unwrap();

        // Create 'admins' table
        executor
            .execute(
                &Statement::CreateTable {
                    name: "admins".to_string(),
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

        // Insert data into users
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

        // Insert data into admins (Bob and Diana overlap)
        for (id, name) in [(2, "Bob"), (4, "Diana")] {
            executor
                .execute(
                    &Statement::Insert {
                        table: "admins".to_string(),
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
    }

    #[test]
    fn test_union() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_test_tables(&mut executor);

        // SELECT * FROM users UNION SELECT * FROM admins
        let stmt = Statement::SetOperation {
            left: Box::new(Statement::Select {
                with: vec![],
                columns: vec![SelectItem::Wildcard],
                from: from_table("users"),
                where_clause: None,
                group_by: vec![],
                having: None,
                using_engine: None,
                order_by: None,
                limit: None,
                fetch: vec![],
            }),
            op: SetOperator::Union,
            right: Box::new(Statement::Select {
                with: vec![],
                columns: vec![SelectItem::Wildcard],
                from: from_table("admins"),
                where_clause: None,
                group_by: vec![],
                having: None,
                using_engine: None,
                order_by: None,
                limit: None,
                fetch: vec![],
            }),
            order_by: None,
            limit: None,
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Users: Alice(1), Bob(2), Charlie(3)
        // Admins: Bob(2), Diana(4)
        // Union: Alice, Bob, Charlie, Diana = 4 unique rows
        assert_eq!(result.rows.len(), 4);
    }

    #[test]
    fn test_union_all() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_test_tables(&mut executor);

        let stmt = Statement::SetOperation {
            left: Box::new(Statement::Select {
                with: vec![],
                columns: vec![SelectItem::Wildcard],
                from: from_table("users"),
                where_clause: None,
                group_by: vec![],
                having: None,
                using_engine: None,
                order_by: None,
                limit: None,
                fetch: vec![],
            }),
            op: SetOperator::UnionAll,
            right: Box::new(Statement::Select {
                with: vec![],
                columns: vec![SelectItem::Wildcard],
                from: from_table("admins"),
                where_clause: None,
                group_by: vec![],
                having: None,
                using_engine: None,
                order_by: None,
                limit: None,
                fetch: vec![],
            }),
            order_by: None,
            limit: None,
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Users: 3 rows, Admins: 2 rows
        // Union All: 5 rows (Bob appears twice)
        assert_eq!(result.rows.len(), 5);
    }

    #[test]
    fn test_intersect() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_test_tables(&mut executor);

        let stmt = Statement::SetOperation {
            left: Box::new(Statement::Select {
                with: vec![],
                columns: vec![SelectItem::Wildcard],
                from: from_table("users"),
                where_clause: None,
                group_by: vec![],
                having: None,
                using_engine: None,
                order_by: None,
                limit: None,
                fetch: vec![],
            }),
            op: SetOperator::Intersect,
            right: Box::new(Statement::Select {
                with: vec![],
                columns: vec![SelectItem::Wildcard],
                from: from_table("admins"),
                where_clause: None,
                group_by: vec![],
                having: None,
                using_engine: None,
                order_by: None,
                limit: None,
                fetch: vec![],
            }),
            order_by: None,
            limit: None,
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Only Bob (id=2) is in both tables
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].values[0], Value::Integer(2));
        assert_eq!(result.rows[0].values[1], Value::Text("Bob".to_string()));
    }

    #[test]
    fn test_except() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_test_tables(&mut executor);

        let stmt = Statement::SetOperation {
            left: Box::new(Statement::Select {
                with: vec![],
                columns: vec![SelectItem::Wildcard],
                from: from_table("users"),
                where_clause: None,
                group_by: vec![],
                having: None,
                using_engine: None,
                order_by: None,
                limit: None,
                fetch: vec![],
            }),
            op: SetOperator::Except,
            right: Box::new(Statement::Select {
                with: vec![],
                columns: vec![SelectItem::Wildcard],
                from: from_table("admins"),
                where_clause: None,
                group_by: vec![],
                having: None,
                using_engine: None,
                order_by: None,
                limit: None,
                fetch: vec![],
            }),
            order_by: None,
            limit: None,
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Users minus Admins = Alice, Charlie (not Bob)
        assert_eq!(result.rows.len(), 2);

        let names: Vec<_> = result
            .rows
            .iter()
            .map(|r| r.values[1].clone())
            .collect();
        assert!(names.contains(&Value::Text("Alice".to_string())));
        assert!(names.contains(&Value::Text("Charlie".to_string())));
        assert!(!names.contains(&Value::Text("Bob".to_string())));
    }

    #[test]
    fn test_union_with_limit() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_test_tables(&mut executor);

        let stmt = Statement::SetOperation {
            left: Box::new(Statement::Select {
                with: vec![],
                columns: vec![SelectItem::Wildcard],
                from: from_table("users"),
                where_clause: None,
                group_by: vec![],
                having: None,
                using_engine: None,
                order_by: None,
                limit: None,
                fetch: vec![],
            }),
            op: SetOperator::Union,
            right: Box::new(Statement::Select {
                with: vec![],
                columns: vec![SelectItem::Wildcard],
                from: from_table("admins"),
                where_clause: None,
                group_by: vec![],
                having: None,
                using_engine: None,
                order_by: None,
                limit: None,
                fetch: vec![],
            }),
            order_by: None,
            limit: Some(2),
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Union would be 4 rows, but LIMIT 2
        assert_eq!(result.rows.len(), 2);
    }
}
