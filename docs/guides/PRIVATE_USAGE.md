# NexusQL - Private Installation & Usage Guide

**PRIVATE PROJECT** - For local/internal use only. Do NOT publish to NPM or PyPI.

## 🏠 Using NexusQL Privately

NexusQL is designed as a **file-based embedded database** for your private projects. Here's how to use it:

---

## 🦀 Option 1: Direct Rust Usage (Recommended)

### In Your Rust Project

Add to your `Cargo.toml`:

```toml
[dependencies]
# Local path dependency
nexusql = { path = "../NexusQL" }

# Or via Git (private repo)
nexusql = { git = "https://github.com/johnhnguyen97/NexusQL.git" }
```

### Usage

```rust
use nexusql::NexusSQL;

#[tokio::main]
async fn main() -> Result<()> {
    let db = NexusSQL::new("./mydb").await?;
    db.execute("CREATE TABLE users (id INT, name TEXT)").await?;
    db.execute("INSERT INTO users VALUES (1, 'Alice')").await?;
    let result = db.execute("SELECT * FROM users").await?;
    db.shutdown().await?;
    Ok(())
}
```

---

## 📦 Option 2: Local NPM Package (TypeScript/Node.js)

### Build and Link Locally

```bash
cd NexusQL/bindings/typescript

# Install dependencies
npm install

# Build TypeScript
npm run build

# Link globally for local use
npm link

# In your Node.js project
cd /path/to/your/project
npm link nexusql
```

### Usage in Your Project

```typescript
import { NexusDB } from 'nexusql';

const db = new NexusDB('./mydb');
await db.execute('CREATE TABLE users (id INT, name TEXT)');
const result = await db.query('SELECT * FROM users');
await db.close();
```

### OR: Direct File Reference

```json
// In your package.json
{
  "dependencies": {
    "nexusql": "file:../NexusQL/bindings/typescript"
  }
}
```

---

## 🐍 Option 3: Local Python Package

### Install Locally

```bash
cd NexusQL/bindings/python

# Install in development mode
pip install -e .

# Or build and install wheel
python -m build
pip install dist/nexusql-2.3.0-py3-none-any.whl
```

### Usage in Your Python Project

```python
from nexusql import NexusDB

db = NexusDB('./mydb')
db.execute('CREATE TABLE users (id INT, name TEXT)')
result = db.query('SELECT * FROM users')
db.close()
```

### OR: Add to requirements.txt

```
# requirements.txt
-e file:../NexusQL/bindings/python
```

---

## 🔒 Private Git Repository Usage

### For Team/Organization Use

```bash
# Rust
[dependencies]
nexusql = { git = "https://github.com/johnhnguyen97/NexusQL.git", branch = "master" }

# NPM (package.json)
{
  "dependencies": {
    "nexusql": "git+https://github.com/johnhnguyen97/NexusQL.git#master:bindings/typescript"
  }
}

# Python (requirements.txt)
git+https://github.com/johnhnguyen97/NexusQL.git@master#subdirectory=bindings/python
```

---

## 🏢 Enterprise: Private Package Registry

### Option A: Private NPM Registry (Verdaccio)

```bash
# Install Verdaccio
npm install -g verdaccio

# Run private registry
verdaccio

# Publish to private registry
npm publish --registry http://localhost:4873

# Install from private registry
npm install nexusql --registry http://localhost:4873
```

### Option B: GitHub Packages (Private)

```bash
# Configure .npmrc
@johnhnguyen97:registry=https://npm.pkg.github.com

# Publish to GitHub Packages
npm publish

# Install from GitHub Packages
npm install @johnhnguyen97/nexusql
```

### Option C: Private PyPI (devpi)

```bash
# Install devpi
pip install devpi-server devpi-client

# Start private PyPI
devpi-server --start

# Upload to private PyPI
devpi upload

# Install from private PyPI
pip install --index-url http://localhost:3141/root/pypi/+simple/ nexusql
```

---

## 🚀 Recommended Setup for Private Use

### For Single Developer

**Best**: Direct path dependency
- Rust: `path = "../NexusQL"`
- NPM: `npm link` or `"file:../NexusQL/bindings/typescript"`
- Python: `pip install -e ../NexusQL/bindings/python`

### For Team (Private Repo)

**Best**: Git dependency
- Keep repo private on GitHub
- Reference via Git URL
- Team members clone and use directly

### For Organization

**Best**: Internal package registry
- Verdaccio (NPM) or devpi (Python)
- Full version control
- Team collaboration

---

## 📝 Summary

**NexusQL is PRIVATE** and should stay that way:

✅ Use via path dependencies (simplest)
✅ Use via private Git repo (for teams)
✅ Use via internal package registry (for orgs)
❌ Do NOT publish to public NPM
❌ Do NOT publish to public PyPI

**Your data, your database, your control!** 🔒

---

## 🎯 Quick Commands

```bash
# Rust project
cargo add nexusql --path ../NexusQL

# Node.js project (local link)
cd NexusQL/bindings/typescript && npm link
cd your-project && npm link nexusql

# Python project (editable install)
pip install -e ../NexusQL/bindings/python
```

**That's it! Use NexusQL privately in all your projects!** 🚀
