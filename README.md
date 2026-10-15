<div align="center">

# 🧠 NexusQL

**Next-Generation AI-Native Database with Brain-Inspired Computing**

[![npm version](https://badge.fury.io/js/%40nguyenetic%2Fnexusql.svg)](https://www.npmjs.com/package/@nguyenetic/nexusql)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Node.js](https://img.shields.io/badge/node-%3E%3D16.0.0-brightgreen.svg)](https://nodejs.org/)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)]()
[![Tests](https://img.shields.io/badge/tests-237%20passing-brightgreen.svg)]()

[Getting Started](#-quick-start) • [Installation](#-installation) • [API Reference](#-api-reference) • [Migration Guide](#-migration-guide) • [Examples](#-examples) • [Changelog](#-changelog)

---

### ⚡ What Makes NexusQL Different

🧠 **Brain-Inspired Computing** • 🔍 **Neural Semantic Search** • 📈 **HNSW Vector Index** • 🗄️ **PostgreSQL Compatible** • 🚀 **1578× Faster**

</div>

---

## 🌟 Overview

NexusQL is a high-performance, PostgreSQL-compatible database engine written in Rust that integrates **cutting-edge neural and brain-inspired technologies** including:

- **Hyperdimensional Computing (HDC)** - One-shot learning with 100× energy efficiency
- **ColBERT Late Interaction** - Token-level semantic search (100× faster than BERT)
- **Small-corpus SQL** - Model-free short-name ranking and persistent one-shot
  classifiers ([usage](docs/guides/small-corpus-ai.md))
- **HNSW Vector Index** - Logarithmic search with 99%+ recall
- **Durable disk SQL transactions** - Checksummed WAL, atomic commit/rollback, FULL sync by default; one shared SQL session (not SQL MVCC)
- **Parameterized Queries** - Built-in SQL injection prevention

### 🎯 Perfect For

- 🤖 **AI/ML Applications** - Embedding storage and semantic search
- 📊 **Analytics Platforms** - Advanced aggregations and window functions
- 🔒 **Secure Applications** - RBAC, RLS, and parameterized queries
- ⚡ **High-Performance Apps** - 1578× index speedup, 49× cache speedup
- 🌐 **Multi-Tenant Systems** - Row-level security and partitioning
- 📱 **Mobile Apps** - Native Android support via JNI bindings (core engine, no ML deps)

---

## 🚀 Features

<details open>
<summary><b>🧠 Neural & Brain-Inspired AI</b> (Unique to NexusQL)</summary>

### Hyperdimensional Computing (HDC)
- **One-shot learning** - Learn from single examples
- **100× energy efficient** vs deep learning
- **Holographic representations** mimicking neural activity
- **10,000+ dimensional** hypervectors

### ColBERT Late Interaction
- **Token-level embeddings** with MaxSim operator
- **100× faster** than BERT cross-encoder
- **Fine-grained relevance** scoring
- **State-of-the-art** semantic search

### HNSW Vector Index
- **O(log N) search** complexity in high dimensions
- **99%+ recall** at low latency
- **Graph-based** multi-layer structure
- **1578× speedup** over sequential scan

📖 [Learn more about Neural Technologies](docs/architecture/neural-technologies.md)

</details>

<details>
<summary><b>🗄️ Core Database Features</b></summary>

- ✅ **Disk SQL transactions** - Committed-frame recovery and checkpointing; FULL sync by default. NORMAL/OFF weaken durability; bulk-mode autocommits require flush. See [durability guarantees and limits](docs/development/durability.md).
- ✅ **Parameterized Queries** - SQL injection prevention with `$1, $2` placeholders
- ✅ **Prepared Statements** - Compile once, execute many times
- ✅ **Schema System** - Tables, columns, constraints (PK, FK, UNIQUE, CHECK)
- ✅ **Authentication** - bcrypt hashing, RBAC, Row-Level Security (RLS)
- ✅ **Multi-Level Caching** - LRU eviction with 49× speedup

</details>

<details>
<summary><b>⚙️ Advanced SQL Features</b></summary>

### Indexing
- **B-Tree** - Standard ordered index
- **HNSW** - Vector similarity search (1578× faster)
- **GIN** - Arrays, JSON, full-text search
- **GiST** - Geometric/spatial data
- **FULLTEXT** - Keyword search with BM25 ranking, phrases, prefixes and snippets ([guide](docs/guides/full-text-search.md))
- **BRIN** - Time-series (100-1000× compression)

### Query Features
- **Aggregate Functions** - COUNT, SUM, AVG, MIN, MAX, STRING_AGG, ARRAY_AGG
- **Window Functions** - ROW_NUMBER, RANK, DENSE_RANK, LAG, LEAD
- **Joins** - INNER, OUTER, CROSS, NATURAL with 3 algorithms (Hash, Merge, Nested Loop)
- **CTEs** - WITH RECURSIVE with cycle detection
- **Subqueries** - Scalar, EXISTS, IN
- **Full-Text Search** - `col MATCH 'query'`, `bm25()`, `snippet()`, `highlight()` with FTS5-style queries and Unicode/CJK tokenization

### Data Organization
- **Table Partitioning** - Range, List, Hash with partition pruning (67% scan reduction)
- **Views** - Regular, materialized, updateable
- **Triggers** - BEFORE/AFTER/INSTEAD OF
- **Sequences** - Lock-free atomic auto-increment
- **Stored Procedures** - PL/NexusQL (PostgreSQL-compatible)

</details>

<details>
<summary><b>🏢 Enterprise Features</b></summary>

- ✅ **Connection Pooling** - Efficient resource management
- ✅ **Query Planner** - ANALYZE, EXPLAIN, cost estimation
- ✅ **VACUUM** - Dead tuple cleanup, visibility maps
- ✅ **Replication** - WAL streaming, pub/sub model
- ✅ **Monitoring** - Query statistics and profiling
- ✅ **JSON/JSONB** - Binary JSON with operators

</details>

---

## 📦 Installation

### Node.js / TypeScript

```bash
npm install @nguyenetic/nexusql
```

### Python (build from source)

```bash
cd bindings/python
python -m venv .venv
# Activate .venv (Windows: .venv\Scripts\activate; Unix: source .venv/bin/activate)
python -m pip install maturin pytest
maturin develop
pytest -q
```

The native binding supports parameterized `execute(sql, params)`, direct bulk
inserts, and WKT geometry values. See the [Python guide](bindings/python/README.md)
and [spatial SQL guide](docs/guides/geometry.md).

### Rust

```toml
[dependencies]
# Full-featured (default): includes HDC, ColBERT, HNSW vector search (pulls in ort + tokenizers)
nexusql = { git = "https://github.com/Nguyenetic/NexusQL.git" }

# Core only (no ML deps): sled-backed SQL + WAL transactions, no vector search.
# Required for mobile / constrained targets where `ort` and `tokenizers` can't build.
nexusql = { git = "https://github.com/Nguyenetic/NexusQL.git", default-features = false }
```

### Android (Native via JNI)

NexusQL runs natively on Android via the `bindings/android/` crate. The full NexusQL engine (sled LSM tree + NexusKernel + SQL executor) compiles for `aarch64-linux-android` when the `vector-search` feature is disabled. HDC / ColBERT / HNSW vector features remain on desktop only until their upstream deps (`ort`, `tokenizers`) gain Android NDK support.

```bash
# Install cargo-ndk
cargo install cargo-ndk
rustup target add aarch64-linux-android

# Set Android NDK path
export ANDROID_NDK_HOME=/path/to/ndk/26.1.10909125

# Cross-compile
cd bindings/android
cargo ndk -t arm64-v8a build --release
# Output: target/aarch64-linux-android/release/libnexusql_android.so
```

**Kotlin API** (from the Android library module in consumer apps):

```kotlin
val db = NexusQL.open(context.filesDir.resolve("app.nxql").absolutePath)
db.execute("CREATE TABLE IF NOT EXISTS kv (k TEXT PRIMARY KEY, v TEXT)")
db.execute("INSERT INTO kv VALUES ('hello', 'world')")
val rows = db.query("SELECT v FROM kv WHERE k = 'hello'")
db.close()
```

See `bindings/android/README.md` for full setup, consumer module gradle config, and packaging the `.so` into an Android app.

### Build from Source

```bash
git clone https://github.com/Nguyenetic/NexusQL.git
cd NexusQL
cargo build --release
```

---

## ⚡ Quick Start

### TypeScript / JavaScript

```typescript
import { NexusDb } from '@nguyenetic/nexusql';

// Create database
const db = new NexusDb('./myapp.db');

// Create table
await db.execute(`
  CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT UNIQUE
  )
`);

// Insert with parameterized query (prevents SQL injection)
await db.execute(
  'INSERT INTO users VALUES ($1, $2, $3)',
  [1, 'Alice', 'alice@example.com']
);

// Query data
const result = await db.query('SELECT * FROM users WHERE id = $1', [1]);
console.log(result.rows); // [{ id: 1, name: 'Alice', email: 'alice@example.com' }]

// Use transactions
const tx = await db.beginTransaction();
await tx.query('UPDATE users SET name = "Alice Smith" WHERE id = 1');
await tx.commit();

// Close database
await db.close();
```

### Prepared Statements (10× Faster)

```typescript
// Compile once, execute many times
const stmt = await db.prepare('INSERT INTO users VALUES ($1, $2, $3)');

for (let i = 1; i <= 1000; i++) {
  await stmt.execute(i, `User${i}`, `user${i}@example.com`);
}

await stmt.close();
```

### Vector Search

```typescript
// Create table with vector column
await db.execute(`
  CREATE TABLE documents (
    id INTEGER PRIMARY KEY,
    content TEXT,
    embedding VECTOR(384)
  )
`);

// Insert vectors with metadata
const embedding = [0.1, 0.2, 0.3, /* ... 384 dimensions */];
await db.insertVector('documents', embedding, JSON.stringify({
  id: 1,
  content: 'Machine learning tutorial'
}));

// Batch insert (100× faster)
const vectors = [...]; // Array of 1000 embeddings
const metadata = [...]; // Array of 1000 metadata objects
await db.batchInsertVectors('documents', vectors, metadata);

// Create HNSW index for fast similarity search
await db.createVectorIndex('documents', 'embedding', 'hnsw', JSON.stringify({
  m: 16,
  ef_construction: 64,
  distance: 'cosine'
}));

// Semantic search
const queryVector = [0.15, 0.25, 0.35, /* ... */];
const results = await db.query(`
  SELECT id, content, cosine_distance(embedding, $1) as similarity
  FROM documents
  ORDER BY similarity
  LIMIT 10
`, [JSON.stringify(queryVector)]);
```

### Rust

```rust
use nexusql::NexusSQL;

#[tokio::main]
async fn main() -> Result<()> {
    let db = NexusSQL::new("./myapp.db").await?;

    db.execute("CREATE TABLE users (id INT, name TEXT)").await?;
    db.execute("INSERT INTO users VALUES (1, 'Alice')").await?;

    let result = db.query("SELECT * FROM users").await?;
    println!("{:?}", result.rows);

    db.shutdown().await?;
    Ok(())
}
```

---

## 📊 Performance Benchmarks

| Operation | Performance | Comparison |
|-----------|-------------|------------|
| **Vector Search (HNSW)** | **1578× faster** | vs sequential scan |
| **Query Cache** | **49× speedup** | on repeated queries |
| **Batch Insert** | **10-100× faster** | vs individual inserts |
| **Prepared Statements** | **10× faster** | vs regular queries |
| **BRIN Index** | **100-1000× smaller** | vs B-Tree for time-series |
| **Partition Pruning** | **67% reduction** | in rows scanned |

---

## 📚 API Reference

<div align="center">

**[📖 Getting Started](#-quick-start)** • **[🔌 TypeScript API](#typescript-api)** • **[🦀 Rust API](#rust-api)** • **[🔄 Migration Guide](#-migration-guide)** • **[📚 Guides](#guides)** • **[🏗️ Architecture](#architecture)**

</div>

---

### TypeScript API

Complete Node.js/TypeScript API reference with interactive examples.

#### Core Classes
| Class | Description | Documentation |
|-------|-------------|---------------|
| **NexusDB** | Main database class | [API Docs](docs/api/typescript/CLASSES.md#nexusdb) |
| **Transaction** | Transaction handle; see [disk guarantees](docs/development/durability.md) | [API Docs](docs/api/typescript/CLASSES.md#transaction) |
| **PreparedStatement** | Compiled query with parameters | [API Docs](docs/api/typescript/CLASSES.md#preparedstatement) |

#### Key Methods
| Method | Purpose | Example |
|--------|---------|---------|
| `execute(sql)` | Run SQL without parameters | [Docs](docs/api/typescript/METHODS.md#execute) |
| `execute_with_params(sql, params)` | Parameterized queries (SQL injection safe) | [Docs](docs/api/typescript/METHODS.md#execute_with_params) |
| `query(sql)` | SELECT queries | [Docs](docs/api/typescript/METHODS.md#query) |
| `begin_transaction()` | Start transaction | [Docs](docs/api/typescript/METHODS.md#begin_transaction) |
| `vectorSearch(table, vector, limit)` | Semantic search with HNSW | [Docs](docs/api/typescript/METHODS.md#vectorsearch) |

📘 **[Complete TypeScript API Reference →](docs/api/typescript/README.md)**

---

### Rust API

Native Rust library with async support and zero-copy operations.

#### Core Types
| Type | Description | Documentation |
|------|-------------|---------------|
| **NexusSQL** | Main database struct | [docs.rs](https://docs.rs/nexusql/latest/nexusql/struct.NexusSQL.html) |
| **NexusError** | Type-safe error handling | [docs.rs](https://docs.rs/nexusql/latest/nexusql/enum.NexusError.html) |
| **Value** | SQL value types | [docs.rs](https://docs.rs/nexusql/latest/nexusql/types/enum.Value.html) |
| **Row** | Query result row | [docs.rs](https://docs.rs/nexusql/latest/nexusql/types/struct.Row.html) |
| **ResultSet** | Query results | [docs.rs](https://docs.rs/nexusql/latest/nexusql/types/struct.ResultSet.html) |

📘 **[Complete Rust API Reference →](https://docs.rs/nexusql/latest/nexusql/)**

---

### Guides

#### Security & Best Practices
- 🔒 [Parameterized Queries](docs/guides/parameterized-queries.md) - SQL injection prevention
- 🔐 [Authentication & Authorization](docs/guides/auth.md) - RBAC and row-level security
- ✅ [Error Handling](docs/guides/error-handling.md) - Type-safe error management

#### Data Operations
- 🔍 [Vector Search](docs/guides/vector-search.md) - Semantic search with HNSW
- 💾 [Transactions](docs/guides/transactions.md) - Usage patterns; [current disk durability contract](docs/development/durability.md)
- 📊 [Batch Operations](docs/guides/batch-operations.md) - 10-100× faster inserts
- [Full-Text Search](docs/guides/full-text-search.md) - BM25 keyword search, snippets, SQLite FTS5 migration

#### Performance
- ⚡ [Performance Optimization](docs/guides/performance.md) - Tuning and best practices
- 📈 [Indexing Strategies](docs/guides/indexing.md) - B-Tree, HNSW, GIN, BRIN
- 💾 [Caching](docs/guides/caching.md) - Query cache (49× speedup)

---

### Architecture

Deep-dive into NexusQL's internal design and neural technologies.

- 🏗️ [System Overview](ARCHITECTURE.md) - High-level architecture
- 🧠 [Neural Technologies](NEURAL_TECHNOLOGIES.md) - HDC, ColBERT, HNSW explained
- 💽 [Storage Engine](docs/architecture/storage-engine.md) - LSM tree and WAL
- 🔄 [Transaction System](docs/architecture/transaction-system.md) - MVCC implementation
- 🎯 [Query Optimizer](docs/architecture/optimizer.md) - Cost-based optimization

---

## 🔄 Migration Guide

<div align="center">

### Upgrading from v2.x to v3.x

**[📖 Complete Migration Guide →](MIGRATION.md)**

</div>

NexusQL v3.x introduces important API changes for better type safety and consistency. The migration guide provides:

- ✅ **Step-by-step migration checklist**
- 🔄 **Before/after code examples** for all breaking changes
- 🛠️ **Automated migration helpers** (deprecation warnings)
- 📚 **Common migration patterns** with real-world scenarios

**Key Changes:**
- `Database` → `NexusSQL` (type alias provided for compatibility)
- `Database::open()` → `NexusSQL::new()`
- `anyhow::Error` → `NexusError` (structured error types)
- `execute_params()` → `execute_with_params()`
- Row access: `row[i]` → `row.values[i]`

**Migration from other databases:**
- [SQLite Migration Guide](docs/guides/migration/from-sqlite.md)
- [PostgreSQL Migration Guide](docs/guides/migration/from-postgres.md)

---

## 📝 Changelog

**[View Complete Changelog →](bindings/typescript/CHANGELOG.md)**

### Latest Release: v3.2.0 (2025-01-07)

- ✨ **NEW:** CREATE INDEX support for B-Tree indexes
- ✨ **NEW:** Multi-column index parsing
- ✅ **274 tests passing** (100% success rate)
- 📚 Updated migration guide and API documentation

### Recent Releases

- **v3.1.0** - HNSW algorithm fixes, all tests passing
- **v3.0.0** - Parameterized queries, SQL injection prevention
- **v2.7.0** - API improvements (NexusSQL rename)

---

## 👨‍💻 Contributing

We welcome contributions! See our guides:

- 📖 [Contributing Guide](CONTRIBUTING.md) - How to contribute
- 🤝 [Code of Conduct](CODE_OF_CONDUCT.md) - Community standards
- 🔒 [Security Policy](SECURITY.md) - Reporting vulnerabilities
- 💬 [GitHub Discussions](https://github.com/Nguyenetic/NexusQL/discussions) - Ask questions

---

## 💡 Examples

### Basic CRUD Operations
```bash
cd bindings/typescript/examples
node basic-usage.js
```

### Parameterized Queries
```bash
node parameterized-queries.js
```

### Transactions
```bash
node transactions.js
```

### Vector Search
```bash
node vector-search.js
```

### Prepared Statements
```bash
node prepared-statements.js
```

### Rust Examples
```bash
cargo run --example week10_features_demo       # Triggers, views, sequences
cargo run --example week13_analytics_demo      # Aggregations, window functions
cargo run --example week11_advanced_indexes_demo # GIN, GiST, BRIN
cargo run --example week12_partitioning_demo   # Table partitioning
cargo run --example week14_joins_demo          # Join algorithms
```

---

## 🔒 Security

NexusQL provides comprehensive security features:

- ✅ **Parameterized Queries** - Prevent SQL injection with `$1, $2` placeholders
- ✅ **Prepared Statements** - Compile-time query validation
- ✅ **Authentication** - bcrypt password hashing (10+ rounds)
- ✅ **Authorization** - Role-Based Access Control (RBAC)
- ✅ **Row-Level Security** - Fine-grained access control
- ✅ **Input Validation** - Type checking and constraints

See [SECURITY.md](SECURITY.md) for security policy and best practices.

---

## 🧪 Testing

```bash
# Rust tests
cargo test                           # Run all tests
cargo test --lib module::tests       # Run specific module

# TypeScript tests
cd bindings/typescript
npm test                             # Run all tests
npm test -- --coverage               # With coverage

# Benchmarks
cargo bench
```

**Test Coverage:** 237 passing tests (229 unit + 8 integration)

---

## 🗺️ Roadmap

### ✅ Current Version: 3.0.0

- ✅ Parameterized queries with SQL injection prevention
- ✅ Prepared statements API
- ✅ WAL-backed disk SQL transactions; standalone MVCC API is separate
- ✅ Vector search with HNSW index
- ✅ Full-text search
- ✅ Advanced indexing (B-Tree, GIN, GiST, BRIN)
- ✅ Table partitioning
- ✅ Window functions and CTEs

### 🚧 Upcoming Features

- [ ] **v3.1.0** - Parameterized queries in transactions
- [ ] **v3.2.0** - Python bindings
- [ ] **v3.3.0** - GraphQL interface
- [ ] **v4.0.0** - Distributed database support

See [ROADMAP.md](docs/ROADMAP.md) for detailed roadmap.

---

## 📈 Project Statistics

- **Lines of Code:** 9,355 production Rust (22,400 total with tests)
- **Test Suite:** 237 tests (100% passing)
- **Benchmarks:** 5 performance benchmarks
- **Examples:** 13 comprehensive demos
- **Documentation:** 4,500+ lines
- **Features:** 22 major subsystems
- **Commits:** 25+ well-documented

---

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Ways to Contribute

- 🐛 Report bugs via [GitHub Issues](https://github.com/Nguyenetic/NexusQL/issues)
- ✨ Request features via [GitHub Issues](https://github.com/Nguyenetic/NexusQL/issues)
- 📝 Improve documentation
- 🧪 Add test coverage
- ⚡ Optimize performance
- 🔒 Enhance security

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 🔗 Links

- **GitHub:** [https://github.com/Nguyenetic/NexusQL](https://github.com/Nguyenetic/NexusQL)
- **npm Package:** [https://www.npmjs.com/package/@nguyenetic/nexusql](https://www.npmjs.com/package/@nguyenetic/nexusql)
- **Documentation:** [docs/](docs/)
- **Issues:** [GitHub Issues](https://github.com/Nguyenetic/NexusQL/issues)
- **Discussions:** [GitHub Discussions](https://github.com/Nguyenetic/NexusQL/discussions)

---

## 🙏 Acknowledgments

Built with:
- **Rust** - Systems programming language
- **Tokio** - Async runtime
- **sqlparser-rs** - SQL parser
- **napi-rs** - Node.js bindings
- **PostgreSQL** - Inspiration and compatibility

Special thanks to the open-source community for their invaluable contributions.

---

<div align="center">

**Built with ❤️ by [Nguyenetic](https://github.com/Nguyenetic)**

⭐ **Star this repo if you find it useful!** ⭐

[Report Bug](https://github.com/Nguyenetic/NexusQL/issues) • [Request Feature](https://github.com/Nguyenetic/NexusQL/issues) • [Documentation](docs/)

</div>
