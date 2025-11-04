# Building NexusQL TypeScript Bindings from Source

**Complete Guide to Compiling and Publishing NexusQL for Node.js**

---

## Table of Contents

1. [Overview](#overview)
2. [Prerequisites](#prerequisites)
3. [Architecture](#architecture)
4. [Build Setup](#build-setup)
5. [Using N-API (Node-API)](#using-n-api-node-api)
6. [Using napi-rs](#using-napi-rs)
7. [Building the Native Module](#building-the-native-module)
8. [Testing Locally](#testing-locally)
9. [Publishing to npm](#publishing-to-npm)
10. [Cross-Platform Builds](#cross-platform-builds)
11. [Troubleshooting](#troubleshooting)

---

## Overview

NexusQL is written in **Rust** and needs to be compiled to **native Node.js addons** for use in TypeScript/JavaScript.

**Two Approaches:**

1. **N-API (Node-API)** - Native Node.js C bindings
2. **napi-rs** ⭐ **Recommended** - Rust → N-API bindings (easier, safer)

We'll use **napi-rs** as it provides:
- ✅ Automatic memory management
- ✅ Type safety (Rust → TypeScript)
- ✅ Cross-platform support
- ✅ Easy build process

---

## Prerequisites

### Required Tools

**1. Rust Toolchain**
```bash
# Install Rust (https://rustup.rs)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Verify
rustc --version
cargo --version
```

**2. Node.js & npm**
```bash
# Install Node.js 16+ (https://nodejs.org)
node --version  # Should be >= 16.0.0
npm --version
```

**3. Build Tools**

**Windows:**
```bash
# Install Visual Studio Build Tools
# https://visualstudio.microsoft.com/downloads/
# Select "Desktop development with C++"
```

**macOS:**
```bash
# Install Xcode Command Line Tools
xcode-select --install
```

**Linux (Ubuntu/Debian):**
```bash
sudo apt update
sudo apt install build-essential pkg-config libssl-dev
```

---

## Architecture

### Project Structure

```
NexusQL/
├── src/                    # Rust source code
│   ├── lib.rs             # Main Rust library
│   ├── engine/            # Database engines
│   ├── storage/           # Storage layer
│   └── ...
├── bindings/
│   └── typescript/
│       ├── index.ts       # TypeScript API (stub)
│       ├── types.ts       # TypeScript types
│       ├── package.json
│       ├── build.rs       # Build script
│       └── src/           # Native bindings (to create)
│           ├── lib.rs     # N-API bindings
│           └── ...
└── Cargo.toml             # Rust dependencies
```

### Data Flow

```
TypeScript Code
     ↓
index.ts (API wrapper)
     ↓
Native Addon (.node file)
     ↓
Rust N-API Bindings
     ↓
NexusQL Core (Rust)
     ↓
LSM Storage / HNSW Index
```

---

## Build Setup

### Step 1: Install napi-rs CLI

```bash
cd bindings/typescript
npm install -D @napi-rs/cli
```

### Step 2: Update `package.json`

```json
{
  "name": "@johnhnguyen97/nexusql",
  "version": "2.4.0",
  "description": "AI-Native, PostgreSQL-compatible database with neural search",
  "main": "index.js",
  "types": "index.d.ts",
  "napi": {
    "name": "nexusql",
    "triples": {
      "defaults": true,
      "additional": [
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-gnu",
        "aarch64-apple-darwin",
        "aarch64-linux-android"
      ]
    }
  },
  "scripts": {
    "artifacts": "napi artifacts",
    "build": "napi build --platform --release",
    "build:debug": "napi build --platform",
    "prepublishOnly": "napi prepublish -t npm",
    "test": "jest",
    "universal": "napi universal",
    "version": "napi version"
  },
  "devDependencies": {
    "@napi-rs/cli": "^2.18.0",
    "@types/node": "^20.0.0",
    "typescript": "^5.0.0"
  },
  "engines": {
    "node": ">= 16"
  },
  "publishConfig": {
    "registry": "https://npm.pkg.github.com",
    "access": "public"
  }
}
```

### Step 3: Create Rust Binding Crate

Create `bindings/typescript/Cargo.toml`:

```toml
[package]
name = "nexusql-node"
version = "2.4.0"
edition = "2021"
license = "MIT"

[lib]
crate-type = ["cdylib"]

[dependencies]
# N-API bindings
napi = { version = "2", default-features = false, features = ["napi8", "async"] }
napi-derive = "2"

# NexusQL core (link to parent crate)
nexusql = { path = "../.." }

# Async runtime
tokio = { version = "1", features = ["full"] }

[build-dependencies]
napi-build = "2"

[profile.release]
lto = true
```

---

## Using napi-rs

### Step 1: Create Bindings Source

Create `bindings/typescript/src/lib.rs`:

```rust
#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use nexusql::{NexusSQL, QueryParser, Value as NexusValue};
use std::sync::Arc;
use tokio::sync::Mutex;

/// NexusDB class - Main database client
#[napi]
pub struct NexusDB {
    db: Arc<Mutex<NexusSQL>>,
    runtime: Arc<tokio::runtime::Runtime>,
}

#[napi]
impl NexusDB {
    /// Create new database connection
    #[napi(constructor)]
    pub fn new(path: String) -> Result<Self> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| Error::from_reason(format!("Failed to create runtime: {}", e)))?;

        let db = runtime.block_on(async {
            NexusSQL::new(&path).await
        }).map_err(|e| Error::from_reason(format!("Failed to create database: {}", e)))?;

        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            runtime: Arc::new(runtime),
        })
    }

    /// Execute a query and return results
    #[napi]
    pub async fn query(&self, sql: String, params: Option<Vec<JsUnknown>>) -> Result<QueryResult> {
        let db = self.db.clone();
        let params = params.unwrap_or_default();

        // Convert JS params to Nexus values
        let nexus_params: Vec<NexusValue> = params
            .into_iter()
            .map(|p| js_to_nexus_value(p))
            .collect::<Result<Vec<_>>>()?;

        // Execute query
        let result = db.lock().await
            .execute(&sql)
            .await
            .map_err(|e| Error::from_reason(format!("Query failed: {}", e)))?;

        // Convert to JS-friendly result
        Ok(QueryResult {
            rows: result.rows.into_iter().map(|r| row_to_js(r)).collect(),
            row_count: result.row_count as u32,
            fields: result.fields.into_iter().map(|f| Field {
                name: f.name,
                field_type: f.field_type,
            }).collect(),
        })
    }

    /// Execute a command (INSERT, UPDATE, DELETE, DDL)
    #[napi]
    pub async fn execute(&self, sql: String, params: Option<Vec<JsUnknown>>) -> Result<u32> {
        let result = self.query(sql, params).await?;
        Ok(result.row_count)
    }

    /// Begin a transaction
    #[napi]
    pub async fn begin_transaction(&self) -> Result<Transaction> {
        let db = self.db.clone();
        let tx_id = db.lock().await
            .begin_transaction()
            .await
            .map_err(|e| Error::from_reason(format!("Transaction failed: {}", e)))?;

        Ok(Transaction {
            db,
            tx_id,
            active: true,
        })
    }

    /// Close database connection
    #[napi]
    pub async fn close(&self) -> Result<()> {
        let db = self.db.clone();
        let mut db_lock = db.lock().await;

        // Move db out of Arc<Mutex>
        let db_inner = Arc::try_unwrap(self.db.clone())
            .map_err(|_| Error::from_reason("Failed to close: references still exist"))?;

        let db = db_inner.into_inner();
        db.shutdown()
            .await
            .map_err(|e| Error::from_reason(format!("Shutdown failed: {}", e)))?;

        Ok(())
    }
}

/// Query result
#[napi(object)]
pub struct QueryResult {
    pub rows: Vec<JsObject>,
    pub row_count: u32,
    pub fields: Vec<Field>,
}

/// Field metadata
#[napi(object)]
pub struct Field {
    pub name: String,
    pub field_type: String,
}

/// Transaction
#[napi]
pub struct Transaction {
    db: Arc<Mutex<NexusSQL>>,
    tx_id: u64,
    active: bool,
}

#[napi]
impl Transaction {
    /// Execute query in transaction
    #[napi]
    pub async fn query(&self, sql: String) -> Result<QueryResult> {
        if !self.active {
            return Err(Error::from_reason("Transaction already completed"));
        }

        // Similar to NexusDB::query but within transaction context
        // ... implementation ...
        todo!("Implement transaction query")
    }

    /// Commit transaction
    #[napi]
    pub async fn commit(&mut self) -> Result<()> {
        if !self.active {
            return Err(Error::from_reason("Transaction already completed"));
        }

        self.db.lock().await
            .commit_transaction(self.tx_id)
            .await
            .map_err(|e| Error::from_reason(format!("Commit failed: {}", e)))?;

        self.active = false;
        Ok(())
    }

    /// Rollback transaction
    #[napi]
    pub async fn rollback(&mut self) -> Result<()> {
        if !self.active {
            return Err(Error::from_reason("Transaction already completed"));
        }

        self.db.lock().await
            .rollback_transaction(self.tx_id)
            .await
            .map_err(|e| Error::from_reason(format!("Rollback failed: {}", e)))?;

        self.active = false;
        Ok(())
    }
}

// Helper functions
fn js_to_nexus_value(val: JsUnknown) -> Result<NexusValue> {
    // Convert JavaScript value to Nexus value
    // ... implementation ...
    todo!("Implement JS to Nexus value conversion")
}

fn row_to_js(row: nexusql::Row) -> JsObject {
    // Convert Nexus row to JS object
    // ... implementation ...
    todo!("Implement row to JS conversion")
}
```

### Step 2: Create Build Script

Create `bindings/typescript/build.rs`:

```rust
extern crate napi_build;

fn main() {
    napi_build::setup();
}
```

---

## Building the Native Module

### Development Build

```bash
cd bindings/typescript

# Build for current platform (debug mode)
npm run build:debug

# Output: nexusql.*.node in current directory
```

### Release Build

```bash
# Build optimized release version
npm run build

# Output: nexusql.*.node (platform-specific)
# Example on Windows: nexusql.win32-x64-msvc.node
# Example on macOS: nexusql.darwin-x64.node
# Example on Linux: nexusql.linux-x64-gnu.node
```

### Build Output

```
bindings/typescript/
├── nexusql.darwin-arm64.node    # macOS ARM (M1/M2)
├── nexusql.darwin-x64.node      # macOS Intel
├── nexusql.linux-x64-gnu.node   # Linux
├── nexusql.win32-x64-msvc.node  # Windows
└── index.js                     # JavaScript entry point
```

---

## Testing Locally

### Step 1: Link Package Locally

```bash
cd bindings/typescript
npm link
```

### Step 2: Test in Another Project

```bash
mkdir test-nexusql
cd test-nexusql
npm init -y
npm link @johnhnguyen97/nexusql

# Create test.js
cat > test.js << 'EOF'
const { NexusDB } = require('@johnhnguyen97/nexusql');

async function test() {
  const db = new NexusDB('./testdb');

  await db.execute('CREATE TABLE users (id INT PRIMARY KEY, name TEXT)');
  await db.execute('INSERT INTO users VALUES (1, "Alice")');

  const result = await db.query('SELECT * FROM users');
  console.log(result.rows);

  await db.close();
}

test().catch(console.error);
EOF

# Run test
node test.js
```

### Step 3: Automated Tests

Create `bindings/typescript/__tests__/nexusdb.test.ts`:

```typescript
import { NexusDB } from '../index';
import { describe, it, expect, beforeEach, afterEach } from '@jest/globals';

describe('NexusDB', () => {
  let db: NexusDB;

  beforeEach(async () => {
    db = new NexusDB('./test_db');
  });

  afterEach(async () => {
    await db.close();
  });

  it('should create table', async () => {
    await db.execute('CREATE TABLE users (id INT PRIMARY KEY, name TEXT)');
    // Success if no error thrown
  });

  it('should insert and query data', async () => {
    await db.execute('CREATE TABLE users (id INT, name TEXT)');
    await db.execute('INSERT INTO users VALUES (1, $1)', ['Alice']);

    const result = await db.query('SELECT * FROM users WHERE id = 1');
    expect(result.rowCount).toBe(1);
    expect(result.rows[0].name).toBe('Alice');
  });

  it('should handle transactions', async () => {
    await db.execute('CREATE TABLE accounts (id INT, balance INT)');

    const tx = await db.beginTransaction();
    await tx.query('INSERT INTO accounts VALUES (1, 1000)');
    await tx.query('INSERT INTO accounts VALUES (2, 500)');
    await tx.commit();

    const result = await db.query('SELECT COUNT(*) as cnt FROM accounts');
    expect(result.rows[0].cnt).toBe(2);
  });
});
```

Run tests:
```bash
npm test
```

---

## Publishing to npm

### Option 1: Public npm Registry

```bash
# 1. Login to npm
npm login

# 2. Update package.json
{
  "name": "@johnhnguyen97/nexusql",  # Your npm username
  "publishConfig": {
    "access": "public"
  }
}

# 3. Build for all platforms (see Cross-Platform Builds section)
npm run build

# 4. Publish
npm publish --access public
```

**Note**: Package is published to npmjs.com as `@nguyenetic/nexusql`

---

## Cross-Platform Builds

### Automated Cross-Compilation with GitHub Actions

Create `.github/workflows/build.yml`:

```yaml
name: Build Native Modules

on:
  push:
    branches: [master]
  release:
    types: [created]

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        settings:
          - host: macos-latest
            target: x86_64-apple-darwin
            build: npm run build -- --target x86_64-apple-darwin
          - host: macos-latest
            target: aarch64-apple-darwin
            build: npm run build -- --target aarch64-apple-darwin
          - host: windows-latest
            target: x86_64-pc-windows-msvc
            build: npm run build -- --target x86_64-pc-windows-msvc
          - host: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            build: npm run build -- --target x86_64-unknown-linux-gnu

    name: Build ${{ matrix.settings.target }}
    runs-on: ${{ matrix.settings.host }}

    steps:
      - uses: actions/checkout@v3

      - name: Setup Node
        uses: actions/setup-node@v3
        with:
          node-version: 18

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.settings.target }}

      - name: Install dependencies
        run: npm ci
        working-directory: bindings/typescript

      - name: Build
        run: ${{ matrix.settings.build }}
        working-directory: bindings/typescript

      - name: Upload artifacts
        uses: actions/upload-artifact@v3
        with:
          name: bindings-${{ matrix.settings.target }}
          path: bindings/typescript/*.node

  publish:
    needs: build
    runs-on: ubuntu-latest
    if: github.event_name == 'release'

    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with:
          node-version: 18
          registry-url: 'https://npm.pkg.github.com'

      - name: Download all artifacts
        uses: actions/download-artifact@v3
        with:
          path: bindings/typescript

      - name: Publish to GitHub Packages
        run: npm publish
        working-directory: bindings/typescript
        env:
          NODE_AUTH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

### Manual Cross-Compilation

**macOS (Build for ARM):**
```bash
rustup target add aarch64-apple-darwin
npm run build -- --target aarch64-apple-darwin
```

**Linux (Build for musl):**
```bash
rustup target add x86_64-unknown-linux-musl
npm run build -- --target x86_64-unknown-linux-musl
```

**Windows (Cross-compile on Linux):**
```bash
sudo apt install mingw-w64
rustup target add x86_64-pc-windows-gnu
npm run build -- --target x86_64-pc-windows-gnu
```

---

## Troubleshooting

### Issue: "cannot find -lnexusql"

**Cause:** Linker can't find NexusQL library.

**Solution:**
```bash
# Make sure Cargo.toml has correct path
[dependencies]
nexusql = { path = "../.." }

# Clean and rebuild
cargo clean
npm run build
```

### Issue: "Module did not self-register"

**Cause:** Node version mismatch or incorrect build.

**Solution:**
```bash
# Rebuild for current Node version
npm rebuild

# Or rebuild from scratch
rm -rf node_modules package-lock.json
npm install
npm run build
```

### Issue: "Error: Cannot find module 'nexusql.node'"

**Cause:** Native module not built.

**Solution:**
```bash
cd bindings/typescript
npm run build
```

### Issue: "Segmentation fault" or Crashes

**Cause:** Memory management issues between Rust and Node.js.

**Debug:**
```bash
# Run with verbose logging
NODE_OPTIONS="--trace-warnings" node test.js

# Use debug build
npm run build:debug
node --inspect test.js
```

**Common Fixes:**
- Ensure all async operations use proper `await`
- Check lifetime management in Rust code
- Verify Arc/Mutex usage for shared state

### Issue: Build Fails on Windows

**Cause:** Missing Visual Studio Build Tools.

**Solution:**
```bash
# Install VS Build Tools
# https://visualstudio.microsoft.com/downloads/
# Select "Desktop development with C++"

# Add to PATH
setx PATH "%PATH%;C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\MSBuild\Current\Bin"
```

### Issue: "error: failed to run custom build command for `openssl-sys`"

**Cause:** OpenSSL not found.

**Solution (Windows):**
```bash
# Use rustls instead of OpenSSL
# In Cargo.toml, replace openssl with:
[dependencies]
tokio-rustls = "0.24"
```

**Solution (macOS):**
```bash
brew install openssl
export OPENSSL_DIR=$(brew --prefix openssl)
```

**Solution (Linux):**
```bash
sudo apt install libssl-dev pkg-config
```

---

## Performance Optimization

### 1. Enable LTO (Link-Time Optimization)

In `Cargo.toml`:
```toml
[profile.release]
lto = true
codegen-units = 1
opt-level = 3
```

### 2. Strip Debug Symbols

```bash
npm run build -- --strip
```

### 3. Use jemalloc Allocator

In `src/lib.rs`:
```rust
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;
```

Add to `Cargo.toml`:
```toml
[dependencies]
jemallocator = "0.5"
```

### 4. Benchmark

```typescript
// benchmark.ts
import { NexusDB } from '@johnhnguyen97/nexusql';

async function benchmark() {
  const db = new NexusDB('./bench');

  await db.execute('CREATE TABLE test (id INT, data TEXT)');

  console.time('insert_1000');
  for (let i = 0; i < 1000; i++) {
    await db.execute('INSERT INTO test VALUES ($1, $2)', [i, `data_${i}`]);
  }
  console.timeEnd('insert_1000');

  console.time('query_1000');
  for (let i = 0; i < 1000; i++) {
    await db.query('SELECT * FROM test WHERE id = $1', [i]);
  }
  console.timeEnd('query_1000');

  await db.close();
}

benchmark();
```

---

## Advanced Topics

### Memory Management

**Rust → JavaScript:**
```rust
// Convert Rust Vec to JavaScript Array
#[napi]
pub fn get_items() -> Vec<String> {
    vec!["item1".to_string(), "item2".to_string()]
}
```

**JavaScript → Rust:**
```rust
// Accept JavaScript Array
#[napi]
pub fn process_items(items: Vec<String>) -> Result<()> {
    for item in items {
        // Process item
    }
    Ok(())
}
```

### Async/Await

```rust
// Rust async function → JavaScript Promise
#[napi]
pub async fn async_operation() -> Result<String> {
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    Ok("Done".to_string())
}
```

### TypeScript Type Generation

napi-rs automatically generates TypeScript types:

```bash
npm run build
# Creates: index.d.ts with full type definitions
```

---

## Summary Checklist

**Setup:**
- [ ] Install Rust, Node.js, build tools
- [ ] Install napi-rs CLI
- [ ] Create binding crate with Cargo.toml
- [ ] Write N-API bindings in src/lib.rs

**Build:**
- [ ] Build for development (`npm run build:debug`)
- [ ] Build for release (`npm run build`)
- [ ] Test locally with `npm link`
- [ ] Run automated tests

**Publish:**
- [ ] Setup GitHub Actions for cross-platform builds
- [ ] Build for all target platforms
- [ ] Test on each platform
- [ ] Publish to npm or GitHub Packages

**Maintain:**
- [ ] Update version in package.json and Cargo.toml
- [ ] Update CHANGELOG.md
- [ ] Tag release in Git
- [ ] Monitor issues and bug reports

---

## Next Steps

- **[TYPESCRIPT_API_COMPLETE.md](./TYPESCRIPT_API_COMPLETE.md)** - Full API documentation
- **[DROP_IN_REPLACEMENT_GUIDE.md](./DROP_IN_REPLACEMENT_GUIDE.md)** - Migrate from SQLite
- **[COMPARISON_SQLITE_NEXUSQL.md](./COMPARISON_SQLITE_NEXUSQL.md)** - Detailed comparison

---

## Resources

- **napi-rs Docs:** https://napi.rs
- **Node-API Docs:** https://nodejs.org/api/n-api.html
- **Rust FFI Guide:** https://doc.rust-lang.org/nomicon/ffi.html
- **Cross-Compilation:** https://rust-lang.github.io/rustup/cross-compilation.html

---

**Last Updated:** November 2, 2025
**Version:** 2.4.0
**Build System:** napi-rs 2.18.0
