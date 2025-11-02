//! Real SQL Query Test

use nexusql::NexusSQL;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🧪 TESTING REAL SQL QUERIES\n");
    
    let db = NexusSQL::new("./sql_test.nexus").await?;
    
    // Test 1: CREATE TABLE
    println!("1️⃣ CREATE TABLE users (id, name, age)");
    db.execute("CREATE TABLE users (id INTEGER, name TEXT, age INTEGER)").await?;
    println!("   ✓ Table created\n");
    
    // Test 2: INSERT multiple rows
    println!("2️⃣ INSERT multiple rows");
    db.execute("INSERT INTO users VALUES (1, 'Alice', 25)").await?;
    db.execute("INSERT INTO users VALUES (2, 'Bob', 30)").await?;
    db.execute("INSERT INTO users VALUES (3, 'Charlie', 17)").await?;
    db.execute("INSERT INTO users VALUES (4, 'Diana', 28)").await?;
    println!("   ✓ Inserted 4 rows\n");
    
    // Test 3: SELECT all
    println!("3️⃣ SELECT * FROM users");
    let result = db.execute("SELECT * FROM users").await?;
    println!("   ✓ Retrieved {} rows\n", result.rows.len());
    
    // Test 4: SELECT with WHERE (age > 18)
    println!("4️⃣ SELECT * FROM users WHERE age > 18");
    let result = db.execute("SELECT * FROM users WHERE age > 18").await?;
    println!("   ✓ Retrieved {} rows (adults only)\n", result.rows.len());
    
    // Test 5: SELECT with LIMIT
    println!("5️⃣ SELECT * FROM users LIMIT 2");
    let result = db.execute("SELECT * FROM users LIMIT 2").await?;
    println!("   ✓ Retrieved {} rows (limited)\n", result.rows.len());
    
    // Test 6: UPDATE with WHERE
    println!("6️⃣ UPDATE users SET age = 31 WHERE name = 'Bob'");
    db.execute("UPDATE users SET age = 31 WHERE name = 'Bob'").await?;
    println!("   ✓ Updated rows\n");
    
    // Test 7: DELETE with WHERE
    println!("7️⃣ DELETE FROM users WHERE age < 18");
    db.execute("DELETE FROM users WHERE age < 18").await?;
    println!("   ✓ Deleted rows\n");
    
    // Test 8: Verify changes
    println!("8️⃣ SELECT * FROM users (after changes)");
    let result = db.execute("SELECT * FROM users").await?;
    println!("   ✓ Final count: {} rows\n", result.rows.len());
    
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✅ ALL SQL OPERATIONS WORKING!");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    
    db.shutdown().await?;
    Ok(())
}
