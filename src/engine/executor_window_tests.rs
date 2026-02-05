// Tests for Window Function operations

#[cfg(test)]
mod window_tests {
    use crate::engine::executor::Executor;
    use crate::query::ast::{
        ColumnDef, DataType, Expr, FromClause, SelectItem, Statement, WindowOrderBy, WindowSpec,
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

    fn setup_test_tables(executor: &mut Executor) {
        let params = empty_params();

        // Create 'employees' table for window function tests
        executor
            .execute(
                &Statement::CreateTable {
                    name: "employees".to_string(),
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
                        ColumnDef {
                            name: "salary".to_string(),
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

        // Insert employees with varying salaries
        let employees = [
            (1, "Alice", "Engineering", 80000),
            (2, "Bob", "Engineering", 90000),
            (3, "Charlie", "Sales", 70000),
            (4, "Diana", "Sales", 75000),
            (5, "Eve", "Sales", 65000),
            (6, "Frank", "HR", 60000),
        ];

        for (id, name, dept, salary) in employees {
            executor
                .execute(
                    &Statement::Insert {
                        table: "employees".to_string(),
                        columns: None,
                        values: vec![vec![
                            Expr::Literal(Value::Integer(id)),
                            Expr::Literal(Value::Text(name.to_string())),
                            Expr::Literal(Value::Text(dept.to_string())),
                            Expr::Literal(Value::Integer(salary)),
                        ]],
                        on_conflict: None,
                    },
                    &params,
                )
                .unwrap();
        }
    }

    #[test]
    fn test_row_number() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_test_tables(&mut executor);

        // SELECT name, salary, ROW_NUMBER() OVER (ORDER BY salary DESC) as row_num FROM employees
        let stmt = Statement::Select {
            with: vec![],
            columns: vec![
                SelectItem::Expr {
                    expr: Expr::Column("name".to_string()),
                    alias: None,
                },
                SelectItem::Expr {
                    expr: Expr::Column("salary".to_string()),
                    alias: None,
                },
                SelectItem::Expr {
                    expr: Expr::WindowFunction {
                        function: Box::new(Expr::Function {
                            name: "ROW_NUMBER".to_string(),
                            args: vec![],
                        }),
                        window_spec: WindowSpec {
                            partition_by: vec![],
                            order_by: vec![WindowOrderBy {
                                expr: Expr::Column("salary".to_string()),
                                ascending: false,
                                nulls_first: None,
                            }],
                            frame: None,
                        },
                    },
                    alias: Some("row_num".to_string()),
                },
            ],
            from: from_table("employees"),
            where_clause: None,
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Should have 6 rows (all employees)
        assert_eq!(result.rows.len(), 6);

        // Verify row_num column exists
        assert!(result.rows.iter().all(|r| r.columns.contains(&"row_num".to_string())));
    }

    #[test]
    fn test_row_number_with_partition() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_test_tables(&mut executor);

        // SELECT name, department, salary, ROW_NUMBER() OVER (PARTITION BY department ORDER BY salary DESC) as dept_rank FROM employees
        let stmt = Statement::Select {
            with: vec![],
            columns: vec![
                SelectItem::Expr {
                    expr: Expr::Column("name".to_string()),
                    alias: None,
                },
                SelectItem::Expr {
                    expr: Expr::Column("department".to_string()),
                    alias: None,
                },
                SelectItem::Expr {
                    expr: Expr::Column("salary".to_string()),
                    alias: None,
                },
                SelectItem::Expr {
                    expr: Expr::WindowFunction {
                        function: Box::new(Expr::Function {
                            name: "ROW_NUMBER".to_string(),
                            args: vec![],
                        }),
                        window_spec: WindowSpec {
                            partition_by: vec![Expr::Column("department".to_string())],
                            order_by: vec![WindowOrderBy {
                                expr: Expr::Column("salary".to_string()),
                                ascending: false,
                                nulls_first: None,
                            }],
                            frame: None,
                        },
                    },
                    alias: Some("dept_rank".to_string()),
                },
            ],
            from: from_table("employees"),
            where_clause: None,
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Should have 6 rows
        assert_eq!(result.rows.len(), 6);

        // Verify dept_rank column exists
        assert!(result.rows.iter().all(|r| r.columns.contains(&"dept_rank".to_string())));
    }

    #[test]
    fn test_rank() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_test_tables(&mut executor);

        // SELECT name, salary, RANK() OVER (ORDER BY salary DESC) as salary_rank FROM employees
        let stmt = Statement::Select {
            with: vec![],
            columns: vec![
                SelectItem::Expr {
                    expr: Expr::Column("name".to_string()),
                    alias: None,
                },
                SelectItem::Expr {
                    expr: Expr::Column("salary".to_string()),
                    alias: None,
                },
                SelectItem::Expr {
                    expr: Expr::WindowFunction {
                        function: Box::new(Expr::Function {
                            name: "RANK".to_string(),
                            args: vec![],
                        }),
                        window_spec: WindowSpec {
                            partition_by: vec![],
                            order_by: vec![WindowOrderBy {
                                expr: Expr::Column("salary".to_string()),
                                ascending: false,
                                nulls_first: None,
                            }],
                            frame: None,
                        },
                    },
                    alias: Some("salary_rank".to_string()),
                },
            ],
            from: from_table("employees"),
            where_clause: None,
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Should have 6 rows
        assert_eq!(result.rows.len(), 6);

        // Verify salary_rank column exists
        assert!(result.rows.iter().all(|r| r.columns.contains(&"salary_rank".to_string())));
    }

    #[test]
    fn test_lag() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_test_tables(&mut executor);

        // SELECT name, salary, LAG(salary) OVER (ORDER BY salary) as prev_salary FROM employees
        let stmt = Statement::Select {
            with: vec![],
            columns: vec![
                SelectItem::Expr {
                    expr: Expr::Column("name".to_string()),
                    alias: None,
                },
                SelectItem::Expr {
                    expr: Expr::Column("salary".to_string()),
                    alias: None,
                },
                SelectItem::Expr {
                    expr: Expr::WindowFunction {
                        function: Box::new(Expr::Function {
                            name: "LAG".to_string(),
                            args: vec![Expr::Column("salary".to_string())],
                        }),
                        window_spec: WindowSpec {
                            partition_by: vec![],
                            order_by: vec![WindowOrderBy {
                                expr: Expr::Column("salary".to_string()),
                                ascending: true,
                                nulls_first: None,
                            }],
                            frame: None,
                        },
                    },
                    alias: Some("prev_salary".to_string()),
                },
            ],
            from: from_table("employees"),
            where_clause: None,
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Should have 6 rows
        assert_eq!(result.rows.len(), 6);

        // Verify prev_salary column exists
        assert!(result.rows.iter().all(|r| r.columns.contains(&"prev_salary".to_string())));
    }

    #[test]
    fn test_lead() {
        let mut executor = Executor::new();
        let params = empty_params();
        setup_test_tables(&mut executor);

        // SELECT name, salary, LEAD(salary) OVER (ORDER BY salary) as next_salary FROM employees
        let stmt = Statement::Select {
            with: vec![],
            columns: vec![
                SelectItem::Expr {
                    expr: Expr::Column("name".to_string()),
                    alias: None,
                },
                SelectItem::Expr {
                    expr: Expr::Column("salary".to_string()),
                    alias: None,
                },
                SelectItem::Expr {
                    expr: Expr::WindowFunction {
                        function: Box::new(Expr::Function {
                            name: "LEAD".to_string(),
                            args: vec![Expr::Column("salary".to_string())],
                        }),
                        window_spec: WindowSpec {
                            partition_by: vec![],
                            order_by: vec![WindowOrderBy {
                                expr: Expr::Column("salary".to_string()),
                                ascending: true,
                                nulls_first: None,
                            }],
                            frame: None,
                        },
                    },
                    alias: Some("next_salary".to_string()),
                },
            ],
            from: from_table("employees"),
            where_clause: None,
            group_by: vec![],
            having: None,
            using_engine: None,
            order_by: None,
            limit: None,
            fetch: vec![],
        };

        let result = executor.execute(&stmt, &params).unwrap();

        // Should have 6 rows
        assert_eq!(result.rows.len(), 6);

        // Verify next_salary column exists
        assert!(result.rows.iter().all(|r| r.columns.contains(&"next_salary".to_string())));
    }
}
