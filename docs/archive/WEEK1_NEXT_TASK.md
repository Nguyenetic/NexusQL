# 📋 Week 1-2 Next Task: Real BERT Inference

## Current Status
- ✅ Foundation complete
- ✅ All tests passing (21/21)
- ✅ HNSW working with best practices
- 🔄 Switching to ONNX Runtime (compiling...)

## Week 1-2 Goal: BERT Inference

### Tasks Remaining:
1. 🔄 Load BERT model with ONNX Runtime (compiling now)
2. ⏳ Implement token embedding extraction
3. ⏳ L2 normalization
4. ⏳ Project 768→128 dimensions
5. ⏳ Integrate with ColBERT engine
6. ⏳ Test semantic search end-to-end

### Success Criteria:
```rust
let encoder = BertEncoder::from_onnx("model.onnx", "tokenizer.json").await?;
let embeddings = encoder.encode("machine learning")?;
assert_eq!(embeddings.shape(), [2, 128]);  // 2 tokens, 128 dims
```

## Why ONNX Instead of Candle?

**Candle:** Still stuck compiling dependencies
**ONNX:** Mature, stable, fast compilation
**Result:** Same performance, faster to implement

**Building now...** 🚀
