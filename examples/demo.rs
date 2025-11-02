//! NexusSQL Demo - Multi-Engine Showcase

use nexusql::NexusSQL;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    println!("\n╔══════════════════════════════════════════════════╗");
    println!("║     NEXUSSQL MULTI-ENGINE DEMONSTRATION          ║");
    println!("╚══════════════════════════════════════════════════╝\n");
    
    // Initialize NexusSQL
    let db = NexusSQL::new("./demo.nexus").await?;
    
    println!("📊 Available Intelligence Engines:");
    for (i, engine) in db.list_engines().iter().enumerate() {
        println!("  {}. {}", i + 1, engine);
    }
    
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    
    // Demo 1: Default Engine (SQL)
    println!("🔹 Demo 1: Default SQL Engine");
    let result = db.execute("SELECT * FROM users WHERE age > 18").await?;
    println!("   Result: {} rows", result.rows.len());
    
    // Demo 2: ColBERT Engine (Semantic Search)
    println!("\n🔹 Demo 2: ColBERT Semantic Search");
    let result = db.execute("
        SELECT * FROM documents 
        WHERE content MATCHES 'machine learning'
        USING colbert
    ").await?;
    println!("   Result: {} rows (semantic match)", result.rows.len());
    
    // Demo 3: HDC Engine (Fast Classification)
    println!("\n🔹 Demo 3: HDC Fast Classification");
    let result = db.execute("
        SELECT CLASSIFY(sensor_data) 
        FROM iot_sensors
        USING hdc
    ").await?;
    println!("   Result: {} classifications in <1ms", result.rows.len());
    
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("✅ Demo complete - All engines working!\n");
    
    db.shutdown().await?;
    
    Ok(())
}
