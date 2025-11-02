//! Optimized Bulk Insert - PostgreSQL/DuckDB Best Practices
//! Using Transactions for Batch Commits

use nexusql::NexusSQL;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🚀 OPTIMIZED BULK INSERT TEST\n");
    
    let db = NexusSQL::new("./optimized.nexus").await?;
    
    db.execute("CREATE TABLE test (id INT, name TEXT, value FLOAT)").await?;
    
    // METHOD 1: Individual inserts (SLOW - what we did before)
    println!("Method 1: Individual INSERTs (baseline)");
    let start = Instant::now();
    
    for i in 1..=1000 {
        db.execute(&format!("INSERT INTO test VALUES ({}, 'User{}', {})", i, i, i as f32)).await?;
    }
    
    let slow_time = start.elapsed();
    let slow_rate = 1000.0 / slow_time.as_secs_f64();
    println!("  Time: {:.2}s", slow_time.as_secs_f64());
    println!("  Rate: {:.0} rows/sec\n", slow_rate);
    
    // METHOD 2: Batch insert (FAST - PostgreSQL best practice)
    db.execute("CREATE TABLE test2 (id INT, name TEXT, value FLOAT)").await?;
    
    println!("Method 2: Batch INSERT (optimized)");
    let start = Instant::now();
    
    // Build single INSERT with multiple VALUES
    let mut values = Vec::new();
    for i in 1..=1000 {
        values.push(format!("({}, 'User{}', {})", i, i, i as f32));
    }
    
    let batch_sql = format!("INSERT INTO test2 VALUES {}", values.join(", "));
    db.execute(&batch_sql).await?;
    
    let fast_time = start.elapsed();
    let fast_rate = 1000.0 / fast_time.as_secs_f64();
    println!("  Time: {:.2}s", fast_time.as_secs_f64());
    println!("  Rate: {:.0} rows/sec\n", fast_rate);
    
    // Compare
    let speedup = slow_time.as_secs_f64() / fast_time.as_secs_f64();
    println!("╔══════════════════════════════════════╗");
    println!("║       PERFORMANCE COMPARISON         ║");
    println!("╠══════════════════════════════════════╣");
    println!("║  Individual: {:.0} rows/sec      ║", slow_rate);
    println!("║  Batched:    {:.0} rows/sec     ║", fast_rate);
    println!("║                                      ║");
    println!("║  SPEEDUP: {:.1}× FASTER!           ║", speedup);
    println!("╚══════════════════════════════════════╝\n");
    
    db.shutdown().await?;
    Ok(())
}
