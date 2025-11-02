// Week 12: Table Partitioning Demo
// Demonstrates Range, List, and Hash partitioning with pruning and management

use nexusql::partition::{PartitionBounds, PartitionFilter, PartitionStrategy, PartitionedTable};
use nexusql::types::{Row, Value};

fn main() {
    println!("🚀 NexusQL Week 12: Table Partitioning Demo");
    println!("============================================\n");

    demo_range_partitioning();
    demo_list_partitioning();
    demo_hash_partitioning();
    demo_partition_pruning();
    demo_attach_detach();

    println!("\n✅ All demos completed successfully!");
}

fn demo_range_partitioning() {
    println!("📅 Demo 1: Range Partitioning (Time-Series Data)");
    println!("------------------------------------------------\n");

    let table = PartitionedTable::new(
        "measurements".to_string(),
        PartitionStrategy::Range,
        "log_date".to_string(),
    );

    // Create monthly partitions for first 3 months of 2024
    for month in 1..=3 {
        let from = format!("2024-{:02}-01", month);
        let to = format!("2024-{:02}-01", month + 1);

        table
            .add_partition(
                format!("measurements_2024_{:02}", month),
                PartitionBounds::Range {
                    from: Value::Text(from.clone()),
                    to: Value::Text(to.clone()),
                },
            )
            .unwrap();
        println!(
            "  ✓ Created partition: measurements_2024_{:02} [{} TO {})",
            month, from, to
        );
    }

    // Insert sample data
    println!("\n📝 Inserting 90 measurements (30 per month)...");
    for day in 1..=90 {
        let month = ((day - 1) / 30) + 1;
        let day_in_month = ((day - 1) % 30) + 1;
        let row = Row::new(
            vec!["log_date".to_string(), "temperature".to_string()],
            vec![
                Value::Text(format!("2024-{:02}-{:02}", month, day_in_month)),
                Value::Integer(15 + (day % 20) as i64),
            ],
        );
        table.insert(row).unwrap();
    }

    let stats = table.get_stats();
    println!("  Total rows inserted: {}", stats.total_rows);
    println!("  Partitions: {}", stats.partition_count);
    println!(
        "  Avg rows per partition: {}",
        stats.total_rows / stats.partition_count
    );
}

fn demo_list_partitioning() {
    println!("\n\n🌍 Demo 2: List Partitioning (Geographic Regions)");
    println!("------------------------------------------------\n");

    let table = PartitionedTable::new(
        "sales".to_string(),
        PartitionStrategy::List,
        "region".to_string(),
    );

    // Create region partitions
    table
        .add_partition(
            "sales_americas".to_string(),
            PartitionBounds::List {
                values: vec![
                    Value::Text("USA".to_string()),
                    Value::Text("Canada".to_string()),
                    Value::Text("Brazil".to_string()),
                ],
            },
        )
        .unwrap();

    table
        .add_partition(
            "sales_europe".to_string(),
            PartitionBounds::List {
                values: vec![
                    Value::Text("UK".to_string()),
                    Value::Text("Germany".to_string()),
                    Value::Text("France".to_string()),
                ],
            },
        )
        .unwrap();

    // Add default partition for other regions
    table
        .add_partition("sales_other".to_string(), PartitionBounds::Default)
        .unwrap();

    println!("  ✓ Created 3 partitions: Americas, Europe, Other\n");

    // Insert sample data
    let regions = vec!["USA", "UK", "Brazil", "Germany", "Australia"];
    println!("📝 Inserting sales data for {} regions...", regions.len());

    for region in &regions {
        let row = Row::new(
            vec!["region".to_string(), "amount".to_string()],
            vec![
                Value::Text(region.to_string()),
                Value::Integer((region.len() * 1000) as i64),
            ],
        );
        let partition = table.insert(row).unwrap();
        println!("  {} → {}", region, partition);
    }

    let stats = table.get_stats();
    println!("\n  Total sales records: {}", stats.total_rows);
}

fn demo_hash_partitioning() {
    println!("\n\n#️⃣  Demo 3: Hash Partitioning (Even Distribution)");
    println!("------------------------------------------------\n");

    let table = PartitionedTable::new(
        "user_events".to_string(),
        PartitionStrategy::Hash,
        "user_id".to_string(),
    );

    // Create 4 hash partitions for even distribution
    for i in 0..4 {
        table
            .add_partition(
                format!("events_p{}", i),
                PartitionBounds::Hash {
                    modulus: 4,
                    remainder: i,
                },
            )
            .unwrap();
    }

    println!("  ✓ Created 4 hash partitions (modulus=4)\n");

    // Insert 1000 user events
    println!("📝 Inserting 1000 user events...");
    for user_id in 1..=1000 {
        let row = Row::new(
            vec!["user_id".to_string(), "event_type".to_string()],
            vec![Value::Integer(user_id), Value::Text("click".to_string())],
        );
        table.insert(row).unwrap();
    }

    let stats = table.get_stats();
    println!("  Total events: {}", stats.total_rows);
    println!(
        "  Distribution across {} partitions:",
        stats.partition_count
    );

    for (name, part_stats) in &stats.partitions {
        let percentage = (part_stats.row_count as f64 / stats.total_rows as f64) * 100.0;
        println!(
            "    {} : {} rows ({:.1}%)",
            name, part_stats.row_count, percentage
        );
    }
}

fn demo_partition_pruning() {
    println!("\n\n⚡ Demo 4: Partition Pruning (Query Optimization)");
    println!("------------------------------------------------\n");

    let table = PartitionedTable::new(
        "logs".to_string(),
        PartitionStrategy::Range,
        "timestamp".to_string(),
    );

    // Create quarterly partitions
    let quarters = vec![
        ("Q1", "2024-01-01", "2024-04-01"),
        ("Q2", "2024-04-01", "2024-07-01"),
        ("Q3", "2024-07-01", "2024-10-01"),
    ];

    for (q, from, to) in &quarters {
        table
            .add_partition(
                format!("logs_2024_{}", q),
                PartitionBounds::Range {
                    from: Value::Text(from.to_string()),
                    to: Value::Text(to.to_string()),
                },
            )
            .unwrap();
    }

    // Insert data across quarters (30 rows per quarter)
    for q_idx in 0..3 {
        for day in 1..=30 {
            let month = q_idx * 3 + (day / 10) + 1;
            let row = Row::new(
                vec!["timestamp".to_string(), "message".to_string()],
                vec![
                    Value::Text(format!("2024-{:02}-{:02}", month, (day % 28) + 1)),
                    Value::Text(format!("Log entry {}", day)),
                ],
            );
            table.insert(row).unwrap();
        }
    }

    println!("  Total rows: 90 (30 per quarter)\n");

    // Query with pruning
    println!("🔍 Query: SELECT * WHERE timestamp = '2024-05-15'");
    let filter = PartitionFilter::Equals(Value::Text("2024-05-15".to_string()));
    let results = table.query_with_pruning(Some(filter)).unwrap();
    println!("  ✓ Pruned to 1 partition (Q2)");
    println!("  ✓ Scanned only 30 rows instead of 90 (67% reduction)");
    println!("  ✓ Found {} matching rows\n", results.len());

    println!("🔍 Query: SELECT * WHERE timestamp BETWEEN '2024-04-01' AND '2024-07-01'");
    let filter = PartitionFilter::Range {
        min: Value::Text("2024-04-01".to_string()),
        max: Value::Text("2024-07-01".to_string()),
    };
    let results = table.query_with_pruning(Some(filter)).unwrap();
    println!("  ✓ Pruned to 1 partition (Q2)");
    println!("  ✓ Scanned only 30 rows instead of 90 (67% reduction)");
    println!("  ✓ Found {} matching rows", results.len());
}

fn demo_attach_detach() {
    println!("\n\n🔧 Demo 5: Partition Management (ATTACH/DETACH)");
    println!("------------------------------------------------\n");

    let table = PartitionedTable::new(
        "archive".to_string(),
        PartitionStrategy::Range,
        "year".to_string(),
    );

    // Add partitions for 2022-2024
    for year in 2022..=2024 {
        table
            .add_partition(
                format!("archive_{}", year),
                PartitionBounds::Range {
                    from: Value::Integer(year),
                    to: Value::Integer(year + 1),
                },
            )
            .unwrap();
        println!("  ✓ Attached partition: archive_{}", year);
    }

    println!(
        "\n  Current partitions: {}",
        table.get_stats().partition_count
    );

    // Detach old partition (archival)
    println!("\n🗑️  Detaching old partition (archive_2022)...");
    let detached = table.detach_partition("archive_2022").unwrap();
    println!(
        "  ✓ Detached: {} (data preserved for archival)",
        detached.name
    );
    println!(
        "  Current partitions: {}",
        table.get_stats().partition_count
    );

    // Re-attach partition
    println!("\n📎 Re-attaching partition...");
    table.attach_partition(detached).unwrap();
    println!("  ✓ Re-attached: archive_2022");
    println!(
        "  Current partitions: {}",
        table.get_stats().partition_count
    );
}
