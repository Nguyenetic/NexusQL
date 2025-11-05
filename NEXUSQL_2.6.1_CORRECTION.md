# NexusQL v2.6.1 - Bug Report Correction & Update

## Response to Bug Report for v2.6.0

Thank you for the detailed testing! Your report helped us identify and fix a critical issue with UPDATE/DELETE WHERE clauses. Here's what we found:

---

## Issue 1: API Usage ❌ (Not a Bug)

**Your Test:**
```javascript
await db.execute("INSERT INTO users (name, age) VALUES ($1, $2)", ["Alice", 30]);
```

**Problem:**
The `execute()` method doesn't accept parameters. This is a user error, not a package bug.

**Correct API:**
```javascript
await db.executeWithParams("INSERT INTO users (name, age) VALUES ($1, $2)", ["Alice", 30]);
// or
await db.queryWithParams("SELECT * FROM users WHERE age > $1", [25]);
```

**Key Points:**
- Use `executeWithParams()` for INSERT/UPDATE/DELETE with parameters
- Use `queryWithParams()` for SELECT with parameters
- Regular `execute()` and `query()` don't support parameters
- Parameter syntax is `$1, $2, $3` (PostgreSQL-style, not `?`)

---

## Issue 2: UPDATE/DELETE WHERE Clauses ✅ (Real Bug - FIXED in v2.6.1)

**Your Discovery:**
UPDATE and DELETE with WHERE clauses and parameters were failing with "Unsupported value type in WHERE".

**Root Cause:**
The parser was using `convert_where_expr()` which didn't support placeholder values (`$1, $2`).

**Fix Applied:**
Changed both UPDATE and DELETE WHERE clause parsing to use `convert_expr_to_ast()` which properly handles all expression types including parameters.

**Files Changed:**
- `src/query/parser.rs:106-110` (UPDATE WHERE)
- `src/query/parser.rs:148-152` (DELETE WHERE)

---

## Working Examples (v2.6.1)

### INSERT with Parameters
```javascript
const NexusDB = require('@nguyenetic/nexusql');
const db = new NexusDB(':memory:');

await db.execute('CREATE TABLE users (name TEXT, age INTEGER)');
await db.executeWithParams(
    'INSERT INTO users (name, age) VALUES ($1, $2)',
    ['Alice', 30]
);
```

### SELECT with Parameters
```javascript
const result = await db.queryWithParams(
    'SELECT * FROM users WHERE age > $1',
    [25]
);
const rows = JSON.parse(result.rows); // Important: parse JSON string
console.log(rows); // [{ name: 'Alice', age: 30 }]
```

### UPDATE with Parameters (NOW WORKING)
```javascript
await db.executeWithParams(
    'UPDATE users SET age = $1 WHERE name = $2',
    [31, 'Alice']
);
```

### DELETE with Parameters (NOW WORKING)
```javascript
await db.executeWithParams(
    'DELETE FROM users WHERE age < $1',
    [25]
);
```

### SQL Injection Prevention
```javascript
const maliciousInput = "'; DROP TABLE users; --";
await db.executeWithParams(
    'INSERT INTO users (name, age) VALUES ($1, $2)',
    [maliciousInput, 25]
);
// Safe! The malicious string is treated as data, not code
```

---

## Important Notes

1. **Result Parsing:** Query results return `rows` as a JSON string. You must call `JSON.parse(result.rows)` to get the actual array.

2. **Parameter Style:** Use PostgreSQL-style `$1, $2, $3` not question marks `?`

3. **Method Names:**
   - `executeWithParams()` - for INSERT, UPDATE, DELETE
   - `queryWithParams()` - for SELECT

4. **Type Conversion:** JavaScript types automatically map to SQL types:
   - String → TEXT
   - Number (integer) → INTEGER
   - Number (float) → REAL
   - Boolean → INTEGER (0/1)
   - null → NULL

---

## Version History

- **v2.6.0** - Initial parameterized query support (INSERT, SELECT worked; UPDATE/DELETE WHERE failed)
- **v2.6.1** - Fixed UPDATE/DELETE WHERE clause parameter support

---

## Upgrade to v2.6.1

```bash
npm install @nguyenetic/nexusql@latest
```

All 8 comprehensive tests now pass, including UPDATE and DELETE with WHERE clause parameters.

## Thank You!

Your bug report helped us discover and fix the UPDATE/DELETE WHERE issue. The package now has complete parameterized query support across all SQL operations.
