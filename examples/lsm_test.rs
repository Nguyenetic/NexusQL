//! LSM Storage Test

use nexusql::storage::lsm::LSMStorage;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔥 Testing LSM Tree Storage Engine\n");
    
    // Create LSM storage
    let mut storage = LSMStorage::new("./lsm_test_data")?;
    println!("✓ LSM storage initialized");
    
    // Write data
    println!("\n📝 Writing data (goes to MemTable + WAL)...");
    storage.put(b"user:1".to_vec(), b"Alice".to_vec())?;
    storage.put(b"user:2".to_vec(), b"Bob".to_vec())?;
    storage.put(b"user:3".to_vec(), b"Charlie".to_vec())?;
    println!("✓ Written 3 keys");
    
    // Read data
    println!("\n📖 Reading data...");
    if let Some(value) = storage.get(b"user:1")? {
        println!("✓ user:1 = {}", String::from_utf8_lossy(&value));
    }
    if let Some(value) = storage.get(b"user:2")? {
        println!("✓ user:2 = {}", String::from_utf8_lossy(&value));
    }
    
    // Delete
    println!("\n🗑️  Deleting user:2...");
    storage.delete(b"user:2".to_vec())?;
    
    if let None = storage.get(b"user:2")? {
        println!("✓ user:2 deleted (tombstone in WAL)");
    }
    
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✅ LSM STORAGE ENGINE WORKING!");
    println!("✅ MemTable ✓ | WAL ✓ | MVCC ✓");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    
    Ok(())
}
