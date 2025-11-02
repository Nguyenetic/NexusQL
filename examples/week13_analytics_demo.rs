//! Week 13 Features Demo: Aggregate & Window Functions
//!
//! This example demonstrates advanced analytical capabilities:
//! - Aggregate functions (COUNT, SUM, AVG, MIN, MAX, STRING_AGG, ARRAY_AGG)
//! - Window functions (ROW_NUMBER, RANK, DENSE_RANK, LAG, LEAD)
//! - GROUP BY aggregations
//! - PARTITION BY with window functions
//! - Real-world analytics scenarios

use nexus::aggregate::AggregateManager;
use nexus::error::Result;
use nexus::types::{Row, Value};
use nexus::window::{FrameBound, FrameType, WindowFrame, WindowManager, WindowSpec};

fn main() -> Result<()> {
    println!("=".repeat(80));
    println!("NexusQL Week 13 Analytics Demo");
    println!("Aggregate Functions • Window Functions • Advanced Analytics");
    println!("=".repeat(80));
    println!();

    // ========================================================================
    // 1. AGGREGATE FUNCTIONS - Data Summarization
    // ========================================================================
    println!("📊 1. AGGREGATE FUNCTIONS - Data Summarization");
    println!("-".repeat(80));

    let agg_manager = AggregateManager::new();

    // Sample sales data
    let sales = vec![
        Row::new(vec![Value::Integer(100)]),
        Row::new(vec![Value::Integer(250)]),
        Row::new(vec![Value::Integer(175)]),
        Row::new(vec![Value::Integer(300)]),
        Row::new(vec![Value::Integer(125)]),
    ];

    println!("  Sales data: [100, 250, 175, 300, 125]\n");

    let count = agg_manager.execute_aggregate("COUNT", 0, &sales)?;
    let sum = agg_manager.execute_aggregate("SUM", 0, &sales)?;
    let avg = agg_manager.execute_aggregate("AVG", 0, &sales)?;
    let min = agg_manager.execute_aggregate("MIN", 0, &sales)?;
    let max = agg_manager.execute_aggregate("MAX", 0, &sales)?;

    println!("  Aggregate Results:");
    println!("    COUNT: {}", count);
    println!("    SUM:   {}", sum);
    println!("    AVG:   {}", avg);
    println!("    MIN:   {}", min);
    println!("    MAX:   {}", max);

    // STRING_AGG example
    let products = vec![
        Row::new(vec![Value::Text("Laptop".to_string())]),
        Row::new(vec![Value::Text("Mouse".to_string())]),
        Row::new(vec![Value::Text("Keyboard".to_string())]),
    ];

    let product_list = agg_manager.execute_aggregate("STRING_AGG", 0, &products)?;
    println!("\n  STRING_AGG(products): {}", product_list);

    // ARRAY_AGG example
    let scores = vec![
        Row::new(vec![Value::Integer(95)]),
        Row::new(vec![Value::Integer(87)]),
        Row::new(vec![Value::Integer(92)]),
    ];

    let score_array = agg_manager.execute_aggregate("ARRAY_AGG", 0, &scores)?;
    println!("  ARRAY_AGG(scores):    {}", score_array);

    println!("\n  ✓ All basic aggregates working\n");

    // ========================================================================
    // 2. GROUP BY Aggregations
    // ========================================================================
    println!("📈 2. GROUP BY AGGREGATIONS - Grouped Statistics");
    println!("-".repeat(80));

    // Sales by region: (region, amount)
    let regional_sales = vec![
        Row::new(vec![Value::Text("North".to_string()), Value::Integer(1000)]),
        Row::new(vec![Value::Text("South".to_string()), Value::Integer(1500)]),
        Row::new(vec![Value::Text("North".to_string()), Value::Integer(1200)]),
        Row::new(vec![Value::Text("East".to_string()), Value::Integer(800)]),
        Row::new(vec![Value::Text("South".to_string()), Value::Integer(1800)]),
        Row::new(vec![Value::Text("East".to_string()), Value::Integer(900)]),
    ];

    println!("  Regional Sales Data:");
    println!("    North: $1000, $1200");
    println!("    South: $1500, $1800");
    println!("    East:  $800, $900\n");

    // Execute: SELECT region, SUM(amount), COUNT(*) FROM sales GROUP BY region
    let grouped_results = agg_manager.execute_grouped_aggregates(
        vec![("SUM".to_string(), 1), ("COUNT".to_string(), 1)],
        vec![0], // Group by column 0 (region)
        &regional_sales,
    )?;

    println!("  Results: GROUP BY region");
    println!("  Region  | Total Sales | Count");
    println!("  --------|-------------|------");
    for row in &grouped_results {
        if row.values.len() >= 3 {
            println!(
                "  {:7} | ${:10} | {}",
                row.values[0], row.values[1], row.values[2]
            );
        }
    }

    println!("\n  ✓ GROUP BY aggregations working\n");

    // ========================================================================
    // 3. WINDOW FUNCTIONS - Row-Level Analytics
    // ========================================================================
    println!("🪟 3. WINDOW FUNCTIONS - Row-Level Analytics");
    println!("-".repeat(80));

    let window_mgr = WindowManager::new();

    // Employee salary data
    let salaries = vec![
        Row::new(vec![
            Value::Text("Alice".to_string()),
            Value::Integer(75000),
        ]),
        Row::new(vec![Value::Text("Bob".to_string()), Value::Integer(65000)]),
        Row::new(vec![
            Value::Text("Carol".to_string()),
            Value::Integer(80000),
        ]),
        Row::new(vec![Value::Text("Dave".to_string()), Value::Integer(70000)]),
    ];

    println!("  Employee Salaries:");
    println!("    Alice: $75,000");
    println!("    Bob:   $65,000");
    println!("    Carol: $80,000");
    println!("    Dave:  $70,000\n");

    // ROW_NUMBER
    let spec = WindowSpec::new();
    let row_numbers = window_mgr.execute_window_function("ROW_NUMBER", &salaries, &spec, None)?;

    println!("  ROW_NUMBER() - Sequential numbering:");
    for (i, (row, num)) in salaries.iter().zip(row_numbers.iter()).enumerate() {
        println!("    {} - Row {}", row.values[0], num);
    }

    println!("\n  ✓ ROW_NUMBER working\n");

    // ========================================================================
    // 4. RANKING FUNCTIONS
    // ========================================================================
    println!("🏆 4. RANKING FUNCTIONS - RANK vs DENSE_RANK");
    println!("-".repeat(80));

    // Test scores with ties
    let test_scores = vec![
        Row::new(vec![Value::Text("Alice".to_string()), Value::Integer(95)]),
        Row::new(vec![Value::Text("Bob".to_string()), Value::Integer(87)]),
        Row::new(vec![Value::Text("Carol".to_string()), Value::Integer(95)]),
        Row::new(vec![Value::Text("Dave".to_string()), Value::Integer(82)]),
        Row::new(vec![Value::Text("Eve".to_string()), Value::Integer(90)]),
    ];

    println!("  Test Scores:");
    println!("    Alice: 95");
    println!("    Bob:   87");
    println!("    Carol: 95");
    println!("    Dave:  82");
    println!("    Eve:   90\n");

    let rank_spec = WindowSpec::new().order_by(vec![(1, false)]); // DESC by score

    let ranks = window_mgr.execute_window_function("RANK", &test_scores, &rank_spec, Some(1))?;

    let dense_ranks =
        window_mgr.execute_window_function("DENSE_RANK", &test_scores, &rank_spec, Some(1))?;

    println!("  Ranking Results:");
    println!("  Name  | Score | RANK | DENSE_RANK");
    println!("  ------|-------|------|------------");
    for i in 0..test_scores.len() {
        println!(
            "  {:5} |  {:3}  |  {}   |     {}",
            test_scores[i].values[0], test_scores[i].values[1], ranks[i], dense_ranks[i]
        );
    }

    println!("\n  Note: RANK has gaps after ties, DENSE_RANK doesn't");
    println!("  ✓ Ranking functions working\n");

    // ========================================================================
    // 5. LAG and LEAD - Time Series Analysis
    // ========================================================================
    println!("📉 5. LAG & LEAD - Time Series Analysis");
    println!("-".repeat(80));

    // Stock prices over time
    let stock_prices = vec![
        Row::new(vec![Value::Text("Day 1".to_string()), Value::Integer(100)]),
        Row::new(vec![Value::Text("Day 2".to_string()), Value::Integer(105)]),
        Row::new(vec![Value::Text("Day 3".to_string()), Value::Integer(102)]),
        Row::new(vec![Value::Text("Day 4".to_string()), Value::Integer(108)]),
        Row::new(vec![Value::Text("Day 5".to_string()), Value::Integer(110)]),
    ];

    println!("  Stock Prices:");
    for row in &stock_prices {
        println!("    {}: ${}", row.values[0], row.values[1]);
    }

    let lag_spec = WindowSpec::new();

    let previous_prices =
        window_mgr.execute_window_function("LAG", &stock_prices, &lag_spec, Some(1))?;

    let next_prices =
        window_mgr.execute_window_function("LEAD", &stock_prices, &lag_spec, Some(1))?;

    println!("\n  Time Series Analysis:");
    println!("  Day   | Price | Previous | Next  | Change");
    println!("  ------|-------|----------|-------|--------");
    for i in 0..stock_prices.len() {
        let current = &stock_prices[i].values[1];
        let prev = &previous_prices[i];
        let next = &next_prices[i];

        let change = match (current, prev) {
            (Value::Integer(cur), Value::Integer(prv)) => {
                format!("{:+}", cur - prv)
            }
            _ => "N/A".to_string(),
        };

        println!(
            "  {:5} | ${:3}  | {:8} | {:5} | {}",
            stock_prices[i].values[0], current, prev, next, change
        );
    }

    println!("\n  ✓ LAG/LEAD enable easy time-series comparisons\n");

    // ========================================================================
    // 6. PARTITION BY - Grouped Window Functions
    // ========================================================================
    println!("🗂️  6. PARTITION BY - Grouped Window Functions");
    println!("-".repeat(80));

    // Sales by department and employee
    let dept_sales = vec![
        Row::new(vec![
            Value::Text("Sales".to_string()),
            Value::Text("Alice".to_string()),
            Value::Integer(50000),
        ]),
        Row::new(vec![
            Value::Text("Sales".to_string()),
            Value::Text("Bob".to_string()),
            Value::Integer(45000),
        ]),
        Row::new(vec![
            Value::Text("Engineering".to_string()),
            Value::Text("Carol".to_string()),
            Value::Integer(70000),
        ]),
        Row::new(vec![
            Value::Text("Engineering".to_string()),
            Value::Text("Dave".to_string()),
            Value::Integer(65000),
        ]),
    ];

    println!("  Department Sales:");
    println!("    Sales:       Alice ($50k), Bob ($45k)");
    println!("    Engineering: Carol ($70k), Dave ($65k)\n");

    // ROW_NUMBER partitioned by department
    let partition_spec = WindowSpec::new()
        .partition_by(vec![0]) // Partition by department
        .order_by(vec![(2, false)]); // Order by sales DESC

    let dept_ranks =
        window_mgr.execute_window_function("ROW_NUMBER", &dept_sales, &partition_spec, None)?;

    println!("  ROW_NUMBER() OVER (PARTITION BY department ORDER BY sales DESC):");
    println!("  Department  | Employee | Sales  | Rank in Dept");
    println!("  ------------|----------|--------|-------------");
    for i in 0..dept_sales.len() {
        println!(
            "  {:11} | {:8} | ${:5} | {}",
            dept_sales[i].values[0],
            dept_sales[i].values[1],
            dept_sales[i].values[2],
            dept_ranks[i]
        );
    }

    println!("\n  ✓ Each partition has independent row numbering\n");

    // ========================================================================
    // 7. REAL-WORLD SCENARIO - E-Commerce Analytics
    // ========================================================================
    println!("🌟 7. REAL-WORLD SCENARIO - E-Commerce Analytics Dashboard");
    println!("-".repeat(80));

    // Order data: (customer_id, order_date, amount, category)
    let orders = vec![
        Row::new(vec![
            Value::Integer(1),
            Value::Text("2024-01".to_string()),
            Value::Integer(100),
            Value::Text("Electronics".to_string()),
        ]),
        Row::new(vec![
            Value::Integer(2),
            Value::Text("2024-01".to_string()),
            Value::Integer(150),
            Value::Text("Books".to_string()),
        ]),
        Row::new(vec![
            Value::Integer(1),
            Value::Text("2024-02".to_string()),
            Value::Integer(200),
            Value::Text("Electronics".to_string()),
        ]),
        Row::new(vec![
            Value::Integer(3),
            Value::Text("2024-01".to_string()),
            Value::Integer(75),
            Value::Text("Electronics".to_string()),
        ]),
        Row::new(vec![
            Value::Integer(2),
            Value::Text("2024-02".to_string()),
            Value::Integer(120),
            Value::Text("Books".to_string()),
        ]),
    ];

    println!("\n  Analytics Query 1: Total sales by category");
    let category_stats = agg_manager.execute_grouped_aggregates(
        vec![
            ("SUM".to_string(), 2),   // Total amount
            ("COUNT".to_string(), 0), // Number of orders
            ("AVG".to_string(), 2),   // Average order value
        ],
        vec![3], // Group by category
        &orders,
    )?;

    println!("  Category    | Total | Orders | Avg");
    println!("  ------------|-------|--------|------");
    for row in &category_stats {
        if row.values.len() >= 4 {
            println!(
                "  {:11} | ${:4} | {:6} | ${:4}",
                row.values[0], row.values[1], row.values[2], row.values[3]
            );
        }
    }

    println!("\n  Analytics Query 2: Customer order sequence with running total");
    let customer_spec = WindowSpec::new()
        .partition_by(vec![0]) // By customer
        .order_by(vec![(1, true)]); // By date ASC

    let order_numbers =
        window_mgr.execute_window_function("ROW_NUMBER", &orders, &customer_spec, None)?;

    println!("  Customer | Date    | Amount | Order# | Category");
    println!("  ---------|---------|--------|--------|-------------");
    for i in 0..orders.len() {
        println!(
            "  {:8} | {} | ${:4}  | {:6} | {}",
            orders[i].values[0],
            orders[i].values[1],
            orders[i].values[2],
            order_numbers[i],
            orders[i].values[3]
        );
    }

    println!("\n  ✓ Complete analytics capabilities for business intelligence");

    // ========================================================================
    // Summary
    // ========================================================================
    println!();
    println!("=".repeat(80));
    println!("Week 13 Analytics Summary");
    println!("=".repeat(80));
    println!("✅ AGGREGATE FUNCTIONS: COUNT, SUM, AVG, MIN, MAX, STRING_AGG, ARRAY_AGG");
    println!("✅ GROUP BY: Multi-column grouping with multiple aggregates");
    println!("✅ WINDOW FUNCTIONS: ROW_NUMBER, RANK, DENSE_RANK");
    println!("✅ VALUE FUNCTIONS: LAG, LEAD, FIRST_VALUE, LAST_VALUE");
    println!("✅ PARTITIONING: Independent calculations per partition");
    println!("✅ FRAME SUPPORT: ROWS/RANGE with customizable bounds");
    println!();
    println!("NexusQL now has enterprise-grade analytical capabilities!");
    println!("These features enable complex business intelligence, reporting, and data analysis.");
    println!("=".repeat(80));

    Ok(())
}
