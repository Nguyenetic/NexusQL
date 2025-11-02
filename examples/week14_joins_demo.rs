// Week 14: Joins & Subqueries Demo
// Demonstrates INNER, OUTER, CROSS, NATURAL joins and subquery operations

use nexusql::join::{JoinAlgorithm, JoinCondition, JoinExecutor, JoinType, SubqueryExecutor};
use nexusql::types::{Row, Value};

fn main() {
    println!("🚀 NexusQL Week 14: Joins & Subqueries Demo");
    println!("===========================================\n");

    demo_inner_join();
    demo_left_outer_join();
    demo_right_outer_join();
    demo_full_outer_join();
    demo_cross_join();
    demo_natural_join();
    demo_join_algorithms();
    demo_subqueries();

    println!("\n✅ All demos completed successfully!");
}

fn demo_inner_join() {
    println!("🔗 Demo 1: INNER JOIN");
    println!("---------------------\n");

    let employees = vec![
        Row::new(
            vec!["id".to_string(), "name".to_string(), "dept_id".to_string()],
            vec![
                Value::Integer(1),
                Value::Text("Alice".to_string()),
                Value::Integer(10),
            ],
        ),
        Row::new(
            vec!["id".to_string(), "name".to_string(), "dept_id".to_string()],
            vec![
                Value::Integer(2),
                Value::Text("Bob".to_string()),
                Value::Integer(20),
            ],
        ),
        Row::new(
            vec!["id".to_string(), "name".to_string(), "dept_id".to_string()],
            vec![
                Value::Integer(3),
                Value::Text("Charlie".to_string()),
                Value::Integer(10),
            ],
        ),
        Row::new(
            vec!["id".to_string(), "name".to_string(), "dept_id".to_string()],
            vec![
                Value::Integer(4),
                Value::Text("David".to_string()),
                Value::Integer(30),
            ],
        ),
    ];

    let departments = vec![
        Row::new(
            vec!["id".to_string(), "dept_name".to_string()],
            vec![Value::Integer(10), Value::Text("Engineering".to_string())],
        ),
        Row::new(
            vec!["id".to_string(), "dept_name".to_string()],
            vec![Value::Integer(20), Value::Text("Sales".to_string())],
        ),
        Row::new(
            vec!["id".to_string(), "dept_name".to_string()],
            vec![Value::Integer(40), Value::Text("Marketing".to_string())],
        ),
    ];

    println!("Employees: {} rows", employees.len());
    println!("Departments: {} rows\n", departments.len());

    let executor = JoinExecutor::new(
        JoinType::Inner,
        JoinAlgorithm::Hash,
        JoinCondition::On {
            left_column: "dept_id".to_string(),
            right_column: "id".to_string(),
        },
    );

    let result = executor.execute(employees, departments).unwrap();

    println!("Result: {} rows (only matching dept_ids)", result.len());
    for row in &result {
        println!(
            "  {} (ID: {}) -> {}",
            row.values[1], row.values[0], row.values[4]
        );
    }
    println!("\n  Note: David (dept_id=30) excluded - no matching department");
    println!("  Note: Marketing (id=40) excluded - no employees");
}

fn demo_left_outer_join() {
    println!("\n\n⬅️  Demo 2: LEFT OUTER JOIN");
    println!("---------------------------\n");

    let orders = vec![
        Row::new(
            vec![
                "order_id".to_string(),
                "customer_id".to_string(),
                "amount".to_string(),
            ],
            vec![Value::Integer(101), Value::Integer(1), Value::Integer(100)],
        ),
        Row::new(
            vec![
                "order_id".to_string(),
                "customer_id".to_string(),
                "amount".to_string(),
            ],
            vec![Value::Integer(102), Value::Integer(2), Value::Integer(200)],
        ),
        Row::new(
            vec![
                "order_id".to_string(),
                "customer_id".to_string(),
                "amount".to_string(),
            ],
            vec![Value::Integer(103), Value::Integer(99), Value::Integer(150)],
        ),
    ];

    let customers = vec![
        Row::new(
            vec!["id".to_string(), "name".to_string()],
            vec![Value::Integer(1), Value::Text("Alice".to_string())],
        ),
        Row::new(
            vec!["id".to_string(), "name".to_string()],
            vec![Value::Integer(2), Value::Text("Bob".to_string())],
        ),
    ];

    let executor = JoinExecutor::new(
        JoinType::LeftOuter,
        JoinAlgorithm::NestedLoop,
        JoinCondition::On {
            left_column: "customer_id".to_string(),
            right_column: "id".to_string(),
        },
    );

    let result = executor.execute(orders, customers).unwrap();

    println!("Result: {} rows (all orders preserved)", result.len());
    for row in &result {
        let customer_name = if row.values[4] == Value::Null {
            "NULL (no customer found)".to_string()
        } else {
            row.values[4].to_string()
        };
        println!("  Order {} - Customer: {}", row.values[0], customer_name);
    }
}

fn demo_right_outer_join() {
    println!("\n\n➡️  Demo 3: RIGHT OUTER JOIN");
    println!("----------------------------\n");

    let products = vec![
        Row::new(
            vec!["id".to_string(), "name".to_string()],
            vec![Value::Integer(1), Value::Text("Widget".to_string())],
        ),
        Row::new(
            vec!["id".to_string(), "name".to_string()],
            vec![Value::Integer(2), Value::Text("Gadget".to_string())],
        ),
        Row::new(
            vec!["id".to_string(), "name".to_string()],
            vec![Value::Integer(3), Value::Text("Doohickey".to_string())],
        ),
    ];

    let sales = vec![
        Row::new(
            vec!["product_id".to_string(), "quantity".to_string()],
            vec![Value::Integer(1), Value::Integer(50)],
        ),
        Row::new(
            vec!["product_id".to_string(), "quantity".to_string()],
            vec![Value::Integer(3), Value::Integer(30)],
        ),
    ];

    let executor = JoinExecutor::new(
        JoinType::RightOuter,
        JoinAlgorithm::NestedLoop,
        JoinCondition::On {
            left_column: "id".to_string(),
            right_column: "product_id".to_string(),
        },
    );

    let result = executor.execute(products, sales).unwrap();

    println!("Result: {} rows (all sales preserved)", result.len());
    for row in &result {
        println!("  Product: {} - Sold: {}", row.values[1], row.values[3]);
    }
    println!("\n  Note: Gadget has no sales (not in right table)");
}

fn demo_full_outer_join() {
    println!("\n\n🔄 Demo 4: FULL OUTER JOIN");
    println!("--------------------------\n");

    let table_a = vec![
        Row::new(
            vec!["id".to_string(), "value_a".to_string()],
            vec![Value::Integer(1), Value::Text("A1".to_string())],
        ),
        Row::new(
            vec!["id".to_string(), "value_a".to_string()],
            vec![Value::Integer(2), Value::Text("A2".to_string())],
        ),
    ];

    let table_b = vec![
        Row::new(
            vec!["id".to_string(), "value_b".to_string()],
            vec![Value::Integer(2), Value::Text("B2".to_string())],
        ),
        Row::new(
            vec!["id".to_string(), "value_b".to_string()],
            vec![Value::Integer(3), Value::Text("B3".to_string())],
        ),
    ];

    let executor = JoinExecutor::new(
        JoinType::FullOuter,
        JoinAlgorithm::NestedLoop,
        JoinCondition::On {
            left_column: "id".to_string(),
            right_column: "id".to_string(),
        },
    );

    let result = executor.execute(table_a, table_b).unwrap();

    println!("Result: {} rows (all rows from both tables)", result.len());
    for row in &result {
        let id = if row.values[0] != Value::Null {
            row.values[0].to_string()
        } else {
            row.values[2].to_string()
        };
        let val_a = &row.values[1];
        let val_b = &row.values[3];
        println!("  ID {} - A: {:?}, B: {:?}", id, val_a, val_b);
    }
}

fn demo_cross_join() {
    println!("\n\n✖️  Demo 5: CROSS JOIN (Cartesian Product)");
    println!("------------------------------------------\n");

    let colors = vec![
        Row::new(
            vec!["color".to_string()],
            vec![Value::Text("Red".to_string())],
        ),
        Row::new(
            vec!["color".to_string()],
            vec![Value::Text("Blue".to_string())],
        ),
    ];

    let sizes = vec![
        Row::new(
            vec!["size".to_string()],
            vec![Value::Text("Small".to_string())],
        ),
        Row::new(
            vec!["size".to_string()],
            vec![Value::Text("Medium".to_string())],
        ),
        Row::new(
            vec!["size".to_string()],
            vec![Value::Text("Large".to_string())],
        ),
    ];

    let executor = JoinExecutor::new(
        JoinType::Cross,
        JoinAlgorithm::NestedLoop,
        JoinCondition::None,
    );

    let result = executor.execute(colors, sizes).unwrap();

    println!("Result: {} rows (2 colors × 3 sizes)", result.len());
    for row in &result {
        println!("  {} - {}", row.values[0], row.values[1]);
    }
}

fn demo_natural_join() {
    println!("\n\n🌿 Demo 6: NATURAL JOIN");
    println!("-----------------------\n");

    let students = vec![
        Row::new(
            vec!["id".to_string(), "name".to_string(), "class_id".to_string()],
            vec![
                Value::Integer(1),
                Value::Text("Alice".to_string()),
                Value::Integer(101),
            ],
        ),
        Row::new(
            vec!["id".to_string(), "name".to_string(), "class_id".to_string()],
            vec![
                Value::Integer(2),
                Value::Text("Bob".to_string()),
                Value::Integer(102),
            ],
        ),
    ];

    let classes = vec![
        Row::new(
            vec!["class_id".to_string(), "subject".to_string()],
            vec![Value::Integer(101), Value::Text("Math".to_string())],
        ),
        Row::new(
            vec!["class_id".to_string(), "subject".to_string()],
            vec![Value::Integer(102), Value::Text("Science".to_string())],
        ),
    ];

    let executor = JoinExecutor::new(
        JoinType::Natural,
        JoinAlgorithm::NestedLoop,
        JoinCondition::Natural,
    );

    let result = executor.execute(students, classes).unwrap();

    println!("Common columns: class_id");
    println!("Result: {} rows", result.len());
    for row in &result {
        println!(
            "  {} - {} (class_id: {})",
            row.values[1], row.values[3], row.values[2]
        );
    }
    println!("\n  Note: class_id appears only once in result");
}

fn demo_join_algorithms() {
    println!("\n\n⚡ Demo 7: Join Algorithm Comparison");
    println!("-------------------------------------\n");

    let left = (1..=100)
        .map(|i| Row::new(vec!["id".to_string()], vec![Value::Integer(i)]))
        .collect::<Vec<_>>();

    let right = (50..=150)
        .map(|i| Row::new(vec!["id".to_string()], vec![Value::Integer(i)]))
        .collect::<Vec<_>>();

    println!("Left table: 100 rows");
    println!("Right table: 101 rows");
    println!("Expected matches: 51 rows (50-100)\n");

    // Hash Join
    let hash_executor = JoinExecutor::new(
        JoinType::Inner,
        JoinAlgorithm::Hash,
        JoinCondition::On {
            left_column: "id".to_string(),
            right_column: "id".to_string(),
        },
    );
    let hash_result = hash_executor.execute(left.clone(), right.clone()).unwrap();
    println!("✓ Hash Join: {} matches", hash_result.len());

    // Merge Join
    let merge_executor = JoinExecutor::new(
        JoinType::Inner,
        JoinAlgorithm::Merge,
        JoinCondition::On {
            left_column: "id".to_string(),
            right_column: "id".to_string(),
        },
    );
    let merge_result = merge_executor.execute(left.clone(), right.clone()).unwrap();
    println!("✓ Merge Join: {} matches", merge_result.len());

    // Nested Loop Join
    let nested_executor = JoinExecutor::new(
        JoinType::Inner,
        JoinAlgorithm::NestedLoop,
        JoinCondition::On {
            left_column: "id".to_string(),
            right_column: "id".to_string(),
        },
    );
    let nested_result = nested_executor
        .execute(left.clone(), right.clone())
        .unwrap();
    println!("✓ Nested Loop: {} matches", nested_result.len());

    println!("\nAlgorithm Selection:");
    println!("  • Hash Join: Best for large datasets (O(n+m))");
    println!("  • Merge Join: Best when data is pre-sorted (O(n+m))");
    println!("  • Nested Loop: Best for small datasets (O(n*m))");

    let recommended = JoinExecutor::choose_algorithm(left.len(), right.len());
    println!("\n  Recommended for this data: {:?}", recommended);
}

fn demo_subqueries() {
    println!("\n\n🔍 Demo 8: Subqueries");
    println!("---------------------\n");

    // Scalar subquery
    let max_result = vec![Row::new(
        vec!["max_salary".to_string()],
        vec![Value::Integer(120000)],
    )];
    let scalar = SubqueryExecutor::execute_scalar(max_result).unwrap();
    println!("Scalar subquery (SELECT MAX(salary)):");
    println!("  Result: {:?}\n", scalar);

    // EXISTS subquery
    let exists_result = vec![Row::new(vec!["id".to_string()], vec![Value::Integer(1)])];
    let exists = SubqueryExecutor::execute_exists(exists_result);
    println!("EXISTS subquery (WHERE EXISTS ...):");
    println!("  Result: {}\n", exists);

    let not_exists = SubqueryExecutor::execute_exists(Vec::new());
    println!("NOT EXISTS (empty subquery):");
    println!("  Result: {}\n", not_exists);

    // IN subquery
    let in_result = vec![
        Row::new(vec!["dept_id".to_string()], vec![Value::Integer(10)]),
        Row::new(vec!["dept_id".to_string()], vec![Value::Integer(20)]),
        Row::new(vec!["dept_id".to_string()], vec![Value::Integer(30)]),
    ];
    let in_check = SubqueryExecutor::execute_in(&Value::Integer(20), in_result.clone());
    println!("IN subquery (WHERE dept_id IN ...):");
    println!("  20 IN (10, 20, 30): {}", in_check);

    let not_in_check = SubqueryExecutor::execute_not_in(&Value::Integer(99), in_result);
    println!("  99 NOT IN (10, 20, 30): {}", not_in_check);
}
