# Publishing NexusQL Packages

Guide for publishing NexusQL to NPM and PyPI.

## 📦 Publishing to NPM (Node.js/TypeScript)

### Prerequisites

```bash
# Install required tools
npm install -g typescript

# Login to NPM
npm login
```

### Build and Publish

```bash
cd bindings/typescript

# Install dependencies
npm install

# Build TypeScript
npm run build

# Test locally
npm link
cd /path/to/test/project
npm link nexusql

# Publish to NPM
npm publish
```

### Installation for Users

```bash
npm install nexusql
```

### Usage

```typescript
import { NexusDB } from 'nexusql';

const db = new NexusDB('./mydb');
await db.execute('CREATE TABLE users (id INT, name TEXT)');
const result = await db.query('SELECT * FROM users');
await db.close();
```

---

## 🐍 Publishing to PyPI (Python)

### Prerequisites

```bash
# Install build tools
pip install build twine

# Create PyPI account at https://pypi.org/account/register/
```

### Build and Publish

```bash
cd bindings/python

# Build distribution
python -m build

# This creates:
# - dist/nexusql-2.3.0-py3-none-any.whl
# - dist/nexusql-2.3.0.tar.gz

# Test locally
pip install dist/nexusql-2.3.0-py3-none-any.whl

# Upload to Test PyPI (optional)
twine upload --repository testpypi dist/*

# Upload to PyPI
twine upload dist/*
```

### Installation for Users

```bash
pip install nexusql
```

### Usage

```python
from nexusql import NexusDB

db = NexusDB('./mydb')
db.execute('CREATE TABLE users (id INT, name TEXT)')
result = db.query('SELECT * FROM users')
db.close()
```

---

## 🦀 Native Module Building (Advanced)

For better performance, you can build native Rust modules:

### Node.js Native Module (NAPI-RS)

```bash
# Add to Cargo.toml
[lib]
crate-type = ["cdylib"]

[dependencies]
napi = "2"
napi-derive = "2"

# Build
cd bindings/typescript
npm install
npm run build:native

# This creates native .node file
```

### Python Native Module (PyO3)

```bash
# Add to Cargo.toml
[lib]
name = "nexusql"
crate-type = ["cdylib"]

[dependencies]
pyo3 = { version = "0.20", features = ["extension-module"] }

# Install maturin
pip install maturin

# Build
cd bindings/python
maturin develop  # For development
maturin build --release  # For distribution
```

---

## 📋 Pre-Publish Checklist

### For Both Platforms

- [ ] Update version in all package files
- [ ] Update CHANGELOG.md
- [ ] Test installation locally
- [ ] Verify all examples work
- [ ] Check README renders correctly
- [ ] Ensure LICENSE is included

### NPM Specific

- [ ] Run `npm run build`
- [ ] Test with `npm link`
- [ ] Check package.json metadata
- [ ] Verify TypeScript types are generated
- [ ] Test on Node.js 16, 18, 20

### PyPI Specific

- [ ] Run `python -m build`
- [ ] Test wheel installation
- [ ] Check setup.py/pyproject.toml metadata
- [ ] Verify dependencies are correct
- [ ] Test on Python 3.8, 3.9, 3.10, 3.11, 3.12

---

## 🚀 Automated Publishing (CI/CD)

### GitHub Actions for NPM

```yaml
# .github/workflows/publish-npm.yml
name: Publish to NPM

on:
  release:
    types: [created]

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with:
          node-version: '18'
          registry-url: 'https://registry.npmjs.org'
      - run: cd bindings/typescript && npm install && npm run build
      - run: cd bindings/typescript && npm publish
        env:
          NODE_AUTH_TOKEN: ${{secrets.NPM_TOKEN}}
```

### GitHub Actions for PyPI

```yaml
# .github/workflows/publish-pypi.yml
name: Publish to PyPI

on:
  release:
    types: [created]

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-python@v4
        with:
          python-version: '3.11'
      - run: pip install build twine
      - run: cd bindings/python && python -m build
      - run: cd bindings/python && twine upload dist/*
        env:
          TWINE_USERNAME: __token__
          TWINE_PASSWORD: ${{secrets.PYPI_TOKEN}}
```

---

## 📚 Additional Resources

### NPM
- [NPM Publishing Guide](https://docs.npmjs.com/packages-and-modules/contributing-packages-to-the-registry)
- [NAPI-RS Documentation](https://napi.rs/)

### PyPI
- [PyPI Publishing Guide](https://packaging.python.org/tutorials/packaging-projects/)
- [PyO3 Guide](https://pyo3.rs/)
- [Maturin Documentation](https://maturin.rs/)

---

## 🎯 Current Status

**Ready to Publish:**
- ✅ Package metadata configured
- ✅ README files created
- ✅ TypeScript/Python bindings ready
- ✅ Build configurations complete
- ✅ LICENSE included (MIT)

**To Actually Publish:**
1. Build native Rust modules with NAPI-RS/PyO3 (optional for v1)
2. Test packages locally
3. Run `npm publish` and `twine upload`
4. Share with the world! 🌍

---

**NexusQL v2.3.0** is now ready to be published to NPM and PyPI!
