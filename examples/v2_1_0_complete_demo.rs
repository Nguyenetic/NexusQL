// NexusQL v2.1.0 - Complete Production Features Demo
// Advanced SQL, CLI tooling, and performance profiling

use nexusql::advanced::{CorrelatedSubquery, LateralJoin, SubqueryOps};
use nexusql::cli::NexusCLI;
use nexusql::profiler::PerformanceProfiler;
use nexusql::types::{Row, Value};
use std::time::Duration;

fn main() {
    println!("🎯 NexusQL v2.1.0 - Complete Production Features");
    println!("===============================================\n");

    demo_lateral_joins();
    demo_correlated_subqueries();
    demo_all_any_operators();
    demo_cli_tool();
    demo_performance_profiling();

    println!("\n✅ v2.1.0 - Production Complete with Advanced Features!");
}

fn demo_lateral_joins() {
    println!("🔗 Demo 1: LATERAL Joins");
    println!("------------------------\n");

    let manufacturers = vec![
        Row::new(
            vec!["id".to_string(), "name".to_string()],
            vec![Value::Integer(1), Value::Text("ACME Corp".to_string())],
        ),
        Row::new(
            vec!["id".to_string(), "name".to_string()],
            vec![Value::Integer(2), Value::Text("TechCo".to_string())],
        ),
    ];

    println!("Query: SELECT m.name, p.product FROM manufacturers m,");
    println!("       LATERAL get_products(m.id) AS p\n");

    let result = LateralJoin::execute_lateral(manufacturers, |manufacturer| {
        // LATERAL function references outer row (manufacturer)
        let manuf_id = match manufacturer.values[0] {
            Value::Integer(id) => id,
            _ => 0,
        };

        // Return products for this manufacturer
        Ok(vec![
            Row::new(
                vec!["product".to_string()],
                vec![Value::Text(format!("Product_{}_A", manuf_id))],
            ),
            Row::new(
                vec!["product".to_string()],
                vec![Value::Text(format!("Product_{}_B", manuf_id))],
            ),
        ])
    })
    .unwrap();

    println!("Results ({} rows):", result.len());
    for row in result.iter().take(4) {
        println!("  {} - {}", row.values[1], row.values[2]);
    }

    println!("\n  💡 LATERAL allows subqueries to reference outer rows!");
    println!("  💡 Perfect for row-by-row function calls!");
}

fn demo_correlated_subqueries() {
    println!("\n\n📊 Demo 2: Correlated Subqueries");
    println!("--------------------------------\n");

    let employees = vec![
        Row::new(
            vec![
                "id".to_string(),
                "dept_id".to_string(),
                "salary".to_string(),
            ],
            vec![Value::Integer(1), Value::Integer(10), Value::Integer(50000)],
        ),
        Row::new(
            vec![
                "id".to_string(),
                "dept_id".to_string(),
                "salary".to_string(),
            ],
            vec![Value::Integer(2), Value::Integer(10), Value::Integer(60000)],
        ),
        Row::new(
            vec![
                "id".to_string(),
                "dept_id".to_string(),
                "salary".to_string(),
            ],
            vec![Value::Integer(3), Value::Integer(20), Value::Integer(55000)],
        ),
    ];

    println!("Query: SELECT *, (SELECT AVG(salary) FROM employees e2");
    println!("                   WHERE e2.dept_id = e1.dept_id) AS dept_avg");
    println!("       FROM employees e1\n");

    let result = CorrelatedSubquery::execute(employees, |outer_row| {
        // Correlated: references outer row's dept_id
        let dept_id = &outer_row.values[1];

        // Simulated: calculate avg salary for this department
        let avg = match dept_id {
            Value::Integer(10) => 55000,
            Value::Integer(20) => 55000,
            _ => 50000,
        };

        Ok(Value::Integer(avg))
    })
    .unwrap();

    println!("Results:");
    for row in &result {
        println!(
            "  Employee {} in dept {}: salary={}, dept_avg={}",
            row.values[0], row.values[1], row.values[2], row.values[3]
        );
    }

    println!("\n  💡 Subquery executes once per outer row!");
    println!("  💡 Can reference outer row columns!");
}

fn demo_all_any_operators() {
    println!("\n\n⚖️  Demo 3: ALL/ANY/SOME Operators");
    println!("----------------------------------\n");

    let subquery_results = vec![
        Value::Integer(100),
        Value::Integer(200),
        Value::Integer(300),
    ];

    println!("Subquery returns: [100, 200, 300]\n");

    // ALL operator
    println!("Testing: value > ALL (subquery)");
    println!(
        "  400 > ALL: {}",
        SubqueryOps::all_greater(&Value::Integer(400), &subquery_results)
    );
    println!(
        "  250 > ALL: {}",
        SubqueryOps::all_greater(&Value::Integer(250), &subquery_results)
    );
    println!(
        "  150 > ALL: {}\n",
        SubqueryOps::all_greater(&Value::Integer(150), &subquery_results)
    );

    // ANY operator
    println!("Testing: value = ANY (subquery)");
    println!(
        "  200 = ANY: {}",
        SubqueryOps::any_equals(&Value::Integer(200), &subquery_results)
    );
    println!(
        "  999 = ANY: {}\n",
        SubqueryOps::any_equals(&Value::Integer(999), &subquery_results)
    );

    // SOME operator (alias for ANY)
    println!("Testing: value = SOME (subquery)");
    println!(
        "  100 = SOME: {}",
        SubqueryOps::some_equals(&Value::Integer(100), &subquery_results)
    );

    println!("\n  💡 ALL: True if condition holds for ALL values");
    println!("  💡 ANY/SOME: True if condition holds for ANY value");
}

fn demo_cli_tool() {
    println!("\n\n💻 Demo 4: CLI Tool (Interactive Shell)");
    println!("----------------------------------------\n");

    let cli = NexusCLI::new();

    println!("NexusQL CLI Features:");
    println!("  • Interactive SQL shell (like psql)");
    println!("  • Meta-commands (\\d, \\dt, \\di, \\q)");
    println!("  • Command history (\\h)");
    println!("  • Help system (\\?)");
    println!("\nExample session:");
    println!("  nexusql> SELECT * FROM users;");
    println!("  nexusql> \\d");
    println!("  nexusql> \\dt");
    println!("  nexusql> \\q");

    println!("\n  💡 Launch with: cargo run --bin nexusql");
    println!("  💡 Full psql-compatible interface!");
}

fn demo_performance_profiling() {
    println!("\n\n⚡ Demo 5: Performance Profiling");
    println!("--------------------------------\n");

    let mut profiler = PerformanceProfiler::new(50.0); // 50ms threshold

    println!("Profiling queries...\n");

    // Profile fast query
    profiler
        .profile_query("SELECT id FROM users WHERE id = 1", || {
            std::thread::sleep(Duration::from_millis(10));
            Ok(())
        })
        .unwrap();
    println!("  ✓ Fast query (indexed): ~10ms");

    // Profile slow query
    profiler
        .profile_query("SELECT * FROM large_table", || {
            std::thread::sleep(Duration::from_millis(120));
            Ok(())
        })
        .unwrap();
    println!("  ⚠ SLOW QUERY detected: ~120ms\n");

    // Profile multiple executions
    for _ in 0..5 {
        profiler
            .profile_query("SELECT * FROM products WHERE category = 'books'", || {
                std::thread::sleep(Duration::from_millis(30));
                Ok(())
            })
            .unwrap();
    }
    println!("  ✓ Profiled 5 executions of same query\n");

    // Analyze performance
    if let Some(analysis) = profiler.analyze("SELECT * FROM products WHERE category = 'books'") {
        println!("Performance Analysis:");
        println!("  Query: {}", analysis.query);
        println!("  Executions: {}", analysis.execution_count);
        println!("  Avg time: {:.2} ms", analysis.avg_time_ms);
        println!("  Min time: {:.2} ms", analysis.min_time_ms);
        println!("  Max time: {:.2} ms\n", analysis.max_time_ms);

        if !analysis.suggestions.is_empty() {
            println!("  Optimization Suggestions:");
            for suggestion in &analysis.suggestions {
                println!("    • {}", suggestion);
            }
        }
    }

    let stats = profiler.stats();
    println!("\nProfiler Statistics:");
    println!("  Total queries: {}", stats.total_queries);
    println!("  Unique queries: {}", stats.unique_queries);
    println!("  Slow queries: {}", stats.slow_queries);

    println!("\n  💡 Automatic slow query detection!");
    println!("  💡 Performance suggestions for optimization!");
    println!("  💡 Built-in query analyzer!");
}
