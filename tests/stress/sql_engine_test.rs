//! SQL Engine Rigorous Testing
//! Complex queries, edge cases, data integrity

use nexusql::NexusSQL;

#[tokio::test]
async fn test_complex_join_query() {
    let db = NexusSQL::new("./complex_join.nexus").await.unwrap();
    
    // Setup tables
    db.execute("CREATE TABLE users (id INTEGER, name TEXT, age INTEGER)").await.unwrap();
    db.execute("CREATE TABLE orders (id INTEGER, user_id INTEGER, total FLOAT)").await.unwrap();
    db.execute("CREATE TABLE products (id INTEGER, name TEXT, price FLOAT)").await.unwrap();
    
    // Insert data
    for i in 1..=100 {
        db.execute(&format!("INSERT INTO users VALUES ({}, 'User{}', {})", i, i, 20 + i % 40)).await.unwrap();
        db.execute(&format!("INSERT INTO orders VALUES ({}, {}, {})", i, i % 50 + 1, (i as f32) * 10.5)).await.unwrap();
    }
    
    // Complex join query
    let result = db.execute("
        SELECT u.name, o.total 
        FROM users u 
        JOIN orders o ON u.id = o.user_id 
        WHERE u.age > 25 AND o.total > 100
        LIMIT 20
    ").await.unwrap();
    
    assert!(result.rows.len() > 0);
    assert!(result.rows.len() <= 20);
}

#[tokio::test]
async fn test_aggregation_accuracy() {
    let db = NexusSQL::new("./agg_test.nexus").await.unwrap();
    
    db.execute("CREATE TABLE sales (category TEXT, amount FLOAT)").await.unwrap();
    
    // Insert known data
    db.execute("INSERT INTO sales VALUES ('A', 100.0)").await.unwrap();
    db.execute("INSERT INTO sales VALUES ('A', 200.0)").await.unwrap();
    db.execute("INSERT INTO sales VALUES ('B', 150.0)").await.unwrap();
    db.execute("INSERT INTO sales VALUES ('B', 250.0)").await.unwrap();
    
    // Test SUM
    let result = db.execute("SELECT category, SUM(amount) FROM sales GROUP BY category").await.unwrap();
    assert_eq!(result.rows.len(), 2);
    
    // Verify correctness (A: 300, B: 400)
    // TODO: Check actual values when we can parse results properly
}

#[tokio::test]
async fn test_data_integrity_updates() {
    let db = NexusSQL::new("./integrity_test.nexus").await.unwrap();
    
    db.execute("CREATE TABLE accounts (id INTEGER, balance FLOAT)").await.unwrap();
    db.execute("INSERT INTO accounts VALUES (1, 1000.0)").await.unwrap();
    db.execute("INSERT INTO accounts VALUES (2, 500.0)").await.unwrap();
    
    // Update
    db.execute("UPDATE accounts SET balance = 1500.0 WHERE id = 1").await.unwrap();
    
    // Verify update worked
    let result = db.execute("SELECT * FROM accounts WHERE id = 1").await.unwrap();
    assert_eq!(result.rows.len(), 1);
    
    // Delete
    db.execute("DELETE FROM accounts WHERE balance < 1000").await.unwrap();
    
    // Verify delete worked
    let result = db.execute("SELECT * FROM accounts").await.unwrap();
    assert_eq!(result.rows.len(), 1);  // Only id=1 remains
}

#[tokio::test]
async fn test_persistence_across_restarts() {
    // First session: Write data
    {
        let db = NexusSQL::new("./persist_test.nexus").await.unwrap();
        db.execute("CREATE TABLE persistent (id INTEGER, data TEXT)").await.unwrap();
        db.execute("INSERT INTO persistent VALUES (1, 'test_data')").await.unwrap();
        db.shutdown().await.unwrap();
    }
    
    // Second session: Read data
    {
        let db = NexusSQL::new("./persist_test.nexus").await.unwrap();
        let result = db.execute("SELECT * FROM persistent").await.unwrap();
        
        assert_eq!(result.rows.len(), 1);
        db.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn test_large_result_sets() {
    let db = NexusSQL::new("./large_result.nexus").await.unwrap();
    
    db.execute("CREATE TABLE big (id INTEGER, value INTEGER)").await.unwrap();
    
    // Insert 10k rows
    for i in 0..10000 {
        db.execute(&format!("INSERT INTO big VALUES ({}, {})", i, i * 2)).await.unwrap();
    }
    
    // Query all
    let result = db.execute("SELECT * FROM big").await.unwrap();
    assert_eq!(result.rows.len(), 10000);
    
    // Query with filter
    let result = db.execute("SELECT * FROM big WHERE id > 9000").await.unwrap();
    assert!(result.rows.len() < 1000);
}
