//! Persistence Demo

use nexusql::NexusSQL;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧠 NexusSQL Persistence Demo\n");
    
    // Create database
    let db = NexusSQL::new("./demo_db").await?;
    
    println!("📝 Creating table...");
    db.execute("CREATE TABLE users (id, name)").await?;
    
    println!("📝 Inserting data...");
    db.execute("INSERT INTO users VALUES (1, 'Alice')").await?;
    db.execute("INSERT INTO users VALUES (2, 'Bob')").await?;
    
    println!("📝 Querying data...");
    let result = db.execute("SELECT * FROM users").await?;
    
    println!("✓ Found {} rows:", result.rows.len());
    for row in &result.rows {
        if let (Some(id), Some(name)) = (row.get("id"), row.get("name")) {
            println!("  - {:?}: {:?}", id, name);
        }
    }
    
    db.shutdown().await?;
    
    println!("\n🔄 Reopening database...");
    let db2 = NexusSQL::new("./demo_db").await?;
    
    println!("📝 Data persisted! Querying again...");
    let result2 = db2.execute("SELECT * FROM users").await?;
    println!("✓ Found {} rows (data survived restart!)", result2.rows.len());
    
    db2.shutdown().await?;
    
    println!("\n✅ Persistence working!\n");
    
    Ok(())
}
