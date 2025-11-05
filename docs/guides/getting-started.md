# Getting Started with NexusQL

Welcome to NexusQL! This guide will help you get up and running in minutes.

## 📦 Installation

### Node.js / TypeScript

```bash
npm install @nguyenetic/nexusql
```

**Requirements:**
- Node.js 16.0.0 or higher
- npm, yarn, or pnpm

### Rust

Add to your `Cargo.toml`:
```toml
[dependencies]
nexusql = { git = "https://github.com/Nguyenetic/NexusQL.git" }
tokio = { version = "1", features = ["full"] }
```

### Python (Coming Soon)

```bash
pip install nexusql
```

---

## 🚀 Your First Database

### TypeScript/JavaScript

Create a file `app.js`:

```javascript
const { NexusDb } = require('@nguyenetic/nexusql');

async function main() {
  // 1. Create/open database
  const db = new NexusDb('./my-first-db.db');

  try {
    // 2. Create a table
    await db.execute(`
      CREATE TABLE users (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        email TEXT UNIQUE,
        created_at TEXT DEFAULT CURRENT_TIMESTAMP
      )
    `);

    // 3. Insert data (with SQL injection protection)
    await db.execute(
      'INSERT INTO users (id, name, email) VALUES ($1, $2, $3)',
      [1, 'Alice', 'alice@example.com']
    );

    await db.execute(
      'INSERT INTO users (id, name, email) VALUES ($1, $2, $3)',
      [2, 'Bob', 'bob@example.com']
    );

    // 4. Query data
    const result = await db.query('SELECT * FROM users');
    console.log('All users:', result.rows);

    // 5. Query with parameters
    const alice = await db.query(
      'SELECT * FROM users WHERE name = $1',
      ['Alice']
    );
    console.log('Found:', alice.rows[0]);

    // 6. Update data
    await db.execute(
      'UPDATE users SET email = $1 WHERE id = $2',
      ['alice.smith@example.com', 1]
    );

    // 7. Delete data
    await db.execute('DELETE FROM users WHERE id = $1', [2]);

    console.log('✅ Success!');

  } catch (error) {
    console.error('❌ Error:', error.message);
  } finally {
    // 8. Always close the database
    await db.close();
  }
}

main();
```

Run it:
```bash
node app.js
```

### TypeScript

Create `app.ts`:

```typescript
import { NexusDb, QueryResult } from '@nguyenetic/nexusql';

interface User {
  id: number;
  name: string;
  email: string;
  created_at: string;
}

async function main() {
  const db = new NexusDb('./my-first-db.db');

  try {
    // Create table
    await db.execute(`
      CREATE TABLE users (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        email TEXT UNIQUE,
        created_at TEXT DEFAULT CURRENT_TIMESTAMP
      )
    `);

    // Insert with type safety
    await db.execute(
      'INSERT INTO users (id, name, email) VALUES ($1, $2, $3)',
      [1, 'Alice', 'alice@example.com']
    );

    // Query with type assertion
    const result = await db.query('SELECT * FROM users');
    const users = result.rows as User[];

    users.forEach(user => {
      console.log(`${user.name} <${user.email}>`);
    });

  } finally {
    await db.close();
  }
}

main();
```

Compile and run:
```bash
npx tsc app.ts
node app.js
```

### Rust

Create `src/main.rs`:

```rust
use nexusql::NexusSQL;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Create/open database
    let db = NexusSQL::new("./my-first-db.db").await?;

    // 2. Create table
    db.execute("
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT UNIQUE
        )
    ").await?;

    // 3. Insert data
    db.execute("INSERT INTO users VALUES (1, 'Alice', 'alice@example.com')").await?;
    db.execute("INSERT INTO users VALUES (2, 'Bob', 'bob@example.com')").await?;

    // 4. Query data
    let result = db.query("SELECT * FROM users").await?;
    println!("Found {} users", result.rows.len());

    for row in result.rows {
        println!("{:?}", row);
    }

    // 5. Cleanup
    db.shutdown().await?;

    println!("✅ Success!");

    Ok(())
}
```

Run:
```bash
cargo run
```

---

## 📚 Core Concepts

### 1. Database Instances

```javascript
// File-based database (persistent)
const db = new NexusDb('./myapp.db');

// In-memory database (temporary, faster)
const memDb = new NexusDb(':memory:');
```

**When to use file-based:**
- Production applications
- Data persistence required
- Multiple processes accessing same data

**When to use in-memory:**
- Testing (3-10× faster)
- Temporary data
- Single process
- No persistence needed

### 2. Parameterized Queries (SQL Injection Prevention)

**Always use parameterized queries for user input:**

```javascript
// ✅ SAFE - Parameterized query
const userId = req.params.id; // User input
const result = await db.query(
  'SELECT * FROM users WHERE id = $1',
  [userId]
);

// ❌ UNSAFE - String concatenation (NEVER DO THIS)
const result = await db.query(
  `SELECT * FROM users WHERE id = ${userId}`
);
```

**Why it matters:**
- Prevents SQL injection attacks
- Automatic type conversion
- Better performance (query caching)

### 3. Transactions

Use transactions for multi-step operations that must succeed or fail together:

```javascript
const tx = await db.beginTransaction();

try {
  await tx.query('UPDATE accounts SET balance = balance - 100 WHERE id = 1');
  await tx.query('UPDATE accounts SET balance = balance + 100 WHERE id = 2');
  await tx.commit(); // ✅ Both updates succeed
} catch (error) {
  await tx.rollback(); // ❌ Both updates fail
  throw error;
}
```

**ACID guarantees:**
- **Atomicity:** All or nothing
- **Consistency:** Valid state maintained
- **Isolation:** Transactions don't interfere
- **Durability:** Changes survive crashes

### 4. Prepared Statements (Performance)

For repeated queries with different parameters:

```javascript
// Compile once
const stmt = await db.prepare(
  'INSERT INTO logs (level, message) VALUES ($1, $2)'
);

// Execute many times (10× faster)
for (let i = 0; i < 1000; i++) {
  await stmt.execute('INFO', `Log message ${i}`);
}

await stmt.close();
```

**Benefits:**
- 10× faster than individual queries
- Compile once, execute many times
- Better for bulk operations

---

## 🎯 Common Patterns

### Pattern 1: CRUD Operations

```javascript
// CREATE
await db.execute(
  'INSERT INTO products (name, price) VALUES ($1, $2)',
  ['Widget', 9.99]
);

// READ
const products = await db.query('SELECT * FROM products');

// UPDATE
await db.execute(
  'UPDATE products SET price = $1 WHERE name = $2',
  [12.99, 'Widget']
);

// DELETE
await db.execute('DELETE FROM products WHERE name = $1', ['Widget']);
```

### Pattern 2: Error Handling

```javascript
async function safeQuery() {
  let db;

  try {
    db = new NexusDb('./app.db');
    const result = await db.query('SELECT * FROM users');
    return result.rows;
  } catch (error) {
    console.error('Database error:', error.message);
    throw error;
  } finally {
    if (db) {
      await db.close();
    }
  }
}
```

### Pattern 3: Connection Management

```javascript
class Database {
  constructor(path) {
    this.db = new NexusDb(path);
  }

  async query(sql, params) {
    return await this.db.query(sql, params);
  }

  async execute(sql, params) {
    return await this.db.execute(sql, params);
  }

  async close() {
    await this.db.close();
  }
}

// Usage
const db = new Database('./app.db');
try {
  const users = await db.query('SELECT * FROM users');
  // ... use data
} finally {
  await db.close();
}
```

---

## 🔒 Security Best Practices

### 1. Always Use Parameterized Queries

```javascript
// User input
const username = req.body.username;
const password = req.body.password;

// ✅ SAFE
await db.query(
  'SELECT * FROM users WHERE username = $1 AND password = $2',
  [username, password]
);

// ❌ VULNERABLE TO SQL INJECTION
await db.query(
  `SELECT * FROM users WHERE username = '${username}' AND password = '${password}'`
);
```

### 2. Validate Input

```javascript
function validateEmail(email) {
  const regex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
  return regex.test(email);
}

if (!validateEmail(userEmail)) {
  throw new Error('Invalid email format');
}

await db.execute(
  'INSERT INTO users (email) VALUES ($1)',
  [userEmail]
);
```

### 3. Hash Passwords

```javascript
const bcrypt = require('bcrypt');

// Register user
const hash = await bcrypt.hash(password, 10);
await db.execute(
  'INSERT INTO users (username, password_hash) VALUES ($1, $2)',
  [username, hash]
);

// Login user
const user = await db.query(
  'SELECT password_hash FROM users WHERE username = $1',
  [username]
);

const valid = await bcrypt.compare(password, user.rows[0].password_hash);
```

### 4. Use Transactions for Critical Operations

```javascript
// Money transfer - must be atomic
const tx = await db.beginTransaction();

try {
  await tx.query('UPDATE accounts SET balance = balance - $1 WHERE id = $2', [amount, fromId]);
  await tx.query('UPDATE accounts SET balance = balance + $1 WHERE id = $2', [amount, toId]);
  await tx.commit();
} catch (error) {
  await tx.rollback();
  throw error;
}
```

---

## 🧪 Testing

### Use In-Memory Databases for Tests

```javascript
const { NexusDb } = require('@nguyenetic/nexusql');

describe('User CRUD', () => {
  let db;

  beforeEach(async () => {
    // Use :memory: for fast, isolated tests
    db = new NexusDb(':memory:');
    await db.execute(`
      CREATE TABLE users (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL
      )
    `);
  });

  afterEach(async () => {
    await db.close();
  });

  test('should insert user', async () => {
    await db.execute('INSERT INTO users VALUES ($1, $2)', [1, 'Alice']);
    const result = await db.query('SELECT * FROM users WHERE id = $1', [1]);
    expect(result.rows[0].name).toBe('Alice');
  });
});
```

**Benefits of `:memory:`:**
- 3-10× faster than disk
- Perfect isolation between tests
- No cleanup needed
- No file locking issues

---

## 🚨 Common Mistakes to Avoid

### ❌ Mistake 1: Forgetting to Close Database

```javascript
// BAD
const db = new NexusDb('./app.db');
await db.query('SELECT * FROM users');
// Database not closed - resource leak!

// GOOD
const db = new NexusDb('./app.db');
try {
  await db.query('SELECT * FROM users');
} finally {
  await db.close();
}
```

### ❌ Mistake 2: String Concatenation with User Input

```javascript
// BAD - SQL injection vulnerability
const userId = req.params.id;
await db.query(`SELECT * FROM users WHERE id = ${userId}`);

// GOOD - Parameterized query
await db.query('SELECT * FROM users WHERE id = $1', [userId]);
```

### ❌ Mistake 3: Not Using Transactions for Multi-Step Operations

```javascript
// BAD - Can leave data in inconsistent state
await db.execute('UPDATE inventory SET quantity = quantity - 1 WHERE product_id = 1');
// If this fails, inventory is already decremented!
await db.execute('INSERT INTO orders (product_id, quantity) VALUES (1, 1)');

// GOOD - Atomic operation
const tx = await db.beginTransaction();
try {
  await tx.query('UPDATE inventory SET quantity = quantity - 1 WHERE product_id = 1');
  await tx.query('INSERT INTO orders (product_id, quantity) VALUES (1, 1)');
  await tx.commit();
} catch (error) {
  await tx.rollback();
  throw error;
}
```

### ❌ Mistake 4: Ignoring Error Handling

```javascript
// BAD - Errors silently fail
await db.execute('INSERT INTO users VALUES ($1, $2)', [1, 'Alice']);

// GOOD - Proper error handling
try {
  await db.execute('INSERT INTO users VALUES ($1, $2)', [1, 'Alice']);
} catch (error) {
  if (error.message.includes('UNIQUE constraint')) {
    console.log('User already exists');
  } else {
    console.error('Insert failed:', error);
  }
}
```

---

## 📖 Next Steps

Now that you've learned the basics, explore these topics:

1. **[Vector Search](./vector-search.md)** - Semantic search with embeddings
2. **[Transactions](./transactions.md)** - Advanced transaction patterns
3. **[Performance](./performance.md)** - Optimization techniques
4. **[Migration](./migration/)** - Migrate from SQLite or PostgreSQL
5. **[API Reference](../api/typescript/README.md)** - Complete API documentation

---

## 💡 Quick Reference

### Common Commands

```javascript
// Database
const db = new NexusDb(path);
await db.close();

// Query (SELECT)
const result = await db.query(sql, [params]);
console.log(result.rows);

// Execute (INSERT/UPDATE/DELETE)
await db.execute(sql, [params]);

// Transaction
const tx = await db.beginTransaction();
await tx.commit();
await tx.rollback();

// Prepared Statement
const stmt = await db.prepare(sql);
await stmt.execute(params);
await stmt.close();
```

### SQL Reference

```sql
-- Create table
CREATE TABLE users (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  email TEXT UNIQUE
);

-- Insert
INSERT INTO users (id, name, email) VALUES (1, 'Alice', 'alice@example.com');

-- Select
SELECT * FROM users WHERE id = 1;

-- Update
UPDATE users SET email = 'new@example.com' WHERE id = 1;

-- Delete
DELETE FROM users WHERE id = 1;

-- Create index
CREATE INDEX idx_users_email ON users(email);
```

---

## 🆘 Getting Help

- **Documentation:** [docs/](../../)
- **Examples:** [bindings/typescript/examples/](../../bindings/typescript/examples/)
- **API Reference:** [docs/api/typescript/](../api/typescript/)
- **Issues:** [GitHub Issues](https://github.com/Nguyenetic/NexusQL/issues)
- **Discussions:** [GitHub Discussions](https://github.com/Nguyenetic/NexusQL/discussions)

---

**Happy coding with NexusQL! 🚀**
