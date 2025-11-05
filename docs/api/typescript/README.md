# TypeScript/Node.js API Reference

**Version:** 3.0.0
**Package:** `@nguyenetic/nexusql`
**Node.js:** >=16.0.0

Complete API reference for using NexusQL in TypeScript and JavaScript applications.

## 📦 Installation

```bash
npm install @nguyenetic/nexusql
```

## 🚀 Quick Start

```typescript
import { NexusDb } from '@nguyenetic/nexusql';

const db = new NexusDb('./my-database.db');

// Create table
await db.execute('CREATE TABLE users (id INT, name TEXT, email TEXT)');

// Insert with parameterized query (prevents SQL injection)
await db.execute('INSERT INTO users VALUES ($1, $2, $3)', [1, 'Alice', 'alice@example.com']);

// Query data
const result = await db.query('SELECT * FROM users WHERE id = $1', [1]);
console.log(result.rows); // [{ id: 1, name: 'Alice', email: 'alice@example.com' }]

await db.close();
```

## 📚 Documentation Sections

- **[Classes](./CLASSES.md)** - NexusDb, Transaction, PreparedStatement
- **[Interfaces](./INTERFACES.md)** - QueryResult, Field
- **[Methods](./METHODS.md)** - Complete method reference with examples
- **[Examples](./EXAMPLES.md)** - Real-world usage patterns

## 🔑 Key Features

| Feature | Description |
|---------|-------------|
| **Parameterized Queries** | Prevent SQL injection with `$1, $2` placeholders |
| **Transactions** | ACID-compliant transactions with MVCC |
| **Prepared Statements** | Compile once, execute many times |
| **Vector Search** | Built-in support for embeddings and similarity search |
| **Async/Await** | Modern promise-based API |
| **TypeScript** | Full type definitions included |

## 🎯 API Quick Reference

### Core Methods

```typescript
// Database Operations
new NexusDb(path: string)                                    // Create database instance
db.query(sql, params?)                                       // Execute SELECT query
db.execute(sql, params?)                                     // Execute INSERT/UPDATE/DELETE
db.prepare(sql)                                              // Create prepared statement
db.beginTransaction()                                        // Start transaction
db.close()                                                   // Close database

// Vector Operations
db.insertVector(table, vector, metadata?)                    // Insert single vector
db.batchInsertVectors(table, vectors, metadataArray?)        // Batch insert (10-100x faster)
db.createVectorIndex(table, column?, indexType?, config?)    // Create HNSW index

// Transaction Methods
tx.query(sql)                                                // Query in transaction
tx.commit()                                                  // Commit transaction
tx.rollback()                                                // Rollback transaction

// Prepared Statement Methods
stmt.execute(...params)                                      // Execute with params
stmt.close()                                                 // Close statement
```

## 🔒 Security

**Always use parameterized queries:**

```typescript
// ✅ SAFE - Parameterized query
await db.execute('INSERT INTO users VALUES ($1, $2)', [userId, userName]);

// ❌ UNSAFE - String concatenation (SQL injection risk)
await db.execute(`INSERT INTO users VALUES (${userId}, '${userName}')`);
```

## 📊 Performance

- **Query Cache:** 49× speedup on repeated queries
- **HNSW Index:** 1578× faster than sequential scan for vector search
- **Batch Insert:** 10-100× faster than individual inserts
- **Prepared Statements:** Compile once, execute many times

## 🔗 Related Documentation

- [Getting Started Guide](../../guides/getting-started.md)
- [Parameterized Queries Guide](../../guides/parameterized-queries.md)
- [Vector Search Guide](../../guides/vector-search.md)
- [Transaction Guide](../../guides/transactions.md)
- [Migration from SQLite](../../guides/migration/from-sqlite.md)

## 💡 Need Help?

- [GitHub Issues](https://github.com/Nguyenetic/NexusQL/issues)
- [Examples Directory](../../../bindings/typescript/examples/)
- [Architecture Documentation](../../architecture/README.md)

## 📄 License

MIT License - See [LICENSE](../../../LICENSE) for details
