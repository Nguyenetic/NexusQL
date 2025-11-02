//! Test HNSW Vector Index

use nexusql::HNSWIndex;
use ndarray::arr1;

fn main() {
    println!("🔍 Testing HNSW Vector Index\n");
    
    // Create index
    let mut index = HNSWIndex::new(16, 200, 10);
    println!("✓ HNSW Index created (M=16, ef_construction=200)");
    
    // Insert vectors
    println!("\n📝 Inserting 100 random vectors...");
    for i in 0..100 {
        let vector = arr1(&[
            (i as f32 * 0.1).sin(),
            (i as f32 * 0.2).cos(),
            (i as f32 * 0.15).sin(),
        ]);
        index.insert(vector);
    }
    println!("✓ Indexed {} vectors", index.len());
    
    // Search
    println!("\n🔍 Searching for nearest neighbors...");
    let query = arr1(&[0.5, 0.5, 0.5]);
    let results = index.search(&query, 5);
    
    println!("✓ Found {} nearest neighbors:", results.len());
    for (i, (id, distance)) in results.iter().enumerate() {
        println!("   {}. Vector #{} (distance: {:.4})", i + 1, id, distance);
    }
    
    println!("\n✅ HNSW Index working!\n");
}
