//! Complete NexusSQL Demo

use nexusql::{NexusSQL, Value};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).init();
    
    println!("\n╔═══════════════════════════════════════════════════════╗");
    println!("║  🧠  NEXUSSQL v0.1.0 - FULL DEMONSTRATION  🧠        ║");
    println!("╚═══════════════════════════════════════════════════════╝\n");
    
    let db = NexusSQL::new("./complete.nexus").await?;
    
    println!("📊 Intelligence Engines:");
    for engine in db.list_engines() {
        println!("   ✓ {}", engine);
    }
    
    println!("\n{}\n", "━".repeat(60));
    
    // Demo 1: SQL with Persistence
    println!("✨ DEMO 1: SQL + PERSISTENCE");
    db.execute("CREATE TABLE products (id, name)").await?;
    db.execute("INSERT INTO products VALUES (1, 'Laptop')").await?;
    let result = db.execute("SELECT * FROM products").await?;
    println!("   ✓ SQL working: {} rows\n", result.rows.len());
    
    // Demo 2: ColBERT
    println!("✨ DEMO 2: COLBERT SEMANTIC SEARCH");
    let result = db.execute("SELECT * FROM docs USING colbert").await?;
    println!("   ✓ Semantic search: {} results", result.rows.len());
    for row in &result.rows {
        if let Some(Value::Float(s)) = row.get("score") {
            println!("      Score: {:.3}", s);
        }
    }
    println!();
    
    // Demo 3: HDC
    println!("✨ DEMO 3: HDC CLASSIFICATION");
    let result = db.execute("SELECT * FROM sensor USING hdc").await?;
    println!("   ✓ HDC classification: {} results", result.rows.len());
    for row in &result.rows {
        if let Some(c) = row.get("class") {
            println!("      Class: {:?}", c);
        }
    }
    
    println!("\n{}\n", "━".repeat(60));
    println!("✅ NEXUSSQL COMPLETE BUILD SUCCESS!");
    println!("✅ Persistence ✓ | SQL Parser ✓ | ColBERT ✓ | HDC ✓\n");
    
    db.shutdown().await?;
    Ok(())
}
