# NexusQL - Python Client

AI-Native, PostgreSQL-compatible database for Python applications.

## Installation

```bash
pip install nexusql
```

## Quick Start

```python
from nexusql import NexusDB

# Connect to database
db = NexusDB('./mydb')

# Execute queries
db.execute('CREATE TABLE users (id INT PRIMARY KEY, name TEXT)')
db.execute('INSERT INTO users VALUES (1, "Alice")')

result = db.query('SELECT * FROM users')
for row in result.rows:
    print(row['name'])

# Use transactions
with db.transaction():
    db.execute('INSERT INTO users VALUES (2, "Bob")')
    db.execute('UPDATE accounts SET balance = balance - 100')
    # Auto-commits on success, rolls back on exception

db.close()
```

## Features

- ✅ **PostgreSQL Compatible**: Full SQL support
- ✅ **AI-Native**: Vector embeddings and semantic search
- ✅ **Embedded**: No separate server needed
- ✅ **ACID**: Full transaction support
- ✅ **Fast**: Advanced indexing (B-Tree, HNSW, GIN, GiST, BRIN)
- ✅ **Pythonic**: Context managers, type hints, docstrings
- ✅ **Scalable**: Partitioning and sharding support

## API Reference

### NexusDB

Main database class.

#### Methods

- `query(sql: str) -> QueryResult` - Execute SELECT query
- `execute(sql: str) -> int` - Execute command, returns affected rows
- `execute_many(sql: str, params_list: List[tuple]) -> int` - Bulk execute
- `transaction()` - Context manager for transactions
- `prepare(sql: str) -> PreparedStatement` - Create prepared statement
- `close()` - Close connection

### Example: Prepared Statements

```python
# Prepare once, execute many times
stmt = db.prepare('SELECT * FROM users WHERE id = $1')

result = stmt.execute(1)
result = stmt.execute(2)
result = stmt.execute(3)

stmt.close()
```

### Example: Connection Pool

```python
from nexusql import ConnectionPool

pool = ConnectionPool('./mydb', max_connections=20)

with pool.connection() as conn:
    result = conn.query('SELECT * FROM users')
    # Connection automatically returned to pool
```

### Example: Bulk Operations

```python
# Insert multiple rows efficiently
db.execute_many(
    'INSERT INTO users VALUES ($1, $2)',
    [(1, 'Alice'), (2, 'Bob'), (3, 'Charlie')]
)
```

## Advanced Features

### AI-Native Search

```python
# Vector similarity search
db.execute('''
    SELECT * FROM documents
    WHERE embedding <-> text_to_vector('search query') < 0.5
    ORDER BY embedding <-> text_to_vector('search query')
    LIMIT 10
''')
```

### Full-Text Search

```python
# Full-text search with ranking
db.execute('''
    SELECT *, ts_rank(document_vector, query) as rank
    FROM articles
    WHERE document_vector @@ to_tsquery('database & optimization')
    ORDER BY rank DESC
''')
```

## Installation from Source

```bash
# Clone repository
git clone https://github.com/johnhnguyen97/NexusQL.git
cd NexusQL/bindings/python

# Install in development mode
pip install -e .

# Or build wheel
pip install build
python -m build
pip install dist/nexusql-2.3.0-py3-none-any.whl
```

## Requirements

- Python 3.8+
- Rust toolchain (for building from source)

## License

MIT

## Links

- [GitHub Repository](https://github.com/johnhnguyen97/NexusQL)
- [Documentation](https://github.com/johnhnguyen97/NexusQL#readme)
- [Issue Tracker](https://github.com/johnhnguyen97/NexusQL/issues)
