# NexusQL v2.6.0 Release Notes

## 🔒 Security Release: Parameterized Queries

We're excited to announce NexusQL v2.6.0, featuring **parameterized query support** to prevent SQL injection vulnerabilities!

## 🎯 What's New

### Parameterized Queries (SQL Injection Prevention)

NexusQL now supports PostgreSQL-style parameterized queries, providing a secure way to handle user input in SQL statements.

**New Methods:**
- `queryWithParams(sql, params)` - Execute SELECT queries with parameters
- `executeWithParams(sql, params)` - Execute INSERT/UPDATE/DELETE with parameters

**Example:**
```javascript
// Safe from SQL injection!
const result = await db.queryWithParams(
  'SELECT * FROM users WHERE id = $1 AND status = $2',
  [userId, 'active']
);

const rows = JSON.parse(result.rows);
```

### Key Features

✅ **PostgreSQL-Style Parameters** - Uses `$1`, `$2`, etc. syntax
✅ **Full CRUD Support** - Works with SELECT, INSERT, UPDATE, DELETE
✅ **Type Safe** - Automatic conversion for strings, numbers, booleans, null
✅ **SQL Injection Prevention** - User input treated as data, never as code
✅ **Backward Compatible** - Existing code continues to work
✅ **Well Documented** - Comprehensive guides and examples

## 📚 Documentation

- **[Quick Start Guide](./QUICK_START_PARAMS.md)** - Get started in 2 minutes
- **[Complete Guide](./PARAMETERIZED_QUERIES.md)** - Best practices, patterns, and troubleshooting
- **[Implementation Details](./PARAMETERIZED_QUERIES_IMPLEMENTATION.md)** - Technical deep-dive

## 🔧 What Changed

### Core Changes
- Added `QueryParams` struct for parameter binding
- Updated AST to support `Expr` in INSERT/UPDATE statements
- Fixed SELECT WHERE clause parsing (was previously broken!)
- Enhanced type conversion between JavaScript and Rust

### API Changes
**New Methods:**
- `db.queryWithParams(sql, params)` - Query with parameters
- `db.executeWithParams(sql, params)` - Execute with parameters

**Existing Methods:**
- `db.query(sql)` - Still works (backward compatible)
- `db.execute(sql)` - Still works (backward compatible)

### Breaking Changes
**None!** All existing code continues to work without modification.

## 🚀 Migration Guide

### Before (Unsafe)
```javascript
const username = req.body.username;
await db.query(`SELECT * FROM users WHERE username = '${username}'`);
// ⚠️ Vulnerable to SQL injection!
```

### After (Safe)
```javascript
const username = req.body.username;
const result = await db.queryWithParams(
  'SELECT * FROM users WHERE username = $1',
  [username]
);
const rows = JSON.parse(result.rows);
// ✅ Safe from SQL injection!
```

## 📦 Installation

```bash
npm install @nguyenetic/nexusql@2.6.0
```

Or update your existing installation:

```bash
npm update @nguyenetic/nexusql
```

## 🔍 Common Patterns

### INSERT with Parameters
```javascript
await db.executeWithParams(
  'INSERT INTO users (name, email, age) VALUES ($1, $2, $3)',
  ['Alice', 'alice@example.com', 30]
);
```

### UPDATE with Parameters
```javascript
await db.executeWithParams(
  'UPDATE users SET email = $1 WHERE id = $2',
  ['newemail@example.com', userId]
);
```

### DELETE with Parameters
```javascript
await db.executeWithParams(
  'DELETE FROM users WHERE age < $1 AND status = $2',
  [18, 'inactive']
);
```

### Complex WHERE Clauses
```javascript
const result = await db.queryWithParams(
  `SELECT * FROM products
   WHERE category = $1
     AND price BETWEEN $2 AND $3
     AND in_stock = $4`,
  ['electronics', 50, 500, true]
);
```

## 🛡️ Security Benefits

### SQL Injection Prevention
Parameters are sent separately from SQL code, making injection attacks impossible:

```javascript
// This is safe - even with malicious input
const malicious = "'; DROP TABLE users; --";
await db.queryWithParams(
  'SELECT * FROM users WHERE username = $1',
  [malicious]
);
// The database treats the entire string as a literal value
// It will NOT execute the DROP TABLE command
```

### Automatic Escaping
No more manual escaping! The database handles all special characters safely.

## ⚡ Performance

Parameterized queries can actually be **faster** than string concatenation:
- Query plans are cached and reused
- No need for manual escaping overhead
- Optimized execution for repeated queries

## 🧪 Testing

This release includes:
- ✅ Comprehensive test suite
- ✅ SQL injection prevention tests
- ✅ Type conversion tests
- ✅ Edge case handling
- ✅ Backward compatibility tests

## 📊 Package Stats

- **Package Size:** 2.1 MB (compressed)
- **Unpacked Size:** 5.4 MB
- **Node Version:** >=16.0.0
- **Platform:** Windows, Linux, macOS (x64, ARM64)

## 🐛 Bug Fixes

- Fixed SELECT WHERE clause parsing (was returning all rows instead of filtering)
- Fixed test compilation errors (57 import errors resolved)
- Improved type safety across the codebase

## 🙏 Acknowledgments

Thanks to the PostgreSQL community and Node-Postgres project for inspiring best practices in parameterized query design.

## 📖 Resources

- **GitHub:** https://github.com/Nguyenetic/NexusQL
- **npm:** https://www.npmjs.com/package/@nguyenetic/nexusql
- **Issues:** https://github.com/Nguyenetic/NexusQL/issues
- **Documentation:** See PARAMETERIZED_QUERIES.md

## 🔮 What's Next

Future enhancements may include:
- Named parameters (`:name` syntax)
- Array parameter support for `IN` clauses
- Prepared statement caching
- Batch parameter binding

## 📝 Full Changelog

See [PARAMETERIZED_QUERIES_IMPLEMENTATION.md](./PARAMETERIZED_QUERIES_IMPLEMENTATION.md) for complete technical details.

---

**Install now:** `npm install @nguyenetic/nexusql@2.6.0`

**Questions?** Open an issue on GitHub: https://github.com/Nguyenetic/NexusQL/issues
