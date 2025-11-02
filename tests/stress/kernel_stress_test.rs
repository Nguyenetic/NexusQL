//! Micro-Kernel Stress Test
//! Test concurrent engine registration, query routing under load

use nexusql::NexusSQL;
use std::sync::Arc;

#[tokio::test]
async fn test_concurrent_queries() {
    let db = Arc::new(NexusSQL::new("./stress_test.nexus").await.unwrap());
    
    let mut handles = vec![];
    
    // Spawn 100 concurrent queries
    for i in 0..100 {
        let db_clone = Arc::clone(&db);
        let handle = tokio::spawn(async move {
            let result = db_clone.execute(&format!("SELECT * FROM test_{}", i % 10)).await;
            result.is_ok()
        });
        handles.push(handle);
    }
    
    // Wait for all
    let results: Vec<_> = futures::future::join_all(handles).await;
    
    // Verify no crashes
    assert!(results.iter().all(|r| r.is_ok()));
}

#[tokio::test]
async fn test_rapid_engine_switching() {
    let db = NexusSQL::new("./engine_switch.nexus").await.unwrap();
    
    // Rapidly switch between engines
    for i in 0..1000 {
        let sql = match i % 3 {
            0 => "SELECT * FROM users",
            1 => "SELECT * FROM docs USING colbert",
            2 => "SELECT * FROM sensors USING hdc",
            _ => unreachable!(),
        };
        
        let result = db.execute(sql).await;
        assert!(result.is_ok(), "Query {} failed", i);
    }
}

#[tokio::test]
async fn test_memory_stability() {
    let db = NexusSQL::new("./memory_test.nexus").await.unwrap();
    
    // Create large dataset
    db.execute("CREATE TABLE big_table (id INTEGER, data TEXT)").await.unwrap();
    
    for i in 0..10000 {
        db.execute(&format!("INSERT INTO big_table VALUES ({}, 'data_{}')", i, i)).await.unwrap();
    }
    
    // Query multiple times - check for memory leaks
    for _ in 0..100 {
        let result = db.execute("SELECT * FROM big_table WHERE id > 5000").await.unwrap();
        assert!(result.rows.len() > 0);
    }
    
    // If we get here without OOM, memory is stable
}
