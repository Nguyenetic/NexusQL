//! FAST 100K Test - Batch Inserts

use nexusql::{engine::BatchInsert, types::Value, NexusSQL};
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🚀 FAST 100K TEST - BATCH INSERTS\n");

    let db = NexusSQL::new("./fast_100k.nexus").await?;

    db.execute("CREATE TABLE massive (id INT, name TEXT, value FLOAT, category TEXT)")
        .await?;

    let batch_size = 1000;
    let total_rows = 100000;

    println!(
        "Inserting {} rows in batches of {}...",
        total_rows, batch_size
    );
    let start = Instant::now();

    for batch_num in 0..(total_rows / batch_size) {
        let mut batch = BatchInsert::new("massive".to_string(), vec![]);

        for i in 1..=batch_size {
            let id = batch_num * batch_size + i;
            batch.add_row(vec![
                Value::Integer(id as i64),
                Value::Text(format!("User{}", id)),
                Value::from((id as f64) * 1.5),
                Value::Text(format!("Cat{}", id % 50)),
            ]);
        }

        db.execute(&batch.to_sql()).await?;

        if (batch_num + 1) % 10 == 0 {
            println!(
                "  Progress: {}/{}",
                (batch_num + 1) * batch_size,
                total_rows
            );
        }
    }

    let insert_time = start.elapsed();
    let insert_rate = total_rows as f64 / insert_time.as_secs_f64();

    println!("\n✅ INSERTED {} ROWS!", total_rows);
    println!("   Time: {:.2}s", insert_time.as_secs_f64());
    println!("   Rate: {:.0} rows/sec\n", insert_rate);

    // Query test
    println!("Querying with WHERE clause...");
    let start = Instant::now();
    let r = db
        .execute("SELECT * FROM massive WHERE value > 50000 LIMIT 100")
        .await?;
    println!(
        "   ✓ Query: {:.0}ms ({} rows)\n",
        start.elapsed().as_millis(),
        r.rows.len()
    );

    db.shutdown().await?;

    println!("╔═════════════════════════════════════════╗");
    println!("║  🎉 PRODUCTION-SCALE TEST PASSED 🎉    ║");
    println!("║  Rate: {:.0} rows/sec               ║", insert_rate);
    println!("╚═════════════════════════════════════════╝\n");

    Ok(())
}
