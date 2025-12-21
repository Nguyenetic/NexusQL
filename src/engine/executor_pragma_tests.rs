#[cfg(test)]
mod pragma_tests {
    use crate::engine::executor::Executor;
    use crate::query::ast::{ColumnDef, DataType, Statement};
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
    fn test_pragma_foreign_keys() {
        let mut executor = Executor::new();
        let params = empty_params();

        // Query foreign_keys setting
        let stmt = Statement::Pragma {
            name: "foreign_keys".to_string(),
            value: None,
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].values[0], Value::Integer(0));
    }

    #[test]
    fn test_pragma_journal_mode_query() {
        let mut executor = Executor::new();
        let params = empty_params();

        // Query journal mode (should return WAL)
        let stmt = Statement::Pragma {
            name: "journal_mode".to_string(),
            value: None,
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].values[0], Value::Text("WAL".to_string()));
    }

    #[test]
    fn test_pragma_journal_mode_set() {
        let mut executor = Executor::new();
        let params = empty_params();

        // Set journal mode to DELETE
        let stmt = Statement::Pragma {
            name: "journal_mode".to_string(),
            value: Some("DELETE".to_string()),
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].values[0], Value::Text("DELETE".to_string()));
    }

    #[test]
    fn test_pragma_synchronous_query() {
        let mut executor = Executor::new();
        let params = empty_params();

        // Query synchronous mode (should return FULL = 2)
        let stmt = Statement::Pragma {
            name: "synchronous".to_string(),
            value: None,
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].values[0], Value::Integer(2));
    }

    #[test]
    fn test_pragma_synchronous_set_full() {
        let mut executor = Executor::new();
        let params = empty_params();

        let stmt = Statement::Pragma {
            name: "synchronous".to_string(),
            value: Some("FULL".to_string()),
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].values[0], Value::Integer(2));
    }

    #[test]
    fn test_pragma_synchronous_set_normal() {
        let mut executor = Executor::new();
        let params = empty_params();

        let stmt = Statement::Pragma {
            name: "synchronous".to_string(),
            value: Some("NORMAL".to_string()),
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].values[0], Value::Integer(1));
    }

    #[test]
    fn test_pragma_synchronous_set_off() {
        let mut executor = Executor::new();
        let params = empty_params();

        let stmt = Statement::Pragma {
            name: "synchronous".to_string(),
            value: Some("OFF".to_string()),
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].values[0], Value::Integer(0));
    }

    #[test]
    fn test_pragma_table_info() {
        let mut executor = Executor::new();
        let params = empty_params();

        // Create a test table first
        let create_stmt = Statement::CreateTable {
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
                    name: "email".to_string(),
                    data_type: DataType::Text,
                    is_primary_key: false,
                    is_autoincrement: false,
                },
            ],
            if_not_exists: false,
        };
        executor.execute(&create_stmt, &params).unwrap();

        // Query table info
        let stmt = Statement::Pragma {
            name: "table_info".to_string(),
            value: Some("users".to_string()),
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert_eq!(result.rows.len(), 3); // 3 columns

        // Check first column info
        assert_eq!(result.rows[0].values[0], Value::Integer(0)); // cid
        assert_eq!(result.rows[0].values[1], Value::Text("id".to_string())); // name
        assert_eq!(result.rows[0].values[2], Value::Text("TEXT".to_string())); // type

        // Check second column
        assert_eq!(result.rows[1].values[0], Value::Integer(1));
        assert_eq!(result.rows[1].values[1], Value::Text("name".to_string()));

        // Check third column
        assert_eq!(result.rows[2].values[0], Value::Integer(2));
        assert_eq!(result.rows[2].values[1], Value::Text("email".to_string()));
    }

    #[test]
    fn test_pragma_table_info_nonexistent_table() {
        let mut executor = Executor::new();
        let params = empty_params();

        let stmt = Statement::Pragma {
            name: "table_info".to_string(),
            value: Some("nonexistent".to_string()),
        };

        let result = executor.execute(&stmt, &params);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_pragma_table_info_no_table_name() {
        let mut executor = Executor::new();
        let params = empty_params();

        let stmt = Statement::Pragma {
            name: "table_info".to_string(),
            value: None,
        };

        let result = executor.execute(&stmt, &params);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires a table name"));
    }

    #[test]
    fn test_pragma_user_version_query() {
        let mut executor = Executor::new();
        let params = empty_params();

        let stmt = Statement::Pragma {
            name: "user_version".to_string(),
            value: None,
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].values[0], Value::Integer(0));
    }

    #[test]
    fn test_pragma_user_version_set() {
        let mut executor = Executor::new();
        let params = empty_params();

        let stmt = Statement::Pragma {
            name: "user_version".to_string(),
            value: Some("42".to_string()),
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].values[0], Value::Integer(42));
    }

    #[test]
    fn test_pragma_schema_version() {
        let mut executor = Executor::new();
        let params = empty_params();

        let stmt = Statement::Pragma {
            name: "schema_version".to_string(),
            value: None,
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].values[0], Value::Integer(1));
    }

    #[test]
    fn test_pragma_encoding() {
        let mut executor = Executor::new();
        let params = empty_params();

        let stmt = Statement::Pragma {
            name: "encoding".to_string(),
            value: None,
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].values[0], Value::Text("UTF-8".to_string()));
    }

    #[test]
    fn test_pragma_cache_size_query() {
        let mut executor = Executor::new();
        let params = empty_params();

        let stmt = Statement::Pragma {
            name: "cache_size".to_string(),
            value: None,
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].values[0], Value::Integer(-2000));
    }

    #[test]
    fn test_pragma_cache_size_set() {
        let mut executor = Executor::new();
        let params = empty_params();

        let stmt = Statement::Pragma {
            name: "cache_size".to_string(),
            value: Some("-5000".to_string()),
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].values[0], Value::Integer(-5000));
    }

    #[test]
    fn test_pragma_case_insensitive() {
        let mut executor = Executor::new();
        let params = empty_params();

        // Test uppercase PRAGMA name
        let stmt = Statement::Pragma {
            name: "FOREIGN_KEYS".to_string(),
            value: None,
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].values[0], Value::Integer(0));

        // Test mixed case
        let stmt2 = Statement::Pragma {
            name: "Journal_Mode".to_string(),
            value: None,
        };

        let result2 = executor.execute(&stmt2, &params).unwrap();
        assert_eq!(result2.rows.len(), 1);
        assert_eq!(result2.rows[0].values[0], Value::Text("WAL".to_string()));
    }

    #[test]
    fn test_pragma_unsupported() {
        let mut executor = Executor::new();
        let params = empty_params();

        // Unsupported PRAGMA should return empty result for compatibility
        let stmt = Statement::Pragma {
            name: "unsupported_pragma".to_string(),
            value: None,
        };

        let result = executor.execute(&stmt, &params).unwrap();
        assert_eq!(result.rows.len(), 0);
    }
}
