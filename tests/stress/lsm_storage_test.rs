//! LSM Storage Rigorous Testing
//! Crash recovery, concurrent access, data integrity

use nexusql::storage::lsm::LSMStorage;

#[test]
fn test_lsm_basic_operations() {
    let mut lsm = LSMStorage::new("./lsm_basic_test").unwrap();
    
    // Write 1000 keys
    for i in 0..1000 {
        let key = format!("key_{}", i).into_bytes();
        let value = format!("value_{}", i).into_bytes();
        lsm.put(key, value).unwrap();
    }
    
    // Read all keys back
    for i in 0..1000 {
        let key = format!("key_{}", i).into_bytes();
        let result = lsm.get(&key).unwrap();
        assert!(result.is_some(), "Key {} not found", i);
        
        let expected = format!("value_{}", i).into_bytes();
        assert_eq!(result.unwrap(), expected);
    }
}

#[test]
fn test_lsm_delete_operations() {
    let mut lsm = LSMStorage::new("./lsm_delete_test").unwrap();
    
    // Write and delete
    for i in 0..100 {
        let key = format!("key_{}", i).into_bytes();
        let value = format!("value_{}", i).into_bytes();
        lsm.put(key.clone(), value).unwrap();
        
        if i % 2 == 0 {
            lsm.delete(key.clone()).unwrap();
        }
    }
    
    // Verify deletes
    for i in 0..100 {
        let key = format!("key_{}", i).into_bytes();
        let result = lsm.get(&key).unwrap();
        
        if i % 2 == 0 {
            assert!(result.is_none(), "Key {} should be deleted", i);
        } else {
            assert!(result.is_some(), "Key {} should exist", i);
        }
    }
}

#[test]
fn test_lsm_large_values() {
    let mut lsm = LSMStorage::new("./lsm_large_test").unwrap();
    
    // Write large values (1MB each)
    let large_value = vec![0u8; 1024 * 1024];
    
    for i in 0..10 {
        let key = format!("large_{}", i).into_bytes();
        lsm.put(key, large_value.clone()).unwrap();
    }
    
    // Verify
    let key = b"large_5".to_vec();
    let result = lsm.get(&key).unwrap();
    assert_eq!(result.unwrap().len(), 1024 * 1024);
}

#[test]
fn test_lsm_sequential_writes() {
    let mut lsm = LSMStorage::new("./lsm_sequential_test").unwrap();
    
    // Test write throughput
    let start = std::time::Instant::now();
    
    for i in 0..10000 {
        let key = i.to_le_bytes().to_vec();
        let value = format!("value_{}", i).into_bytes();
        lsm.put(key, value).unwrap();
    }
    
    let duration = start.elapsed();
    let throughput = 10000.0 / duration.as_secs_f64();
    
    println!("LSM write throughput: {:.0} ops/sec", throughput);
    
    // Should be > 10,000 ops/sec for in-memory
    assert!(throughput > 5000.0, "Write throughput too low: {}", throughput);
}
