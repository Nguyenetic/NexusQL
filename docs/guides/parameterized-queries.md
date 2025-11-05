# Parameterized Queries in NexusQL

## Overview

Parameterized queries are the **recommended and secure way** to pass dynamic values to SQL statements. They prevent SQL injection attacks by separating SQL code from user-provided data.

## Why Use Parameterized Queries?

### SQL Injection Prevention

**❌ Dangerous (Vulnerable to SQL Injection):**
```javascript
const username = req.body.username; // Could be: "admin' OR '1'='1"
await db.query(`SELECT * FROM users WHERE username = '${username}'`);
// Results in: SELECT * FROM users WHERE username = 'admin' OR '1'='1'
// This returns ALL users!
```

**✅ Safe (Using Parameters):**
```javascript
const username = req.body.username;
await db.queryWithParams(
  'SELECT * FROM users WHERE username = $1',
  [username]
);
// The database treats the entire input as a string value, not SQL code
```

### Performance Benefits

- Query plans can be cached and reused
- Database doesn't need to re-parse similar queries
- Optimized execution for repeated queries

## Supported Parameter Styles

NexusQL supports **PostgreSQL-style positional parameters**:

### Positional Parameters ($1, $2, $3, ...)

```javascript
// Single parameter
const result = await db.queryWithParams(
  'SELECT * FROM users WHERE id = $1',
  [userId]
);

// Multiple parameters
const result = await db.queryWithParams(
  'SELECT * FROM products WHERE category = $1 AND price < $2',
  ['electronics', 99.99]
);

// Parameters can be reused
const result = await db.queryWithParams(
  'SELECT * FROM users WHERE age BETWEEN $1 AND $2 OR status = $3',
  [18, 65, 'active']
);
```

## API Reference

### queryWithParams(sql, params)

Execute a SELECT query with parameters and return results.

**Parameters:**
- `sql` (string): SQL query with $1, $2, etc. placeholders
- `params` (array): Array of values to bind to placeholders

**Returns:** Promise<QueryResult>
- `rows` (string): JSON string of result rows (parse with `JSON.parse()`)
- `rowCount` (number): Number of rows returned
- `fields` (array): Column metadata

**Example:**
```javascript
const result = await db.queryWithParams(
  'SELECT * FROM users WHERE age > $1 AND city = $2',
  [25, 'New York']
);

const rows = JSON.parse(result.rows);
console.log(rows); // [{ id: 1, name: 'Alice', age: 30, city: 'New York' }, ...]
```

### executeWithParams(sql, params)

Execute INSERT, UPDATE, or DELETE statements with parameters.

**Parameters:**
- `sql` (string): SQL statement with $1, $2, etc. placeholders
- `params` (array): Array of values to bind to placeholders

**Returns:** Promise<number> - Number of affected rows

**Example:**
```javascript
// INSERT
const rowsAffected = await db.executeWithParams(
  'INSERT INTO users (name, email, age) VALUES ($1, $2, $3)',
  ['Bob Smith', 'bob@example.com', 35]
);

// UPDATE
const updated = await db.executeWithParams(
  'UPDATE users SET email = $1 WHERE id = $2',
  ['newemail@example.com', 123]
);

// DELETE
const deleted = await db.executeWithParams(
  'DELETE FROM users WHERE age < $1 AND status = $2',
  [18, 'inactive']
);
```

## Supported Data Types

NexusQL automatically handles type conversion for common JavaScript types:

| JavaScript Type | SQL Type | Example |
|----------------|----------|---------|
| `number` (integer) | INTEGER | `42` |
| `number` (float) | REAL | `3.14159` |
| `string` | TEXT | `'Hello World'` |
| `boolean` | BOOLEAN | `true`, `false` |
| `null` | NULL | `null` |

**Example:**
```javascript
await db.executeWithParams(
  'INSERT INTO products (name, price, in_stock, description) VALUES ($1, $2, $3, $4)',
  [
    'Laptop',        // string -> TEXT
    999.99,          // number -> REAL
    true,            // boolean -> BOOLEAN
    'High performance laptop'  // string -> TEXT
  ]
);
```

## Common Patterns

### INSERT with Multiple Rows

```javascript
const users = [
  ['Alice', 'alice@example.com', 30],
  ['Bob', 'bob@example.com', 25],
  ['Charlie', 'charlie@example.com', 35]
];

for (const [name, email, age] of users) {
  await db.executeWithParams(
    'INSERT INTO users (name, email, age) VALUES ($1, $2, $3)',
    [name, email, age]
  );
}
```

### Dynamic WHERE Clauses

```javascript
async function searchUsers(filters) {
  const { minAge, maxAge, city, status } = filters;

  const result = await db.queryWithParams(
    `SELECT * FROM users
     WHERE age BETWEEN $1 AND $2
       AND city = $3
       AND status = $4`,
    [minAge, maxAge, city, status]
  );

  return JSON.parse(result.rows);
}

const users = await searchUsers({
  minAge: 25,
  maxAge: 45,
  city: 'San Francisco',
  status: 'active'
});
```

### Conditional Parameters

```javascript
async function getUsers({ minAge, city }) {
  let sql = 'SELECT * FROM users WHERE 1=1';
  const params = [];
  let paramIndex = 1;

  if (minAge !== undefined) {
    sql += ` AND age >= $${paramIndex}`;
    params.push(minAge);
    paramIndex++;
  }

  if (city !== undefined) {
    sql += ` AND city = $${paramIndex}`;
    params.push(city);
    paramIndex++;
  }

  const result = await db.queryWithParams(sql, params);
  return JSON.parse(result.rows);
}
```

### LIKE Queries

```javascript
// Search by prefix
const result = await db.queryWithParams(
  'SELECT * FROM users WHERE name LIKE $1',
  ['John%']  // Finds: John, Johnny, Johnson
);

// Search by suffix
const result = await db.queryWithParams(
  'SELECT * FROM users WHERE email LIKE $1',
  ['%@gmail.com']  // Finds all Gmail addresses
);

// Search anywhere
const result = await db.queryWithParams(
  'SELECT * FROM products WHERE description LIKE $1',
  ['%laptop%']  // Finds descriptions containing "laptop"
);
```

### IN Clauses (Array of Values)

```javascript
// Note: Arrays need special handling in current version
// Workaround for multiple values:
const ids = [1, 2, 3, 4, 5];
const placeholders = ids.map((_, i) => `$${i + 1}`).join(',');
const sql = `SELECT * FROM users WHERE id IN (${placeholders})`;

const result = await db.queryWithParams(sql, ids);
```

## Security Best Practices

### ✅ DO: Always Use Parameters for User Input

```javascript
// Good
const userId = req.params.id;
const result = await db.queryWithParams(
  'SELECT * FROM users WHERE id = $1',
  [userId]
);
```

### ❌ DON'T: Concatenate User Input

```javascript
// Bad - vulnerable to SQL injection!
const userId = req.params.id;
const result = await db.query(
  `SELECT * FROM users WHERE id = ${userId}`
);
```

### ✅ DO: Validate Input Before Queries

```javascript
const userId = parseInt(req.params.id);
if (isNaN(userId)) {
  throw new Error('Invalid user ID');
}

const result = await db.queryWithParams(
  'SELECT * FROM users WHERE id = $1',
  [userId]
);
```

### ✅ DO: Use Specific Queries Over Dynamic SQL

```javascript
// Good - specific and safe
const result = await db.queryWithParams(
  'SELECT id, name, email FROM users WHERE id = $1',
  [userId]
);

// Avoid - overly dynamic
// const columns = req.body.columns.join(','); // DON'T DO THIS
// const result = await db.query(`SELECT ${columns} FROM users`);
```

## Migration Guide

### From String Concatenation

**Before:**
```javascript
const username = 'alice';
const age = 30;
await db.execute(
  `INSERT INTO users (name, age) VALUES ('${username}', ${age})`
);
```

**After:**
```javascript
const username = 'alice';
const age = 30;
await db.executeWithParams(
  'INSERT INTO users (name, age) VALUES ($1, $2)',
  [username, age]
);
```

### From Template Literals

**Before:**
```javascript
const userId = 123;
const result = await db.query(
  `SELECT * FROM users WHERE id = ${userId}`
);
```

**After:**
```javascript
const userId = 123;
const result = await db.queryWithParams(
  'SELECT * FROM users WHERE id = $1',
  [userId]
);
```

## Testing SQL Injection Prevention

Here's how to verify your queries are secure:

```javascript
// This should safely handle malicious input
const maliciousInput = "'; DROP TABLE users; --";

const result = await db.queryWithParams(
  'SELECT * FROM users WHERE username = $1',
  [maliciousInput]
);

// The database treats the entire string as a literal value
// It will search for a user with username: '; DROP TABLE users; --
// It will NOT execute the DROP TABLE command
```

## Performance Considerations

### Query Plan Caching

Parameterized queries allow the database to cache execution plans:

```javascript
// First execution - query plan is created
await db.queryWithParams('SELECT * FROM users WHERE id = $1', [1]);

// Subsequent executions reuse the cached plan (faster!)
await db.queryWithParams('SELECT * FROM users WHERE id = $1', [2]);
await db.queryWithParams('SELECT * FROM users WHERE id = $1', [3]);
```

### Batch Operations

For bulk inserts, parameters are still recommended:

```javascript
const users = [...]; // Large array of users

// Good - uses parameters for each insert
for (const user of users) {
  await db.executeWithParams(
    'INSERT INTO users (name, email) VALUES ($1, $2)',
    [user.name, user.email]
  );
}
```

## Backward Compatibility

Old code without parameters continues to work:

```javascript
// Still works (for non-dynamic queries only!)
await db.execute("INSERT INTO users (name) VALUES ('Alice')");
await db.query('SELECT * FROM users');
```

But we **strongly recommend** migrating to parameterized queries for all dynamic values.

## Troubleshooting

### "Parse error: Unsupported value type"

This error means the query contains a value type that cannot be parsed. Make sure you're using parameters correctly:

```javascript
// Wrong - passing object directly
await db.executeWithParams('INSERT INTO users VALUES ($1)', [{ name: 'Alice' }]);

// Right - pass individual values
await db.executeWithParams('INSERT INTO users (name) VALUES ($1)', ['Alice']);
```

### Parameter Count Mismatch

```javascript
// Error - 2 placeholders but only 1 parameter
await db.queryWithParams(
  'SELECT * FROM users WHERE age > $1 AND city = $2',
  [25]  // Missing city parameter
);

// Fixed
await db.queryWithParams(
  'SELECT * FROM users WHERE age > $1 AND city = $2',
  [25, 'New York']
);
```

### Result Rows are Strings

Query results return `rows` as a JSON string. Always parse it:

```javascript
const result = await db.queryWithParams('SELECT * FROM users WHERE id = $1', [1]);

// Wrong
console.log(result.rows[0]); // undefined (rows is a string, not an array)

// Right
const rows = JSON.parse(result.rows);
console.log(rows[0]); // { id: 1, name: 'Alice', ... }
```

## Examples

See `test-parameterized-queries.js` for comprehensive examples including:
- Basic SELECT/INSERT/UPDATE/DELETE with parameters
- SQL injection prevention tests
- Multiple parameter types
- Error handling
- Edge cases

## Further Reading

- [PostgreSQL Prepared Statements](https://www.postgresql.org/docs/current/sql-prepare.html)
- [OWASP SQL Injection Prevention](https://cheatsheetseries.owasp.org/cheatsheets/SQL_Injection_Prevention_Cheat_Sheet.html)
- [Node-Postgres Parameterized Queries](https://node-postgres.com/features/queries#parameterized-query)
