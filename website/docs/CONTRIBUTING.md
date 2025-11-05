# Contributing to NexusQL

Thank you for your interest in contributing to NexusQL! This document provides guidelines and instructions for contributing to the project.

## 📋 Table of Contents

- [📜 Code of Conduct](#-code-of-conduct)
- [🚀 Getting Started](#-getting-started)
- [💻 Development Setup](#-development-setup)
- [🔄 Contribution Workflow](#-contribution-workflow)
- [🎨 Coding Standards](#-coding-standards)
- [🧪 Testing Requirements](#-testing-requirements)
- [📝 Documentation](#-documentation)
- [🔀 Pull Request Process](#-pull-request-process)
- [🐛 Issue Guidelines](#-issue-guidelines)

## 📜 Code of Conduct

This project adheres to a Code of Conduct that all contributors are expected to follow. Please read [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) before contributing.

## 🚀 Getting Started

### Prerequisites

- **Rust:** 1.70 or higher
- **Node.js:** 16.0.0 or higher (for TypeScript bindings)
- **Cargo:** Latest stable version
- **Git:** For version control

### Areas for Contribution

We welcome contributions in the following areas:

- 🐛 **Bug fixes** - Fix reported issues
- ✨ **New features** - Implement planned features from the roadmap
- 📝 **Documentation** - Improve guides, API docs, and examples
- 🧪 **Tests** - Add test coverage, integration tests
- 🎨 **Examples** - Create real-world usage examples
- ⚡ **Performance** - Optimize queries, indexing, storage
- 🔒 **Security** - Enhance security features, fix vulnerabilities

## 💻 Development Setup

### Clone the Repository

```bash
git clone https://github.com/Nguyenetic/NexusQL.git
cd NexusQL
```

### Build Rust Core

```bash
# Build library
cargo build

# Run tests
cargo test

# Run with optimizations
cargo build --release
```

### Build TypeScript Bindings

```bash
cd bindings/typescript

# Install dependencies
npm install

# Build native module and TypeScript definitions
npm run build

# Run tests
npm test
```

### Build Python Bindings (Future)

```bash
cd bindings/python
pip install -e .
pytest
```

## 🔄 Contribution Workflow

### 1. Fork the Repository

Click the "Fork" button on GitHub to create your own copy of the repository.

### 2. Create a Feature Branch

```bash
git checkout -b feature/your-feature-name
# or
git checkout -b fix/issue-number-description
```

**Branch naming conventions:**
- `feature/` - New features
- `fix/` - Bug fixes
- `docs/` - Documentation improvements
- `test/` - Test additions or improvements
- `refactor/` - Code refactoring
- `perf/` - Performance improvements

### 3. Make Your Changes

- Write clean, readable code
- Follow the coding standards (see below)
- Add tests for new functionality
- Update documentation as needed
- Keep commits focused and atomic

### 4. Commit Your Changes

```bash
git add .
git commit -m "feat: Add vector similarity search optimization"
```

**Commit message format:**
```
<type>: <description>

[optional body]

[optional footer]
```

**Types:**
- `feat:` - New feature
- `fix:` - Bug fix
- `docs:` - Documentation changes
- `test:` - Test additions/updates
- `refactor:` - Code refactoring
- `perf:` - Performance improvement
- `chore:` - Build/tooling changes

**Examples:**
```
feat: Add HNSW vector index support

Implements hierarchical navigable small world graph for
approximate nearest neighbor search with O(log N) complexity.

Closes #123
```

```
fix: Prevent SQL injection in parameterized queries

Properly escapes user input in prepared statements.

Fixes #456
```

### 5. Push to Your Fork

```bash
git push origin feature/your-feature-name
```

### 6. Open a Pull Request

- Go to the original repository on GitHub
- Click "New Pull Request"
- Select your fork and branch
- Fill out the PR template with details
- Link related issues

## 🎨 Coding Standards

### Rust Code Style

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `rustfmt` for formatting: `cargo fmt`
- Use `clippy` for linting: `cargo clippy`
- Document public APIs with doc comments

**Example:**
```rust
/// Execute a SQL query with optional parameters.
///
/// # Arguments
///
/// * `sql` - SQL statement with $1, $2, etc. placeholders
/// * `params` - Optional vector of parameter values
///
/// # Returns
///
/// Returns a `Result` containing the query results or an error.
///
/// # Examples
///
/// ```
/// let result = db.query("SELECT * FROM users WHERE id = $1", vec![Value::Integer(1)])?;
/// ```
pub async fn query(&self, sql: &str, params: Option<Vec<Value>>) -> Result<ResultSet> {
    // Implementation
}
```

### TypeScript Code Style

- Use TypeScript for type safety
- Follow [TypeScript guidelines](https://www.typescriptlang.org/docs/handbook/declaration-files/do-s-and-don-ts.html)
- Use Prettier for formatting
- Use ESLint for linting

**Example:**
```typescript
/**
 * Execute a query with optional parameters
 * @param sql - SQL query with $1, $2 placeholders
 * @param params - Optional array of parameter values
 * @returns Promise resolving to query results
 * @throws Error if SQL is invalid or execution fails
 */
async query(sql: string, params?: any[]): Promise<QueryResult> {
  // Implementation
}
```

### General Guidelines

- **Keep it simple:** Prefer clarity over cleverness
- **DRY principle:** Don't repeat yourself
- **Single responsibility:** One function, one purpose
- **Error handling:** Always handle errors gracefully
- **Comments:** Explain *why*, not *what*
- **Naming:** Use descriptive, meaningful names

## 🧪 Testing Requirements

### Rust Tests

All new features must include tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parameterized_query() {
        let db = NexusSQL::new(":memory:").await.unwrap();

        db.execute("CREATE TABLE users (id INT, name TEXT)").await.unwrap();
        db.execute("INSERT INTO users VALUES ($1, $2)")
            .with_params(vec![Value::Integer(1), Value::Text("Alice".to_string())])
            .await
            .unwrap();

        let result = db.query("SELECT * FROM users WHERE id = $1")
            .with_params(vec![Value::Integer(1)])
            .await
            .unwrap();

        assert_eq!(result.rows.len(), 1);
    }
}
```

**Run tests:**
```bash
cargo test
cargo test --lib module_name::tests
```

### TypeScript Tests

Use Jest for testing:

```typescript
describe('NexusDb', () => {
  let db: NexusDb;

  beforeEach(() => {
    db = new NexusDb(':memory:');
  });

  afterEach(async () => {
    await db.close();
  });

  test('should execute parameterized query', async () => {
    await db.execute('CREATE TABLE users (id INT, name TEXT)');
    await db.execute('INSERT INTO users VALUES ($1, $2)', [1, 'Alice']);

    const result = await db.query('SELECT * FROM users WHERE id = $1', [1]);

    expect(result.rowCount).toBe(1);
    expect(result.rows[0].name).toBe('Alice');
  });
});
```

**Run tests:**
```bash
npm test
npm test -- --coverage
```

### Test Coverage

- Aim for >80% code coverage
- Test edge cases and error conditions
- Test with both `:memory:` and file-based databases
- Include integration tests for complex features

## 📝 Documentation

### Documentation Requirements

When contributing, please update:

1. **Code comments** - Document complex logic
2. **API documentation** - Update docs/api/ if API changes
3. **README files** - Update relevant README files
4. **CHANGELOG.md** - Add entry for your changes
5. **Examples** - Add usage examples if applicable

### Writing Good Documentation

- Use clear, concise language
- Include code examples
- Explain *why* and *when* to use features
- Document edge cases and limitations
- Keep documentation in sync with code

## 🔀 Pull Request Process

### Before Submitting

- ✅ Run all tests: `cargo test && cd bindings/typescript && npm test`
- ✅ Run formatters: `cargo fmt && npm run format`
- ✅ Run linters: `cargo clippy && npm run lint`
- ✅ Update documentation
- ✅ Add changelog entry
- ✅ Rebase on latest main branch

### PR Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Documentation update
- [ ] Performance improvement
- [ ] Refactoring

## Related Issues
Closes #123

## Testing
- [ ] Added unit tests
- [ ] Added integration tests
- [ ] Manual testing completed

## Checklist
- [ ] Code follows project style guidelines
- [ ] Self-review completed
- [ ] Comments added for complex code
- [ ] Documentation updated
- [ ] No new warnings
- [ ] Tests pass locally
```

### Review Process

1. **Automated checks** - CI/CD runs tests and linters
2. **Code review** - Maintainers review your code
3. **Requested changes** - Address feedback
4. **Approval** - Maintainer approves PR
5. **Merge** - PR is merged into main branch

### After Merge

- Your contribution will be included in the next release
- You'll be added to the contributors list
- Thank you for making NexusQL better! 🎉

## 🐛 Issue Guidelines

### Reporting Bugs

Use the bug report template and include:

- **Description:** Clear description of the bug
- **Steps to reproduce:** Minimal reproduction steps
- **Expected behavior:** What should happen
- **Actual behavior:** What actually happens
- **Environment:** OS, Rust version, Node.js version
- **Code sample:** Minimal code that reproduces the issue

### Feature Requests

Use the feature request template and include:

- **Problem:** What problem does this solve?
- **Solution:** Proposed solution or API
- **Alternatives:** Other solutions considered
- **Use case:** Real-world usage example

### Questions

For questions:
- Check existing documentation
- Search closed issues
- Open a discussion (not an issue)

## 🙏 Recognition

Contributors are recognized in:
- `README.md` Contributors section
- Release notes
- `CHANGELOG.md`

## 📞 Getting Help

- **Documentation:** https://github.com/Nguyenetic/NexusQL/tree/main/docs
- **Issues:** https://github.com/Nguyenetic/NexusQL/issues
- **Discussions:** https://github.com/Nguyenetic/NexusQL/discussions

## 📄 License

By contributing to NexusQL, you agree that your contributions will be licensed under the MIT License.

---

**Thank you for contributing to NexusQL!** 🚀
