# NexusQL System Architecture

## Overview

NexusQL is a hybrid database system that combines traditional SQL capabilities with advanced vector search and machine learning features. The architecture is designed for high performance, modularity, and PostgreSQL compatibility.

---

## System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                      Client Applications                     │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                     SQL Interface Layer                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ SQL Parser   │  │  Validator   │  │ Type Checker │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                   Query Optimizer                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ Logical      │  │ Physical     │  │ Cost-Based   │      │
│  │ Optimizer    │  │ Planner      │  │ Estimator    │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                  Execution Router                            │
│  Routes queries to appropriate engine based on workload      │
└────────────────────┬────────────────────────────────────────┘
                     │
         ┌───────────┼───────────┬───────────────┐
         ▼           ▼           ▼               ▼
    ┌─────────┐ ┌─────────┐ ┌─────────┐   ┌─────────┐
    │ Default │ │ ColBERT │ │   HDC   │   │  Future │
    │  SQL    │ │ Semantic│ │Hyperveć │   │ Engines │
    │ Engine  │ │  Search │ │  Engine │   │   ...   │
    └────┬────┘ └────┬────┘ └────┬────┘   └────┬────┘
         │           │           │             │
         └───────────┴───────────┴─────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                   Storage Layer                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  MemTable    │  │     WAL      │  │  LSM Tree    │      │
│  │  (Active)    │  │  (Durability)│  │  (SST Files) │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │   B-Tree     │  │    HNSW      │  │     GIN      │      │
│  │   Index      │  │  Vector Index│  │  FTS Index   │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────────────────────────────────────────┘
         │           │           │             │
         └───────────┴───────────┴─────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                 Transaction Manager                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │     MVCC     │  │ Lock Manager │  │  Savepoints  │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────────────────────────────────────────┘
```

---

## Core Components

### 1. SQL Interface Layer

**Purpose:** Parse and validate SQL queries

**Components:**
- **SQL Parser:** Converts SQL text into Abstract Syntax Tree (AST)
- **Validator:** Checks syntax correctness and semantic validity
- **Type Checker:** Ensures type safety and schema compliance

**Key Files:**
- `src/query/parser.rs` - SQL parsing logic
- `src/query/ast.rs` - AST data structures
- `src/schema/validator.rs` - Schema validation

**Technology Decisions:**
- Hand-written recursive descent parser for performance
- Zero-copy parsing where possible
- Comprehensive error messages with line/column info

---

### 2. Query Optimizer

**Purpose:** Transform queries into optimal execution plans

**Logical Optimizer (Based on DuckDB):**
- Constant folding (`WHERE 1+1=2` → `WHERE true`)
- Expression simplification
- Predicate pushdown
- Projection pruning
- Join reordering

**Physical Planner:**
- Access method selection (index scan vs. table scan)
- Join strategy selection (hash, merge, nested loop)
- Parallel execution planning

**Cost-Based Estimator:**
- Uses table statistics (row count, distinct values, histograms)
- Estimates query cost (CPU + I/O)
- Selects lowest cost plan

**Key Files:**
- `src/optimizer/mod.rs` - Optimization rules
- `src/optimizer/rules.rs` - Individual optimization rules
- `src/optimizer/pushdown.rs` - Pushdown optimizations
- `src/engine/advanced_optimizer.rs` - Advanced optimizations

---

### 3. Multi-Engine Architecture

**Purpose:** Route queries to specialized engines for optimal performance

#### Default SQL Engine
- Standard SQL queries (SELECT, INSERT, UPDATE, DELETE)
- JOINs (hash, merge, nested loop)
- Aggregations (GROUP BY, window functions)
- Subqueries and CTEs

**Key Files:**
- `src/engine/default.rs` - Default engine implementation
- `src/engine/executor.rs` - Query execution
- `src/engine/aggregation.rs` - Aggregate functions
- `src/window/mod.rs` - Window functions

#### ColBERT Semantic Search Engine
- Vector similarity search
- Semantic text matching
- Late interaction mechanism
- MaxSim scoring

**Key Files:**
- `src/engine/colbert/mod.rs` - ColBERT engine
- `src/engine/colbert/embeddings.rs` - Text embeddings
- `src/engine/colbert/maxsim.rs` - MaxSim scoring

#### HDC Hypervector Engine
- High-dimensional computing
- Fast classification and pattern matching
- Holographic representations

**Key Files:**
- `src/engine/hdc/mod.rs` - HDC engine
- `src/engine/hdc/hypervector.rs` - Hypervector operations

---

### 4. Storage Layer

**Architecture:** LSM-Tree inspired by RocksDB

#### Components:

**MemTable (In-Memory Buffer):**
- Active write buffer (SkipList or BTreeMap)
- Sorted for fast lookups
- Flushed to disk when full (typically 64MB)

**Write-Ahead Log (WAL):**
- Durability guarantee for uncommitted data
- Sequential writes for performance
- Used for crash recovery
- Checkpointed periodically

**SST Files (Sorted String Tables):**
- Immutable on-disk files
- Multiple levels (Level 0, 1, 2, ...)
- Compaction merges levels
- Block-based format with bloom filters

**Key Files:**
- `src/storage/lsm/memtable.rs` - In-memory write buffer
- `src/storage/lsm/wal.rs` - Write-ahead log
- `src/storage/lsm/sst.rs` - SST file format
- `src/storage/lsm/mod.rs` - LSM tree coordinator

---

### 5. Index Subsystem

#### B-Tree Index
- General-purpose index for ordered data
- Point queries, range scans
- O(log n) lookup time

**Implementation:**
- `src/index/btree.rs`

#### HNSW (Hierarchical Navigable Small World) Index
- Vector similarity search
- Approximate nearest neighbor (ANN)
- Multi-layer graph structure
- Sub-linear search time

**Implementation:**
- `src/index/hnsw.rs`

#### GIN (Generalized Inverted Index)
- Full-text search
- Array and JSON containment queries
- Inverted posting lists

**Implementation:**
- Coming in Week 11

#### GiST (Generalized Search Tree)
- Geometric data (points, polygons)
- Range types
- Extensible framework

**Implementation:**
- Coming in Week 11

#### BRIN (Block Range Index)
- Very large tables (billions of rows)
- Min/max values per block range
- Minimal space overhead

**Implementation:**
- Coming in Week 11

---

### 6. Transaction Manager

**Purpose:** ACID compliance with high concurrency

#### MVCC (Multi-Version Concurrency Control)
- Each transaction sees a consistent snapshot
- No read locks
- Writers don't block readers

**Implementation:**
- Each row has transaction ID and version
- Snapshot isolation by default
- Serializable isolation for strict consistency

#### Lock Manager
- Row-level locking
- Deadlock detection
- Lock timeouts

#### Savepoints
- Partial rollback within transactions
- Nested savepoints supported

**Key Files:**
- `src/transaction/mod.rs` - Transaction coordinator
- `src/transaction/mvcc.rs` - MVCC implementation
- `src/transaction/wal.rs` - Transaction logging

---

### 7. Security System

#### Authentication
- User management (CREATE USER, ALTER USER, DROP USER)
- Password hashing with bcrypt (work factor 12)
- Session management with JWTs or UUIDs

**Implementation:**
- `src/auth/mod.rs`

#### Authorization (RBAC)
- Role hierarchy (role inheritance)
- GRANT/REVOKE privileges
- Table-level and column-level permissions
- Database-level permissions

**Implementation:**
- `src/auth/mod.rs`

#### Row-Level Security (RLS)
- Per-table policies
- USING clause (visibility)
- WITH CHECK clause (insert/update validation)
- Per-role policies

**Implementation:**
- `src/auth/rls.rs`

---

### 8. Caching System

**Purpose:** 49x speedup for repeated queries

**Components:**
- Query result cache (LRU eviction)
- Prepared statement cache
- Plan cache
- Metadata cache (schemas, statistics)

**Key Features:**
- Automatic cache invalidation on writes
- Configurable cache size
- Cache hit/miss statistics

**Implementation:**
- `src/cache/mod.rs`

---

### 9. Advanced Features

#### Triggers
- BEFORE/AFTER timing
- FOR EACH ROW / FOR EACH STATEMENT
- Multiple triggers per event
- Trigger ordering

**Implementation:**
- `src/trigger/mod.rs`

#### Views
- Virtual tables
- Updatable views (WITH CHECK OPTION)
- Recursive views
- Materialized views (with refresh)

**Implementation:**
- `src/view/mod.rs`
- `src/materialized_view/mod.rs`

#### Stored Procedures
- PL/NexusQL procedural language
- Variables and control flow (IF, LOOP, WHILE)
- Exception handling
- IN/OUT/INOUT parameters

**Implementation:**
- `src/procedure/mod.rs`
- `src/procedure/plnexus.rs`

#### Sequences
- Auto-incrementing integers
- SERIAL and BIGSERIAL types
- nextval(), currval(), setval()
- Concurrent sequence access

**Implementation:**
- `src/sequence/mod.rs`

---

## Data Flow Examples

### Example 1: Simple SELECT Query

```sql
SELECT name, price FROM products WHERE category = 'Electronics' ORDER BY price DESC LIMIT 10;
```

**Flow:**
1. **Parser** → AST with SELECT node
2. **Validator** → Check that 'products' table exists, columns are valid
3. **Optimizer** →
   - Push down filter (`category = 'Electronics'`)
   - Prune projection (only read 'name', 'price', 'category')
   - Use index on 'category' if available
4. **Executor** →
   - Index scan or table scan with filter
   - Sort by price DESC
   - Apply LIMIT 10
5. **Storage** → Read from MemTable + SST files
6. **Result** → Return 10 rows to client

---

### Example 2: INSERT with Transaction

```sql
BEGIN;
INSERT INTO orders (customer_id, total) VALUES (123, 99.99);
UPDATE customers SET balance = balance - 99.99 WHERE id = 123;
COMMIT;
```

**Flow:**
1. **BEGIN** → Create transaction with unique ID
2. **INSERT** →
   - Parser → AST
   - Validator → Check constraints (NOT NULL, FOREIGN KEY)
   - Executor → Add row to MemTable
   - WAL → Write insert record
   - Transaction → Mark as uncommitted
3. **UPDATE** →
   - Parser → AST
   - Validator → Check constraints
   - Executor → Update row in MemTable (new version)
   - WAL → Write update record
4. **COMMIT** →
   - Lock Manager → Acquire commit lock
   - MVCC → Mark all changes as visible
   - WAL → Write commit record, fsync
   - Cache → Invalidate affected queries
   - Lock Manager → Release locks

---

### Example 3: Semantic Search with ColBERT

```sql
SELECT * FROM documents WHERE SEMANTIC_SEARCH(content, 'machine learning', 10);
```

**Flow:**
1. **Parser** → Detect SEMANTIC_SEARCH function
2. **Router** → Route to ColBERT engine
3. **ColBERT Engine** →
   - Tokenize query: ["machine", "learning"]
   - Generate embeddings (768-dim vectors)
   - HNSW search for similar document vectors
   - MaxSim scoring for late interaction
   - Return top 10 documents
4. **Result** → Return ranked documents

---

## Performance Optimizations

### 1. Vectorized Execution (ClickHouse-style)
- Process rows in batches of 1024
- SIMD-friendly operations
- Reduced function call overhead

### 2. Parallel Execution
- Partition scans across CPU cores
- Parallel aggregations
- Parallel sorts

### 3. Query Result Caching
- LRU cache with 49x speedup
- Automatic invalidation
- Cache statistics

### 4. Index Acceleration
- B-Tree: 1578x speedup for point queries
- HNSW: Sub-linear vector search
- Bloom filters in SST files

### 5. Memory Management
- Arena allocation for query execution
- Zero-copy where possible
- Lazy materialization

---

## Design Principles

### 1. PostgreSQL Compatibility
- Aim for wire protocol compatibility
- Support PostgreSQL SQL dialect
- Similar system catalogs and views

### 2. Performance First
- In-memory optimized
- Vectorized execution
- Aggressive caching

### 3. Modularity
- Pluggable storage engines
- Pluggable execution engines
- Clear component boundaries

### 4. Production Ready
- ACID transactions
- Crash recovery
- Comprehensive testing (500+ tests target)
- Monitoring and metrics

### 5. Modern Architecture
- Written in Rust for safety and performance
- Async I/O with Tokio
- Lock-free data structures where possible

---

## Technology Stack

**Language:** Rust 1.70+
- Memory safety without garbage collection
- Zero-cost abstractions
- Excellent concurrency primitives

**Core Dependencies:**
- `tokio` - Async runtime
- `serde` - Serialization
- `tracing` - Logging and instrumentation
- `bcrypt` - Password hashing
- `dashmap` - Concurrent HashMap

**Storage:**
- Custom LSM-tree implementation
- Memory-mapped files for SST
- O_DIRECT for WAL

**Networking (Future):**
- PostgreSQL wire protocol
- HTTP/REST API
- gRPC for replication

---

## Scalability Strategy

### Vertical Scaling (Current)
- Multi-core parallelism
- Memory-mapped I/O
- NUMA-aware allocations (future)

### Horizontal Scaling (v1.3.0+)
- Sharding by hash or range
- Distributed query execution
- Raft consensus for metadata

---

## Monitoring and Observability

**Metrics:**
- Query latency (p50, p95, p99)
- Throughput (QPS)
- Cache hit rates
- Transaction commit rate
- Replication lag

**Logs:**
- Structured logging with tracing
- Query logs (slow queries)
- Error logs with stack traces
- Audit logs (DDL, permissions)

**Statistics:**
- Table statistics (row count, size)
- Index statistics (usage, size)
- Lock contention
- I/O statistics

---

## Comparison to Other Databases

### vs. PostgreSQL
- **Faster** for analytical queries (vectorized execution)
- **Simpler** architecture (no vacuum, simpler MVCC)
- **Modern** codebase (Rust vs. C)
- **Less mature** (decades of production hardening)

### vs. SQLite
- **Faster** for concurrent access (MVCC vs. file locking)
- **Better** for analytics (vectorized execution)
- **More features** (replication, parallel queries)
- **Larger** binary size

### vs. DuckDB
- **Similar** vectorized execution
- **Different** focus (OLTP + OLAP vs. pure OLAP)
- **Additional** vector search and ML features

---

## Future Architecture Enhancements

### v1.1.0 - Columnar Storage
- Hybrid row/column storage
- Better compression ratios
- Faster analytical queries

### v1.2.0 - Query Compilation
- JIT compile hot queries to machine code
- LLVM or Cranelift backend
- 10x speedup for tight loops

### v1.3.0 - Distributed Architecture
- Sharding with consistent hashing
- Distributed transactions (2PC)
- Global query optimizer

### v2.0.0 - AI-Native Features
- Native embedding generation
- Automatic index selection with ML
- Query optimization with reinforcement learning

---

## References

**Research Sources:**
- PostgreSQL documentation and source code
- DuckDB optimizer design
- RocksDB LSM tree implementation
- ClickHouse vectorized execution
- SQLite testing methodology

**Papers:**
- "Access Path Selection in a Relational Database" (Selinger et al.)
- "Efficiently Compiling Efficient Query Plans" (Neumann)
- "The Design and Implementation of Modern Column-Oriented Database Systems" (Abadi et al.)

---

**Last Updated:** November 2, 2025
**Version:** 0.1.0
**Architecture Status:** v1.0.0 design complete, 62% implemented
