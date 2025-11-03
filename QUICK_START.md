# NexusQL - Quick Start Guide (Private Use)

Simple guide to start using NexusQL in your projects.

---

## 🚀 Using in Your Rust Project

```bash
cd /path/to/your/rust/project

# Add NexusQL as dependency
cargo add nexusql --path C:/Users/jnguyen/Documents/Project/NexusQL
```

**Your Cargo.toml will have:**
```toml
[dependencies]
nexusql = { path = "C:/Users/jnguyen/Documents/Project/NexusQL" }
```

**Then use it:**
```rust
use nexusql::NexusSQL;

#[tokio::main]
async fn main() {
    let db = NexusSQL::new("./mydb").await.unwrap();
    db.execute("CREATE TABLE users (id INT, name TEXT)").await.unwrap();
    db.shutdown().await.unwrap();
}
```

---

## 📦 Using in Your Node.js/TypeScript Project

```bash
cd C:/Users/jnguyen/Documents/Project/NexusQL/bindings/typescript

# Install deps and build
npm install
npm run build

# Link for global use
npm link

# Now in YOUR project
cd /path/to/your/nodejs/project
npm link nexusql
```

**Then use it:**
```typescript
import { NexusDB } from 'nexusql';

const db = new NexusDB('./mydb');
await db.execute('CREATE TABLE users (id INT, name TEXT)');
const result = await db.query('SELECT * FROM users');
await db.close();
```

---

## 🐍 Using in Your Python Project

```bash
cd C:/Users/jnguyen/Documents/Project/NexusQL/bindings/python

# Install in editable mode
pip install -e .
```

**Then use it:**
```python
from nexusql import NexusDB

db = NexusDB('./mydb')
db.execute('CREATE TABLE users (id INT, name TEXT)')
result = db.query('SELECT * FROM users')
db.close()
```

---

## 🎯 Summary

**For Rust**: Add path dependency in Cargo.toml
**For Node.js**: Use `npm link` for local packages
**For Python**: Use `pip install -e` for editable install

**All your projects can now use NexusQL locally without publishing anything publicly!**

---

## 📚 What You Get

- ✅ Full PostgreSQL-compatible SQL
- ✅ AI-native vector search
- ✅ File-based (no server needed)
- ✅ ACID transactions
- ✅ All from a single embedded database
- ✅ Private and secure

**Start building with NexusQL in your projects now!** 🚀
