# NexusQL TypeScript API - Complete Reference

**Comprehensive TypeScript/Node.js Documentation for NexusQL**

---

## Table of Contents

1. [Installation](#installation)
2. [Quick Start](#quick-start)
3. [Core API](#core-api)
4. [Vector Search](#vector-search)
5. [Semantic Search (ColBERT)](#semantic-search-colbert)
6. [Hyperdimensional Computing (HDC)](#hyperdimensional-computing-hdc)
7. [Transactions](#transactions)
8. [Connection Pooling](#connection-pooling)
9. [Prepared Statements](#prepared-statements)
10. [Advanced Features](#advanced-features)
11. [TypeScript Types](#typescript-types)
12. [Error Handling](#error-handling)
13. [Best Practices](#best-practices)
14. [Examples](#examples)

---

## Installation

### From npm

```bash
npm install @nguyenetic/nexusql
```

### From GitHub Repository (Free)

```bash
npm install git+https://github.com/johnhnguyen97/NexusQL.git#master
```

### Requirements

- **Node.js:** 16.0.0 or higher
- **TypeScript:** 5.0.0 or higher (optional, for type checking)
- **Platform:** Windows, macOS, Linux

---

## Quick Start

```typescript
import { NexusDB } from 'nexusql';

async function main() {
  // 1. Connect to database (creates directory if not exists)
  const db = new NexusDB('./mydb');

  // 2. Create table
  await db.execute(`
    CREATE TABLE users (
      id SERIAL PRIMARY KEY,
      name TEXT NOT NULL,
      email TEXT UNIQUE,
      created_at TIMESTAMP DEFAULT NOW()
    )
  `);

  // 3. Insert data
  await db.execute(
    'INSERT INTO users (name, email) VALUES ($1, $2)',
    ['Alice', 'alice@example.com']
  );

  // 4. Query data
  const result = await db.query('SELECT * FROM users WHERE name = $1', ['Alice']);
  console.log(result.rows);

  // 5. Close connection
  await db.close();
}

main();
```

**Output:**
```
[
  {
    id: 1,
    name: 'Alice',
    email: 'alice@example.com',
    created_at: '2025-11-02T10:30:00Z'
  }
]
```

---

## Core API

### Class: `NexusDB`

Main database client class for connecting and executing queries.

#### Constructor

```typescript
new NexusDB(dbPath: string, options?: ConnectionOptions)
```

**Parameters:**
- `dbPath` - Path to database directory (will be created if not exists)
- `options` - Optional connection configuration

**Example:**
```typescript
const db = new NexusDB('./mydb');

// With options
const db = new NexusDB('./mydb', {
  isolationLevel: IsolationLevel.ReadCommitted,
  maxRetries: 3
});
```

#### `query(sql: string, params?: any[]): Promise<QueryResult>`

Execute a SELECT query and return results.

**Parameters:**
- `sql` - SQL query string (use `$1, $2, ...` for parameters)
- `params` - Optional array of parameter values

**Returns:** `Promise<QueryResult>`

**Example:**
```typescript
// Simple query
const result = await db.query('SELECT * FROM users');

// With parameters
const result = await db.query(
  'SELECT * FROM users WHERE age > $1 AND city = $2',
  [25, 'New York']
);

// Access results
console.log(result.rows);      // Array of row objects
console.log(result.rowCount);  // Number of rows
console.log(result.fields);    // Field metadata
```

#### `execute(sql: string, params?: any[]): Promise<number>`

Execute an INSERT, UPDATE, DELETE, or DDL statement.

**Parameters:**
- `sql` - SQL command string
- `params` - Optional array of parameter values

**Returns:** `Promise<number>` - Number of affected rows

**Example:**
```typescript
// CREATE TABLE
await db.execute(`
  CREATE TABLE products (
    id SERIAL PRIMARY KEY,
    name TEXT,
    price DECIMAL(10, 2)
  )
`);

// INSERT
const affected = await db.execute(
  'INSERT INTO products (name, price) VALUES ($1, $2)',
  ['Widget', 19.99]
);
console.log(`Inserted ${affected} rows`);

// UPDATE
const updated = await db.execute(
  'UPDATE products SET price = price * 1.1 WHERE category = $1',
  ['electronics']
);

// DELETE
const deleted = await db.execute(
  'DELETE FROM products WHERE price < $1',
  [10.00]
);
```

#### `beginTransaction(): Promise<Transaction>`

Start a new transaction.

**Returns:** `Promise<Transaction>`

**Example:**
```typescript
const tx = await db.beginTransaction();
try {
  await tx.query('INSERT INTO orders (total) VALUES (100)');
  await tx.query('UPDATE inventory SET quantity = quantity - 1');
  await tx.commit();
} catch (error) {
  await tx.rollback();
  throw error;
}
```

#### `prepare(sql: string): Promise<PreparedStatement>`

Create a prepared statement for efficient repeated execution.

**Returns:** `Promise<PreparedStatement>`

**Example:**
```typescript
const stmt = await db.prepare('SELECT * FROM users WHERE id = $1');

const user1 = await stmt.execute(1);
const user2 = await stmt.execute(2);
const user3 = await stmt.execute(3);

await stmt.close();
```

#### `close(): Promise<void>`

Close the database connection and release resources.

**Example:**
```typescript
await db.close();
```

---

## Vector Search

### Creating Vector Tables

```typescript
// Create table with vector column
await db.execute(`
  CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    content TEXT,
    embedding VECTOR(128)  -- 128-dimensional vector
  )
`);

// Create HNSW index for fast O(log N) search
await db.execute(`
  CREATE INDEX vec_idx ON documents(embedding)
  USING HNSW WITH (M=16, ef_construction=200)
`);
```

### HNSW Index Parameters

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `M` | Max connections per node | 16 | 5-48 |
| `ef_construction` | Search width during build | 200 | 100-400 |
| `ef_search` | Search width during query | 100 | 50-200 |

**Trade-offs:**
- **Higher M** → Better quality, more memory
- **Higher ef_construction** → Better quality, slower indexing
- **Higher ef_search** → Better recall, slower queries

### Inserting Vectors

```typescript
// Create embedding (128 dimensions)
const embedding = new Float32Array(128);
for (let i = 0; i < 128; i++) {
  embedding[i] = Math.random();
}

// Insert
await db.execute(
  'INSERT INTO documents (content, embedding) VALUES ($1, $2)',
  ['Machine learning is awesome', embedding]
);
```

### Vector Similarity Search

NexusQL uses the `<->` operator for L2 distance (Euclidean).

```typescript
// Query embedding
const queryVec = new Float32Array(128);
// ... generate query embedding ...

// Find 10 nearest neighbors
const results = await db.query(`
  SELECT
    id,
    content,
    embedding <-> $1 AS distance
  FROM documents
  ORDER BY embedding <-> $1
  LIMIT 10
`, [queryVec]);

results.rows.forEach(row => {
  console.log(`${row.content} (distance: ${row.distance})`);
});
```

### Distance Operators

| Operator | Distance Type | Description |
|----------|--------------|-------------|
| `<->` | L2 (Euclidean) | Standard distance |
| `<#>` | Inner product | Dot product (higher = more similar) |
| `<=>` | Cosine distance | Normalized cosine similarity |

**Example:**
```typescript
// L2 distance
const l2 = await db.query(`
  SELECT * FROM documents ORDER BY embedding <-> $1 LIMIT 10
`, [queryVec]);

// Cosine similarity (best for normalized embeddings)
const cosine = await db.query(`
  SELECT * FROM documents ORDER BY embedding <=> $1 LIMIT 10
`, [queryVec]);

// Inner product (for un-normalized vectors)
const innerProd = await db.query(`
  SELECT * FROM documents ORDER BY embedding <#> $1 LIMIT 10
`, [queryVec]);
```

### Filtering Vector Search

Combine vector search with traditional SQL filters:

```typescript
// Filtered vector search
const results = await db.query(`
  SELECT
    id,
    content,
    category,
    embedding <-> $1 AS distance
  FROM documents
  WHERE
    category = $2
    AND created_at > $3
    AND embedding <-> $1 < 0.5  -- Distance threshold
  ORDER BY embedding <-> $1
  LIMIT 20
`, [queryVec, 'technology', '2025-01-01']);
```

---

## Semantic Search (ColBERT)

### What is ColBERT?

**ColBERT** (Contextualized Late Interaction over BERT) is a neural search technique that:
- Uses **token-level embeddings** (not just one vector per document)
- Performs **late interaction** with MaxSim operator
- **100x faster** than BERT while maintaining quality
- Achieves **fine-grained semantic matching**

### Basic Semantic Search

```typescript
// Semantic search without pre-computing embeddings
const results = await db.query(`
  SELECT * FROM documents
  WHERE SEMANTIC_SEARCH(content, 'machine learning algorithms', 10)
`);

// How it works:
// 1. Tokenizes query: ["machine", "learning", "algorithms"]
// 2. Generates embeddings for each token
// 3. Compares token-by-token with document embeddings
// 4. MaxSim operator: for each query token, finds best matching document token
// 5. Returns top 10 most semantically similar documents
```

### MaxSim Scoring

```typescript
// Get similarity scores
const results = await db.query(`
  SELECT
    id,
    content,
    SEMANTIC_SCORE(content, 'neural networks') AS score
  FROM documents
  ORDER BY score DESC
  LIMIT 10
`);

results.rows.forEach(row => {
  console.log(`Score: ${row.score.toFixed(3)} - ${row.content}`);
});
```

### Advanced Semantic Search with Filters

```typescript
// Combine semantic search with SQL filters
const results = await db.query(`
  SELECT
    id,
    title,
    content,
    category,
    SEMANTIC_SCORE(content, $1) AS semantic_score,
    ts_rank(search_vector, to_tsquery($2)) AS text_score
  FROM articles
  WHERE
    -- Semantic filter
    SEMANTIC_SCORE(content, $1) > 0.7
    -- Category filter
    AND category IN ('AI', 'Machine Learning')
    -- Date filter
    AND published_at > NOW() - INTERVAL '30 days'
    -- Full-text search
    AND search_vector @@ to_tsquery($2)
  ORDER BY (semantic_score * 0.7 + text_score * 0.3) DESC
  LIMIT 20
`, ['deep learning transformers', 'neural & network']);
```

### Hybrid Search: Vector + Semantic + Full-Text

```typescript
// Best of all worlds: HNSW vectors + ColBERT semantic + PostgreSQL full-text
const results = await db.query(`
  SELECT
    id,
    content,
    -- Vector similarity score
    1 / (1 + embedding <-> $1) AS vector_score,
    -- Semantic similarity score
    SEMANTIC_SCORE(content, $2) AS semantic_score,
    -- Full-text relevance score
    ts_rank(search_vector, to_tsquery($3)) AS text_score
  FROM documents
  WHERE
    embedding <-> $1 < 1.0
    OR SEMANTIC_SCORE(content, $2) > 0.5
    OR search_vector @@ to_tsquery($3)
  ORDER BY
    (vector_score * 0.4 + semantic_score * 0.4 + text_score * 0.2) DESC
  LIMIT 10
`, [queryEmbedding, 'natural language processing', 'nlp & transformer']);
```

---

## Hyperdimensional Computing (HDC)

### What is HDC?

**Hyperdimensional Computing** is brain-inspired computing using:
- **10,000+ dimensional hypervectors** (vs 128-1024 for typical embeddings)
- **One-shot learning** - learn from single example
- **100x more energy efficient** than deep learning
- **Holographic memory** - information distributed across all dimensions

### Creating HDC Hypervectors

```typescript
// Create table for hypervectors
await db.execute(`
  CREATE TABLE patterns (
    id SERIAL PRIMARY KEY,
    name TEXT,
    hypervector HYPERVECTOR(10000)  -- 10,000 dimensions
  )
`);
```

### Encoding Data to Hypervectors

```typescript
// Encode text to hypervector
const text = 'The cat sat on the mat';
const hypervec = await db.query(`
  SELECT HDC_ENCODE($1) AS hv
`, [text]);

// Store hypervector
await db.execute(`
  INSERT INTO patterns (name, hypervector)
  VALUES ($1, HDC_ENCODE($2))
`, ['cat_sentence', text]);
```

### One-Shot Learning

```typescript
// Learn from single example!
async function oneShot Learning() {
  // 1. Encode single training example
  await db.execute(`
    INSERT INTO categories (name, hypervector)
    VALUES ('cat', HDC_ENCODE($1))
  `, ['cat image data...']);

  await db.execute(`
    INSERT INTO categories (name, hypervector)
    VALUES ('dog', HDC_ENCODE($1))
  `, ['dog image data...']);

  // 2. Classify new data (no training loop needed!)
  const newData = 'new cat image data...';

  const result = await db.query(`
    SELECT
      name,
      HDC_SIMILARITY(hypervector, HDC_ENCODE($1)) AS similarity
    FROM categories
    ORDER BY similarity DESC
    LIMIT 1
  `, [newData]);

  console.log(`Classified as: ${result.rows[0].name}`);
  console.log(`Confidence: ${result.rows[0].similarity}`);
}
```

### HDC Operations

#### Binding (⊗) - Combine Concepts

```typescript
// Create composite representation: "red" + "car" = "red car"
const redCar = await db.query(`
  SELECT HDC_BIND(
    HDC_ENCODE('red'),
    HDC_ENCODE('car')
  ) AS hypervec
`);

// Unbind: "red car" ⊗ "car" ≈ "red"
const unbind = await db.query(`
  SELECT HDC_UNBIND($1, HDC_ENCODE('car')) AS result
`, [redCar.rows[0].hypervec]);
```

#### Bundling (⊕) - Aggregate Concepts

```typescript
// Bundle multiple concepts into one
const animals = await db.query(`
  SELECT HDC_BUNDLE(
    HDC_ENCODE('cat'),
    HDC_ENCODE('dog'),
    HDC_ENCODE('bird')
  ) AS animals_hv
`);

// The result is similar to all inputs
const similarity = await db.query(`
  SELECT
    HDC_SIMILARITY($1, HDC_ENCODE('cat')) AS cat_sim,
    HDC_SIMILARITY($1, HDC_ENCODE('dog')) AS dog_sim,
    HDC_SIMILARITY($1, HDC_ENCODE('bird')) AS bird_sim
`, [animals.rows[0].animals_hv]);
```

### HDC Pattern Matching

```typescript
// Fast, noise-tolerant pattern matching
async function patternMatch() {
  // Store known patterns
  await db.execute(`
    INSERT INTO patterns (name, hypervector) VALUES
      ('circle', HDC_ENCODE($1)),
      ('square', HDC_ENCODE($2)),
      ('triangle', HDC_ENCODE($3))
  `, [circleData, squareData, triangleData]);

  // Match noisy input
  const noisyCircle = '...noisy circle data with 20% corruption...';

  const match = await db.query(`
    SELECT name, HDC_SIMILARITY(hypervector, HDC_ENCODE($1)) AS sim
    FROM patterns
    ORDER BY sim DESC
    LIMIT 1
  `, [noisyCircle]);

  console.log(`Matched: ${match.rows[0].name} (${match.rows[0].sim})`);
  // Output: "Matched: circle (0.89)" - even with noise!
}
```

---

## Transactions

### Basic Transactions

```typescript
const tx = await db.beginTransaction();

try {
  // Multiple operations in transaction
  await tx.query('INSERT INTO accounts (id, balance) VALUES (1, 1000)');
  await tx.query('INSERT INTO accounts (id, balance) VALUES (2, 500)');
  await tx.query('UPDATE accounts SET balance = balance - 100 WHERE id = 1');
  await tx.query('UPDATE accounts SET balance = balance + 100 WHERE id = 2');

  // Commit if all succeed
  await tx.commit();
  console.log('Transaction committed');
} catch (error) {
  // Rollback on any error
  await tx.rollback();
  console.error('Transaction rolled back:', error);
  throw error;
}
```

### Isolation Levels

```typescript
enum IsolationLevel {
  ReadUncommitted = 'READ UNCOMMITTED',
  ReadCommitted = 'READ COMMITTED',
  RepeatableRead = 'REPEATABLE READ',
  Serializable = 'SERIALIZABLE'
}

// Set isolation level
const tx = await db.beginTransaction();
await tx.query('SET TRANSACTION ISOLATION LEVEL SERIALIZABLE');
```

### Savepoints

```typescript
const tx = await db.beginTransaction();

try {
  await tx.query('INSERT INTO users (name) VALUES ($1)', ['Alice']);

  // Create savepoint
  await tx.query('SAVEPOINT sp1');

  await tx.query('INSERT INTO users (name) VALUES ($1)', ['Bob']);

  // Rollback to savepoint (undoes Bob insert, keeps Alice)
  await tx.query('ROLLBACK TO SAVEPOINT sp1');

  await tx.query('INSERT INTO users (name) VALUES ($1)', ['Charlie']);

  // Commit (Alice and Charlie inserted, Bob not)
  await tx.commit();
} catch (error) {
  await tx.rollback();
}
```

---

## Connection Pooling

For high-performance applications with concurrent requests.

```typescript
import { ConnectionPool } from 'nexusql';

// Create pool
const pool = new ConnectionPool('./mydb', {
  max: 20,              // Maximum connections
  min: 5,               // Minimum idle connections
  idleTimeoutMillis: 30000  // Close idle connections after 30s
});

// Get connection from pool
async function handleRequest() {
  const conn = await pool.getConnection();

  try {
    const result = await conn.query('SELECT * FROM users WHERE id = $1', [123]);
    return result.rows;
  } finally {
    // Always release connection back to pool
    pool.releaseConnection(conn);
  }
}

// Better: Use with automatic release
async function handleRequestAuto() {
  return pool.withConnection(async (conn) => {
    const result = await conn.query('SELECT * FROM users WHERE id = $1', [123]);
    return result.rows;
  });
}
```

### Pool Events

```typescript
pool.on('acquire', (conn) => {
  console.log('Connection acquired from pool');
});

pool.on('release', (conn) => {
  console.log('Connection released back to pool');
});

pool.on('error', (error) => {
  console.error('Pool error:', error);
});
```

---

## Prepared Statements

Prepared statements are **faster** for repeated execution and **prevent SQL injection**.

```typescript
// Prepare statement once
const stmt = await db.prepare(`
  SELECT * FROM users WHERE age > $1 AND city = $2
`);

// Execute multiple times with different parameters
const youngNY = await stmt.execute(25, 'New York');
const oldLA = await stmt.execute(50, 'Los Angeles');
const teensSF = await stmt.execute(13, 'San Francisco');

// Close when done
await stmt.close();
```

### Batch Operations

```typescript
const insert = await db.prepare(
  'INSERT INTO products (name, price) VALUES ($1, $2)'
);

const products = [
  ['Widget', 19.99],
  ['Gadget', 29.99],
  ['Doohickey', 39.99]
];

for (const [name, price] of products) {
  await insert.execute(name, price);
}

await insert.close();
```

---

## Advanced Features

### Full-Text Search

```typescript
// Create table with tsvector column
await db.execute(`
  CREATE TABLE articles (
    id SERIAL PRIMARY KEY,
    title TEXT,
    content TEXT,
    search_vector TSVECTOR
  )
`);

// Create GIN index for full-text search
await db.execute(`
  CREATE INDEX search_idx ON articles USING GIN(search_vector)
`);

// Insert with automatic tsvector generation
await db.execute(`
  INSERT INTO articles (title, content, search_vector)
  VALUES ($1, $2, to_tsvector('english', $1 || ' ' || $2))
`, ['Machine Learning', 'Article about neural networks and AI']);

// Search
const results = await db.query(`
  SELECT
    title,
    content,
    ts_rank(search_vector, query) AS rank
  FROM articles, to_tsquery('english', $1) query
  WHERE search_vector @@ query
  ORDER BY rank DESC
`, ['machine & learning']);
```

### JSON/JSONB Queries

```typescript
// Create table with JSONB column
await db.execute(`
  CREATE TABLE products (
    id SERIAL PRIMARY KEY,
    name TEXT,
    metadata JSONB
  )
`);

// Insert JSON
await db.execute(`
  INSERT INTO products (name, metadata) VALUES ($1, $2)
`, ['Widget', { color: 'red', tags: ['new', 'sale'], price: 19.99 }]);

// Query JSON fields
const redProducts = await db.query(`
  SELECT * FROM products WHERE metadata->>'color' = 'red'
`);

// Query JSON arrays
const saleProducts = await db.query(`
  SELECT * FROM products WHERE metadata @> '{"tags": ["sale"]}'
`);

// Extract JSON values
const prices = await db.query(`
  SELECT
    name,
    (metadata->>'price')::decimal AS price
  FROM products
  WHERE (metadata->>'price')::decimal < 30
`);
```

### Window Functions

```typescript
// Ranking within partitions
const results = await db.query(`
  SELECT
    employee_name,
    department,
    salary,
    ROW_NUMBER() OVER (PARTITION BY department ORDER BY salary DESC) AS rank,
    AVG(salary) OVER (PARTITION BY department) AS dept_avg
  FROM employees
`);
```

### CTEs (Common Table Expressions)

```typescript
// Recursive CTE for hierarchical data
const results = await db.query(`
  WITH RECURSIVE org_chart AS (
    -- Base case: CEO
    SELECT id, name, manager_id, 1 AS level
    FROM employees
    WHERE manager_id IS NULL

    UNION ALL

    -- Recursive case: employees reporting to previous level
    SELECT e.id, e.name, e.manager_id, oc.level + 1
    FROM employees e
    JOIN org_chart oc ON e.manager_id = oc.id
  )
  SELECT * FROM org_chart ORDER BY level, name
`);
```

---

## TypeScript Types

### Core Types

```typescript
import {
  NexusDB,
  QueryResult,
  Row,
  Field,
  Transaction,
  PreparedStatement,
  ConnectionPool,
  PoolOptions,
  NexusValue,
  IsolationLevel,
  ConnectionOptions
} from 'nexusql';
```

### Type Definitions

```typescript
// Row type
interface Row {
  [key: string]: any;
}

// Field metadata
interface Field {
  name: string;
  type: string;
}

// Query result
interface QueryResult {
  rows: Row[];
  rowCount: number;
  fields: Field[];
}

// Value types
type NexusValue = string | number | boolean | null | Uint8Array | Float32Array;

// Isolation levels
enum IsolationLevel {
  ReadUncommitted = 'READ UNCOMMITTED',
  ReadCommitted = 'READ COMMITTED',
  RepeatableRead = 'REPEATABLE READ',
  Serializable = 'SERIALIZABLE'
}

// Connection options
interface ConnectionOptions {
  database?: string;
  isolationLevel?: IsolationLevel;
  maxRetries?: number;
}

// Pool options
interface PoolOptions {
  max?: number;           // Max connections
  min?: number;           // Min idle connections
  idleTimeoutMillis?: number;
}
```

### Type-Safe Queries

```typescript
// Define your schema types
interface User {
  id: number;
  name: string;
  email: string;
  created_at: Date;
}

// Type-safe query wrapper
async function getUser(db: NexusDB, id: number): Promise<User | null> {
  const result = await db.query('SELECT * FROM users WHERE id = $1', [id]);
  return result.rows[0] as User || null;
}

// Usage
const user = await getUser(db, 123);
if (user) {
  console.log(user.name);  // TypeScript knows this is string
}
```

---

## Error Handling

### Error Types

```typescript
try {
  await db.execute('INVALID SQL');
} catch (error) {
  if (error.code === 'SYNTAX_ERROR') {
    console.error('SQL syntax error:', error.message);
  } else if (error.code === 'UNIQUE_VIOLATION') {
    console.error('Duplicate key:', error.message);
  } else if (error.code === 'FOREIGN_KEY_VIOLATION') {
    console.error('Foreign key constraint:', error.message);
  } else {
    console.error('Database error:', error);
  }
}
```

### Common Error Codes

| Error Code | Description | Solution |
|------------|-------------|----------|
| `SYNTAX_ERROR` | Invalid SQL syntax | Check SQL statement |
| `UNIQUE_VIOLATION` | Duplicate key | Use ON CONFLICT or check first |
| `FOREIGN_KEY_VIOLATION` | FK constraint | Ensure referenced row exists |
| `NOT_NULL_VIOLATION` | NULL in NOT NULL column | Provide value |
| `CHECK_VIOLATION` | CHECK constraint failed | Validate data |
| `CONNECTION_ERROR` | Can't connect | Check database path |
| `TIMEOUT` | Query timeout | Optimize query or increase timeout |

### Retry Logic

```typescript
async function queryWithRetry(
  db: NexusDB,
  sql: string,
  params: any[],
  maxRetries: number = 3
): Promise<QueryResult> {
  for (let i = 0; i < maxRetries; i++) {
    try {
      return await db.query(sql, params);
    } catch (error) {
      if (i === maxRetries - 1) throw error;
      if (error.code === 'TIMEOUT' || error.code === 'CONNECTION_ERROR') {
        await new Promise(resolve => setTimeout(resolve, 1000 * (i + 1)));
        continue;
      }
      throw error;
    }
  }
  throw new Error('Unreachable');
}
```

---

## Best Practices

### 1. Always Use Parameters

```typescript
// ❌ Bad: SQL injection vulnerability
const userId = req.params.id;
await db.query(`SELECT * FROM users WHERE id = ${userId}`);

// ✅ Good: Parameterized query
await db.query('SELECT * FROM users WHERE id = $1', [userId]);
```

### 2. Close Connections

```typescript
// ✅ Always close
async function doWork() {
  const db = new NexusDB('./mydb');
  try {
    await db.query('SELECT * FROM users');
  } finally {
    await db.close();
  }
}
```

### 3. Use Transactions for Multiple Operations

```typescript
// ✅ Atomic operations
const tx = await db.beginTransaction();
try {
  await tx.query('INSERT INTO orders (total) VALUES (100)');
  await tx.query('UPDATE inventory SET qty = qty - 1');
  await tx.commit();
} catch (error) {
  await tx.rollback();
  throw error;
}
```

### 4. Use Connection Pooling for Web Servers

```typescript
// ✅ One pool for entire app
const pool = new ConnectionPool('./mydb', { max: 20 });

app.get('/users/:id', async (req, res) => {
  const conn = await pool.getConnection();
  try {
    const result = await conn.query('SELECT * FROM users WHERE id = $1', [req.params.id]);
    res.json(result.rows[0]);
  } finally {
    pool.releaseConnection(conn);
  }
});
```

### 5. Create Indexes for Performance

```typescript
// ✅ Index frequently queried columns
await db.execute('CREATE INDEX user_email_idx ON users(email)');
await db.execute('CREATE INDEX product_category_idx ON products(category)');

// ✅ HNSW index for vector search
await db.execute(`
  CREATE INDEX vec_idx ON documents(embedding)
  USING HNSW WITH (M=16, ef_construction=200)
`);
```

### 6. Use Prepared Statements for Repeated Queries

```typescript
// ✅ Prepare once, execute many times
const stmt = await db.prepare('SELECT * FROM users WHERE city = $1');

const nyUsers = await stmt.execute('New York');
const laUsers = await stmt.execute('Los Angeles');
const sfUsers = await stmt.execute('San Francisco');

await stmt.close();
```

---

## Examples

### Example 1: E-Commerce Product Search

```typescript
import { NexusDB } from 'nexusql';

async function productSearch() {
  const db = new NexusDB('./ecommerce');

  // Create schema
  await db.execute(`
    CREATE TABLE products (
      id SERIAL PRIMARY KEY,
      name TEXT,
      description TEXT,
      category TEXT,
      price DECIMAL(10, 2),
      embedding VECTOR(128),
      search_vector TSVECTOR
    )
  `);

  // Create indexes
  await db.execute(`
    CREATE INDEX vec_idx ON products(embedding) USING HNSW;
    CREATE INDEX search_idx ON products USING GIN(search_vector);
    CREATE INDEX category_idx ON products(category);
  `);

  // Hybrid search: vector + text + filters
  const queryEmbedding = new Float32Array(128); // from ML model
  const results = await db.query(`
    SELECT
      id,
      name,
      description,
      price,
      1 / (1 + embedding <-> $1) AS vector_score,
      ts_rank(search_vector, to_tsquery($2)) AS text_score,
      SEMANTIC_SCORE(description, $3) AS semantic_score
    FROM products
    WHERE
      category = $4
      AND price BETWEEN $5 AND $6
      AND (
        embedding <-> $1 < 1.0
        OR search_vector @@ to_tsquery($2)
        OR SEMANTIC_SCORE(description, $3) > 0.5
      )
    ORDER BY (vector_score * 0.4 + text_score * 0.3 + semantic_score * 0.3) DESC
    LIMIT 20
  `, [
    queryEmbedding,
    'wireless & headphones',
    'noise cancelling bluetooth',
    'electronics',
    50,
    300
  ]);

  console.log(`Found ${results.rowCount} products`);
  results.rows.forEach(p => {
    console.log(`${p.name} - $${p.price} (score: ${p.vector_score})`);
  });

  await db.close();
}
```

### Example 2: Document Similarity with Semantic Search

```typescript
async function documentSimilarity() {
  const db = new NexusDB('./documents');

  await db.execute(`
    CREATE TABLE documents (
      id SERIAL PRIMARY KEY,
      title TEXT,
      content TEXT,
      embedding VECTOR(768)
    )
  `);

  // Insert documents
  const docs = [
    'Neural networks are the foundation of deep learning',
    'Machine learning algorithms learn from data',
    'Transformers revolutionized natural language processing'
  ];

  for (const doc of docs) {
    const embedding = await generateEmbedding(doc); // Your ML model
    await db.execute(
      'INSERT INTO documents (content, embedding) VALUES ($1, $2)',
      [doc, embedding]
    );
  }

  // Find similar documents using ColBERT
  const query = 'deep learning neural architectures';
  const similar = await db.query(`
    SELECT
      content,
      SEMANTIC_SCORE(content, $1) AS score
    FROM documents
    ORDER BY score DESC
    LIMIT 5
  `, [query]);

  console.log('Most similar documents:');
  similar.rows.forEach(doc => {
    console.log(`${doc.score.toFixed(3)}: ${doc.content}`);
  });

  await db.close();
}
```

### Example 3: Real-Time Classification with HDC

```typescript
async function hdcClassifier() {
  const db = new NexusDB('./classifier');

  // Create category table
  await db.execute(`
    CREATE TABLE categories (
      id SERIAL PRIMARY KEY,
      name TEXT,
      hypervector HYPERVECTOR(10000)
    )
  `);

  // One-shot learning: add categories with single example each
  await db.execute(`
    INSERT INTO categories (name, hypervector) VALUES
      ('spam', HDC_ENCODE($1)),
      ('important', HDC_ENCODE($2)),
      ('promotional', HDC_ENCODE($3))
  `, [
    'Buy now! Limited offer!',
    'Meeting at 3pm tomorrow',
    'New products available'
  ]);

  // Classify new emails
  const testEmails = [
    'Urgent: Project deadline',
    'Get 50% off today only!',
    'Check out our latest deals'
  ];

  for (const email of testEmails) {
    const result = await db.query(`
      SELECT
        name,
        HDC_SIMILARITY(hypervector, HDC_ENCODE($1)) AS confidence
      FROM categories
      ORDER BY confidence DESC
      LIMIT 1
    `, [email]);

    console.log(`"${email}" → ${result.rows[0].name} (${result.rows[0].confidence})`);
  }

  await db.close();
}
```

---

## Performance Tips

### 1. Batch Inserts

```typescript
// ❌ Slow: One query per row
for (const user of users) {
  await db.execute('INSERT INTO users (name) VALUES ($1)', [user.name]);
}

// ✅ Fast: Batch insert
await db.execute(`
  INSERT INTO users (name) VALUES
    ${users.map((_, i) => `($${i + 1})`).join(', ')}
`, users.map(u => u.name));
```

### 2. Use EXPLAIN to Analyze Queries

```typescript
const plan = await db.query('EXPLAIN SELECT * FROM users WHERE email = $1', ['test@example.com']);
console.log(plan.rows);

// Check if index is used
// Look for "Index Scan" vs "Sequential Scan"
```

### 3. Monitor Query Cache

```typescript
const stats = await db.query('SELECT * FROM pg_stat_user_tables');
console.log('Cache stats:', stats.rows);
```

---

## Next Steps

- **[DROP_IN_REPLACEMENT_GUIDE.md](./DROP_IN_REPLACEMENT_GUIDE.md)** - Migrate from SQLite
- **[BUILDING_BINDINGS.md](./BUILDING_BINDINGS.md)** - Build from source
- **[COMPARISON_SQLITE_NEXUSQL.md](./COMPARISON_SQLITE_NEXUSQL.md)** - Detailed comparison

---

**Last Updated:** November 2, 2025
**Version:** 2.4.0
**TypeScript Bindings:** Beta
