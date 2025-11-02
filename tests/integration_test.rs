//! Integration tests for NexusSQL

use nexusql::{NexusSQL, Value};

#[tokio::test]
async fn test_initialization() {
    let db = NexusSQL::new("./test.nexus").await.unwrap();
    let engines = db.list_engines();
    
    assert!(engines.contains(&"default".to_string()));
    assert!(engines.contains(&"colbert".to_string()));
    assert!(engines.contains(&"hdc".to_string()));
    assert_eq!(engines.len(), 3);
    
    db.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_basic_query() {
    let db = NexusSQL::new("./test.nexus").await.unwrap();
    
    let result = db.execute("SELECT * FROM test").await.unwrap();
    
    assert!(result.rows.len() > 0);
    
    db.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_colbert_routing() {
    let db = NexusSQL::new("./test.nexus").await.unwrap();
    
    let result = db.execute("SELECT * FROM docs WHERE content MATCHES 'test' USING colbert").await.unwrap();
    
    assert!(result.rows.len() > 0);
    
    db.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_hdc_routing() {
    let db = NexusSQL::new("./test.nexus").await.unwrap();
    
    let result = db.execute("SELECT CLASSIFY(data) USING hdc").await.unwrap();
    
    assert!(result.rows.len() > 0);
    
    db.shutdown().await.unwrap();
}
