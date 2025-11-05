# TypeScript API - Classes

Complete reference for all classes in the NexusQL TypeScript API.

---

## NexusDb

Main database client for Node.js applications.

### Constructor

```typescript
new NexusDb(path: string): NexusDb
```

Creates a new database instance.

**Parameters:**
- `path` - Database file path or `:memory:` for in-memory database

**Returns:** NexusDb instance

**Example:**
```typescript
// File-based database
const db = new NexusDb('./my-app.db');

// In-memory database (3-10× faster, no persistence)
const memDb = new NexusDb(':memory:');
```

**Notes:**
- Creates the database file if it doesn't exist
- Uses LSM storage with Write-Ahead Logging (WAL)
- In-memory databases are destroyed when closed
- Thread-safe with Arc<Mutex<>> internally

---

### Properties

The NexusDb class has no public properties. All state is managed internally.

---

### Methods

See [METHODS.md](./METHODS.md) for complete method documentation.

**Core Operations:**
- `query(sql, params?)` - Execute SELECT queries
- `execute(sql, params?)` - Execute INSERT/UPDATE/DELETE
- `prepare(sql)` - Create prepared statement
- `beginTransaction()` - Start transaction
- `close()` - Close database connection

**Vector Operations:**
- `insertVector(table, vector, metadata?)` - Insert vector with metadata
- `batchInsertVectors(table, vectors, metadataArray?)` - Batch insert vectors
- `createVectorIndex(table, column?, indexType?, config?)` - Create vector index

**Deprecated (use new API):**
- `queryWithParams(sql, params)` - Use `query(sql, params)` instead
- `executeWithParams(sql, params)` - Use `execute(sql, params)` instead

---

### Lifecycle

```typescript
// 1. Create
const db = new NexusDb('./app.db');

// 2. Use
await db.execute('CREATE TABLE ...');
const result = await db.query('SELECT ...');

// 3. Close (important for file-based databases)
await db.close();
```

**Best Practices:**
- Always call `close()` when done
- Use in-memory databases for tests
- One database instance per file (avoid conflicts)
- Handle errors with try/catch

---

### Error Handling

```typescript
try {
  const db = new NexusDb('./app.db');
  await db.execute('INVALID SQL');
  await db.close();
} catch (error) {
  console.error('Database error:', error.message);
  // Error types:
  // - SQL syntax errors
  // - Type mismatches
  // - Constraint violations
  // - File I/O errors
}
```

---

## Transaction

ACID-compliant transaction handle.

### How to Create

```typescript
const tx = await db.beginTransaction();
```

**Do not use:** `new Transaction()` - This is not exposed publicly

---

### Methods

#### query(sql: string): Promise`QueryResult`

Execute a query within the transaction.

**Parameters:**
- `sql` - SQL query (no parameters supported in transactions yet)

**Returns:** Promise`QueryResult`

**Example:**
```typescript
const tx = await db.beginTransaction();
const result = await tx.query('SELECT * FROM users');
```

**Note:** Parameterized queries are not yet supported in transactions. Use prepared statements outside transactions instead.

---

#### commit(): Promise<void>

Commit the transaction, making all changes permanent.

**Example:**
```typescript
const tx = await db.beginTransaction();
await tx.query('INSERT INTO users VALUES (1, "Alice")');
await tx.query('UPDATE accounts SET balance = balance - 100');
await tx.commit(); // ✅ Changes saved
```

**ACID Guarantees:**
- **Atomicity:** All or nothing
- **Consistency:** Database remains valid
- **Isolation:** MVCC snapshot isolation
- **Durability:** WAL ensures crash recovery

---

#### rollback(): Promise<void>

Rollback the transaction, discarding all changes.

**Example:**
```typescript
const tx = await db.beginTransaction();
try {
  await tx.query('INSERT INTO users VALUES (1, "Alice")');
  await tx.query('INVALID SQL');
  await tx.commit();
} catch (error) {
  await tx.rollback(); // ❌ Changes discarded
}
```

---

### Transaction Lifecycle

```typescript
// 1. Begin
const tx = await db.beginTransaction();

// 2. Execute queries
await tx.query('INSERT ...');
await tx.query('UPDATE ...');

// 3. Commit or rollback
try {
  await tx.commit();
} catch (error) {
  await tx.rollback();
}
```

**Best Practices:**
- Always commit or rollback
- Use try/catch for error handling
- Keep transactions short
- Don't nest transactions (not supported)

---

## PreparedStatement

Compiled SQL statement for repeated execution with different parameters.

### How to Create

```typescript
const stmt = await db.prepare('INSERT INTO users VALUES ($1, $2)');
```

**Do not use:** `new PreparedStatement()` - This is not exposed publicly

---

### Methods

#### execute(...params: any[]): Promise`QueryResult`

Execute the prepared statement with the given parameters.

**Parameters:**
- `...params` - Variable arguments matching the number of placeholders

**Returns:** Promise`QueryResult`

**Example:**
```typescript
const stmt = await db.prepare('INSERT INTO users (id, name) VALUES ($1, $2)');

// Execute multiple times with different parameters
await stmt.execute(1, 'Alice');
await stmt.execute(2, 'Bob');
await stmt.execute(3, 'Charlie');

await stmt.close();
```

**Performance:** Compile once, execute many times (faster than individual queries)

---

#### close(): Promise<void>

Close the prepared statement.

**Example:**
```typescript
const stmt = await db.prepare('SELECT * FROM users WHERE id = $1');
const result = await stmt.execute(123);
await stmt.close(); // Clean up resources
```

**Note:** In Rust, this is a no-op (resources are automatically cleaned up), but it's provided for API consistency with other database libraries.

---

### Prepared Statement Lifecycle

```typescript
// 1. Prepare
const stmt = await db.prepare('SELECT * FROM users WHERE id = $1');

// 2. Execute multiple times
for (let i = 1; i `<=` 100; i++) {
  const result = await stmt.execute(i);
  console.log(result.rows);
}

// 3. Close
await stmt.close();
```

**When to Use:**
- Same query executed many times
- Performance-critical code
- Batch operations

**When NOT to Use:**
- One-time queries (use `db.query()` instead)
- Dynamic SQL (different queries each time)

---

## Type Compatibility

```typescript
import { NexusDb, Transaction, PreparedStatement, QueryResult, Field } from '@nguyenetic/nexusql';

// Type-safe usage
const db: NexusDb = new NexusDb('./app.db');
const tx: Transaction = await db.beginTransaction();
const stmt: PreparedStatement = await db.prepare('SELECT * FROM users WHERE id = $1');
const result: QueryResult = await db.query('SELECT * FROM users');
```

---

## Next Steps

- [Interface Reference](./INTERFACES.md) - QueryResult, Field
- [Method Reference](./METHODS.md) - Complete method documentation
- [Examples](./EXAMPLES.md) - Real-world usage patterns
