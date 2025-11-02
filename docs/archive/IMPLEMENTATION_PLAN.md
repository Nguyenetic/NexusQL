# 🎯 NexusSQL From-Scratch Implementation Plan

## Current Status: Week 0 Complete ✅

What we have working:
- Micro-kernel architecture ✅
- 3 engine skeletons ✅
- Basic routing ✅
- Persistence layer ✅
- Tests passing ✅

---

## Week 1-2: REAL BERT INFERENCE (Starting Now)

### Day 1-2: Get Candle Working
- ✅ Fix dependency issues (in progress)
- Download small BERT model (distilbert-base-uncased, 66M params)
- Test model loading
- Run single inference

### Day 3-4: Token Embeddings
- Extract hidden states from BERT
- Implement token-level embedding extraction
- L2 normalization
- Dimension projection (768 → 128)

### Day 5-7: ColBERT Integration
- Replace mock embeddings with real BERT
- Batch processing (multiple docs)
- Memory optimization
- Benchmark (target: <50ms inference)

### Success Criteria Week 2:
```rust
let encoder = BertEncoder::from_pretrained("distilbert-base-uncased").await?;
let embeddings = encoder.encode("machine learning")?;
// embeddings.shape() = [2 tokens, 128 dimensions]
assert_eq!(embeddings.shape(), [2, 128]);
```

---

## Week 3-4: VECTOR INDEX (HNSW)

### Day 8-10: HNSW Core Algorithm
Build from scratch:

```rust
struct HNSWIndex {
    layers: Vec<Layer>,
    entry_point: usize,
    m: usize,           // Max connections
    ef_construction: usize,
    ef_search: usize,
}

impl HNSWIndex {
    fn insert(&mut self, vector: Vec<f32>) -> usize {
        // 1. Select layer for new node
        let layer = self.select_layer();
        
        // 2. Find nearest neighbors
        let neighbors = self.search_layer(vector, layer, ef_construction);
        
        // 3. Connect to M nearest
        self.connect_neighbors(new_id, neighbors);
        
        // 4. Prune if needed
        self.prune_connections(new_id);
    }
    
    fn search(&self, query: Vec<f32>, k: usize) -> Vec<(usize, f32)> {
        // Greedy search from entry point
        // Navigate down layers
        // Return top-k nearest
    }
}
```

### Day 11-14: Optimization & Persistence
- Save/load index to disk
- Batch insertion optimization
- Parallel search
- Memory-mapped files

### Success Criteria Week 4:
- Index 10k vectors in <10 seconds
- Search (k=10) in <5ms
- Recall@10 > 90%
- Persistence working

---

## Week 5-6: FULL SQL PARSER

### Day 15-18: Integrate sqlparser-rs

```rust
use sqlparser::parser::Parser;
use sqlparser::dialect::GenericDialect;
use sqlparser::ast::*;

pub struct NexusParser {
    dialect: GenericDialect,
}

impl NexusParser {
    pub fn parse(&self, sql: &str) -> Result<Vec<Statement>> {
        let ast = Parser::parse_sql(&self.dialect, sql)?;
        
        // Convert sqlparser AST to our AST
        ast.into_iter()
            .map(|stmt| self.convert_statement(stmt))
            .collect()
    }
    
    fn convert_statement(&self, stmt: sqlparser::ast::Statement) -> Result<Statement> {
        match stmt {
            sqlparser::ast::Statement::Query(q) => self.convert_query(q),
            sqlparser::ast::Statement::Insert { .. } => self.convert_insert(stmt),
            // ... handle all statement types
        }
    }
}
```

### Day 19-21: WHERE Clause Execution

```rust
pub fn evaluate_where(row: &Row, expr: &Expr) -> bool {
    match expr {
        Expr::BinaryOp { left, op, right } => {
            let left_val = evaluate_expr(row, left);
            let right_val = evaluate_expr(row, right);
            compare(left_val, op, right_val)
        },
        Expr::Column(name) => row.get(name).is_some(),
        Expr::Literal(val) => true,
        // ... all expression types
    }
}
```

### Success Criteria Week 6:
```sql
-- All these work:
SELECT name, age FROM users WHERE age > 18 AND city = 'NYC';
SELECT COUNT(*) FROM orders GROUP BY product_id;
SELECT * FROM a JOIN b ON a.id = b.user_id;
```

---

## Week 7-10: STORAGE ENGINE (B+ Tree + WAL)

### Week 7: B+ Tree Implementation

```rust
struct BPlusTree {
    root: NodeId,
    order: usize,  // Max keys per node
}

struct Node {
    keys: Vec<Key>,
    children: Vec<NodeId>,  // For internal
    values: Vec<Value>,     // For leaf
    next: Option<NodeId>,   // Leaf chain
}

impl BPlusTree {
    fn insert(&mut self, key: Key, value: Value) {
        // 1. Find leaf node
        let leaf = self.find_leaf(key);
        
        // 2. Insert into leaf
        leaf.insert(key, value);
        
        // 3. Split if overflow
        if leaf.is_full() {
            self.split_node(leaf);
        }
    }
    
    fn search(&self, key: Key) -> Option<Value> {
        let leaf = self.find_leaf(key);
        leaf.get(key)
    }
}
```

### Week 8: Write-Ahead Log

```rust
struct WAL {
    log_file: File,
    lsn: u64,  // Log Sequence Number
}

impl WAL {
    fn append(&mut self, record: LogRecord) -> u64 {
        let lsn = self.lsn;
        self.lsn += 1;
        
        // Write to log
        let bytes = bincode::serialize(&record)?;
        self.log_file.write_all(&bytes)?;
        self.log_file.sync_all()?;
        
        lsn
    }
    
    fn recover(&self) -> Vec<LogRecord> {
        // Read all log records
        // Replay to restore state
    }
}
```

### Week 9-10: Transactions (MVCC)

```rust
struct Transaction {
    id: TransactionId,
    snapshot: Snapshot,
    read_set: HashSet<Key>,
    write_set: HashMap<Key, Value>,
}

impl Transaction {
    fn read(&mut self, key: Key) -> Option<Value> {
        // 1. Check write set
        if let Some(val) = self.write_set.get(&key) {
            return Some(val.clone());
        }
        
        // 2. Read from snapshot
        self.read_from_snapshot(key, self.snapshot)
    }
    
    fn write(&mut self, key: Key, value: Value) {
        self.write_set.insert(key, value);
    }
    
    fn commit(&mut self) -> Result<()> {
        // 1. Write to WAL
        // 2. Validate (no conflicts)
        // 3. Apply writes
        // 4. Release locks
    }
}
```

---

## REALISTIC EFFORT ESTIMATE

### If Building EVERYTHING from scratch:
- **BERT Inference:** 2 weeks (with Candle)
- **Vector Index:** 2 weeks (HNSW implementation)
- **SQL Parser:** 1 week (using sqlparser-rs)
- **Storage Engine:** 4 weeks (B+ Tree + WAL + MVCC)
- **Query Optimizer:** 2 weeks
- **GNN Engine:** 3 weeks
- **HDC Full:** 1 week
- **Integration + Testing:** 3 weeks

**TOTAL: 18 weeks (4.5 months) solo, full-time**

---

## SMART SHORTCUTS

### Use Existing Libraries:

1. **SQL:** sqlparser-rs ✅ (save 3 weeks)
2. **BERT:** Candle ✅ (save 6 weeks vs implementing from scratch)
3. **Vector Index:** hnswlib-rs or usearch (save 2 weeks)
4. **Storage:** Use sled + add B+ tree wrapper (save 3 weeks)

**TOTAL: 4-6 weeks with libraries** ⭐ RECOMMENDED

---

## CURRENT TASK: Fix Candle Compilation

The build is running. While it compiles, should we:

1. **Wait for Candle** (5-10 min) then implement real BERT
2. **Use simpler ML library** (e.g., ort for ONNX models)
3. **Start on Vector Index** (don't need BERT yet)
4. **Start on SQL Parser** (independent task)

**What's your preference?** 🎯
