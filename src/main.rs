//! NexusSQL CLI
//! PROPRIETARY

use nexusql::NexusSQL;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    println!("\n🧠 NexusSQL v0.1.0");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Where Intelligence Converges\n");
    
    // Create database
    let db = NexusSQL::new("./data/nexus.db").await?;
    
    println!("✓ Database initialized");
    println!("✓ Engines loaded: {:?}", db.list_engines());
    
    // Test query
    println!("\n🔍 Running test query...");
    let results = db.execute("SELECT * FROM test").await?;
    
    println!("✓ Query executed successfully");
    println!("✓ Rows returned: {}", results.rows.len());
    
    if let Some(row) = results.rows.first() {
        println!("\nSample row:");
        for (i, col) in row.columns.iter().enumerate() {
            if let Some(val) = row.values.get(i) {
                println!("  {}: {:?}", col, val);
            }
        }
    }
    
    // Shutdown
    db.shutdown().await?;
    println!("\n✓ Shutdown complete\n");
    
    Ok(())
}
