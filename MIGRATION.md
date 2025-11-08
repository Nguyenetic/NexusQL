# NexusQL Migration Guide

This guide helps you migrate between major API versions of NexusQL.

## Table of Contents

- [v2.x → v3.x](#v2x--v3x)
- [Common Migration Patterns](#common-migration-patterns)
- [Breaking Changes Summary](#breaking-changes-summary)

---

## v2.x → v3.x

### 1. Database Struct Renamed

The main database struct was renamed from `Database` to `NexusSQL`.

**Before (v2.x):**
```rust
use nexusql::Database;

let db = Database::open("data.db").await?;
```

**After (v3.x):**
```rust
use nexusql::NexusSQL;

let db = NexusSQL::new("data.db").await?;
```

**Why:** Better clarity and consistency with the crate name.

---

### 2. Error Type Changed

Error handling now uses `NexusError` instead of `anyhow::Error`.

**Before (v2.x):**
```rust
use anyhow::Error;

fn get_data() -> Result<Data, Error> {
    let db = Database::open("data.db").await?;
    // ...
}
```

**After (v3.x):**
```rust
use nexusql::NexusError;

fn get_data() -> Result<Data, NexusError> {
    let db = NexusSQL::new("data.db").await?;
    // ...
}

// Or with custom error type using From trait:
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(#[from] nexusql::NexusError),

    #[error("Parse error: {0}")]
    Parse(String),
}

fn get_data() -> Result<Data, StorageError> {
    let db = NexusSQL::new("data.db").await?;  // Auto-converts via From
    // ...
}
```

**Why:** Type-safe error handling with structured error variants instead of string-based errors.

---

### 3. Method Name Changes

Several method names were standardized for better API consistency.

**Before (v2.x):**
```rust
// Parameterized queries
db.execute_params(sql, params).await?;
db.query_params(sql, params).await?;
```

**After (v3.x):**
```rust
// Option 1: Using execute/query with optional params
db.execute(sql).await?;  // Without params
db.execute_with_params(sql, Some(params)).await?;  // With params

// Option 2: Using the shorter method names
db.exec_params(sql, params).await?;
db.query_with_params(sql, Some(params)).await?;
```

**Why:** Consistent naming convention and support for optional parameters.

---

### 4. Result Row Access Changed

Row structure changed from `Vec<Value>` to `Row` struct with `.values` field.

**Before (v2.x):**
```rust
let result = db.query("SELECT id, name FROM users").await?;

for row in result.rows {
    let id = match &row[0] {
        Value::Integer(i) => *i,
        _ => panic!("Invalid type"),
    };
    let name = match &row[1] {
        Value::Text(s) => s.clone(),
        _ => panic!("Invalid type"),
    };
}
```

**After (v3.x):**
```rust
use nexusql::types::Row;

let result = db.query("SELECT id, name FROM users").await?;

for row in &result.rows {
    let id = match &row.values[0] {
        Value::Integer(i) => *i,
        _ => panic!("Invalid type"),
    };
    let name = match &row.values[1] {
        Value::Text(s) => s.clone(),
        _ => panic!("Invalid type"),
    };
}
```

**Why:** Better type safety and allows for future row metadata (column names, types, etc.).

---

### 5. ExecuteParams Wrapper Removed

Parameter passing simplified to use Option<Vec<Value>> directly.

**Before (v2.x):**
```rust
use nexusql::ExecuteParams;

let params = ExecuteParams::new(vec![
    Value::Text("Alice".to_string()),
    Value::Integer(25),
]);
db.execute_params("INSERT INTO users VALUES ($1, $2)", params).await?;
```

**After (v3.x):**
```rust
use nexusql::types::Value;

let params = vec![
    Value::Text("Alice".to_string()),
    Value::Integer(25),
];
db.execute_with_params(
    "INSERT INTO users VALUES ($1, $2)",
    Some(params)
).await?;

// Or without params:
db.execute("INSERT INTO users VALUES (1, 'Bob')").await?;
```

**Why:** Simpler API with direct use of standard Rust types.

---

## Common Migration Patterns

### Pattern 1: Basic Query Execution

**Before:**
```rust
use nexusql::Database;

let db = Database::open("data.db").await?;
db.execute("CREATE TABLE users (id INTEGER, name TEXT)").await?;
```

**After:**
```rust
use nexusql::NexusSQL;

let db = NexusSQL::new("data.db").await?;
db.execute("CREATE TABLE users (id INTEGER, name TEXT)").await?;
```

---

### Pattern 2: Parameterized Queries

**Before:**
```rust
use nexusql::{Database, ExecuteParams, Value};

let db = Database::open("data.db").await?;
let params = ExecuteParams::new(vec![Value::Integer(1)]);
let result = db.query_params("SELECT * FROM users WHERE id = $1", params).await?;
```

**After:**
```rust
use nexusql::{NexusSQL, types::Value};

let db = NexusSQL::new("data.db").await?;
let result = db.query_with_params(
    "SELECT * FROM users WHERE id = $1",
    Some(vec![Value::Integer(1)])
).await?;
```

---

### Pattern 3: Row Parsing

**Before:**
```rust
fn parse_user(row: &[Value]) -> User {
    User {
        id: match &row[0] {
            Value::Integer(i) => *i,
            _ => 0,
        },
        name: match &row[1] {
            Value::Text(s) => s.clone(),
            _ => String::new(),
        },
    }
}

let result = db.query("SELECT * FROM users").await?;
let users: Vec<User> = result.rows.iter().map(|r| parse_user(r)).collect();
```

**After:**
```rust
use nexusql::types::Row;

fn parse_user(row: &Row) -> User {
    User {
        id: match &row.values[0] {
            Value::Integer(i) => *i,
            _ => 0,
        },
        name: match &row.values[1] {
            Value::Text(s) => s.clone(),
            _ => String::new(),
        },
    }
}

let result = db.query("SELECT * FROM users").await?;
let users: Vec<User> = result.rows.iter().map(|r| parse_user(r)).collect();
```

---

### Pattern 4: Error Handling

**Before:**
```rust
use anyhow::{Error, Context};

fn get_user(id: i64) -> Result<User, Error> {
    let db = Database::open("data.db").await
        .context("Failed to open database")?;

    let result = db.query_params(
        "SELECT * FROM users WHERE id = $1",
        ExecuteParams::new(vec![Value::Integer(id)])
    ).await.context("Query failed")?;

    // ...
}
```

**After:**
```rust
use nexusql::NexusError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] NexusError),

    #[error("User not found")]
    NotFound,
}

async fn get_user(id: i64) -> Result<User, AppError> {
    let db = NexusSQL::new("data.db").await?;  // Auto-converts

    let result = db.query_with_params(
        "SELECT * FROM users WHERE id = $1",
        Some(vec![Value::Integer(id)])
    ).await?;

    if result.rows.is_empty() {
        return Err(AppError::NotFound);
    }

    // ...
}
```

---

### Pattern 5: Transactions

**Before:**
```rust
let db = Database::open("data.db").await?;
db.execute("BEGIN").await?;
db.execute("INSERT INTO users VALUES (1, 'Alice')").await?;
db.execute("COMMIT").await?;
```

**After:**
```rust
let db = NexusSQL::new("data.db").await?;

// Option 1: Manual transaction control (same as before)
db.execute("BEGIN").await?;
db.execute("INSERT INTO users VALUES (1, 'Alice')").await?;
db.execute("COMMIT").await?;

// Option 2: Transaction API (if available)
let tx = db.begin_transaction().await?;
tx.execute("INSERT INTO users VALUES (1, 'Alice')").await?;
tx.commit().await?;
```

---

## Breaking Changes Summary

| Category | v2.x | v3.x | Reason |
|----------|------|------|--------|
| **Struct Name** | `Database` | `NexusSQL` | Clarity and consistency |
| **Error Type** | `anyhow::Error` | `NexusError` | Type-safe error handling |
| **Open Method** | `Database::open()` | `NexusSQL::new()` | Standard Rust conventions |
| **Execute Params** | `execute_params(sql, params)` | `execute_with_params(sql, Some(params))` | Consistent API |
| **Row Type** | `Vec<Value>` | `Row { values: Vec<Value> }` | Future extensibility |
| **Params Wrapper** | `ExecuteParams` | `Option<Vec<Value>>` | Simplified API |

---

## Quick Migration Checklist

- [ ] Replace `Database` with `NexusSQL` in imports
- [ ] Change `Database::open()` to `NexusSQL::new()`
- [ ] Update error types from `anyhow::Error` to `NexusError`
- [ ] Add `#[from] NexusError` to custom error enums
- [ ] Change `execute_params()` to `execute_with_params()`
- [ ] Update row access from `row[i]` to `row.values[i]`
- [ ] Remove `ExecuteParams` wrapper, use `Some(vec![...])`
- [ ] Update function signatures that return `Result<T, Error>`

---

## Need Help?

If you encounter issues during migration:

1. Check the [API Documentation](https://docs.rs/nexusql)
2. Review [Examples](./examples/)
3. Open an [Issue](https://github.com/Nguyenetic/NexusQL/issues)
4. See [CLAUDE.md](./CLAUDE.md) for development guidance

---

## Version History

- **v3.2.0** - CREATE INDEX support added
- **v3.1.0** - HNSW algorithm fixes, all tests passing
- **v3.0.0** - Parameterized queries, SQL injection prevention
- **v2.7.x** - Breaking API changes (NexusSQL rename, error type change)
- **v2.6.x** - Last stable v2.x release

For detailed changelog, see [CHANGELOG.md](./bindings/typescript/CHANGELOG.md).
