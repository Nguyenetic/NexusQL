# 📚 Key Learnings from Production Databases

## From DuckDB (Query Optimizer)

### Optimizer Architecture
- **20+ optimization rules** (constant folding, expression simplification, etc.)
- **Pushdown optimization** (filter, projection, aggregate, limit)
- **Join order optimization** (cost-based)
- **Statistics propagation** for better estimates

### Key Techniques to Implement:
1. **Filter Pushdown** - Push WHERE to storage before fetching
2. **Projection Pushdown** - Only read needed columns
3. **Constant Folding** - Evaluate `WHERE 1+1=2` at compile time
4. **Expression Simplification** - `age > 18 AND age > 20` → `age > 20`

---

## From RocksDB (Storage Engine)

### Write-Ahead Log (WAL)
- **Log records** with sequence numbers
- **Batch commits** (group multiple writes)
- **Recovery** via log replay
- **Checkpointing** for efficiency

### Transactions (MVCC)
- **Snapshot isolation** using sequence numbers
- **WritePrepared** - Two-phase commit
- **Commit cache** for fast lookups
- **Lock management** (point locks + range locks)

### Key Techniques:
1. **LSM Tree** - Log-Structured Merge tree for fast writes
2. **Memtable** - In-memory write buffer
3. **SST Files** - Sorted String Tables on disk
4. **Compaction** - Background merging of levels

---

## From ClickHouse (Columnar Storage)

### Performance Optimizations
- **Lazy projection** - Don't read unnecessary columns
- **Sparse serialization** - Compress columns with many defaults
- **Data skipping indices** - Skip irrelevant data blocks
- **Low cardinality** - Efficient storage for repeated values

### Query Execution
- **Vectorized execution** - Process data in batches
- **Pipeline execution** - Stream processing
- **Parallel execution** - Multi-threaded query processing

---

## What to Apply to NexusSQL

### Priority 1 (This Week):
1. ✅ **Filter pushdown** in default engine
   - Evaluate WHERE before loading all data
   
2. ✅ **Projection pushdown**
   - Only load requested columns

3. ✅ **Constant folding** in parser
   - Simplify expressions at parse time

### Priority 2 (Week 2-3):
4. **LSM-style storage** instead of B+ tree
   - Better for write-heavy workloads
   - Simpler than B+ tree
   
5. **WAL implementation** from RocksDB patterns
   - Sequence numbers
   - Batch commits
   - Recovery logic

### Priority 3 (Week 4+):
6. **Columnar storage** from ClickHouse
   - Better for analytics
   - Compress columns independently
   
7. **Vectorized execution**
   - Process rows in batches
   - SIMD optimizations

---

## Revised Architecture Based on Research

```rust
// NexusSQL should use:

// 1. Storage: LSM Tree (like RocksDB)
pub struct LSMStorage {
    memtable: MemTable,          // In-memory writes
    wal: WriteAheadLog,          // Durability
    sst_files: Vec<SSTFile>,     // On-disk sorted files
    compaction: CompactionEngine, // Background merging
}

// 2. Execution: Vectorized (like ClickHouse)
pub struct VectorizedExecutor {
    batch_size: usize,  // Process 1024 rows at a time
    // SIMD-friendly operations
}

// 3. Optimizer: Rule-based + Cost-based (like DuckDB)
pub struct Optimizer {
    rules: Vec<Box<dyn OptimizationRule>>,
    statistics: Statistics,
    cost_model: CostModel,
}
```

---

## Candle Status

Still compiling... Once ready, we'll have real BERT!

**Next:** Apply these learnings to NexusSQL architecture
