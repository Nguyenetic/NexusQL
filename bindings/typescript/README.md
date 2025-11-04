# NexusQL - TypeScript/Node.js Client

AI-Native, PostgreSQL-compatible database for Node.js applications.

## Installation

### From NPM (Recommended)

```bash
npm install @nguyenetic/nexusql
```

### From GitHub Repository

```bash
npm install git+https://github.com/johnhnguyen97/NexusQL.git#master
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

## Advanced Examples

### Full-Text Search

```typescript
// Create full-text search index
await db.execute(`
    CREATE TABLE articles (
        id INT PRIMARY KEY,
        title TEXT,
        content TEXT,
        search_vector TSVECTOR
    )
`);

// Search articles
const results = await db.query(`
    SELECT * FROM articles
    WHERE search_vector @@ to_tsquery('database & optimization')
    ORDER BY ts_rank(search_vector, to_tsquery('database')) DESC
`);
```

### AI-Native Vector Search

```typescript
// Store documents with embeddings
await db.execute(`
    CREATE TABLE documents (
        id INT PRIMARY KEY,
        content TEXT,
        embedding VECTOR(128)
    )
`);

// Semantic similarity search
const results = await db.query(`
    SELECT * FROM documents
    ORDER BY embedding <-> text_to_vector('search query')
    LIMIT 10
`);
```

### Table Partitioning

```typescript
// Create partitioned table
await db.execute(`
    CREATE TABLE events (
        event_date DATE,
        event_type TEXT,
        data JSONB
    ) PARTITION BY RANGE (event_date)
`);

// Add partitions
await db.execute(`
    CREATE TABLE events_2024_q1 PARTITION OF events
    FOR VALUES FROM ('2024-01-01') TO ('2024-04-01')
`);
```

### JSON/JSONB Operations

```typescript
// Store and query JSON data
await db.execute(`
    CREATE TABLE products (
        id INT PRIMARY KEY,
        metadata JSONB
    )
`);

await db.execute(`
    INSERT INTO products VALUES (1, '{"name": "Widget", "tags": ["new", "sale"]}')
`);

// Query JSON fields
const results = await db.query(`
    SELECT * FROM products
    WHERE metadata @> '{"tags": ["sale"]}'
`);
```

### Distributed Queries (Multi-Node)

```typescript
// Query across shards
const results = await db.query(`
    SELECT COUNT(*) FROM users
    -- Automatically distributes across all shards
`);
```

## Configuration Options

```typescript
const db = new NexusDB('./mydb', {
    // Connection pool size
    maxConnections: 20,

    // Query timeout
    queryTimeout: 30000,

    // Enable query logging
    logQueries: true,

    // Cache size
    cacheSize: '100MB'
});
```

## Error Handling

```typescript
try {
    await db.execute('INSERT INTO users VALUES (1, "Alice")');
} catch (error) {
    if (error.code === 'UNIQUE_VIOLATION') {
        console.log('User already exists');
    } else {
        console.error('Database error:', error.message);
    }
}
```

## Best Practices

### 1. Use Prepared Statements for Repeated Queries

```typescript
const stmt = await db.prepare('SELECT * FROM users WHERE id = $1');

// Execute multiple times efficiently
const user1 = await stmt.execute(1);
const user2 = await stmt.execute(2);
const user3 = await stmt.execute(3);

await stmt.close();
```

### 2. Use Connection Pooling for High Traffic

```typescript
import { ConnectionPool } from 'nexusql';

const pool = new ConnectionPool('./mydb', { max: 50 });

// Get connection
const conn = await pool.getConnection();
try {
    const result = await conn.query('SELECT * FROM users');
    // Use result
} finally {
    pool.releaseConnection(conn);
}
```

### 3. Always Use Transactions for Multiple Operations

```typescript
const tx = await db.beginTransaction();
try {
    await tx.query('INSERT INTO orders (user_id, amount) VALUES (1, 100)');
    await tx.query('UPDATE accounts SET balance = balance - 100 WHERE user_id = 1');
    await tx.commit();
} catch (error) {
    await tx.rollback();
    throw error;
}
```

## Troubleshooting

### Connection Issues

```typescript
// Check if database is accessible
try {
    const result = await db.query('SELECT 1');
    console.log('Database connected!');
} catch (error) {
    console.error('Cannot connect:', error);
}
```

### Performance Issues

```typescript
// Use EXPLAIN to analyze queries
const plan = await db.query('EXPLAIN SELECT * FROM large_table WHERE id > 1000');
console.log(plan.rows);

// Create indexes for better performance
await db.execute('CREATE INDEX users_email_idx ON users(email)');
```

## TypeScript Types

Full TypeScript support with type definitions:

```typescript
import { NexusDB, QueryResult, Row, Transaction } from 'nexusql';

const db: NexusDB = new NexusDB('./mydb');
const result: QueryResult = await db.query('SELECT * FROM users');
const rows: Row[] = result.rows;
```

## License

MIT

## Links

- **GitHub**: https://github.com/johnhnguyen97/NexusQL
- **Documentation**: See main README
- **Issues**: https://github.com/johnhnguyen97/NexusQL/issues

## Support

For questions or issues, please open an issue on GitHub.
