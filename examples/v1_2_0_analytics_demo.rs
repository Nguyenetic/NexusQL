// NexusQL v1.2.0 - Advanced Analytics Demo
// Time-series optimization, columnar storage, vectorized execution

use nexusql::analytics::{
    ColumnarStorage, ContinuousAggregate, TimeSeriesOptimizer, TimeWindow, VectorizedExecutor,
};
use nexusql::types::{Row, Value};

fn main() {
    println!("📊 NexusQL v1.2.0 - Advanced Analytics Demo");
    println!("==========================================\n");

    demo_timeseries();
    demo_continuous_aggregates();
    demo_columnar_storage();
    demo_vectorized_execution();

    println!("\n✅ v1.2.0 - High-Performance Analytics Ready!");
}

fn demo_timeseries() {
    println!("⏰ Demo 1: Time-Series Optimization");
    println!("------------------------------------\n");

    // Generate time-series data (1 minute intervals)
    let rows: Vec<Row> = (0..1440)
        .map(|i| {
            Row::new(
                vec!["timestamp".to_string(), "temperature".to_string()],
                vec![Value::Integer(i * 60), Value::Integer(20 + (i % 10))],
            )
        })
        .collect();

    println!("Time-series data: 1,440 data points (1 day, 1-min intervals)\n");

    // Aggregate by 1-hour windows
    let result = TimeSeriesOptimizer::time_bucket_aggregate(
        &rows,
        "timestamp",
        "temperature",
        TimeWindow::Hours(1),
    )
    .unwrap();

    println!("TIME_BUCKET aggregation (1-hour windows):");
    println!("  Input: 1,440 rows");
    println!("  Output: {} hourly aggregates", result.len());
    println!("  Compression: {:.1}x\n", 1440.0 / result.len() as f64);

    println!("Sample buckets:");
    for (i, (ts, avg)) in result.iter().take(5).enumerate() {
        println!("  Hour {}: avg temp = {:.1}°C", i, avg);
    }

    println!("\n  💡 Perfect for IoT, metrics, and monitoring data!");
}

fn demo_continuous_aggregates() {
    println!("\n\n📈 Demo 2: Continuous Aggregates");
    println!("----------------------------------\n");

    let mut cagg = ContinuousAggregate::new(TimeWindow::Minutes(5));

    println!("Creating continuous aggregate (5-minute windows)...");

    // Stream data points
    for i in 0..300 {
        let timestamp = i * 10; // Every 10 seconds
        let value = 100.0 + (i as f64 % 20.0);
        cagg.add_point(timestamp, value);
    }

    println!("  ✓ Processed 300 data points\n");

    // Query specific time range
    let results = cagg.query_range(0, 600);
    println!("Query: Last 10 minutes");
    println!("  Buckets returned: {}", results.len());

    for (ts, avg, min, max) in results.iter().take(3) {
        println!("  t={}: avg={:.1}, min={:.1}, max={:.1}", ts, avg, min, max);
    }

    println!("\n  💡 Incremental updates - no need to recompute!");
    println!("  💡 Like TimescaleDB continuous aggregates!");
}

fn demo_columnar_storage() {
    println!("\n\n📊 Demo 3: Columnar Storage");
    println!("---------------------------\n");

    // Create row-oriented data
    let rows: Vec<Row> = (0..10000)
        .map(|i| {
            Row::new(
                vec![
                    "id".to_string(),
                    "name".to_string(),
                    "amount".to_string(),
                    "region".to_string(),
                ],
                vec![
                    Value::Integer(i),
                    Value::Text(format!("user_{}", i)),
                    Value::Integer(i * 100),
                    Value::Text(format!("region_{}", i % 5)),
                ],
            )
        })
        .collect();

    println!("Converting 10,000 rows to columnar format...\n");
    let columnar = ColumnarStorage::from_rows("sales".to_string(), &rows);

    println!("Columnar Storage:");
    println!("  Rows: {}", columnar.row_count);
    println!("  Columns: {}", columnar.columns.len());
    println!(
        "  Compression ratio: {:.1}x\n",
        columnar.compression_ratio()
    );

    // Scan specific columns only
    println!("Query: SELECT amount FROM sales (columnar scan)");
    let columns = columnar.scan_columns(&["amount".to_string()]).unwrap();
    println!("  ✓ Scanned 1 column instead of 4 (75% I/O reduction)");
    println!("  ✓ Retrieved {} values\n", columns[0].len());

    // Vectorized aggregation
    let sum = columnar.vectorized_sum("amount").unwrap();
    println!("Vectorized SUM(amount):");
    println!("  Result: {:.0}", sum);
    println!("\n  💡 Columnar layout perfect for analytics workloads!");
    println!("  💡 Scan only needed columns, not entire rows!");
}

fn demo_vectorized_execution() {
    println!("\n\n⚡ Demo 4: Vectorized Execution");
    println!("-------------------------------\n");

    let rows: Vec<Row> = (0..10000)
        .map(|i| {
            Row::new(
                vec!["id".to_string(), "value".to_string()],
                vec![Value::Integer(i), Value::Integer(i * 10)],
            )
        })
        .collect();

    println!("Vectorized filtering (batch size: 1024)...");

    let start = std::time::Instant::now();
    let filtered = VectorizedExecutor::filter_batch(
        &rows,
        |row| matches!(row.values[1], Value::Integer(v) if v > 50000),
    );
    let elapsed = start.elapsed();

    println!("  Input: 10,000 rows");
    println!("  Output: {} rows", filtered.len());
    println!("  Time: {:.2} ms\n", elapsed.as_secs_f64() * 1000.0);

    // SIMD-style sum
    println!("Vectorized SUM (SIMD-friendly)...");
    let values: Vec<i64> = (1..=10000).collect();

    let start = std::time::Instant::now();
    let sum = VectorizedExecutor::sum_vectorized(&values);
    let elapsed = start.elapsed();

    println!("  Values: 10,000");
    println!("  Sum: {}", sum);
    println!("  Time: {:.2} ms", elapsed.as_secs_f64() * 1000.0);

    println!("\n  💡 Vectorized execution processes data in batches!");
    println!("  💡 SIMD-friendly for modern CPUs!");
}
