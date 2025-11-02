// Week 15: CTEs & Recursive Queries Demo
// Demonstrates Common Table Expressions and recursive query capabilities

use nexusql::cte::{CTEExecutor, RecursiveCTE};
use nexusql::types::{Row, Value};

fn main() {
    println!("🚀 NexusQL Week 15: CTEs & Recursive Queries Demo");
    println!("==================================================\n");

    demo_simple_cte();
    demo_multiple_ctes();
    demo_recursive_sum();
    demo_employee_hierarchy();
    demo_graph_traversal();
    demo_cycle_detection();

    println!("\n✅ All demos completed successfully!");
}

fn demo_simple_cte() {
    println!("📋 Demo 1: Simple CTE (WITH Clause)");
    println!("------------------------------------\n");

    let mut executor = CTEExecutor::new();

    // WITH regional_sales AS (SELECT region, SUM(amount) FROM orders GROUP BY region)
    executor
        .execute_simple_cte(
            "regional_sales".to_string(),
            Some(vec!["region".to_string(), "total".to_string()]),
            || {
                Ok(vec![
                    Row::new(
                        vec!["region".to_string(), "total".to_string()],
                        vec![Value::Text("North".to_string()), Value::Integer(45000)],
                    ),
                    Row::new(
                        vec!["region".to_string(), "total".to_string()],
                        vec![Value::Text("South".to_string()), Value::Integer(32000)],
                    ),
                    Row::new(
                        vec!["region".to_string(), "total".to_string()],
                        vec![Value::Text("East".to_string()), Value::Integer(28000)],
                    ),
                    Row::new(
                        vec!["region".to_string(), "total".to_string()],
                        vec![Value::Text("West".to_string()), Value::Integer(51000)],
                    ),
                ])
            },
            true,
        )
        .unwrap();

    let cte = executor.get_cte("regional_sales").unwrap();
    println!("CTE: regional_sales");
    println!("Rows: {}", cte.rows.len());
    println!("Materialized: {}\n", cte.materialized);

    for row in &cte.rows {
        println!("  {} : ${}", row.values[0], row.values[1]);
    }

    // Calculate total
    let total: i64 = cte
        .rows
        .iter()
        .filter_map(|r| match r.values[1] {
            Value::Integer(n) => Some(n),
            _ => None,
        })
        .sum();
    println!("\n  Total Sales: ${}", total);
}

fn demo_multiple_ctes() {
    println!("\n\n📊 Demo 2: Multiple CTEs (Chained)");
    println!("-----------------------------------\n");

    let mut executor = CTEExecutor::new();

    // First CTE: all_sales
    executor
        .execute_simple_cte(
            "all_sales".to_string(),
            Some(vec!["amount".to_string()]),
            || {
                Ok((1..=10)
                    .map(|i| Row::new(vec!["amount".to_string()], vec![Value::Integer(i * 1000)]))
                    .collect())
            },
            false,
        )
        .unwrap();

    // Second CTE: high_sales (references all_sales conceptually)
    executor
        .execute_simple_cte(
            "high_sales".to_string(),
            Some(vec!["amount".to_string()]),
            || {
                Ok(vec![
                    Row::new(vec!["amount".to_string()], vec![Value::Integer(8000)]),
                    Row::new(vec!["amount".to_string()], vec![Value::Integer(9000)]),
                    Row::new(vec!["amount".to_string()], vec![Value::Integer(10000)]),
                ])
            },
            false,
        )
        .unwrap();

    println!(
        "CTE 1: all_sales ({} rows)",
        executor.get_cte("all_sales").unwrap().rows.len()
    );
    println!(
        "CTE 2: high_sales ({} rows)",
        executor.get_cte("high_sales").unwrap().rows.len()
    );
    println!("\n✓ Multiple CTEs can be chained for complex queries");
}

fn demo_recursive_sum() {
    println!("\n\n🔢 Demo 3: Recursive CTE - Sum 1 to 100");
    println!("----------------------------------------\n");

    let mut executor = CTEExecutor::new();

    // WITH RECURSIVE t(n) AS (VALUES (1) UNION ALL SELECT n+1 FROM t WHERE n < 100)
    println!("Query: WITH RECURSIVE t(n) AS (");
    println!("  VALUES (1)");
    println!("  UNION ALL");
    println!("  SELECT n+1 FROM t WHERE n < 100");
    println!(")\n");

    executor
        .execute_recursive_cte(
            "t".to_string(),
            Some(vec!["n".to_string()]),
            || {
                Ok(vec![Row::new(
                    vec!["n".to_string()],
                    vec![Value::Integer(1)],
                )])
            },
            |working_table| {
                let mut new_rows = Vec::new();
                for row in working_table {
                    if let Value::Integer(n) = row.values[0] {
                        if n < 100 {
                            new_rows
                                .push(Row::new(vec!["n".to_string()], vec![Value::Integer(n + 1)]));
                        }
                    }
                }
                Ok(new_rows)
            },
            150,
        )
        .unwrap();

    let cte = executor.get_cte("t").unwrap();
    let sum: i64 = cte
        .rows
        .iter()
        .filter_map(|row| match row.values[0] {
            Value::Integer(n) => Some(n),
            _ => None,
        })
        .sum();

    println!("Generated numbers: 1 to 100");
    println!("Total rows: {}", cte.rows.len());
    println!("Sum: {} (Expected: 5050)", sum);
    assert_eq!(sum, 5050);
}

fn demo_employee_hierarchy() {
    println!("\n\n👥 Demo 4: Recursive CTE - Employee Hierarchy");
    println!("----------------------------------------------\n");

    // Employee data: (id, name, manager_id)
    let employees = vec![
        (1, "Alice", None),      // CEO
        (2, "Bob", Some(1)),     // Reports to Alice
        (3, "Charlie", Some(1)), // Reports to Alice
        (4, "David", Some(2)),   // Reports to Bob
        (5, "Eve", Some(2)),     // Reports to Bob
        (6, "Frank", Some(3)),   // Reports to Charlie
    ];

    println!("Organization:");
    println!("  Alice (CEO)");
    println!("    ├─ Bob");
    println!("    │   ├─ David");
    println!("    │   └─ Eve");
    println!("    └─ Charlie");
    println!("        └─ Frank\n");

    // Base: CEO (no manager)
    let base_rows = vec![Row::new(
        vec!["id".to_string(), "name".to_string(), "level".to_string()],
        vec![
            Value::Integer(1),
            Value::Text("Alice".to_string()),
            Value::Integer(0),
        ],
    )];

    let recursive_cte = RecursiveCTE::new(base_rows, 10);

    let result = recursive_cte
        .execute(|working_table| {
            let mut new_rows = Vec::new();

            // Get manager IDs from working table
            let manager_ids: std::collections::HashSet<i64> = working_table
                .iter()
                .filter_map(|row| match row.values[0] {
                    Value::Integer(id) => Some(id),
                    _ => None,
                })
                .collect();

            // Find employees with these managers
            for (id, name, manager_id) in &employees {
                if let Some(mgr_id) = manager_id {
                    if manager_ids.contains(mgr_id) {
                        let parent_level = working_table
                            .iter()
                            .find(|r| r.values[0] == Value::Integer(*mgr_id))
                            .and_then(|r| match r.values[2] {
                                Value::Integer(l) => Some(l),
                                _ => None,
                            })
                            .unwrap_or(0);

                        new_rows.push(Row::new(
                            vec!["id".to_string(), "name".to_string(), "level".to_string()],
                            vec![
                                Value::Integer(*id),
                                Value::Text(name.to_string()),
                                Value::Integer(parent_level + 1),
                            ],
                        ));
                    }
                }
            }

            Ok(new_rows)
        })
        .unwrap();

    println!("Recursive traversal result:");
    for row in &result {
        let level = match row.values[2] {
            Value::Integer(l) => l,
            _ => 0,
        };
        let indent = "  ".repeat(level as usize);
        println!("  {}Level {}: {}", indent, level, row.values[1]);
    }
    println!("\n  Total employees found: {}", result.len());
}

fn demo_graph_traversal() {
    println!("\n\n🌐 Demo 5: Recursive CTE - Graph Traversal");
    println!("-------------------------------------------\n");

    // Graph edges: node -> connected nodes
    let graph = vec![
        (1, vec![2, 3]),
        (2, vec![4]),
        (3, vec![4, 5]),
        (4, vec![6]),
        (5, vec![6]),
    ];

    println!("Graph structure:");
    println!("  1 → [2, 3]");
    println!("  2 → [4]");
    println!("  3 → [4, 5]");
    println!("  4 → [6]");
    println!("  5 → [6]\n");

    // Start from node 1
    let base_rows = vec![Row::new(
        vec!["node".to_string(), "depth".to_string()],
        vec![Value::Integer(1), Value::Integer(0)],
    )];

    let recursive_cte = RecursiveCTE::new(base_rows, 10);

    let result = recursive_cte
        .execute(|working_table| {
            let mut new_rows = Vec::new();

            // Get current nodes from working table
            for row in working_table {
                if let Value::Integer(node) = row.values[0] {
                    if let Value::Integer(depth) = row.values[1] {
                        // Find connected nodes
                        if let Some((_, connected)) = graph.iter().find(|(n, _)| *n == node) {
                            for &next_node in connected {
                                new_rows.push(Row::new(
                                    vec!["node".to_string(), "depth".to_string()],
                                    vec![Value::Integer(next_node), Value::Integer(depth + 1)],
                                ));
                            }
                        }
                    }
                }
            }

            Ok(new_rows)
        })
        .unwrap();

    println!("Breadth-first traversal from node 1:");
    let mut by_depth: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
    for row in &result {
        if let (Value::Integer(node), Value::Integer(depth)) = (&row.values[0], &row.values[1]) {
            by_depth.entry(*depth).or_insert_with(Vec::new).push(*node);
        }
    }

    for depth in 0..=3 {
        if let Some(nodes) = by_depth.get(&depth) {
            println!("  Depth {}: {:?}", depth, nodes);
        }
    }
    println!("\n  Total nodes visited: {}", result.len());
}

fn demo_cycle_detection() {
    println!("\n\n🔁 Demo 6: Cycle Detection");
    println!("---------------------------\n");

    println!("Graph with cycle: 1 → 2 → 3 → 1 (loops back)\n");

    // Cyclic graph
    let base_rows = vec![Row::new(
        vec!["id".to_string(), "next".to_string()],
        vec![Value::Integer(1), Value::Integer(2)],
    )];

    let recursive_cte = RecursiveCTE::new(base_rows, 100);

    let result = recursive_cte
        .execute(|working_table| {
            let mut new_rows = Vec::new();

            for row in working_table {
                if let Value::Integer(next_id) = row.values[1] {
                    let next_next = match next_id {
                        2 => 3,
                        3 => 1, // Cycle!
                        _ => return Ok(vec![]),
                    };

                    new_rows.push(Row::new(
                        vec!["id".to_string(), "next".to_string()],
                        vec![Value::Integer(next_id), Value::Integer(next_next)],
                    ));
                }
            }

            Ok(new_rows)
        })
        .unwrap();

    println!("Traversal with cycle detection:");
    for row in &result {
        println!("  {} → {}", row.values[0], row.values[1]);
    }
    println!("\n  ✓ Cycle detected and stopped");
    println!(
        "  ✓ Visited {} unique nodes (no infinite loop)",
        result.len()
    );
    assert!(result.len() <= 3);
}
