# Installing NexusQL Directly from GitHub

Install NexusQL in your projects using `npm install` or `pip install` directly from GitHub!

---

## 📦 Node.js/TypeScript - Install from GitHub

### One-Line Install

```bash
npm install git+https://github.com/johnhnguyen97/NexusQL.git#master
```

### In Your package.json

```json
{
  "dependencies": {
    "nexusql": "git+https://github.com/johnhnguyen97/NexusQL.git#master"
  }
}
```

Then just:
```bash
npm install
```

### Usage (Same as Before!)

```typescript
import { NexusDB } from 'nexusql';

const db = new NexusDB('./mydb');
await db.execute('CREATE TABLE users (id INT, name TEXT)');
const result = await db.query('SELECT * FROM users');
```

**That's it! No npm link needed!** 🎉

---

## 🐍 Python - Install from GitHub

### One-Line Install

```bash
pip install git+https://github.com/johnhnguyen97/NexusQL.git@master#subdirectory=bindings/python
```

### In Your requirements.txt

```
nexusql @ git+https://github.com/johnhnguyen97/NexusQL.git@master#subdirectory=bindings/python
```

Then just:
```bash
pip install -r requirements.txt
```

### Usage (Same as Before!)

```python
from nexusql import NexusDB

db = NexusDB('./mydb')
db.execute('CREATE TABLE users (id INT, name TEXT)')
result = db.query('SELECT * FROM users')
```

**That's it! No pip install -e needed!** 🎉

---

## 🦀 Rust - Install from GitHub

### In Your Cargo.toml

```toml
[dependencies]
nexusql = { git = "https://github.com/johnhnguyen97/NexusQL.git" }
```

Then just:
```bash
cargo build
```

**Cargo handles everything automatically!** 🎉

---

## 🔄 Updating to Latest Version

### Node.js
```bash
npm update nexusql
# Or force reinstall
npm install git+https://github.com/johnhnguyen97/NexusQL.git#master
```

### Python
```bash
pip install --upgrade git+https://github.com/johnhnguyen97/NexusQL.git@master#subdirectory=bindings/python
```

### Rust
```bash
cargo update nexusql
```

---

## 🏷️ Using Specific Versions

### Node.js - Use Git Tag
```bash
npm install git+https://github.com/johnhnguyen97/NexusQL.git#v2.4.0
```

### Python - Use Git Tag
```bash
pip install git+https://github.com/johnhnguyen97/NexusQL.git@v2.4.0#subdirectory=bindings/python
```

### Rust - Use Git Tag
```toml
[dependencies]
nexusql = { git = "https://github.com/johnhnguyen97/NexusQL.git", tag = "v2.4.0" }
```

---

## 🔒 Private Repository (If You Make It Private)

### Node.js with Private Repo
```bash
npm install git+https://YOUR_GITHUB_TOKEN@github.com/johnhnguyen97/NexusQL.git#master
```

### Python with Private Repo
```bash
pip install git+https://YOUR_GITHUB_TOKEN@github.com/johnhnguyen97/NexusQL.git@master#subdirectory=bindings/python
```

### OR: Use SSH
```bash
# Node.js
npm install git+ssh://git@github.com:johnhnguyen97/NexusQL.git#master

# Python
pip install git+ssh://git@github.com/johnhnguyen97/NexusQL.git@master#subdirectory=bindings/python
```

---

## ✅ Summary

**Just run ONE command in your project:**

### Node.js/TypeScript:
```bash
npm install git+https://github.com/johnhnguyen97/NexusQL.git#master
```

### Python:
```bash
pip install git+https://github.com/johnhnguyen97/NexusQL.git@master#subdirectory=bindings/python
```

### Rust:
```bash
# Add to Cargo.toml then cargo build
nexusql = { git = "https://github.com/johnhnguyen97/NexusQL.git" }
```

**Done! NexusQL installs directly from GitHub like any other package!** 🎉

No publishing to NPM/PyPI needed. Just keep your GitHub repo and anyone (or just you) can install it with one command!

---

## 🎯 Recommendation

**For Your Private Use:**
- Keep the GitHub repo (public or private)
- Share the install command with team members
- Everyone can `npm install` or `pip install` directly from GitHub
- Updates happen via `git push` to GitHub
- Everyone pulls updates with `npm update` or `pip install --upgrade`

**Perfect for private projects with the convenience of package managers!** 🔒📦
