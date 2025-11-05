# @nguyenetic/nexusql - TypeScript/Node.js Bindings

[![npm version](https://badge.fury.io/js/%40nguyenetic%2Fnexusql.svg)](https://www.npmjs.com/package/@nguyenetic/nexusql)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Node.js](https://img.shields.io/badge/node-%3E%3D16.0.0-brightgreen.svg)](https://nodejs.org/)

AI-native, PostgreSQL-compatible database with vector search support. Fast, embedded, and easy to use with built-in SQL injection prevention.

## 🚀 Features

- ✅ **Parameterized Queries** - SQL injection prevention with `$1, $2` placeholders
- ✅ **Prepared Statements** - Compile once, execute many times (10× faster)
- ✅ **ACID Transactions** - Full MVCC with automatic rollback
- ✅ **Vector Search** - Built-in HNSW index for semantic search (1578× faster)
- ✅ **Async/Await** - Modern promise-based API
- ✅ **TypeScript** - Full type definitions included
- ✅ **Zero Config** - Works out of the box

## 📦 Installation

### npm
```bash
npm install @nguyenetic/nexusql
```

### yarn
```bash
yarn add @nguyenetic/nexusql
```

### pnpm
```bash
pnpm add @nguyenetic/nexusql
```

## ⚡ Quick Start

### Basic Usage

```typescript
import { NexusDb } from '@nguyenetic/nexusql';

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
console.log(result.rows[0]); // { id: 1, name: 'Alice', email: 'alice@example.com' }

await db.close();
```

### Transactions

```typescript
const tx = await db.beginTransaction();

try {
  await tx.query('INSERT INTO users VALUES (2, "Bob", "bob@example.com")');
  await tx.query('UPDATE accounts SET balance = balance - 100 WHERE user_id = 2');
  await tx.commit(); // ✅ Both succeed
} catch (error) {
  await tx.rollback(); // ❌ Both fail
  console.error('Transaction failed:', error);
}
```

### Prepared Statements (10× Faster)

```typescript
const stmt = await db.prepare('INSERT INTO users VALUES ($1, $2, $3)');

// Execute multiple times with different parameters
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

// Insert vector with metadata
const embedding = new Array(384).fill(0).map(() => Math.random());
await db.insertVector('documents', embedding, JSON.stringify({
  id: 1,
  content: 'Introduction to machine learning'
}));

// Batch insert (100× faster)
const vectors = Array.from({ length: 1000 }, () =>
  new Array(384).fill(0).map(() => Math.random())
);
const metadata = Array.from({ length: 1000 }, (_, i) => JSON.stringify({
  id: i + 1,
  content: `Document ${i + 1}`
}));
await db.batchInsertVectors('documents', vectors, metadata);

// Create HNSW index
await db.createVectorIndex('documents', 'embedding', 'hnsw', JSON.stringify({
  m: 16,
  ef_construction: 64,
  distance: 'cosine'
}));

// Semantic search
const queryVector = new Array(384).fill(0).map(() => Math.random());
const results = await db.query(`
  SELECT id, content, cosine_distance(embedding, $1) as similarity
  FROM documents
  ORDER BY similarity
  LIMIT 10
`, [JSON.stringify(queryVector)]);
```

## 📖 Complete API Reference

See the [Full TypeScript API Documentation](../../docs/api/typescript/README.md) for detailed reference:

- **[Classes](../../docs/api/typescript/CLASSES.md)** - NexusDb, Transaction, PreparedStatement
- **[Methods](../../docs/api/typescript/METHODS.md)** - Complete method reference with examples
- **[Interfaces](../../docs/api/typescript/INTERFACES.md)** - QueryResult, Field
- **[Examples](../../docs/api/typescript/EXAMPLES.md)** - Real-world usage patterns

## 🔑 API Quick Reference

### Database Operations

```typescript
const db = new NexusDb(path: string);              // Create database instance

await db.query(sql, params?)                       // Execute SELECT query
await db.execute(sql, params?)                     // Execute INSERT/UPDATE/DELETE
await db.prepare(sql)                              // Create prepared statement
await db.beginTransaction()                        // Start transaction
await db.close()                                   // Close database
```

### Vector Operations

```typescript
await db.insertVector(table, vector, metadata?)              // Insert single vector
await db.batchInsertVectors(table, vectors, metadataArray?)  // Batch insert (10-100× faster)
await db.createVectorIndex(table, column?, type?, config?)   // Create HNSW index
```

### Transaction Methods

```typescript
const tx = await db.beginTransaction();

await tx.query(sql)                                // Query in transaction
await tx.commit()                                  // Commit changes
await tx.rollback()                                // Rollback changes
```

### Prepared Statement Methods

```typescript
const stmt = await db.prepare(sql);

await stmt.execute(...params)                      // Execute with parameters
await stmt.close()                                 // Close statement
```

## 🔒 Security

### Parameterized Queries (SQL Injection Prevention)

```typescript
// ✅ SAFE - Parameterized query
const userId = req.params.id;
const result = await db.query('SELECT * FROM users WHERE id = $1', [userId]);

// ❌ UNSAFE - String concatenation (NEVER do this!)
const result = await db.query(`SELECT * FROM users WHERE id = ${userId}`);
```

NexusQL automatically escapes all parameters to prevent SQL injection attacks.

### Best Practices

1. **Always use parameterized queries** for user input
2. **Use prepared statements** for repeated queries
3. **Validate input** before database operations
4. **Use transactions** for multi-step operations
5. **Handle errors gracefully** with try/catch

See [Security Guide](../../SECURITY.md) for comprehensive security practices.

## 💻 Platform Support

| Platform | Architecture | Status |
|----------|-------------|--------|
| **Windows** | x64 | ✅ Supported |
| **Windows** | arm64 | ✅ Supported |
| **macOS** | x64 (Intel) | ✅ Supported |
| **macOS** | arm64 (Apple Silicon) | ✅ Supported |
| **Linux** | x64 (glibc) | ✅ Supported |
| **Linux** | x64 (musl) | ✅ Supported |
| **Linux** | arm64 | ✅ Supported |

## 📊 Performance Tips

### Use Prepared Statements for Bulk Operations

```typescript
// ❌ SLOW - Individual executes (1000 queries = ~1000ms)
for (let i = 0; i < 1000; i++) {
  await db.execute('INSERT INTO users VALUES ($1, $2)', [i, `User${i}`]);
}

// ✅ FAST - Prepared statement (1000 executions = ~100ms)
const stmt = await db.prepare('INSERT INTO users VALUES ($1, $2)');
for (let i = 0; i < 1000; i++) {
  await stmt.execute(i, `User${i}`);
}
await stmt.close();
```

### Use Batch Operations for Vectors

```typescript
// ❌ SLOW - Individual inserts (1000 vectors = ~5000ms)
for (const vector of vectors) {
  await db.insertVector('embeddings', vector);
}

// ✅ FAST - Batch insert (1000 vectors = ~50ms)
await db.batchInsertVectors('embeddings', vectors);
```

### Create Indexes for Better Query Performance

```typescript
// Create index on frequently queried columns
await db.execute('CREATE INDEX idx_users_email ON users(email)');

// Create vector index for similarity search (1578× speedup)
await db.createVectorIndex('embeddings', 'embedding', 'hnsw');
```

## 🐛 Troubleshooting

### Module Not Found

If you get "Cannot find module" errors:

```bash
npm install @nguyenetic/nexusql
# or
npm rebuild @nguyenetic/nexusql
```

### Binary Compatibility

If you encounter binary compatibility issues:

```bash
npm rebuild @nguyenetic/nexusql --build-from-source
```

### Windows File Locking

On Windows, always close databases properly:

```typescript
try {
  const db = new NexusDb('./app.db');
  // ... use database
} finally {
  await db.close(); // Important on Windows!
}
```

### In-Memory Databases for Testing

Use `:memory:` for tests to avoid file locking issues:

```typescript
const db = new NexusDb(':memory:'); // No cleanup needed
```

## 📝 TypeScript Configuration

Add to your `tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "commonjs",
    "lib": ["ES2020"],
    "moduleResolution": "node",
    "esModuleInterop": true,
    "skipLibCheck": true
  }
}
```

## 🧪 Testing

```bash
npm test
npm test -- --coverage
```

See [examples/](examples/) for runnable test examples.

## 📚 Examples

Check out the [examples/](examples/) directory for complete, runnable examples:

- **[basic-usage.js](examples/basic-usage.js)** - Getting started with NexusQL
- **[parameterized-queries.js](examples/parameterized-queries.js)** - SQL injection prevention
- **[transactions.js](examples/transactions.js)** - ACID transaction patterns
- **[prepared-statements.js](examples/prepared-statements.js)** - Performance optimization
- **[vector-search.js](examples/vector-search.js)** - Semantic search and embeddings

## 🔄 Migration Guides

- [From SQLite](../../docs/guides/migration/from-sqlite.md)
- [From PostgreSQL](../../docs/guides/migration/from-postgres.md)
- [From sqlite-vec](../../docs/guides/migration/from-sqlite-vec.md)

## 🤝 Contributing

See [CONTRIBUTING.md](../../CONTRIBUTING.md) for contribution guidelines.

## 📄 License

MIT License - See [LICENSE](../../LICENSE) for details.

## 🔗 Links

- **GitHub:** [https://github.com/Nguyenetic/NexusQL](https://github.com/Nguyenetic/NexusQL)
- **npm:** [https://www.npmjs.com/package/@nguyenetic/nexusql](https://www.npmjs.com/package/@nguyenetic/nexusql)
- **Documentation:** [Full API Reference](../../docs/api/typescript/README.md)
- **Issues:** [GitHub Issues](https://github.com/Nguyenetic/NexusQL/issues)

## 💡 Support

- 📖 **Documentation:** [docs/](../../docs/)
- 🐛 **Bug Reports:** [GitHub Issues](https://github.com/Nguyenetic/NexusQL/issues)
- 💬 **Discussions:** [GitHub Discussions](https://github.com/Nguyenetic/NexusQL/discussions)
- 🔒 **Security:** [Security Policy](../../SECURITY.md)

---

**Built with ❤️ using Rust, N-API, and modern JavaScript**
