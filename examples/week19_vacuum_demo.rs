// Week 19: VACUUM & Storage Management Demo
// Demonstrates VACUUM, bloat detection, visibility/free space maps

use nexusql::types::{Row, Value};
use nexusql::vacuum::{AutoVacuumConfig, TableVacuum, TupleId};

fn main() {
    println!("🚀 NexusQL Week 19: VACUUM & Storage Management Demo");
    println!("====================================================\n");

    demo_vacuum_basic();
    demo_bloat_detection();
    demo_visibility_map();
    demo_free_space_map();
    demo_auto_vacuum();

    println!("\n✅ All demos completed successfully!");
}

fn demo_vacuum_basic() {
    println!("🧹 Demo 1: Basic VACUUM Operation");
    println!("----------------------------------\n");

    let mut table = TableVacuum::new("orders".to_string());

    // Insert 1000 rows
    println!("Inserting 1,000 rows...");
    for i in 0..1000 {
        let row = Row::new(
            vec!["id".to_string(), "amount".to_string()],
            vec![Value::Integer(i), Value::Integer(i * 100)],
        );
        table.insert(row);
    }

    println!("  ✓ Inserted 1,000 rows\n");

    // Simulate DELETEs - mark 300 rows as dead
    println!("Simulating 300 DELETEs (mark tuples as dead)...");
    for i in 0..300 {
        table.mark_dead(TupleId {
            page: (i / 100) as u32,
            offset: (i % 100) as u16,
        });
    }

    let stats_before = table.get_stats();
    println!("  Before VACUUM:");
    println!("    Live tuples: {}", stats_before.live_tuples);
    println!("    Dead tuples: {}\n", stats_before.dead_tuples);

    // Execute VACUUM
    println!("Executing VACUUM...");
    let vacuum_stats = table.vacuum().unwrap();

    println!("  ✓ VACUUM completed in {:.2} ms", vacuum_stats.duration_ms);
    println!("    Tuples removed: {}", vacuum_stats.tuples_removed);
    println!("    Pages freed: {}", vacuum_stats.pages_freed);
    println!(
        "    Space recovered: {} bytes\n",
        vacuum_stats.space_recovered_bytes
    );

    let stats_after = table.get_stats();
    println!("  After VACUUM:");
    println!("    Live tuples: {}", stats_after.live_tuples);
    println!("    Dead tuples: {}", stats_after.dead_tuples);
}

fn demo_bloat_detection() {
    println!("\n\n📊 Demo 2: Bloat Detection");
    println!("--------------------------\n");

    let mut table = TableVacuum::new("users".to_string());

    // Insert 500 rows
    for i in 0..500 {
        let row = Row::new(
            vec!["id".to_string(), "name".to_string()],
            vec![Value::Integer(i), Value::Text(format!("user_{}", i))],
        );
        table.insert(row);
    }

    // Gradually create bloat
    let bloat_scenarios = vec![(50, "10%"), (100, "20%"), (200, "40%")];

    for (dead_count, percent) in bloat_scenarios {
        // Mark rows as dead
        for i in 0..dead_count {
            table.mark_dead(TupleId {
                page: (i / 100) as u32,
                offset: (i % 100) as u16,
            });
        }

        let bloat = table.detect_bloat();
        println!("Bloat at {} dead tuples:", percent);
        println!("  Total tuples: {}", bloat.total_tuples);
        println!("  Dead tuples: {}", bloat.dead_tuples);
        println!("  Bloat ratio: {:.1}%", bloat.bloat_ratio * 100.0);
        println!("  Wasted space: {} bytes", bloat.wasted_bytes);
        println!("  Needs VACUUM: {}\n", bloat.needs_vacuum);

        // Vacuum to reset
        table.vacuum().unwrap();
    }
}

fn demo_visibility_map() {
    println!("\n📍 Demo 3: Visibility Map");
    println!("-------------------------\n");

    let mut table = TableVacuum::new("products".to_string());

    // Insert rows across 5 pages (100 rows per page)
    for i in 0..500 {
        let row = Row::new(vec!["id".to_string()], vec![Value::Integer(i)]);
        table.insert(row);
    }

    println!("Inserted 500 rows across 5 pages\n");

    // Mark some tuples as dead on page 1
    for i in 100..120 {
        table.mark_dead(TupleId {
            page: 1,
            offset: (i % 100) as u16,
        });
    }

    println!("Marked 20 tuples as dead on page 1");
    println!("  Page 1 visibility: DIRTY (has dead tuples)\n");

    // VACUUM
    table.vacuum().unwrap();

    let stats = table.get_stats();
    println!("After VACUUM:");
    println!("  Total pages: {}", stats.total_pages);
    println!("  All-visible pages: {}", stats.all_visible_pages);
    println!("\n  💡 All-visible pages can skip visibility checks!");
}

fn demo_free_space_map() {
    println!("\n\n💾 Demo 4: Free Space Map");
    println!("-------------------------\n");

    let mut table = TableVacuum::new("logs".to_string());

    // Fill pages to different levels
    for i in 0..250 {
        let row = Row::new(vec!["id".to_string()], vec![Value::Integer(i)]);
        table.insert(row);
    }

    let stats = table.get_stats();
    println!("Table: logs");
    println!("  Total pages: {}", stats.total_pages);
    println!("  Free space: {} bytes\n", stats.free_space_bytes);

    println!("Free Space Map usage:");
    println!("  • Tracks available space per page");
    println!("  • Helps find pages for new insertions");
    println!("  • Reduces page fragmentation");
    println!("  • Updated by VACUUM operations");
}

fn demo_auto_vacuum() {
    println!("\n\n🤖 Demo 5: Auto-VACUUM");
    println!("----------------------\n");

    let config = AutoVacuumConfig::default();

    println!("Auto-VACUUM Configuration:");
    println!("  Enabled: {}", config.enabled);
    println!("  Base threshold: {} dead tuples", config.vacuum_threshold);
    println!(
        "  Scale factor: {:.0}%\n",
        config.vacuum_scale_factor * 100.0
    );

    println!("Threshold calculation:");
    println!("  threshold = base + (scale_factor × live_tuples)\n");

    let scenarios = vec![
        (100, 10, "Small table, few dead"),
        (100, 70, "Small table, many dead"),
        (1000, 200, "Large table, some dead"),
        (1000, 300, "Large table, many dead"),
    ];

    println!("Should auto-vacuum?");
    for (live, dead, desc) in scenarios {
        let should = config.should_vacuum(live, dead);
        let threshold =
            config.vacuum_threshold + ((live as f64 * config.vacuum_scale_factor) as usize);
        println!("  {} live, {} dead ({}):", live, dead, desc);
        println!("    Threshold: {}, Trigger: {}\n", threshold, should);
    }
}
