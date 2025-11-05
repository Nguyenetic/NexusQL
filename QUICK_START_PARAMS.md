# Parameterized Queries - Quick Start

## Installation

No additional setup required - parameterized queries are built into NexusQL!

## Basic Usage

### SELECT Query

```javascript
const { NexusDb } = require('@nguyenetic/nexusql');
const db = new NexusDb('./mydb.db');

// Query with parameters
const result = await db.queryWithParams(
  'SELECT * FROM users WHERE id = $1',
  [userId]
);

// Parse results
const rows = JSON.parse(result.rows);
console.log(rows[0]); // { id: 1, name: 'Alice', ... }
```

### INSERT Statement

```javascript
await db.executeWithParams(
  'INSERT INTO users (name, email, age) VALUES ($1, $2, $3)',
  ['Bob Smith', 'bob@example.com', 30]
);
```

### UPDATE Statement

```javascript
await db.executeWithParams(
  'UPDATE users SET email = $1 WHERE id = $2',
  ['newemail@example.com', userId]
);
```

### DELETE Statement

```javascript
await db.executeWithParams(
  'DELETE FROM users WHERE age < $1',
  [18]
);
```

## Why Use Parameters?

### ❌ Unsafe (SQL Injection Risk)
```javascript
const username = req.body.username; // Could be malicious!
await db.query(`SELECT * FROM users WHERE username = '${username}'`);
```

### ✅ Safe (Protected)
```javascript
const username = req.body.username;
await db.queryWithParams(
  'SELECT * FROM users WHERE username = $1',
  [username]
);
```

## Common Patterns

### Multiple Parameters
```javascript
const result = await db.queryWithParams(
  'SELECT * FROM products WHERE category = $1 AND price < $2',
  ['electronics', 100]
);
```

### Search with LIKE
```javascript
const result = await db.queryWithParams(
  'SELECT * FROM users WHERE name LIKE $1',
  ['John%']  // Starts with "John"
);
```

### Conditional WHERE
```javascript
const result = await db.queryWithParams(
  'SELECT * FROM users WHERE age BETWEEN $1 AND $2 AND status = $3',
  [25, 45, 'active']
);
```

## Remember

1. **Always parse rows**: `const rows = JSON.parse(result.rows)`
2. **Use $1, $2, $3** for parameters (not ?)
3. **Parameters start at $1** (not $0)
4. **executeWithParams** for INSERT/UPDATE/DELETE
5. **queryWithParams** for SELECT

## Full Documentation

See [PARAMETERIZED_QUERIES.md](./PARAMETERIZED_QUERIES.md) for complete documentation.
