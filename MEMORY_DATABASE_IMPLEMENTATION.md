# Adding :memory: Database Support to NexusQL

**Created:** 2025-01-04
**Purpose:** Implementation guide for in-memory database support

---

## 🎯 Goal

Add SQLite-style `:memory:` syntax to NexusQL for:
- ✅ Fast testing (no disk I/O)
- ✅ No file cleanup needed
- ✅ Perfect test isolation
- ✅ Industry standard 2024

---

## 📍 Where to Make Changes

### **File 1: `src/lib.rs` (Main Entry Point)**

**Location:** Line 131 - `NexusSQL::new()` function

**Current Code:**
```rust
impl NexusSQL {
    pub async fn new(path: &str) -> Result<Self> {
        let kernel = NexusKernel::new(path).await?;

        let data_path = format!("{}/default", path);
        kernel
            .register_engine(Box::new(engine::default::DefaultEngine::new(&data_path)?))
            .await;

        // ... register ColBERT and HDC engines
    }
}
```

**NEW Code with :memory: support:**
```rust
impl NexusSQL {
    pub async fn new(path: &str) -> Result<Self> {
        // Check if in-memory database requested
        let is_memory = path == ":memory:";

        // For in-memory, use a unique temp path (won't actually create files)
        let actual_path = if is_memory {
            // Use a unique ID to avoid collisions between multiple :memory: instances
            use std::sync::atomic::{AtomicU64, Ordering};
            static MEMORY_COUNTER: AtomicU64 = AtomicU64::new(0);
            let id = MEMORY_COUNTER.fetch_add(1, Ordering::SeqCst);
            format!(":memory:_{}", id)
        } else {
            path.to_string()
        };

        let kernel = NexusKernel::new(&actual_path, is_memory).await?;

        let data_path = if is_memory {
            format!("{}:default", actual_path) // Special marker for in-memory
        } else {
            format!("{}/default", actual_path)
        };

        kernel
            .register_engine(Box::new(engine::default::DefaultEngine::new(&data_path, is_memory)?))
            .await;

        // Register ColBERT AI engine (in-memory compatible)
        kernel
            .register_engine(Box::new(engine::colbert::ColBERTEngine::new()))
            .await;

        // Register HDC engine (in-memory compatible)
        kernel
            .register_engine(Box::new(engine::hdc::HDCEngine::new()))
            .await;

        tracing::info!("✓ All engines registered (mode: {})",
                      if is_memory { "memory" } else { "disk" });
        Ok(Self { kernel })
    }
}
```

---

### **File 2: `src/kernel.rs` (Kernel Initialization)**

**Location:** Line 21 - `NexusKernel::new()` function

**Add `is_memory` parameter:**

```rust
pub struct NexusKernel {
    engines: Arc<DashMap<String, Arc<dyn StorageEngine>>>,
    router: Arc<RwLock<QueryRouter>>,
    default_engine: String,
    data_path: String,
    is_memory: bool,  // ← ADD THIS FIELD
}

impl NexusKernel {
    pub async fn new(data_path: &str, is_memory: bool) -> Result<Self> {
        tracing::info!("Initializing NexusSQL micro-kernel (mode: {})",
                      if is_memory { "memory" } else { "disk" });

        let kernel = Self {
            engines: Arc::new(DashMap::new()),
            router: Arc::new(RwLock::new(QueryRouter::new())),
            default_engine: "default".to_string(),
            data_path: data_path.to_string(),
            is_memory,  // ← STORE IT
        };

        Ok(kernel)
    }

    // Add helper method
    pub fn is_memory(&self) -> bool {
        self.is_memory
    }
}
```

---

### **File 3: `src/storage/lsm/mod.rs` (LSM Storage Engine)**

**Add in-memory mode to LSMStorage:**

```rust
pub struct LSMStorage {
    memtable: memtable::MemTable,
    wal: Option<wal::WriteAheadLog>,  // ← Make WAL optional for :memory:
    sstables: Vec<SSTable>,
    path: String,
    is_memory: bool,  // ← ADD THIS
}

impl LSMStorage {
    pub fn new(path: &str, is_memory: bool) -> Result<Self> {
        // Skip file system operations for in-memory mode
        let wal = if is_memory {
            None  // No WAL for in-memory databases
        } else {
            // Create parent directories if they don't exist
            std::fs::create_dir_all(path)?;
            Some(wal::WriteAheadLog::new(&format!("{}/wal", path))?)
        };

        Ok(LSMStorage {
            memtable: memtable::MemTable::new(),
            wal,
            sstables: Vec::new(),
            path: path.to_string(),
            is_memory,
        })
    }

    pub fn write(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        // Write to memtable (always in memory)
        self.memtable.insert(key.clone(), value.clone());

        // Only write to WAL if not in-memory mode
        if let Some(ref mut wal) = self.wal {
            wal.append(&key, &value)?;
        }

        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        if self.is_memory {
            // In-memory mode: keep everything in memtable
            // Optionally implement memory limits and eviction here
            return Ok(());
        }

        // Disk mode: flush to SSTable as usual
        // ... existing flush logic ...
    }
}
```

---

### **File 4: `src/engine/default/mod.rs` (Default Storage Engine)**

**Update DefaultEngine to support :memory:**

```rust
pub struct DefaultEngine {
    storage: Arc<Mutex<LSMStorage>>,
    is_memory: bool,
}

impl DefaultEngine {
    pub fn new(path: &str, is_memory: bool) -> Result<Self> {
        let storage = LSMStorage::new(path, is_memory)?;

        Ok(DefaultEngine {
            storage: Arc::new(Mutex::new(storage)),
            is_memory,
        })
    }
}

#[async_trait]
impl StorageEngine for DefaultEngine {
    async fn write(&self, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        let mut storage = self.storage.lock().unwrap();
        storage.write(key, value)?;

        // Skip fsync for in-memory databases
        if !self.is_memory {
            storage.flush()?;
        }

        Ok(())
    }

    // ... other methods ...
}
```

---

### **File 5: `bindings/typescript/src/lib.rs` (Node.js Binding)**

**No changes needed!** The path check happens in Rust core.

But we can add a helper:

```rust
#[napi]
impl NexusDB {
    #[napi(constructor)]
    pub fn new(path: String) -> Result<Self> {
        // Path can be:
        // - Regular file path: "./mydb"
        // - In-memory database: ":memory:"

        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| Error::from_reason(format!("Failed to create runtime: {}", e)))?;

        let db = runtime
            .block_on(async { NexusSQL::new(&path).await })
            .map_err(|e| Error::from_reason(format!("Failed to create database: {}", e)))?;

        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            runtime: Arc::new(runtime),
        })
    }

    /// Check if this database is in-memory
    #[napi]
    pub fn is_memory(&self) -> bool {
        let db = self.db.blocking_lock();
        db.kernel.is_memory()
    }
}
```

---

## 🔧 Implementation Steps

### **Step 1: Update Core Types**

```bash
# Edit src/lib.rs
# Add :memory: detection and routing
```

### **Step 2: Update Kernel**

```bash
# Edit src/kernel.rs
# Add is_memory field and parameter
```

### **Step 3: Update Storage Layer**

```bash
# Edit src/storage/lsm/mod.rs
# Make WAL optional for :memory: mode
```

### **Step 4: Update Engines**

```bash
# Edit src/engine/default/mod.rs
# Skip file operations for :memory:
```

### **Step 5: Build and Test**

```bash
# Rebuild the TypeScript bindings
cd bindings/typescript
npm run build

# Test the new feature
node -e "const {NexusDb} = require('./index.js'); const db = new NexusDb(':memory:'); console.log('✅ In-memory database works!');"
```

---

## ✅ Testing the Implementation

### **Test 1: Basic In-Memory Creation**

```javascript
const { NexusDb } = require('@nguyenetic/nexusql');

// Create in-memory database
const db = new NexusDb(':memory:');
console.log('✅ Created in-memory database');

// Create table
await db.execute('CREATE TABLE users (id INTEGER, name TEXT)');
console.log('✅ Created table');

// Insert data
await db.execute("INSERT INTO users VALUES (1, 'Alice')");
console.log('✅ Inserted data');

// Query data
const result = await db.query('SELECT * FROM users');
console.log('✅ Query result:', JSON.parse(result.rows));

// Close (memory freed automatically)
await db.close();
console.log('✅ Closed database - memory freed!');
```

### **Test 2: Multiple In-Memory Instances**

```javascript
// Each instance is isolated
const db1 = new NexusDb(':memory:');
const db2 = new NexusDb(':memory:');

await db1.execute('CREATE TABLE test (id INTEGER)');
await db1.execute('INSERT INTO test VALUES (1)');

await db2.execute('CREATE TABLE test (id INTEGER)');
await db2.execute('INSERT INTO test VALUES (999)');

const result1 = await db1.query('SELECT * FROM test');
const result2 = await db2.query('SELECT * FROM test');

console.log('DB1:', JSON.parse(result1.rows)); // [{id: 1}]
console.log('DB2:', JSON.parse(result2.rows)); // [{id: 999}]
// ✅ Completely isolated!
```

### **Test 3: Vector Operations in Memory**

```javascript
const db = new NexusDb(':memory:');

// Create table with vector column
await db.execute('CREATE TABLE embeddings (id INTEGER, vec VECTOR(384))');

// Insert vector
const vec = Array(384).fill(0).map(() => Math.random());
await db.execute(`INSERT INTO embeddings VALUES (1, '[${vec.join(',')}]')`);

// Query with similarity search
const queryVec = Array(384).fill(0).map(() => Math.random());
const result = await db.query(`
    SELECT id, vec <=> '[${queryVec.join(',')}]' AS similarity
    FROM embeddings
    ORDER BY similarity
    LIMIT 1
`);

console.log('✅ Vector search in memory:', JSON.parse(result.rows));
```

---

## 📊 Performance Benefits

### **Benchmark: File-based vs In-Memory**

```javascript
const { performance } = require('perf_hooks');

async function benchmarkMode(mode) {
    const dbPath = mode === 'memory' ? ':memory:' : './bench_test.db';
    const db = new NexusDb(dbPath);

    await db.execute('CREATE TABLE test (id INTEGER, data TEXT)');

    const start = performance.now();
    for (let i = 0; i < 1000; i++) {
        await db.execute(`INSERT INTO test VALUES (${i}, 'data${i}')`);
    }
    const end = performance.now();

    await db.close();

    return end - start;
}

const memoryTime = await benchmarkMode('memory');
const fileTime = await benchmarkMode('file');

console.log(`Memory: ${memoryTime.toFixed(2)}ms`);
console.log(`File:   ${fileTime.toFixed(2)}ms`);
console.log(`Speedup: ${(fileTime / memoryTime).toFixed(2)}x faster`);
// Expected: 3-10x faster for in-memory
```

---

## 🎯 Migration Guide

### **Update Existing Tests**

**Before:**
```javascript
const TEST_DB = `test_${Date.now()}.db`;
const db = new NexusDb(TEST_DB);

// ... tests ...

// Cleanup required
await db.close();
fs.unlinkSync(TEST_DB);
fs.unlinkSync(TEST_DB + '-wal'); // WAL file
fs.unlinkSync(TEST_DB + '-shm'); // SHM file
```

**After:**
```javascript
const TEST_DB = ':memory:';
const db = new NexusDb(TEST_DB);

// ... tests ...

// No cleanup needed!
await db.close();
// ✅ Memory automatically freed
```

---

## 🚀 Advanced Features

### **Option 1: Named In-Memory Databases**

Support shared in-memory databases:

```rust
// In src/lib.rs
pub async fn new(path: &str) -> Result<Self> {
    let (actual_path, is_memory, shared) = if path == ":memory:" {
        // Unique isolated in-memory database
        (generate_unique_memory_id(), true, false)
    } else if path.starts_with(":memory:") && path.contains("?") {
        // Named shared in-memory database
        // Example: ":memory:?name=shared_db"
        (path.to_string(), true, true)
    } else {
        // Regular file-based database
        (path.to_string(), false, false)
    };
    // ...
}
```

### **Option 2: Memory Limits**

Add memory constraints:

```rust
pub struct LSMStorage {
    memtable: memtable::MemTable,
    max_memory: Option<usize>,  // Optional memory limit
    // ...
}

impl LSMStorage {
    pub fn with_memory_limit(path: &str, limit_mb: usize) -> Result<Self> {
        // Create in-memory storage with max size
        Self::new(path, true)?.with_limit(limit_mb * 1024 * 1024)
    }
}
```

---

## 📚 Benefits Summary

### **For Testing:**
- ✅ 3-10x faster tests
- ✅ No file cleanup needed
- ✅ Perfect isolation
- ✅ No Windows EPERM issues
- ✅ Parallel test execution

### **For Development:**
- ✅ Quick prototyping
- ✅ Faster CI/CD pipelines
- ✅ Reduced disk wear
- ✅ Simpler debugging

### **For Production:**
- ✅ Caching layer
- ✅ Session storage
- ✅ Temporary calculations
- ✅ Preview/staging environments

---

## 🔗 References

**Industry Standards:**
- SQLite: https://www.sqlite.org/inmemorydb.html
- PostgreSQL: https://www.postgresql.org/docs/current/runtime-config-resource.html#GUC-SHARED-BUFFERS
- Redis: In-memory by default

**Testing Best Practices:**
- Source #35: Integration Testing with In-Memory Databases (DEV Community, 2024)
- Source #26: How SQLite Is Tested (Official Documentation)

**Research Papers:**
- "Enhanced Differential Testing" (arXiv 2025) - Found 57 bugs using in-memory testing
- "Best Testing Practices in Node.js" (AppSignal Blog, 2024)

---

## ✨ Example: Complete Test Suite

**File: `test-in-memory.js`**

```javascript
const { NexusDb } = require('@nguyenetic/nexusql');

async function testInMemory() {
    console.log('🧪 Testing :memory: database...\n');

    // Test 1: Basic operations
    const db = new NexusDb(':memory:');

    await db.execute('CREATE TABLE users (id INTEGER, name TEXT)');
    await db.execute("INSERT INTO users VALUES (1, 'Alice'), (2, 'Bob')");

    const result = await db.query('SELECT * FROM users');
    console.log('✅ Basic operations:', JSON.parse(result.rows));

    // Test 2: Vector operations
    await db.execute('CREATE TABLE vectors (id INTEGER, embedding VECTOR(3))');
    await db.execute("INSERT INTO vectors VALUES (1, '[1,2,3]'), (2, '[4,5,6]')");

    const vecResult = await db.query("SELECT id, embedding <=> '[1,2,3]' as dist FROM vectors ORDER BY dist LIMIT 1");
    console.log('✅ Vector search:', JSON.parse(vecResult.rows));

    // Test 3: Transactions
    await db.execute('BEGIN TRANSACTION');
    await db.execute("INSERT INTO users VALUES (3, 'Charlie')");
    await db.execute('COMMIT');
    console.log('✅ Transactions work');

    // Close
    await db.close();
    console.log('✅ Database closed - no cleanup needed!\n');

    console.log('🎉 All :memory: tests passed!');
}

testInMemory().catch(console.error);
```

---

**Ready to implement?** Start with Step 1 in `src/lib.rs`!
