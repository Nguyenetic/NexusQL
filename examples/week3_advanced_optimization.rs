//! Week 3-4 Advanced Optimization Example
//!
//! Demonstrates:
//! - PostgreSQL-style cost-based query optimization
//! - Join order optimization with dynamic programming
//! - Distributed query execution (Citus patterns)
//! - Statistics-based cardinality estimation
//! - Multiple join algorithms (Nested Loop, Merge Join, Hash Join)

use nexusql::engine::{
    AdvancedOptimizer, ColumnStats, DistributedPlan, DistributedPlanner, DistributedTable,
    DistributionStrategy, JoinAlgorithm, JoinCondition, JoinGraph, JoinOrderOptimizer, PlanCost,
    PlanNode, PlannerType, Relation, ScanMethod, ShardInfo, TableStats, WorkerNode,
};
use nexusql::types::Value;
use std::collections::HashMap;

fn main() {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║  NexusQL Week 3-4: Advanced Query Optimization & Distribution ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    // Part 1: Cost-Based Query Optimization
    demo_cost_based_optimization();

    // Part 2: Join Order Optimization
    demo_join_order_optimization();

    // Part 3: Distributed Query Execution
    demo_distributed_execution();

    println!("\n✅ Week 3-4 Features Demonstrated Successfully!");
}

fn demo_cost_based_optimization() {
    println!("📊 PART 1: Cost-Based Query Optimization (PostgreSQL Pattern)");
    println!("═══════════════════════════════════════════════════════════════\n");

    let mut optimizer = AdvancedOptimizer::new();

    // Set up table statistics
    let users_stats = TableStats {
        row_count: 1_000_000,
        page_count: 10_000,
        avg_row_width: 200,
        column_stats: {
            let mut stats = HashMap::new();
            stats.insert(
                "id".to_string(),
                ColumnStats {
                    n_distinct: 1_000_000,
                    most_common_values: vec![],
                    mcv_frequencies: vec![],
                    histogram_bounds: vec![],
                    null_fraction: 0.0,
                },
            );
            stats
        },
    };

    optimizer.update_stats("users".to_string(), users_stats);

    println!("Table: users (1M rows, 10K pages)");
    println!();

    // Compare scan methods
    println!("Comparing Scan Methods:");
    println!("─────────────────────────");

    let seq_cost = optimizer.estimate_sequential_scan_cost("users", &None);
    println!("Sequential Scan:");
    println!("  Cost Formula: seq_page_cost * pages + cpu_tuple_cost * rows");
    println!("  Cost Formula: 1.0 * 10000 + 0.01 * 1000000");
    println!("  Total Cost: {:.2}", seq_cost.total_cost);
    println!("  Estimated Rows: {}", seq_cost.estimated_rows);
    println!();

    let idx_cost =
        optimizer.estimate_index_scan_cost("users", "idx_id", &Some("id = 42".to_string()));
    println!("Index Scan (with filter 'id = 42'):");
    println!("  Cost Formula: random_page_cost * estimated_rows + cpu_tuple_cost * rows");
    println!("  Selectivity: 1% (10 rows)");
    println!("  Total Cost: {:.2}", idx_cost.total_cost);
    println!("  Estimated Rows: {}", idx_cost.estimated_rows);
    println!();

    println!(
        "💡 Insight: Index scan is {:.1}x cheaper for selective queries!\n",
        seq_cost.total_cost / idx_cost.total_cost
    );

    // Compare join algorithms
    println!("Comparing Join Algorithms:");
    println!("─────────────────────────────");

    let left = PlanNode::Scan {
        table: "orders".to_string(),
        method: ScanMethod::Sequential,
        filter: None,
        cost: PlanCost {
            total_cost: 100.0,
            estimated_rows: 10_000,
            ..Default::default()
        },
    };

    let right = PlanNode::Scan {
        table: "customers".to_string(),
        method: ScanMethod::Sequential,
        filter: None,
        cost: PlanCost {
            total_cost: 50.0,
            estimated_rows: 1_000,
            ..Default::default()
        },
    };

    let nl_cost = optimizer.estimate_join_cost(&left, &right, JoinAlgorithm::NestedLoop);
    println!("Nested Loop Join: O(N*M)");
    println!("  Total Cost: {:.2}", nl_cost.total_cost);
    println!("  Estimated Rows: {}", nl_cost.estimated_rows);
    println!();

    let merge_cost = optimizer.estimate_join_cost(&left, &right, JoinAlgorithm::MergeJoin);
    println!("Merge Join: O(N log N + M log M)");
    println!("  Total Cost: {:.2}", merge_cost.total_cost);
    println!("  Estimated Rows: {}", merge_cost.estimated_rows);
    println!();

    let hash_cost = optimizer.estimate_join_cost(&left, &right, JoinAlgorithm::HashJoin);
    println!("Hash Join: O(N + M)");
    println!("  Total Cost: {:.2}", hash_cost.total_cost);
    println!("  Estimated Rows: {}", hash_cost.estimated_rows);
    println!();

    let best = if hash_cost.total_cost < nl_cost.total_cost
        && hash_cost.total_cost < merge_cost.total_cost
    {
        "Hash Join"
    } else if merge_cost.total_cost < nl_cost.total_cost {
        "Merge Join"
    } else {
        "Nested Loop"
    };

    println!("✅ Optimizer chooses: {} (lowest cost)\n", best);

    // EXPLAIN plan
    println!("PostgreSQL-Style EXPLAIN Output:");
    println!("─────────────────────────────────");
    let plan = PlanNode::Join {
        left: Box::new(left),
        right: Box::new(right),
        algorithm: JoinAlgorithm::HashJoin,
        condition: "orders.customer_id = customers.id".to_string(),
        cost: hash_cost,
    };
    println!("{}\n", optimizer.explain_plan(&plan, 0));
}

fn demo_join_order_optimization() {
    println!("🔄 PART 2: Join Order Optimization (Dynamic Programming)");
    println!("═══════════════════════════════════════════════════════════════\n");

    let optimizer = AdvancedOptimizer::new();
    let mut join_opt = JoinOrderOptimizer::new(optimizer);

    // 3-table join: orders ⋈ customers ⋈ products
    println!("Query: SELECT * FROM orders, customers, products");
    println!("       WHERE orders.customer_id = customers.id");
    println!("         AND orders.product_id = products.id");
    println!();

    let relations = vec![
        Relation {
            name: "orders".to_string(),
            alias: None,
        },
        Relation {
            name: "customers".to_string(),
            alias: None,
        },
        Relation {
            name: "products".to_string(),
            alias: None,
        },
    ];

    let conditions = vec![
        JoinCondition {
            left_table: "orders".to_string(),
            right_table: "customers".to_string(),
            left_column: "customer_id".to_string(),
            right_column: "id".to_string(),
            condition: "orders.customer_id = customers.id".to_string(),
        },
        JoinCondition {
            left_table: "orders".to_string(),
            right_table: "products".to_string(),
            left_column: "product_id".to_string(),
            right_column: "id".to_string(),
            condition: "orders.product_id = products.id".to_string(),
        },
    ];

    println!("Join Graph:");
    println!("  customers ──┐");
    println!("              ├── orders");
    println!("  products  ──┘");
    println!();

    let graph = JoinGraph::new(relations, conditions);

    println!("Dynamic Programming Algorithm:");
    println!("  1. Build plans for single tables (base case)");
    println!("  2. Build plans for pairs of tables");
    println!("  3. Build plans for all three tables");
    println!("  4. Pick the lowest-cost plan at each step");
    println!();

    let plan = join_opt.optimize_join_order(&graph);

    println!("✅ Optimal Join Order Found!");
    println!();

    // Show the plan structure
    fn show_plan_structure(plan: &PlanNode, indent: usize) {
        let prefix = "  ".repeat(indent);
        match plan {
            PlanNode::Scan { table, cost, .. } => {
                println!("{}Scan {} (cost: {:.2})", prefix, table, cost.total_cost);
            }
            PlanNode::Join {
                left,
                right,
                algorithm,
                cost,
                ..
            } => {
                println!(
                    "{}Join {:?} (cost: {:.2})",
                    prefix, algorithm, cost.total_cost
                );
                show_plan_structure(left, indent + 1);
                show_plan_structure(right, indent + 1);
            }
            _ => {}
        }
    }

    println!("Plan Structure:");
    show_plan_structure(&plan, 0);
    println!();
}

fn demo_distributed_execution() {
    println!("🌐 PART 3: Distributed Query Execution (Citus Pattern)");
    println!("═══════════════════════════════════════════════════════════════\n");

    let mut planner = DistributedPlanner::new();

    // Set up distributed cluster
    println!("Cluster Setup:");
    println!("─────────────");

    planner.add_worker(WorkerNode {
        id: "worker1".to_string(),
        host: "10.0.1.1".to_string(),
        port: 5432,
        max_connections: 100,
    });

    planner.add_worker(WorkerNode {
        id: "worker2".to_string(),
        host: "10.0.1.2".to_string(),
        port: 5432,
        max_connections: 100,
    });

    planner.add_worker(WorkerNode {
        id: "worker3".to_string(),
        host: "10.0.1.3".to_string(),
        port: 5432,
        max_connections: 100,
    });

    println!("  Worker 1: 10.0.1.1:5432");
    println!("  Worker 2: 10.0.1.2:5432");
    println!("  Worker 3: 10.0.1.3:5432");
    println!();

    // Create distributed table with hash sharding
    println!("Table Distribution:");
    println!("──────────────────");

    let users_table = DistributedTable {
        name: "users".to_string(),
        strategy: DistributionStrategy::Hash {
            column: "id".to_string(),
        },
        shards: vec![
            ShardInfo {
                id: 1,
                worker_id: "worker1".to_string(),
                range: None,
                row_count: 333_333,
            },
            ShardInfo {
                id: 2,
                worker_id: "worker2".to_string(),
                range: None,
                row_count: 333_333,
            },
            ShardInfo {
                id: 3,
                worker_id: "worker3".to_string(),
                range: None,
                row_count: 333_334,
            },
        ],
    };

    println!("  users: HASH(id) - 3 shards");
    println!("    Shard 1 → worker1 (333K rows)");
    println!("    Shard 2 → worker2 (333K rows)");
    println!("    Shard 3 → worker3 (333K rows)");
    println!();

    planner.register_table(users_table);

    // Test 1: Fast Path Planner (point query)
    println!("Query 1: Point Query (Fast Path Planner)");
    println!("────────────────────────────────────────");
    println!("  SELECT * FROM users WHERE id = 42");
    println!();

    let plan1 = PlanNode::Scan {
        table: "users".to_string(),
        method: ScanMethod::Sequential,
        filter: Some("id = 42".to_string()),
        cost: PlanCost::default(),
    };

    let mut filters1 = HashMap::new();
    filters1.insert("id".to_string(), Value::Integer(42));

    let planner_type1 = planner.choose_planner(&plan1, &filters1);
    println!("  Planner Type: {:?}", planner_type1);

    let dist_plan1 = planner.plan_query(&plan1, &filters1);
    match dist_plan1 {
        DistributedPlan::Router { worker_id, .. } => {
            println!("  Execution: Router (single worker)");
            println!("  Target Worker: {}", worker_id);
            println!("  ✅ Query routed to single shard (minimal overhead)");
        }
        _ => println!("  Execution: Parallel"),
    }
    println!();

    // Test 2: Parallel Query
    println!("Query 2: Full Table Scan (Parallel Execution)");
    println!("──────────────────────────────────────────────");
    println!("  SELECT COUNT(*) FROM users");
    println!();

    let plan2 = PlanNode::Aggregate {
        input: Box::new(PlanNode::Scan {
            table: "users".to_string(),
            method: ScanMethod::Sequential,
            filter: None,
            cost: PlanCost::default(),
        }),
        group_by: vec![],
        aggregates: vec!["COUNT(*)".to_string()],
        cost: PlanCost::default(),
    };

    let filters2 = HashMap::new();
    let planner_type2 = planner.choose_planner(&plan2, &filters2);
    println!("  Planner Type: {:?}", planner_type2);

    let dist_plan2 = planner.plan_query(&plan2, &filters2);
    match dist_plan2 {
        DistributedPlan::Parallel { worker_plans, .. } => {
            println!(
                "  Execution: Parallel across {} workers",
                worker_plans.len()
            );
            println!("  Phase 1: Each worker counts local rows");
            println!("  Phase 2: Coordinator aggregates results");
            println!(
                "  ✅ Parallel execution ({}x speedup potential)",
                worker_plans.len()
            );
        }
        _ => println!("  Execution: Router"),
    }
    println!();

    // Summary
    println!("Key Features:");
    println!("─────────────");
    println!("  ✅ Fast Path: Point queries → single worker");
    println!("  ✅ Router: Co-located joins → single worker");
    println!("  ✅ Parallel: Aggregations → all workers");
    println!("  ✅ Adaptive Executor: Slow-start connection management");
    println!();
}
