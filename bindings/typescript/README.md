# NexusQL - TypeScript/Node.js Client

AI-Native, PostgreSQL-compatible database for Node.js applications.

## Installation

```bash
npm install nexusql
```

## Quick Start

```typescript
import { NexusDB } from 'nexusql';

// Connect to database
const db = new NexusDB('./mydb');

// Execute queries
await db.execute('CREATE TABLE users (id INT PRIMARY KEY, name TEXT)');
await db.execute('INSERT INTO users VALUES (1, "Alice")');

const result = await db.query('SELECT * FROM users');
console.log(result.rows);

// Use transactions
const tx = await db.beginTransaction();
try {
    await tx.query('INSERT INTO users VALUES (2, "Bob")');
    await tx.query('UPDATE accounts SET balance = balance - 100');
    await tx.commit();
} catch (error) {
    await tx.rollback();
}

await db.close();
```

## Features

- ✅ **PostgreSQL Compatible**: Full SQL support
- ✅ **AI-Native**: Vector embeddings and semantic search
- ✅ **Embedded**: No separate server needed
- ✅ **ACID**: Full transaction support
- ✅ **Fast**: Advanced indexing (B-Tree, HNSW, GIN, GiST, BRIN)
- ✅ **Scalable**: Partitioning and sharding support
- ✅ **Full-Text**: Built-in text search
- ✅ **Type-Safe**: Full TypeScript support

## API Reference

### NexusDB

Main database class.

#### Methods

- `query(sql: string): Promise<QueryResult>` - Execute SELECT query
- `execute(sql: string): Promise<number>` - Execute command (INSERT, UPDATE, etc.)
- `beginTransaction(): Promise<Transaction>` - Start transaction
- `prepare(sql: string): Promise<PreparedStatement>` - Create prepared statement
- `close(): Promise<void>` - Close connection

### Transaction

Transaction management.

#### Methods

- `query(sql: string): Promise<QueryResult>` - Execute query in transaction
- `commit(): Promise<void>` - Commit transaction
- `rollback(): Promise<void>` - Rollback transaction

### PreparedStatement

Efficient repeated execution.

#### Methods

- `execute(...params): Promise<QueryResult>` - Execute with parameters
- `close(): Promise<void>` - Deallocate statement

### ConnectionPool

Connection pooling for high performance.

```typescript
import { ConnectionPool } from 'nexusql';

const pool = new ConnectionPool('./mydb', { max: 20 });
const conn = await pool.getConnection();
const result = await conn.query('SELECT * FROM users');
pool.releaseConnection(conn);
```

## License

MIT
