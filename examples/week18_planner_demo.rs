// Week 18: Query Planner & Statistics Demo
// Demonstrates ANALYZE, statistics collection, cost estimation, and EXPLAIN

use nexusql::planner::{PlanCost, QueryPlan, TableStatistics};
use nexusql::types::{Row, Value};
use std::time::Instant;

fn main() {
    println!("🚀 NexusQL Week 18: Query Planner & Statistics Demo");
    println!("===================================================\n");

    demo_analyze();
    demo_statistics();
    demo_cost_estimation();
    demo_explain();
    demo_explain_analyze();

    println!("\n✅ All demos completed successfully!");
}

fn create_test_data(size: usize) -> Vec<Row> {
    (1..=size)
        .map(|i| {
            Row::new(
                vec!["id".to_string(), "amount".to_string(), "region".to_string()],
                vec![
                    Value::Integer(i as i64),
                    Value::Integer((i * 100) as i64),
                    Value::Text(format!("region_{}", i % 10)),
                ],
            )
        })
        .collect()
}

fn demo_analyze() {
    println!("📊 Demo 1: ANALYZE Command (Statistics Collection)");
    println!("---------------------------------------------------\n");

    let rows = create_test_data(10000);

    println!("Analyzing table with 10,000 rows...\n");
    let start = Instant::now();
    let stats = TableStatistics::analyze("sales".to_string(), &rows).unwrap();
    let elapsed = start.elapsed();

    println!("ANALYZE sales;");
    println!("\nTable Statistics:");
    println!("  Table: {}", stats.table_name);
    println!("  Rows: {}", stats.row_count);
    println!("  Pages: {}", stats.page_count);
    println!("  Avg row size: {} bytes", stats.avg_row_size);
    println!("  Columns analyzed: {}", stats.column_stats.len());
    println!(
        "\n  Analysis completed in {:.2} ms",
        elapsed.as_secs_f64() * 1000.0
    );
}

fn demo_statistics() {
    println!("\n\n📈 Demo 2: Column Statistics");
    println!("-----------------------------\n");

    let rows = create_test_data(1000);
    let stats = TableStatistics::analyze("sales".to_string(), &rows).unwrap();

    // Show statistics for 'region' column
    let region_stats = stats.column_stats.get("region").unwrap();

    println!("Column: region");
    println!("  Null fraction: {:.4}", region_stats.null_frac);
    println!("  Distinct values: {}", region_stats.n_distinct);
    println!(
        "  Most common values (MCVs): {}",
        region_stats.most_common_vals.len()
    );

    println!("\n  Top 5 MCVs:");
    for i in 0..5.min(region_stats.most_common_vals.len()) {
        println!(
            "    {} : {:.2}%",
            region_stats.most_common_vals[i],
            region_stats.most_common_freqs[i] * 100.0
        );
    }

    println!(
        "\n  Histogram buckets: {}",
        region_stats.histogram_bounds.len()
    );
}

fn demo_cost_estimation() {
    println!("\n\n💰 Demo 3: Cost Estimation");
    println!("--------------------------\n");

    let rows = create_test_data(10000);
    let stats = TableStatistics::analyze("orders".to_string(), &rows).unwrap();

    // Sequential scan cost
    let seq_cost = PlanCost::seq_scan(&stats);
    println!("Sequential Scan:");
    println!("  Startup cost: {:.2}", seq_cost.startup_cost);
    println!("  Total cost: {:.2}", seq_cost.total_cost);
    println!("  Estimated rows: {}\n", seq_cost.estimated_rows);

    // Index scan cost (10% selectivity)
    let idx_cost = PlanCost::index_scan(&stats, 0.1);
    println!("Index Scan (10% selectivity):");
    println!("  Startup cost: {:.2}", idx_cost.startup_cost);
    println!("  Total cost: {:.2}", idx_cost.total_cost);
    println!("  Estimated rows: {}\n", idx_cost.estimated_rows);

    // Hash join cost
    let join_cost = PlanCost::hash_join(&seq_cost, &idx_cost);
    println!("Hash Join:");
    println!("  Startup cost: {:.2}", join_cost.startup_cost);
    println!("  Total cost: {:.2}", join_cost.total_cost);
    println!("  Estimated rows: {}", join_cost.estimated_rows);

    println!(
        "\n  💡 Index scan is {:.1}x cheaper than seq scan for selective queries",
        seq_cost.total_cost / idx_cost.total_cost
    );
}

fn demo_explain() {
    println!("\n\n🔍 Demo 4: EXPLAIN (Query Plan Visualization)");
    println!("----------------------------------------------\n");

    let rows = create_test_data(5000);
    let stats = TableStatistics::analyze("products".to_string(), &rows).unwrap();

    // Example 1: Simple sequential scan
    println!("Query: SELECT * FROM products WHERE category = 'electronics'\n");
    let plan1 = QueryPlan::SeqScan {
        table: "products".to_string(),
        filter: Some("category = 'electronics'".to_string()),
        cost: PlanCost::seq_scan(&stats),
    };

    println!("EXPLAIN:");
    println!("{}\n", plan1.explain(0));

    // Example 2: Index scan
    println!("\nQuery: SELECT * FROM products WHERE id < 1000\n");
    let plan2 = QueryPlan::IndexScan {
        table: "products".to_string(),
        index: "products_id_idx".to_string(),
        condition: "id < 1000".to_string(),
        cost: PlanCost::index_scan(&stats, 0.2),
    };

    println!("EXPLAIN:");
    println!("{}", plan2.explain(0));
}

fn demo_explain_analyze() {
    println!("\n\n⚡ Demo 5: EXPLAIN ANALYZE (Actual Execution Stats)");
    println!("---------------------------------------------------\n");

    let rows = create_test_data(1000);
    let stats = TableStatistics::analyze("test_table".to_string(), &rows).unwrap();

    let plan = QueryPlan::SeqScan {
        table: "test_table".to_string(),
        filter: Some("amount > 50000".to_string()),
        cost: PlanCost::seq_scan(&stats),
    };

    println!("Query: SELECT * FROM test_table WHERE amount > 50000\n");
    println!("EXPLAIN ANALYZE:");

    let result = plan
        .explain_analyze(|| {
            let start = Instant::now();

            // Simulate query execution
            let filtered: Vec<Row> = rows
                .iter()
                .filter(|r| {
                    if let Value::Integer(amt) = r.values[1] {
                        amt > 50000
                    } else {
                        false
                    }
                })
                .cloned()
                .collect();

            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            Ok((filtered, elapsed))
        })
        .unwrap();

    println!("{}\n", result);
    println!("  Estimated rows: {} | Actual rows: ~500", stats.row_count);
    println!("  Shows both planner estimates AND actual execution metrics!");
}
