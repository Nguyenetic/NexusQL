# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

NexusQL is a high-performance, PostgreSQL-compatible database engine written in Rust with brain-inspired AI technologies (HDC, ColBERT) and advanced vector search capabilities. The project includes:
- **Core Rust library** (`src/`) - Database engine with ACID transactions, MVCC, multiple storage engines
- **TypeScript/Node.js bindings** (`bindings/typescript/`) - N-API native module for JavaScript
- **Python bindings** (`bindings/python/`) - PyO3-based bindings (planned)

## Build and Test Commands

### Rust Core Library

```bash
# Build library (production)
cargo build --release

# Build library (debug, faster compilation)
cargo build

# Run all tests
cargo test

# Run tests for specific module
cargo test --lib cache::tests
cargo test --lib trigger::tests
cargo test --lib aggregate::tests

# Run benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench quick_bench
cargo bench --bench performance_benchmarks
```

### TypeScript Bindings

```bash
cd bindings/typescript

# Build native module + TypeScript definitions
npm run build

# Build only native module (Rust → .node file)
npm run build:native

# Build only TypeScript definitions
npm run build:ts

# Run tests
npm test

# Build for specific platform
npm run build:native -- --target x86_64-pc-windows-msvc
```

### Important Build Notes

- **CRITICAL**: Tests currently fail to compile due to missing type imports (57 errors). See `.github/ISSUE_TEST_IMPORTS.md` for details.
- The **library builds successfully** (`cargo build` works), but `cargo test` fails.
- TypeScript bindings build and work correctly with `npm run build`.

## Architecture

### Core Components

**Storage Layer** (`src/storage/`)
- `LSMStorage` - Log-structured merge-tree with memtable, WAL, and SSTables
- Supports both disk-based and in-memory (`:memory:`) databases
- WAL is optional for in-memory mode

**Storage Engines** (`src/engine/`)
- `DefaultEngine` - Standard relational storage (LSM-based)
- `ColBERTEngine` - Neural semantic search with token-level embeddings
- `HDCEngine` - Hyperdimensional computing for brain-inspired AI
- Engines are registered with the kernel and routed via SQL hints

**Transaction System** (`src/transaction/`)
- MVCC (Multi-Version Concurrency Control)
- Write-Ahead Logging (WAL) for durability
- ACID compliance with snapshot isolation

**Query Processing**
- `QueryParser` (`src/query/parser.rs`) - Uses `sqlparser-rs` with PostgreSQL dialect
- `Optimizer` (`src/optimizer/`) - Cost-based query optimization
- `Planner` (`src/planner/`) - Query planning with cost estimation
- `Evaluator` (`src/query/evaluator.rs`) - Query execution

**Indexing** (`src/index/`)
- B-Tree - Standard ordered index
- HNSW - Hierarchical Navigable Small World for vector search (1578× speedup)
- GIN - Generalized Inverted Index for arrays/JSON/full-text
- GiST - Generalized Search Tree for spatial/geometric data
- BRIN - Block Range Index for sequential data (100-1000× compression)

**Vector Search** (`src/vector/`)
- Supports L2 (Euclidean), cosine, and inner product distance metrics
- HNSW index for approximate nearest neighbor search
- 384-dimensional vectors (compatible with all-MiniLM-L6-v2 embeddings)

### Key Architectural Patterns

**Kernel-Based Design** (`src/kernel.rs`)
- `NexusKernel` acts as a microkernel routing queries to registered engines
- Multiple engines can coexist (default, ColBERT, HDC)
- Engine selection via SQL hints: `USING ENGINE colbert`

**In-Memory Database Support**
- Special path `:memory:` creates an in-memory database
- Uses atomic counter for unique instance IDs (`:memory:_0`, `:memory:_1`, etc.)
- WAL is disabled for in-memory mode
- See `MEMORY_DATABASE_IMPLEMENTATION.md` for implementation details

**Type System** (`src/types.rs`)
- Core `Value` enum: `Null`, `Integer`, `Float`, `Text`, `Blob`, `Boolean`, `Vector`
- `OrderedFloat<f64>` wrapper for float comparisons
- Vector distance methods built into `Value` type

**Node.js Bindings Architecture** (`bindings/typescript/src/lib.rs`)
- Uses `napi-rs` for zero-copy native bindings
- Tokio async runtime wrapped for Node.js event loop compatibility
- Returns JSON strings from Rust (parsed in JavaScript) to avoid complex type marshalling
- `Arc<Mutex<NexusSQL>>` for thread-safe database access

## Common Development Workflows

### Fixing Test Compilation Errors

**Problem**: 57 test compilation errors due to missing imports in test modules.

**Files to fix**:
- `src/adaptive/mod.rs`
- `src/cache/mod.rs`
- `src/materialized_view/mod.rs`
- `src/view/mod.rs`
- `src/cte/mod.rs`
- `src/schema/validator.rs`

**Solution**: Add imports to `#[cfg(test)] mod tests` blocks:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Value, DataType, Row, ResultSet};
    use crate::planner::PlanCost;

    // ... existing tests
}
```

See `.github/ISSUE_TEST_IMPORTS.md` for complete details.

### Adding Parameterized Query Support

**Context**: Currently requires manual string escaping (SQL injection risk). See `.github/ISSUE_PARAMETERIZED_QUERIES.md`.

**Implementation location**: `src/query/parser.rs`

**Required changes**:
1. Detect parameter placeholders (`?` or `:name`) in SQL
2. Add optional `params` argument to `execute()` and `query()` methods
3. Type mapping: JS array → `Vec<Value>`, JS object → `HashMap<String, Value>`
4. Bind parameters during query execution (bypass SQL parsing)

**TypeScript API example**:
```typescript
await db.execute(
  'INSERT INTO users (name, email) VALUES (?, ?)',
  ['Alice', 'alice@example.com']
);
```

### Working with Vector Search

**Embedding dimensions**: 384 (standard for all-MiniLM-L6-v2)

**Distance metrics**:
- `vector_distance(a, b)` - L2/Euclidean (default)
- `cosine_distance(a, b)` - Cosine similarity (recommended for sentence embeddings)
- `inner_product(a, b)` - Dot product (for normalized vectors)

**SQL syntax**:
```sql
-- Using operators
SELECT id, embedding <=> '[0.1,0.2,...]' as distance
FROM vectors
ORDER BY distance
LIMIT 10;

-- Using functions
SELECT id, cosine_distance(embedding, '[0.1,0.2,...]') as similarity
FROM vectors
ORDER BY similarity
LIMIT 10;
```

**TypeScript helper methods**:
```typescript
// High-level API
const results = await db.vectorSearch(
  'embeddings',
  queryVector,
  limit: 10,
  'cosine'
);

// Batch insert (10-100× faster)
await db.batchInsertVectors('table', vectors, metadataArray);
```

### Test Database Patterns

**Always use in-memory databases for tests**:
```javascript
const TEST_DB = ':memory:';  // NOT './test.db'
const db = new NexusDb(TEST_DB);

// ... tests ...

await db.close();  // Memory freed automatically, no cleanup needed
```

**Benefits**:
- 3-10× faster than disk
- No Windows EPERM/file locking issues
- Perfect isolation between tests
- No file cleanup required

See `TESTING_AND_SECURITY_REFERENCES.md` for 37+ research citations supporting this approach.

## Project Structure Highlights

```
NexusQL/
├── src/
│   ├── lib.rs              # Main entry point, NexusSQL struct
│   ├── kernel.rs           # Query routing microkernel
│   ├── types.rs            # Core Value enum and data types
│   ├── query/
│   │   ├── parser.rs       # SQL parsing (sqlparser-rs)
│   │   ├── evaluator.rs    # Query execution
│   │   └── ast.rs          # Abstract syntax tree
│   ├── storage/
│   │   └── lsm/            # LSM storage with WAL
│   ├── engine/
│   │   ├── default.rs      # Standard storage engine
│   │   ├── colbert/        # Neural semantic search
│   │   └── hdc/            # Hyperdimensional computing
│   ├── transaction/        # MVCC and WAL
│   ├── index/              # B-Tree, HNSW, GIN, GiST, BRIN
│   └── vector/             # Vector operations and HNSW
├── bindings/
│   └── typescript/
│       ├── src/lib.rs      # N-API bindings
│       ├── index.js        # JavaScript exports
│       └── test-*.js       # Integration tests
└── docs/                   # Additional documentation
```

## Important Implementation Details

### Security Considerations

**SQL Injection**: Currently vulnerable because queries use string concatenation. Parameterized queries must be implemented before production use.

**Workaround in tests** (until parameterized queries available):
```javascript
// Escape single quotes
const escaped = content.replace(/'/g, "''");
await db.execute(`INSERT INTO table VALUES ('${escaped}')`);
```

**DO NOT USE** `JSON.stringify()` as protection - it doesn't prevent SQL injection.

### Windows-Specific Issues

**File Locking**: Windows holds file handles longer than Unix systems. The in-memory database (`:memory:`) avoids these issues entirely.

**WAL/SHM Files**: SQLite creates `.db-wal` and `.db-shm` files. These are automatically handled by the storage layer.

### TypeScript Type Definitions

Generated automatically from N-API bindings. Key types:
- `NexusDB` - Main database class
- `QueryResult` - Contains `rows` (JSON string), `row_count`, `fields`
- `Transaction` - Transaction handle with `query()`, `commit()`, `rollback()`

**Important**: `QueryResult.rows()` returns a JSON **string**, not an array. Parse it:
```typescript
const result = await db.query('SELECT * FROM users');
const rows = JSON.parse(result.rows());
```

## Performance Characteristics

- **HNSW Index**: 1578× faster than sequential scan for vector search
- **Query Cache**: 49× speedup on repeated queries (LRU eviction)
- **Batch Insert**: 10-100× faster than individual inserts
- **BRIN Index**: 100-1000× smaller than B-Tree for sequential data
- **In-Memory Mode**: 3-10× faster than disk-based for small datasets

## Documentation References

- `README.md` - Project overview and feature list
- `ARCHITECTURE.md` - Detailed architecture documentation
- `NEURAL_TECHNOLOGIES.md` - HDC, ColBERT, HNSW deep dive
- `TESTING_AND_SECURITY_REFERENCES.md` - 37+ research citations for best practices
- `MEMORY_DATABASE_IMPLEMENTATION.md` - In-memory database implementation guide
- `COMPILATION_ERRORS.md` - Test compilation error analysis
- `.github/ISSUE_PARAMETERIZED_QUERIES.md` - Parameterized query implementation spec
- `.github/ISSUE_TEST_IMPORTS.md` - Test import fix instructions

## Known Issues

1. **Test compilation fails** - 57 errors from missing type imports (fix documented in `.github/ISSUE_TEST_IMPORTS.md`)
2. **No parameterized queries** - SQL injection risk (spec in `.github/ISSUE_PARAMETERIZED_QUERIES.md`)
3. **Limited error messages** - Parser errors could be more descriptive
4. **No query timeout** - Long-running queries can't be cancelled

## Version Information

- **Current Version**: 2.5.0
- **Rust Edition**: 2021
- **Node.js Compatibility**: >=16.0.0
- **Key Dependencies**:
  - `tokio` - Async runtime
  - `sqlparser` - SQL parsing
  - `napi-rs` - Node.js bindings
  - `ndarray` - Vector operations
  - `ort` - ONNX runtime for ColBERT
