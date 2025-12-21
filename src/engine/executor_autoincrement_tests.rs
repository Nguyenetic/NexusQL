#[cfg(test)]
mod autoincrement_tests {
    use crate::engine::executor::Executor;
    use crate::query::ast::{ColumnDef, DataType, Expr, SelectItem, Statement};
    use crate::query::evaluator::QueryParams;
    use crate::types::Value;
    use std::collections::HashMap;

    fn empty_params() -> QueryParams {
        QueryParams {
            positional: Vec::new(),
            named: HashMap::new(),
        }
    }

    #[test]
    fn test_autoincrement_basic() {
        let mut executor = Executor::new();
        let params = empty_params();

        // Create table with AUTO_INCREMENT
        let create_stmt = Statement::CreateTable {
            name: "users".to_string(),
            columns: vec![
                ColumnDef {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                    is_primary_key: true,
                    is_autoincrement: true,
                },
                ColumnDef {
                    name: "name".to_string(),
                    data_type: DataType::Text,
                    is_primary_key: false,
                    is_autoincrement: false,
                },
            ],
            if_not_exists: false,
        };
        executor.execute(&create_stmt, &params).unwrap();

        // Insert without specifying ID (should auto-generate)
        let insert1 = Statement::Insert {
            table: "users".to_string(),
            columns: Some(vec!["name".to_string()]),
            values: vec![vec![Expr::Literal(Value::Text("Alice".to_string()))]],
        };
        executor.execute(&insert1, &params).unwrap();

        // Insert another row
        let insert2 = Statement::Insert {
            table: "users".to_string(),
            columns: Some(vec!["name".to_string()]),
            values: vec![vec![Expr::Literal(Value::Text("Bob".to_string()))]],
        };
        executor.execute(&insert2, &params).unwrap();

        // Query all rows
        let select = Statement::Select {
            columns: vec![SelectItem::Wildcard],
            from: "users".to_string(),
            where_clause: None,
            using_engine: None,
            order_by: None,
            limit: None,
        };
        let result = executor.execute(&select, &params).unwrap();

        // Check that IDs were auto-generated
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.rows[0].values[0], Value::Integer(1));
        assert_eq!(result.rows[0].values[1], Value::Text("Alice".to_string()));
        assert_eq!(result.rows[1].values[0], Value::Integer(2));
        assert_eq!(result.rows[1].values[1], Value::Text("Bob".to_string()));
    }

    #[test]
    fn test_autoincrement_with_explicit_id() {
        let mut executor = Executor::new();
        let params = empty_params();

        // Create table
        let create_stmt = Statement::CreateTable {
            name: "items".to_string(),
            columns: vec![
                ColumnDef {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                    is_primary_key: true,
                    is_autoincrement: true,
                },
                ColumnDef {
                    name: "value".to_string(),
                    data_type: DataType::Text,
                    is_primary_key: false,
                    is_autoincrement: false,
                },
            ],
            if_not_exists: false,
        };
        executor.execute(&create_stmt, &params).unwrap();

        // Insert with explicit ID (should use provided ID, not auto-generate)
        let insert1 = Statement::Insert {
            table: "items".to_string(),
            columns: Some(vec!["id".to_string(), "value".to_string()]),
            values: vec![vec![
                Expr::Literal(Value::Integer(100)),
                Expr::Literal(Value::Text("Custom".to_string())),
            ]],
        };
        executor.execute(&insert1, &params).unwrap();

        // Insert without ID (should auto-increment from next available)
        let insert2 = Statement::Insert {
            table: "items".to_string(),
            columns: Some(vec!["value".to_string()]),
            values: vec![vec![Expr::Literal(Value::Text("Auto".to_string()))]],
        };
        executor.execute(&insert2, &params).unwrap();

        // Query
        let select = Statement::Select {
            columns: vec![SelectItem::Wildcard],
            from: "items".to_string(),
            where_clause: None,
            using_engine: None,
            order_by: None,
            limit: None,
        };
        let result = executor.execute(&select, &params).unwrap();

        // First row should have explicit ID 100
        assert_eq!(result.rows[0].values[0], Value::Integer(100));
        // Second row should auto-increment from 1 (not affected by explicit ID)
        assert_eq!(result.rows[1].values[0], Value::Integer(1));
    }

    #[test]
    fn test_autoincrement_without_column_list() {
        let mut executor = Executor::new();
        let params = empty_params();

        // Create table
        let create_stmt = Statement::CreateTable {
            name: "test".to_string(),
            columns: vec![
                ColumnDef {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                    is_primary_key: true,
                    is_autoincrement: true,
                },
                ColumnDef {
                    name: "name".to_string(),
                    data_type: DataType::Text,
                    is_primary_key: false,
                    is_autoincrement: false,
                },
            ],
            if_not_exists: false,
        };
        executor.execute(&create_stmt, &params).unwrap();

        // Insert without column list (all columns specified positionally)
        let insert = Statement::Insert {
            table: "test".to_string(),
            columns: None,
            values: vec![vec![
                Expr::Literal(Value::Null), // NULL for AUTO_INCREMENT
                Expr::Literal(Value::Text("Test".to_string())),
            ]],
        };
        executor.execute(&insert, &params).unwrap();

        // Query
        let select = Statement::Select {
            columns: vec![SelectItem::Wildcard],
            from: "test".to_string(),
            where_clause: None,
            using_engine: None,
            order_by: None,
            limit: None,
        };
        let result = executor.execute(&select, &params).unwrap();

        // Should auto-generate ID even when NULL provided
        assert_eq!(result.rows[0].values[0], Value::Integer(1));
        assert_eq!(result.rows[0].values[1], Value::Text("Test".to_string()));
    }

    #[test]
    fn test_autoincrement_multiple_inserts() {
        let mut executor = Executor::new();
        let params = empty_params();

        // Create table
        let create_stmt = Statement::CreateTable {
            name: "counter".to_string(),
            columns: vec![
                ColumnDef {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                    is_primary_key: true,
                    is_autoincrement: true,
                },
                ColumnDef {
                    name: "data".to_string(),
                    data_type: DataType::Text,
                    is_primary_key: false,
                    is_autoincrement: false,
                },
            ],
            if_not_exists: false,
        };
        executor.execute(&create_stmt, &params).unwrap();

        // Insert 5 rows
        for i in 1..=5 {
            let insert = Statement::Insert {
                table: "counter".to_string(),
                columns: Some(vec!["data".to_string()]),
                values: vec![vec![Expr::Literal(Value::Text(format!("Row {}", i)))]],
            };
            executor.execute(&insert, &params).unwrap();
        }

        // Query
        let select = Statement::Select {
            columns: vec![SelectItem::Wildcard],
            from: "counter".to_string(),
            where_clause: None,
            using_engine: None,
            order_by: None,
            limit: None,
        };
        let result = executor.execute(&select, &params).unwrap();

        // Check IDs are 1, 2, 3, 4, 5
        assert_eq!(result.rows.len(), 5);
        for i in 0..5 {
            assert_eq!(result.rows[i].values[0], Value::Integer((i + 1) as i64));
            assert_eq!(
                result.rows[i].values[1],
                Value::Text(format!("Row {}", i + 1))
            );
        }
    }

    #[test]
    fn test_no_autoincrement_column() {
        let mut executor = Executor::new();
        let params = empty_params();

        // Create table WITHOUT AUTO_INCREMENT
        let create_stmt = Statement::CreateTable {
            name: "manual".to_string(),
            columns: vec![
                ColumnDef {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                    is_primary_key: false,
                    is_autoincrement: false,
                },
                ColumnDef {
                    name: "value".to_string(),
                    data_type: DataType::Text,
                    is_primary_key: false,
                    is_autoincrement: false,
                },
            ],
            if_not_exists: false,
        };
        executor.execute(&create_stmt, &params).unwrap();

        // Insert with NULL ID (should stay NULL, no auto-increment)
        let insert = Statement::Insert {
            table: "manual".to_string(),
            columns: Some(vec!["value".to_string()]),
            values: vec![vec![Expr::Literal(Value::Text("Test".to_string()))]],
        };
        executor.execute(&insert, &params).unwrap();

        // Query
        let select = Statement::Select {
            columns: vec![SelectItem::Wildcard],
            from: "manual".to_string(),
            where_clause: None,
            using_engine: None,
            order_by: None,
            limit: None,
        };
        let result = executor.execute(&select, &params).unwrap();

        // ID should be NULL (no auto-increment)
        assert_eq!(result.rows[0].values[0], Value::Null);
        assert_eq!(result.rows[0].values[1], Value::Text("Test".to_string()));
    }

    #[test]
    fn test_autoincrement_reordered_columns() {
        let mut executor = Executor::new();
        let params = empty_params();

        // Create table
        let create_stmt = Statement::CreateTable {
            name: "reorder".to_string(),
            columns: vec![
                ColumnDef {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                    is_primary_key: true,
                    is_autoincrement: true,
                },
                ColumnDef {
                    name: "name".to_string(),
                    data_type: DataType::Text,
                    is_primary_key: false,
                    is_autoincrement: false,
                },
                ColumnDef {
                    name: "email".to_string(),
                    data_type: DataType::Text,
                    is_primary_key: false,
                    is_autoincrement: false,
                },
            ],
            if_not_exists: false,
        };
        executor.execute(&create_stmt, &params).unwrap();

        // Insert with columns in different order (email, name) - id should still auto-increment
        let insert = Statement::Insert {
            table: "reorder".to_string(),
            columns: Some(vec!["email".to_string(), "name".to_string()]),
            values: vec![vec![
                Expr::Literal(Value::Text("alice@example.com".to_string())),
                Expr::Literal(Value::Text("Alice".to_string())),
            ]],
        };
        executor.execute(&insert, &params).unwrap();

        // Query
        let select = Statement::Select {
            columns: vec![SelectItem::Wildcard],
            from: "reorder".to_string(),
            where_clause: None,
            using_engine: None,
            order_by: None,
            limit: None,
        };
        let result = executor.execute(&select, &params).unwrap();

        // Check correct mapping
        assert_eq!(result.rows[0].values[0], Value::Integer(1)); // id auto-generated
        assert_eq!(result.rows[0].values[1], Value::Text("Alice".to_string())); // name
        assert_eq!(
            result.rows[0].values[2],
            Value::Text("alice@example.com".to_string())
        ); // email
    }
}
