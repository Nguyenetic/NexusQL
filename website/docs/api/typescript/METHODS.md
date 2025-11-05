# TypeScript API - Methods

Complete reference for all methods in the NexusQL TypeScript API with detailed examples.

---

## Table of Contents

**Database Methods:**
- [query()](#query) - Execute SELECT queries
- [execute()](#execute) - Execute INSERT/UPDATE/DELETE
- [prepare()](#prepare) - Create prepared statements
- [beginTransaction()](#begintransaction) - Start transactions
- [close()](#close) - Close database

**Vector Methods:**
- [insertVector()](#insertvector) - Insert single vector
- [batchInsertVectors()](#batchinsertvectors) - Batch insert vectors
- [createVectorIndex()](#createvectorindex) - Create vector index

**Transaction Methods:**
- [tx.query()](#txquery) - Query in transaction
- [tx.commit()](#txcommit) - Commit transaction
- [tx.rollback()](#txrollback) - Rollback transaction

**Prepared Statement Methods:**
- [stmt.execute()](#stmtexecute) - Execute prepared statement
- [stmt.close()](#stmtclose) - Close prepared statement

---

## Database Methods

### query()

Execute a SELECT query and return results.

```typescript
query(sql: string, params?: any[]): Promise`QueryResult`
```

**Parameters:**
- `sql` - SQL SELECT statement with optional `$1, $2, ...` placeholders
- `params` (optional) - Array of values to bind to placeholders

**Returns:** `Promise`QueryResult`` with rows, rowCount, and fields

**Throws:** Error on SQL syntax errors, type mismatches, or constraint violations

---

#### Basic Usage

```typescript
// Simple query (no parameters)
const result = await db.query('SELECT * FROM users');
console.log(result.rows);
// [{ id: 1, name: 'Alice' }, { id: 2, name: 'Bob' }]

// Query with WHERE clause
const result2 = await db.query('SELECT * FROM users WHERE age > 25');
```

---

#### Parameterized Queries (SQL Injection Prevention)

```typescript
// ✅ SAFE - Parameterized query
const userId = 1;
const result = await db.query('SELECT * FROM users WHERE id = $1', [userId]);

// ✅ SAFE - Multiple parameters
const minAge = 25;
const status = 'active';
const result2 = await db.query(
  'SELECT * FROM users WHERE age > $1 AND status = $2',
  [minAge, status]
);

// ❌ UNSAFE - String interpolation (NEVER do this!)
const result3 = await db.query(`SELECT * FROM users WHERE id = ${userId}`);
```

---

#### Working with Results

```typescript
const result = await db.query('SELECT id, name, email FROM users WHERE age > $1', [25]);

// Access rows
console.log(`Found ${result.rowCount} users`);
result.rows.forEach(user => {
  console.log(`${user.name} (${user.email})`);
});

// Type assertion for TypeScript
interface User {
  id: number;
  name: string;
  email: string;
}

const users = result.rows as User[];
users.forEach(user => {
  console.log(user.name.toUpperCase());
});
```

---

#### Complex Queries

```typescript
// JOIN queries
const orders = await db.query(`
  SELECT users.name, orders.total, orders.created_at
  FROM orders
  INNER JOIN users ON orders.user_id = users.id
  WHERE orders.total > $1
  ORDER BY orders.created_at DESC
`, [100]);

// Aggregate functions
const stats = await db.query(`
  SELECT
    COUNT(*) as total,
    AVG(age) as avg_age,
    MAX(age) as max_age
  FROM users
  WHERE active = $1
`, [true]);

console.log(stats.rows[0]);
// { total: 150, avg_age: 32.5, max_age: 65 }

// Subqueries
const result = await db.query(`
  SELECT name, email
  FROM users
  WHERE age > (SELECT AVG(age) FROM users)
`);
```

---

#### Error Handling

```typescript
try {
  const result = await db.query('SELECT * FROM non_existent_table');
} catch (error) {
  console.error('Query failed:', error.message);
  // Error types:
  // - Table doesn't exist
  // - Column doesn't exist
  // - SQL syntax error
  // - Type mismatch
}
```

---

### execute()

Execute INSERT, UPDATE, DELETE, or DDL statements.

```typescript
execute(sql: string, params?: any[]): Promise<number>
```

**Parameters:**
- `sql` - SQL statement with optional `$1, $2, ...` placeholders
- `params` (optional) - Array of values to bind to placeholders

**Returns:** `Promise<number>` - Number of rows affected

**Throws:** Error on SQL syntax errors, constraint violations, or type mismatches

---

#### INSERT Statements

```typescript
// Insert single row
const affected = await db.execute(
  'INSERT INTO users (id, name, email) VALUES ($1, $2, $3)',
  [1, 'Alice', 'alice@example.com']
);
console.log(`Inserted ${affected} row(s)`); // Inserted 1 row(s)

// Insert multiple rows (use batch for better performance)
await db.execute('INSERT INTO users VALUES ($1, $2, $3)', [2, 'Bob', 'bob@example.com']);
await db.execute('INSERT INTO users VALUES ($1, $2, $3)', [3, 'Charlie', 'charlie@example.com']);

// Insert with NULL values
await db.execute('INSERT INTO users VALUES ($1, $2, $3)', [4, 'Dave', null]);
```

---

#### UPDATE Statements

```typescript
// Update single column
const updated = await db.execute(
  'UPDATE users SET email = $1 WHERE id = $2',
  ['newemail@example.com', 1]
);
console.log(`Updated ${updated} row(s)`);

// Update multiple columns
await db.execute(
  'UPDATE users SET name = $1, email = $2, updated_at = $3 WHERE id = $4',
  ['Alice Smith', 'alice.smith@example.com', new Date().toISOString(), 1]
);

// Update with conditions
const affected = await db.execute(
  'UPDATE users SET active = $1 WHERE last_login < $2',
  [false, '2024-01-01']
);
console.log(`Deactivated ${affected} inactive users`);
```

---

#### DELETE Statements

```typescript
// Delete specific rows
const deleted = await db.execute('DELETE FROM users WHERE id = $1', [5]);
console.log(`Deleted ${deleted} row(s)`);

// Delete with conditions
const affected = await db.execute('DELETE FROM users WHERE active = $1 AND age < $2', [false, 18]);

// Delete all rows (be careful!)
await db.execute('DELETE FROM old_logs');
```

---

#### DDL Statements (CREATE, ALTER, DROP)

```typescript
// Create table
await db.execute(`
  CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT UNIQUE,
    age INTEGER,
    active BOOLEAN DEFAULT true,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
  )
`);

// Create index
await db.execute('CREATE INDEX idx_users_email ON users(email)');

// Create unique index
await db.execute('CREATE UNIQUE INDEX idx_users_name ON users(name)');

// Alter table
await db.execute('ALTER TABLE users ADD COLUMN phone TEXT');

// Drop table
await db.execute('DROP TABLE IF EXISTS old_table');

// Drop index
await db.execute('DROP INDEX IF EXISTS idx_old_index');
```

---

#### Constraints and Foreign Keys

```typescript
// Primary key
await db.execute(`
  CREATE TABLE orders (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL,
    total REAL NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id)
  )
`);

// Unique constraint
await db.execute(`
  CREATE TABLE emails (
    address TEXT UNIQUE NOT NULL
  )
`);

// Check constraint
await db.execute(`
  CREATE TABLE products (
    id INTEGER PRIMARY KEY,
    price REAL CHECK(price > 0)
  )
`);
```

---

### prepare()

Create a prepared statement for executing the same query multiple times with different parameters.

```typescript
prepare(sql: string): Promise<PreparedStatement>
```

**Parameters:**
- `sql` - SQL statement with `$1, $2, ...` placeholders

**Returns:** `Promise<PreparedStatement>` - Prepared statement instance

**Performance:** Compile once, execute many times (faster than repeated `query()` calls)

---

#### Basic Usage

```typescript
// Prepare statement
const stmt = await db.prepare('SELECT * FROM users WHERE id = $1');

// Execute multiple times
const user1 = await stmt.execute(1);
const user2 = await stmt.execute(2);
const user3 = await stmt.execute(3);

// Clean up
await stmt.close();
```

---

#### INSERT with Prepared Statements

```typescript
const stmt = await db.prepare('INSERT INTO users (id, name, email) VALUES ($1, $2, $3)');

// Insert 1000 users (much faster than 1000 individual execute() calls)
for (let i = 1; i `<=` 1000; i++) {
  await stmt.execute(i, `User${i}`, `user${i}@example.com`);
}

await stmt.close();
console.log('Inserted 1000 users with prepared statement');
```

---

#### SELECT with Prepared Statements

```typescript
const stmt = await db.prepare(`
  SELECT name, email, age
  FROM users
  WHERE age > $1 AND active = $2
  ORDER BY name
`);

// Query different age groups
const young = await stmt.execute(18, true);
const adults = await stmt.execute(25, true);
const seniors = await stmt.execute(65, true);

console.log(`Young: ${young.rowCount}, Adults: ${adults.rowCount}, Seniors: ${seniors.rowCount}`);

await stmt.close();
```

---

#### Performance Comparison

```typescript
// ❌ SLOW - Individual queries (1000 queries = ~1000ms)
console.time('Individual queries');
for (let i = 1; i `<=` 1000; i++) {
  await db.execute('INSERT INTO users VALUES ($1, $2)', [i, `User${i}`]);
}
console.timeEnd('Individual queries'); // ~1000ms

// ✅ FAST - Prepared statement (1000 executions = ~100ms)
console.time('Prepared statement');
const stmt = await db.prepare('INSERT INTO users VALUES ($1, $2)');
for (let i = 1; i `<=` 1000; i++) {
  await stmt.execute(i, `User${i}`);
}
await stmt.close();
console.timeEnd('Prepared statement'); // ~100ms (10× faster)
```

---

### beginTransaction()

Start a new ACID-compliant transaction.

```typescript
beginTransaction(): Promise<Transaction>
```

**Returns:** `Promise<Transaction>` - Transaction handle

**ACID Guarantees:**
- **Atomicity:** All changes commit together or rollback together
- **Consistency:** Database remains in valid state
- **Isolation:** MVCC snapshot isolation (other transactions don't see uncommitted changes)
- **Durability:** Write-Ahead Log ensures crash recovery

---

#### Basic Transaction

```typescript
const tx = await db.beginTransaction();

try {
  await tx.query('INSERT INTO users VALUES (1, "Alice")');
  await tx.query('INSERT INTO orders VALUES (1, 1, 100.00)');
  await tx.commit(); // ✅ Both inserts succeed
} catch (error) {
  await tx.rollback(); // ❌ Neither insert happens
  console.error('Transaction failed:', error);
}
```

---

#### Bank Transfer Example (Atomicity)

```typescript
async function transferMoney(fromId: number, toId: number, amount: number) {
  const tx = await db.beginTransaction();

  try {
    // Debit from sender
    await tx.query('UPDATE accounts SET balance = balance - $1 WHERE id = $2', [amount, fromId]);

    // Credit to receiver
    await tx.query('UPDATE accounts SET balance = balance + $1 WHERE id = $2', [amount, toId]);

    // Both succeed or both fail
    await tx.commit();
    console.log(`Transferred $${amount} from ${fromId} to ${toId}`);
  } catch (error) {
    await tx.rollback();
    console.error('Transfer failed:', error);
  }
}

await transferMoney(1, 2, 50.00);
```

---

#### Error Handling in Transactions

```typescript
const tx = await db.beginTransaction();

try {
  await tx.query('INSERT INTO users VALUES (1, "Alice")');

  // This will fail (duplicate primary key)
  await tx.query('INSERT INTO users VALUES (1, "Bob")');

  await tx.commit();
} catch (error) {
  // Automatically rollback on error
  await tx.rollback();
  console.log('Transaction rolled back - neither user inserted');
}
```

---

#### Current Limitations

```typescript
// ❌ NOT SUPPORTED YET - Parameterized queries in transactions
const tx = await db.beginTransaction();
await tx.query('INSERT INTO users VALUES ($1, $2)', [1, 'Alice']); // Error!

// ✅ WORKAROUND - Use escaped strings (temporary)
const name = "Alice".replace(/'/g, "''");
await tx.query(`INSERT INTO users VALUES (1, '${name}')`);
```

---

### close()

Close the database connection and release resources.

```typescript
close(): Promise<void>
```

**Returns:** `Promise<void>`

**Important:** Always call `close()` when done with the database, especially for file-based databases.

---

#### Basic Usage

```typescript
const db = new NexusDb('./app.db');

// Use database
await db.execute('CREATE TABLE users (id INT, name TEXT)');
await db.execute('INSERT INTO users VALUES (1, "Alice")');

// Close when done
await db.close();
```

---

#### Try-Finally Pattern

```typescript
const db = new NexusDb('./app.db');

try {
  const result = await db.query('SELECT * FROM users');
  console.log(result.rows);
} finally {
  // Ensure database is always closed
  await db.close();
}
```

---

#### In-Memory Databases

```typescript
// In-memory database (auto-cleaned up on close)
const memDb = new NexusDb(':memory:');

await memDb.execute('CREATE TABLE temp (id INT)');
await memDb.execute('INSERT INTO temp VALUES (1)');

// All data is lost after close
await memDb.close();
```

---

## Vector Methods

### insertVector()

Insert a vector embedding with optional metadata.

```typescript
insertVector(
  table: string,
  vector: number[],
  metadata?: string | null
): Promise<void>
```

**Parameters:**
- `table` - Table name
- `vector` - Vector embedding as array of numbers (typically 384 or 1536 dimensions)
- `metadata` (optional) - JSON string with additional columns

**Returns:** `Promise<void>`

---

#### Basic Usage

```typescript
// Simple insert (embedding column only)
const embedding = [0.1, 0.2, 0.3, ...]; // 384 dimensions
await db.insertVector('embeddings', embedding);

// Insert with metadata
const metadata = JSON.stringify({
  id: 1,
  text: 'Hello world',
  source: 'user_input'
});

await db.insertVector('embeddings', embedding, metadata);
```

---

#### MCP Sage Style (Multiple Columns)

```typescript
const vector = [0.1, 0.2, /* ... */]; // 384-dim embedding

const metadata = JSON.stringify({
  user_id: 'user123',
  content: 'User prefers TypeScript over JavaScript',
  memory_level: 'user',
  metadata: JSON.stringify({ confidence: 0.95 }),
  created_at: new Date().toISOString()
});

await db.insertVector('memories', vector, metadata);
```

---

### batchInsertVectors()

Batch insert multiple vectors for optimal performance (10-100× faster than individual inserts).

```typescript
batchInsertVectors(
  table: string,
  vectors: number[][],
  metadataArray?: string[] | null
): Promise<number>
```

**Parameters:**
- `table` - Table name
- `vectors` - Array of vector embeddings
- `metadataArray` (optional) - Array of JSON strings (must match vectors length)

**Returns:** `Promise<number>` - Number of vectors inserted

**Performance:** 10-100× faster than individual `insertVector()` calls

---

#### Basic Batch Insert

```typescript
const vectors = [
  [0.1, 0.2, 0.3, ...],
  [0.4, 0.5, 0.6, ...],
  [0.7, 0.8, 0.9, ...]
];

const count = await db.batchInsertVectors('embeddings', vectors);
console.log(`Inserted ${count} vectors`);
```

---

#### Batch Insert with Metadata

```typescript
const vectors = [
  [0.1, 0.2, ...],
  [0.4, 0.5, ...],
  [0.7, 0.8, ...]
];

const metadata = [
  JSON.stringify({ id: 1, text: 'First document' }),
  JSON.stringify({ id: 2, text: 'Second document' }),
  JSON.stringify({ id: 3, text: 'Third document' })
];

const count = await db.batchInsertVectors('documents', vectors, metadata);
console.log(`Inserted ${count} document embeddings`);
```

---

#### Performance Comparison

```typescript
// ❌ SLOW - Individual inserts (1000 vectors = ~5000ms)
console.time('Individual inserts');
for (let i = 0; i < 1000; i++) {
  await db.insertVector('embeddings', generateVector());
}
console.timeEnd('Individual inserts'); // ~5000ms

// ✅ FAST - Batch insert (1000 vectors = ~50ms)
console.time('Batch insert');
const vectors = Array.from({ length: 1000 }, () => generateVector());
await db.batchInsertVectors('embeddings', vectors);
console.timeEnd('Batch insert'); // ~50ms (100× faster!)
```

---

### createVectorIndex()

Create a vector index for fast similarity search.

```typescript
createVectorIndex(
  table: string,
  column?: string | null,
  indexType?: string | null,
  config?: string | null
): Promise<string>
```

**Parameters:**
- `table` - Table name
- `column` (optional) - Column name (default: `'embedding'`)
- `indexType` (optional) - Index type: `'hnsw'` or `'flat'` (default: `'hnsw'`)
- `config` (optional) - JSON string with index configuration

**Returns:** `Promise<string>` - Confirmation message

**Performance:** HNSW provides 1578× speedup over sequential scan with 99%+ recall

---

#### Basic Usage

```typescript
// Create default HNSW index
await db.createVectorIndex('embeddings');

// Create index on specific column
await db.createVectorIndex('documents', 'doc_embedding');

// Create flat index (exact search, slower)
await db.createVectorIndex('small_table', 'embedding', 'flat');
```

---

#### HNSW Configuration

```typescript
const config = JSON.stringify({
  m: 16,                // Links per node (higher = better recall, more memory)
  ef_construction: 64,   // Construction quality (higher = better index, slower build)
  distance: 'cosine'     // Distance metric: 'l2', 'cosine', or 'inner_product'
});

await db.createVectorIndex('embeddings', 'embedding', 'hnsw', config);
```

**HNSW Parameters:**
- `m`: Number of connections per layer (default: 16, range: 4-64)
  - Higher = better recall, more memory
  - Typical: 8-32
- `ef_construction`: Build-time quality (default: 64, range: 16-512)
  - Higher = better index quality, slower build
  - Typical: 64-200
- `distance`: Metric for similarity
  - `'l2'` - Euclidean distance (default)
  - `'cosine'` - Cosine similarity (recommended for normalized vectors)
  - `'inner_product'` - Dot product

---

#### Performance Tuning

```typescript
// Fast build, good recall (recommended for most cases)
await db.createVectorIndex('embeddings', 'embedding', 'hnsw', JSON.stringify({
  m: 16,
  ef_construction: 64
}));

// High recall, slower build (for critical applications)
await db.createVectorIndex('embeddings', 'embedding', 'hnsw', JSON.stringify({
  m: 32,
  ef_construction: 200
}));

// Fast build, lower recall (for development/testing)
await db.createVectorIndex('embeddings', 'embedding', 'hnsw', JSON.stringify({
  m: 8,
  ef_construction: 32
}));
```

---

## Transaction Methods

### tx.query()

Execute a query within the transaction.

```typescript
tx.query(sql: string): Promise`QueryResult`
```

**See:** [beginTransaction()](#begintransaction) for examples

**Note:** Parameterized queries not yet supported in transactions. Use outside transactions with `db.query()` instead.

---

### tx.commit()

Commit the transaction, making all changes permanent.

```typescript
tx.commit(): Promise<void>
```

**See:** [beginTransaction()](#begintransaction) for examples

---

### tx.rollback()

Rollback the transaction, discarding all changes.

```typescript
tx.rollback(): Promise<void>
```

**See:** [beginTransaction()](#begintransaction) for examples

---

## Prepared Statement Methods

### stmt.execute()

Execute the prepared statement with parameters.

```typescript
stmt.execute(...params: any[]): Promise`QueryResult`
```

**See:** [prepare()](#prepare) for examples

---

### stmt.close()

Close the prepared statement and release resources.

```typescript
stmt.close(): Promise<void>
```

**See:** [prepare()](#prepare) for examples

---

## Next Steps

- [Class Reference](./CLASSES.md) - NexusDb, Transaction, PreparedStatement
- [Interface Reference](./INTERFACES.md) - QueryResult, Field
- [Examples](./EXAMPLES.md) - Real-world usage patterns
- [Getting Started Guide](../../guides/getting-started.md)
