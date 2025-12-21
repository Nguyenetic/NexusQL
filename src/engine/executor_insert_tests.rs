// Comprehensive tests for INSERT column mapping fix
// Tests for GitHub issues #18 and #19

#[cfg(test)]
mod insert_column_mapping_tests {
    use crate::engine::executor::Executor;
    use crate::query::ast::{ColumnDef, DataType, Expr, Statement};
    use crate::query::evaluator::QueryParams;
    use crate::types::Value;

    #[test]
    fn test_insert_with_explicit_columns() {
        let mut executor = Executor::new();

        // Create table: id, name, age, email
        executor
            .execute(
                &Statement::CreateTable {
                    name: "users".to_string(),
                    columns: vec![
                        ColumnDef {
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                        ColumnDef {
                            name: "name".to_string(),
                            data_type: DataType::Text,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                        ColumnDef {
                            name: "age".to_string(),
                            data_type: DataType::Integer,
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
                },
                &QueryParams::default(),
            )
            .unwrap();

        // INSERT with explicit columns (name, age, email) - skipping id
        executor
            .execute(
                &Statement::Insert {
                    table: "users".to_string(),
                    columns: Some(vec![
                        "name".to_string(),
                        "age".to_string(),
                        "email".to_string(),
                    ]),
                    values: vec![vec![
                        Expr::Literal(Value::Text("Alice".to_string())),
                        Expr::Literal(Value::Integer(25)),
                        Expr::Literal(Value::Text("alice@example.com".to_string())),
                    ]],
                },
                &QueryParams::default(),
            )
            .unwrap();

        // Verify the row
        let table = executor.get_tables().get("users").unwrap();
        assert_eq!(table.rows.len(), 1);

        let row = &table.rows[0];
        assert_eq!(row.values[0], Value::Null); // id should be NULL (not set)
        assert_eq!(row.values[1], Value::Text("Alice".to_string())); // name
        assert_eq!(row.values[2], Value::Integer(25)); // age
        assert_eq!(row.values[3], Value::Text("alice@example.com".to_string()));
        // email
    }

    #[test]
    fn test_insert_with_reordered_columns() {
        let mut executor = Executor::new();

        // Create table: id, title, embedding, author
        executor
            .execute(
                &Statement::CreateTable {
                    name: "docs".to_string(),
                    columns: vec![
                        ColumnDef {
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                        ColumnDef {
                            name: "title".to_string(),
                            data_type: DataType::Text,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                        ColumnDef {
                            name: "embedding".to_string(),
                            data_type: DataType::Vector(3),
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                        ColumnDef {
                            name: "author".to_string(),
                            data_type: DataType::Text,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                    ],
                    if_not_exists: false,
                },
                &QueryParams::default(),
            )
            .unwrap();

        // INSERT with columns in different order than table schema
        executor
            .execute(
                &Statement::Insert {
                    table: "docs".to_string(),
                    columns: Some(vec![
                        "title".to_string(),
                        "author".to_string(),
                        "embedding".to_string(),
                    ]),
                    values: vec![vec![
                        Expr::Literal(Value::Text("Doc 1".to_string())),
                        Expr::Literal(Value::Text("Alice".to_string())),
                        Expr::Literal(Value::Vector(vec![0.1, 0.2, 0.3])),
                    ]],
                },
                &QueryParams::default(),
            )
            .unwrap();

        // Verify the row - values should map to correct columns by name
        let table = executor.get_tables().get("docs").unwrap();
        assert_eq!(table.rows.len(), 1);

        let row = &table.rows[0];
        assert_eq!(row.values[0], Value::Null); // id (not provided)
        assert_eq!(row.values[1], Value::Text("Doc 1".to_string())); // title
        assert_eq!(row.values[2], Value::Vector(vec![0.1, 0.2, 0.3])); // embedding
        assert_eq!(row.values[3], Value::Text("Alice".to_string())); // author
    }

    #[test]
    fn test_insert_without_columns_uses_positional() {
        let mut executor = Executor::new();

        executor
            .execute(
                &Statement::CreateTable {
                    name: "simple".to_string(),
                    columns: vec![
                        ColumnDef {
                            name: "a".to_string(),
                            data_type: DataType::Text,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                        ColumnDef {
                            name: "b".to_string(),
                            data_type: DataType::Integer,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                    ],
                    if_not_exists: false,
                },
                &QueryParams::default(),
            )
            .unwrap();

        // INSERT without explicit columns - should use positional
        executor
            .execute(
                &Statement::Insert {
                    table: "simple".to_string(),
                    columns: None,
                    values: vec![vec![
                        Expr::Literal(Value::Text("test".to_string())),
                        Expr::Literal(Value::Integer(42)),
                    ]],
                },
                &QueryParams::default(),
            )
            .unwrap();

        let table = executor.get_tables().get("simple").unwrap();
        let row = &table.rows[0];
        assert_eq!(row.values[0], Value::Text("test".to_string()));
        assert_eq!(row.values[1], Value::Integer(42));
    }

    #[test]
    fn test_insert_partial_columns() {
        let mut executor = Executor::new();

        executor
            .execute(
                &Statement::CreateTable {
                    name: "partial".to_string(),
                    columns: vec![
                        ColumnDef {
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                        ColumnDef {
                            name: "name".to_string(),
                            data_type: DataType::Text,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                        ColumnDef {
                            name: "optional".to_string(),
                            data_type: DataType::Text,
                            is_primary_key: false,
                            is_autoincrement: false,
                        },
                    ],
                    if_not_exists: false,
                },
                &QueryParams::default(),
            )
            .unwrap();

        // INSERT only some columns - others should be NULL
        executor
            .execute(
                &Statement::Insert {
                    table: "partial".to_string(),
                    columns: Some(vec!["name".to_string()]),
                    values: vec![vec![Expr::Literal(Value::Text("Bob".to_string()))]],
                },
                &QueryParams::default(),
            )
            .unwrap();

        let table = executor.get_tables().get("partial").unwrap();
        let row = &table.rows[0];
        assert_eq!(row.values[0], Value::Null); // id
        assert_eq!(row.values[1], Value::Text("Bob".to_string())); // name
        assert_eq!(row.values[2], Value::Null); // optional
    }

    #[test]
    fn test_insert_invalid_column_name() {
        let mut executor = Executor::new();

        executor
            .execute(
                &Statement::CreateTable {
                    name: "test".to_string(),
                    columns: vec![ColumnDef {
                        name: "valid".to_string(),
                        data_type: DataType::Text,
                        is_primary_key: false,
                        is_autoincrement: false,
                    }],
                    if_not_exists: false,
                },
                &QueryParams::default(),
            )
            .unwrap();

        // INSERT with non-existent column should fail
        let result = executor.execute(
            &Statement::Insert {
                table: "test".to_string(),
                columns: Some(vec!["invalid".to_string()]),
                values: vec![vec![Expr::Literal(Value::Text("test".to_string()))]],
            },
            &QueryParams::default(),
        );

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Column invalid not found"));
    }

    #[test]
    fn test_insert_multiple_rows_with_columns() {
        let mut executor = Executor::new();

        executor
            .execute(
                &Statement::CreateTable {
                    name: "multi".to_string(),
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
                },
                &QueryParams::default(),
            )
            .unwrap();

        // INSERT multiple rows with explicit columns
        executor
            .execute(
                &Statement::Insert {
                    table: "multi".to_string(),
                    columns: Some(vec!["value".to_string()]),
                    values: vec![
                        vec![Expr::Literal(Value::Text("first".to_string()))],
                        vec![Expr::Literal(Value::Text("second".to_string()))],
                        vec![Expr::Literal(Value::Text("third".to_string()))],
                    ],
                },
                &QueryParams::default(),
            )
            .unwrap();

        let table = executor.get_tables().get("multi").unwrap();
        assert_eq!(table.rows.len(), 3);

        // All id columns should be NULL, value columns should have correct values
        assert_eq!(table.rows[0].values[0], Value::Null);
        assert_eq!(table.rows[0].values[1], Value::Text("first".to_string()));

        assert_eq!(table.rows[1].values[0], Value::Null);
        assert_eq!(table.rows[1].values[1], Value::Text("second".to_string()));

        assert_eq!(table.rows[2].values[0], Value::Null);
        assert_eq!(table.rows[2].values[1], Value::Text("third".to_string()));
    }
}
