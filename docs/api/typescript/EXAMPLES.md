# TypeScript API - Examples

Real-world usage patterns and complete examples for NexusQL.

---

## Table of Contents

1. [Basic CRUD Operations](#basic-crud-operations)
2. [Parameterized Queries & SQL Injection Prevention](#parameterized-queries--sql-injection-prevention)
3. [Transaction Patterns](#transaction-patterns)
4. [Prepared Statements for Performance](#prepared-statements-for-performance)
5. [Vector Search Use Cases](#vector-search-use-cases)
6. [Error Handling](#error-handling)
7. [Batch Operations](#batch-operations)
8. [Real-World Applications](#real-world-applications)

---

## Basic CRUD Operations

### Create Table and Insert Data

```typescript
import { NexusDb } from '@nguyenetic/nexusql';

const db = new NexusDb('./myapp.db');

// Create table
await db.execute(`
  CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT UNIQUE,
    age INTEGER,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
  )
`);

// Insert single row
await db.execute(
  'INSERT INTO users (id, name, email, age) VALUES ($1, $2, $3, $4)',
  [1, 'Alice', 'alice@example.com', 30]
);

// Insert multiple rows
const users = [
  [2, 'Bob', 'bob@example.com', 25],
  [3, 'Charlie', 'charlie@example.com', 35],
  [4, 'Diana', 'diana@example.com', 28]
];

for (const user of users) {
  await db.execute('INSERT INTO users (id, name, email, age) VALUES ($1, $2, $3, $4)', user);
}

await db.close();
```

---

### Read Data

```typescript
const db = new NexusDb('./myapp.db');

// Select all
const all = await db.query('SELECT * FROM users');
console.log('All users:', all.rows);

// Select with condition
const young = await db.query('SELECT * FROM users WHERE age < $1', [30]);
console.log('Young users:', young.rows);

// Select specific columns
const names = await db.query('SELECT id, name FROM users ORDER BY name');
console.log('User names:', names.rows);

// Count rows
const count = await db.query('SELECT COUNT(*) as total FROM users');
console.log('Total users:', count.rows[0].total);

await db.close();
```

---

### Update Data

```typescript
const db = new NexusDb('./myapp.db');

// Update single row
const updated = await db.execute(
  'UPDATE users SET email = $1 WHERE id = $2',
  ['alice.smith@example.com', 1]
);
console.log(`Updated ${updated} row(s)`);

// Update multiple rows
await db.execute(
  'UPDATE users SET age = age + 1 WHERE age < $1',
  [30]
);

// Update with current timestamp
await db.execute(
  'UPDATE users SET updated_at = $1 WHERE id = $2',
  [new Date().toISOString(), 1]
);

await db.close();
```

---

### Delete Data

```typescript
const db = new NexusDb('./myapp.db');

// Delete single row
const deleted = await db.execute('DELETE FROM users WHERE id = $1', [4]);
console.log(`Deleted ${deleted} row(s)`);

// Delete with condition
await db.execute('DELETE FROM users WHERE age > $1', [60]);

// Delete all rows (be careful!)
await db.execute('DELETE FROM old_logs');

await db.close();
```

---

## Parameterized Queries & SQL Injection Prevention

### Safe User Input Handling

```typescript
const db = new NexusDb('./myapp.db');

// ❌ UNSAFE - Vulnerable to SQL injection
async function loginUnsafe(email: string, password: string) {
  const result = await db.query(
    `SELECT * FROM users WHERE email = '${email}' AND password = '${password}'`
  );
  return result.rows.length > 0;
}

// If attacker provides: email = "' OR '1'='1", entire user table is exposed!

// ✅ SAFE - Parameterized query prevents injection
async function loginSafe(email: string, password: string) {
  const result = await db.query(
    'SELECT * FROM users WHERE email = $1 AND password = $2',
    [email, password]
  );
  return result.rows.length > 0;
}

// Attacker input is treated as literal string, not SQL code
await loginSafe("' OR '1'='1", "anything"); // Returns false, attack prevented!

await db.close();
```

---

### Handling Special Characters

```typescript
const db = new NexusDb('./myapp.db');

// Parameterized queries automatically handle special characters
const dangerousInputs = [
  "Robert'); DROP TABLE users; --",
  "' OR '1'='1",
  "admin'--",
  "<script>alert('XSS')</script>",
  "O'Reilly", // Legitimate single quote in name
];

for (const input of dangerousInputs) {
  // ✅ All inputs are safely stored as text
  await db.execute('INSERT INTO users (name) VALUES ($1)', [input]);
}

// Verify data was stored safely
const result = await db.query('SELECT * FROM users');
console.log('Safely stored:', result.rows);

await db.close();
```

---

### Type Safety with Parameters

```typescript
const db = new NexusDb('./myapp.db');

// Automatic type conversion
await db.execute(
  'INSERT INTO users (id, name, age, active) VALUES ($1, $2, $3, $4)',
  [1, 'Alice', 30, true] // number, string, number, boolean
);

// NULL handling
await db.execute(
  'INSERT INTO users (id, name, email) VALUES ($1, $2, $3)',
  [2, 'Bob', null] // NULL value
);

// Date handling
await db.execute(
  'INSERT INTO logs (timestamp, message) VALUES ($1, $2)',
  [new Date().toISOString(), 'User logged in']
);

await db.close();
```

---

## Transaction Patterns

### Bank Transfer (Atomicity)

```typescript
const db = new NexusDb('./bank.db');

await db.execute(`
  CREATE TABLE accounts (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    balance REAL NOT NULL
  )
`);

await db.execute('INSERT INTO accounts VALUES (1, "Alice", 1000)');
await db.execute('INSERT INTO accounts VALUES (2, "Bob", 500)');

async function transferMoney(fromId: number, toId: number, amount: number) {
  const tx = await db.beginTransaction();

  try {
    // Debit sender
    await tx.query(`UPDATE accounts SET balance = balance - ${amount} WHERE id = ${fromId}`);

    // Credit receiver
    await tx.query(`UPDATE accounts SET balance = balance + ${amount} WHERE id = ${toId}`);

    // Both succeed together
    await tx.commit();
    console.log(`✅ Transferred $${amount} from account ${fromId} to ${toId}`);
  } catch (error) {
    // Both fail together
    await tx.rollback();
    console.error('❌ Transfer failed:', error);
    throw error;
  }
}

// Execute transfer
await transferMoney(1, 2, 100);

// Verify balances
const balances = await db.query('SELECT * FROM accounts');
console.log(balances.rows);
// [{ id: 1, name: 'Alice', balance: 900 }, { id: 2, name: 'Bob', balance: 600 }]

await db.close();
```

---

### Order Processing with Rollback

```typescript
const db = new NexusDb('./ecommerce.db');

async function processOrder(userId: number, items: Array<{productId: number, quantity: number}>) {
  const tx = await db.beginTransaction();

  try {
    // Create order
    const orderId = Date.now();
    await tx.query(`INSERT INTO orders (id, user_id, status) VALUES (${orderId}, ${userId}, 'pending')`);

    // Add order items and update inventory
    for (const item of items) {
      await tx.query(`
        INSERT INTO order_items (order_id, product_id, quantity)
        VALUES (${orderId}, ${item.productId}, ${item.quantity})
      `);

      await tx.query(`
        UPDATE products
        SET stock = stock - ${item.quantity}
        WHERE id = ${item.productId} AND stock >= ${item.quantity}
      `);
    }

    // Mark order as completed
    await tx.query(`UPDATE orders SET status = 'completed' WHERE id = ${orderId}`);

    await tx.commit();
    console.log(`✅ Order ${orderId} processed successfully`);
    return orderId;

  } catch (error) {
    await tx.rollback();
    console.error('❌ Order processing failed, all changes rolled back');
    throw error;
  }
}

await db.close();
```

---

### Batch Updates with Transaction

```typescript
const db = new NexusDb('./myapp.db');

async function batchUpdateUsers(updates: Array<{id: number, email: string}>) {
  const tx = await db.beginTransaction();

  try {
    for (const update of updates) {
      await tx.query(`UPDATE users SET email = '${update.email}' WHERE id = ${update.id}`);
    }

    await tx.commit();
    console.log(`✅ Updated ${updates.length} users`);
  } catch (error) {
    await tx.rollback();
    console.error('❌ Batch update failed');
    throw error;
  }
}

await batchUpdateUsers([
  { id: 1, email: 'alice@newdomain.com' },
  { id: 2, email: 'bob@newdomain.com' },
  { id: 3, email: 'charlie@newdomain.com' }
]);

await db.close();
```

---

## Prepared Statements for Performance

### Bulk Insert with Prepared Statement

```typescript
const db = new NexusDb('./myapp.db');

await db.execute(`
  CREATE TABLE logs (
    id INTEGER PRIMARY KEY,
    timestamp TEXT,
    level TEXT,
    message TEXT
  )
`);

// ❌ SLOW - 1000 individual executes (~1000ms)
console.time('Individual inserts');
for (let i = 1; i <= 1000; i++) {
  await db.execute(
    'INSERT INTO logs VALUES ($1, $2, $3, $4)',
    [i, new Date().toISOString(), 'INFO', `Log message ${i}`]
  );
}
console.timeEnd('Individual inserts'); // ~1000ms

// ✅ FAST - Prepared statement (~100ms, 10× faster)
console.time('Prepared statement');
const stmt = await db.prepare('INSERT INTO logs VALUES ($1, $2, $3, $4)');
for (let i = 1; i <= 1000; i++) {
  await stmt.execute(i, new Date().toISOString(), 'INFO', `Log message ${i}`);
}
await stmt.close();
console.timeEnd('Prepared statement'); // ~100ms

await db.close();
```

---

### Batch Query with Prepared Statement

```typescript
const db = new NexusDb('./myapp.db');

// Query multiple users by ID
const userIds = [1, 5, 10, 15, 20, 25, 30];

// Prepare the query once
const stmt = await db.prepare('SELECT * FROM users WHERE id = $1');

// Execute for each ID
for (const id of userIds) {
  const result = await stmt.execute(id);
  if (result.rowCount > 0) {
    console.log(`User ${id}:`, result.rows[0]);
  }
}

await stmt.close();
await db.close();
```

---

## Vector Search Use Cases

### Document Similarity Search

```typescript
import { NexusDb } from '@nguyenetic/nexusql';

const db = new NexusDb('./documents.db');

// Create table for document embeddings
await db.execute(`
  CREATE TABLE documents (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    embedding VECTOR(384)
  )
`);

// Insert documents with embeddings (using a hypothetical embedding function)
async function indexDocument(id: number, title: string, content: string) {
  const embedding = await generateEmbedding(content); // 384-dim vector

  const metadata = JSON.stringify({
    id: id,
    title: title,
    content: content
  });

  await db.insertVector('documents', embedding, metadata);
}

await indexDocument(1, 'Introduction to TypeScript', 'TypeScript is a typed superset...');
await indexDocument(2, 'Python for Beginners', 'Python is an easy-to-learn language...');
await indexDocument(3, 'Advanced TypeScript Patterns', 'Advanced TypeScript features include...');

// Create HNSW index for fast similarity search
await db.createVectorIndex('documents', 'embedding', 'hnsw', JSON.stringify({
  m: 16,
  ef_construction: 64,
  distance: 'cosine'
}));

// Search for similar documents
async function searchSimilarDocuments(query: string, limit: number = 5) {
  const queryEmbedding = await generateEmbedding(query);

  const result = await db.query(`
    SELECT id, title, content, cosine_distance(embedding, $1) as similarity
    FROM documents
    ORDER BY similarity
    LIMIT $2
  `, [JSON.stringify(queryEmbedding), limit]);

  return result.rows;
}

const similar = await searchSimilarDocuments('TypeScript programming', 3);
console.log('Similar documents:', similar);

await db.close();
```

---

### Batch Vector Insert for Performance

```typescript
const db = new NexusDb('./embeddings.db');

await db.execute(`
  CREATE TABLE embeddings (
    id INTEGER,
    text TEXT,
    source TEXT,
    embedding VECTOR(384)
  )
`);

// Prepare data
const documents = [
  { id: 1, text: 'First document', source: 'docs' },
  { id: 2, text: 'Second document', source: 'blogs' },
  // ... 1000 more documents
];

// ❌ SLOW - Individual inserts (~5000ms for 1000 vectors)
console.time('Individual vector inserts');
for (const doc of documents) {
  const embedding = await generateEmbedding(doc.text);
  const metadata = JSON.stringify({ id: doc.id, text: doc.text, source: doc.source });
  await db.insertVector('embeddings', embedding, metadata);
}
console.timeEnd('Individual vector inserts'); // ~5000ms

// ✅ FAST - Batch insert (~50ms for 1000 vectors, 100× faster!)
console.time('Batch vector insert');

const vectors = await Promise.all(documents.map(doc => generateEmbedding(doc.text)));
const metadata = documents.map(doc => JSON.stringify({
  id: doc.id,
  text: doc.text,
  source: doc.source
}));

await db.batchInsertVectors('embeddings', vectors, metadata);
console.timeEnd('Batch vector insert'); // ~50ms

await db.close();
```

---

### Semantic Search with Metadata Filtering

```typescript
const db = new NexusDb('./knowledge-base.db');

// Create index
await db.createVectorIndex('knowledge', 'embedding', 'hnsw');

// Search with metadata filtering
async function semanticSearch(query: string, source?: string, limit: number = 10) {
  const queryEmbedding = await generateEmbedding(query);

  let sql = `
    SELECT id, text, source, cosine_distance(embedding, $1) as similarity
    FROM knowledge
  `;

  const params: any[] = [JSON.stringify(queryEmbedding)];

  if (source) {
    sql += ` WHERE source = $2`;
    params.push(source);
  }

  sql += ` ORDER BY similarity LIMIT ${limit}`;

  const result = await db.query(sql, params);
  return result.rows;
}

// Search all sources
const allResults = await semanticSearch('machine learning basics');

// Search specific source
const docsOnly = await semanticSearch('machine learning basics', 'documentation');

await db.close();
```

---

## Error Handling

### Comprehensive Error Handling

```typescript
import { NexusDb } from '@nguyenetic/nexusql';

async function robustDatabaseOperation() {
  let db: NexusDb | null = null;

  try {
    // 1. Database creation errors
    db = new NexusDb('./myapp.db');

    // 2. SQL syntax errors
    try {
      await db.execute('INVALID SQL STATEMENT');
    } catch (error) {
      console.error('SQL syntax error:', error);
    }

    // 3. Constraint violations
    await db.execute('CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT UNIQUE)');
    await db.execute('INSERT INTO users VALUES (1, "test@example.com")');

    try {
      await db.execute('INSERT INTO users VALUES (1, "duplicate@example.com")');
    } catch (error) {
      console.error('Primary key violation:', error);
    }

    try {
      await db.execute('INSERT INTO users VALUES (2, "test@example.com")');
    } catch (error) {
      console.error('Unique constraint violation:', error);
    }

    // 4. Type mismatches
    try {
      await db.execute('INSERT INTO users VALUES ($1, $2)', ['not a number', 'email']);
    } catch (error) {
      console.error('Type mismatch:', error);
    }

    // 5. Missing parameters
    try {
      await db.execute('INSERT INTO users VALUES ($1, $2)', [1]); // Missing $2
    } catch (error) {
      console.error('Missing parameter:', error);
    }

    // 6. Table doesn't exist
    try {
      await db.query('SELECT * FROM non_existent_table');
    } catch (error) {
      console.error('Table not found:', error);
    }

  } catch (error) {
    console.error('Database operation failed:', error);
  } finally {
    // 7. Always close database
    if (db) {
      try {
        await db.close();
      } catch (error) {
        console.error('Failed to close database:', error);
      }
    }
  }
}

await robustDatabaseOperation();
```

---

### Transaction Error Handling

```typescript
const db = new NexusDb('./myapp.db');

async function safeTransaction() {
  const tx = await db.beginTransaction();
  let committed = false;

  try {
    await tx.query('INSERT INTO users VALUES (1, "Alice")');
    await tx.query('INSERT INTO users VALUES (2, "Bob")');

    // Simulate error
    if (Math.random() > 0.5) {
      throw new Error('Random failure for demonstration');
    }

    await tx.commit();
    committed = true;
    console.log('✅ Transaction committed');

  } catch (error) {
    if (!committed) {
      await tx.rollback();
      console.log('❌ Transaction rolled back due to error:', error);
    }
  }
}

await safeTransaction();
await db.close();
```

---

## Batch Operations

### Bulk Insert Pattern

```typescript
const db = new NexusDb('./myapp.db');

await db.execute('CREATE TABLE products (id INT, name TEXT, price REAL)');

// Sample data
const products = Array.from({ length: 10000 }, (_, i) => ({
  id: i + 1,
  name: `Product ${i + 1}`,
  price: Math.random() * 100
}));

// ✅ Best: Prepared statement + batch
console.time('Prepared statement batch');
const stmt = await db.prepare('INSERT INTO products VALUES ($1, $2, $3)');

for (const product of products) {
  await stmt.execute(product.id, product.name, product.price);
}

await stmt.close();
console.timeEnd('Prepared statement batch'); // Fastest approach

await db.close();
```

---

## Real-World Applications

### User Authentication System

```typescript
import { NexusDb } from '@nguyenetic/nexusql';
import bcrypt from 'bcrypt';

class UserAuth {
  private db: NexusDb;

  constructor(dbPath: string) {
    this.db = new NexusDb(dbPath);
  }

  async initialize() {
    await this.db.execute(`
      CREATE TABLE IF NOT EXISTS users (
        id INTEGER PRIMARY KEY,
        email TEXT UNIQUE NOT NULL,
        password_hash TEXT NOT NULL,
        created_at TEXT DEFAULT CURRENT_TIMESTAMP
      )
    `);
  }

  async register(email: string, password: string): Promise<number> {
    const hash = await bcrypt.hash(password, 10);

    const id = Date.now();
    await this.db.execute(
      'INSERT INTO users (id, email, password_hash) VALUES ($1, $2, $3)',
      [id, email, hash]
    );

    return id;
  }

  async login(email: string, password: string): Promise<boolean> {
    const result = await this.db.query(
      'SELECT password_hash FROM users WHERE email = $1',
      [email]
    );

    if (result.rowCount === 0) {
      return false;
    }

    const user = result.rows[0];
    return await bcrypt.compare(password, user.password_hash);
  }

  async close() {
    await this.db.close();
  }
}

// Usage
const auth = new UserAuth('./auth.db');
await auth.initialize();

await auth.register('user@example.com', 'securePassword123');
const loggedIn = await auth.login('user@example.com', 'securePassword123');
console.log('Login successful:', loggedIn);

await auth.close();
```

---

### Caching Layer

```typescript
class CachedDatabase {
  private db: NexusDb;
  private cache: Map<string, { data: any, timestamp: number }>;
  private ttl: number;

  constructor(dbPath: string, ttlSeconds: number = 60) {
    this.db = new NexusDb(dbPath);
    this.cache = new Map();
    this.ttl = ttlSeconds * 1000;
  }

  async query(sql: string, params?: any[]): Promise<any> {
    const cacheKey = JSON.stringify({ sql, params });

    // Check cache
    const cached = this.cache.get(cacheKey);
    if (cached && Date.now() - cached.timestamp < this.ttl) {
      console.log('✅ Cache hit');
      return cached.data;
    }

    // Query database
    console.log('❌ Cache miss - querying database');
    const result = await this.db.query(sql, params);

    // Update cache
    this.cache.set(cacheKey, {
      data: result,
      timestamp: Date.now()
    });

    return result;
  }

  clearCache() {
    this.cache.clear();
  }

  async close() {
    await this.db.close();
  }
}

// Usage
const cachedDb = new CachedDatabase('./myapp.db', 60);

// First call - cache miss
await cachedDb.query('SELECT * FROM users WHERE id = $1', [1]);

// Second call - cache hit (within 60 seconds)
await cachedDb.query('SELECT * FROM users WHERE id = $1', [1]);

await cachedDb.close();
```

---

## Next Steps

- [Class Reference](./CLASSES.md)
- [Method Reference](./METHODS.md)
- [Interface Reference](./INTERFACES.md)
- [Example Scripts](../../../bindings/typescript/examples/) - Runnable code examples

// Helper function (not part of NexusQL)
function generateEmbedding(text: string): Promise<number[]> {
  // This would typically call an embedding API like OpenAI or a local model
  return Promise.resolve(Array.from({ length: 384 }, () => Math.random()));
}
