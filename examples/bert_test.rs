//! Test BERT Encoder with ONNX

use nexusql::engine::colbert::bert_onnx::BertEncoder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🤖 Testing BERT Encoder with ONNX\n");
    
    let mut encoder = BertEncoder::new(128);
    
    // Test 1: Simple encoding (fallback)
    println!("Test 1: Simple encoding (no model)");
    let embeddings = encoder.encode("hello world")?;
    println!("  ✓ Shape: {:?}", embeddings.shape());
    println!("  ✓ Tokens: {}, Dimensions: {}\n", embeddings.shape()[0], embeddings.shape()[1]);
    
    // Test 2: Try to load ONNX model (will fail if not present, that's OK)
    println!("Test 2: ONNX model loading");
    match encoder.load_model("models/bert.onnx", "models/tokenizer.json").await {
        Ok(_) => {
            println!("  ✓ BERT model loaded!");
            
            let embeddings = encoder.encode("machine learning")?;
            println!("  ✓ Real BERT embeddings: {:?}", embeddings.shape());
        },
        Err(e) => {
            println!("  ⚠ Model not found (expected): {}", e);
            println!("  ℹ To use real BERT:");
            println!("    1. Download BERT ONNX model");
            println!("    2. Place in ./models/ directory");
            println!("    3. Run again");
        }
    }
    
    println!("\n✅ BERT encoder ready!");
    println!("✅ Can use simple embeddings now");
    println!("✅ Can load real BERT when model available");
    
    Ok(())
}
