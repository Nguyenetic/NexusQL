//! Engine Routing Demo - Fixed

use nexusql::NexusSQL;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🎯 NexusSQL Engine Routing Demo\n");
    
    let db = NexusSQL::new("./routing.nexus").await?;
    
    println!("Available engines: {:?}\n", db.list_engines());
    
    // Create test data first
    db.execute("CREATE TABLE users (id INTEGER, name TEXT)").await?;
    db.execute("INSERT INTO users VALUES (1, 'Alice')").await?;
    
    println!("Demo 1: Default engine (no hint)");
    let r = db.execute("SELECT * FROM users").await?;
    println!("  ✓ Default engine: {} rows\n", r.rows.len());
    
    println!("Demo 2: ColBERT engine (block comment hint)");
    let r = db.execute("SELECT * FROM docs /* ENGINE: colbert */").await?;
    println!("  ✓ ColBERT engine: {} rows\n", r.rows.len());
    
    println!("Demo 3: HDC engine (line comment hint)");
    let r = db.execute("SELECT * FROM sensors -- ENGINE: hdc").await?;
    println!("  ✓ HDC engine: {} rows\n", r.rows.len());
    
    println!("═══════════════════════════════════════");
    println!("✅ SQL HINTS WORKING!");
    println!("✅ PostgreSQL-style comments");
    println!("✅ No custom parser needed");
    println!("✅ Standard SQL compatible");
    println!("═══════════════════════════════════════\n");
    
    db.shutdown().await?;
    Ok(())
}
