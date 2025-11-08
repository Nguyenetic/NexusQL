# NexusQL Syntax Specification v1.0

**The Modern AI-Native Query Language**

## Philosophy

NexusQL is a brand-new query language designed from scratch for:
- **AI-First**: Native vector search, semantic queries, neural operations
- **Human-Readable**: Clear, intuitive syntax (like reading English)
- **Performance**: Optimized for modern hardware and AI workloads  
- **Type-Safe**: Strong typing with inference
- **Composable**: Functions chain naturally

---

## Core Syntax Principles

### 1. Natural Language Flow
```nexusql
-- Traditional SQL (confusing order)
SELECT name FROM users WHERE age > 18 ORDER BY created_at LIMIT 10;

-- NexusQL (natural reading order)
FIND users
  WHERE age > 18
  SORT BY created_at
  LIMIT 10
  SHOW name;
```

### 2. AI-Native Operations
```nexusql
-- Vector search (built-in, not extension)
FIND documents
  WHERE embedding SIMILAR TO $query_vector
  USING cosine
  LIMIT 10;

-- Semantic search
FIND articles
  WHERE content MATCHES "neural networks"
  USING colbert
  LIMIT 5;

-- HDC pattern matching
FIND images
  WHERE features PATTERN LIKE $target_pattern
  USING hdc
  THRESHOLD 0.8;
```

### 3. Clean Type System
```nexusql
-- Types are first-class
Int, Float, Text, Bool, Blob
Vector[384]           -- 384-dimensional vector
Json                  -- Native JSON
DateTime, Timestamp
Array[T]              -- Generic arrays
Map[K, V]             -- Key-value maps
```

---

## Complete Syntax Reference

### Table Creation

```nexusql
-- Simple table
CREATE TABLE users {
  id: Int AUTO,              -- AUTO = auto-increment
  name: Text NOT NULL,
  email: Text UNIQUE,
  age: Int,
  created: DateTime DEFAULT NOW()
};

-- Table with vectors (AI-native)
CREATE TABLE embeddings {
  id: Int AUTO,
  content: Text,
  vector: Vector[384],       -- Native vector type
  metadata: Json
};

-- Table with indexes
CREATE TABLE documents {
  id: Int AUTO,
  title: Text INDEX,         -- Inline index declaration
  content: Text,
  embedding: Vector[384] INDEX USING hnsw  -- Vector index
};
```

### Data Insertion

```nexusql
-- Simple insert
INSERT INTO users {
  name: "Alice",
  email: "alice@example.com",
  age: 25
};

-- Batch insert (JSON-like syntax)
INSERT INTO users [
  { name: "Bob", email: "bob@example.com", age: 30 },
  { name: "Charlie", email: "charlie@example.com", age: 28 }
];

-- Insert with vector
INSERT INTO embeddings {
  content: "Hello world",
  vector: [0.1, 0.2, 0.3]  -- Native array syntax
};
```

### Querying Data

```nexusql
-- Basic query (natural order)
FIND users
  WHERE age > 18
  SHOW name, email;

-- With sorting and limit
FIND users
  WHERE age BETWEEN 18 AND 65
  SORT BY created DESC
  LIMIT 10
  SHOW *;

-- Aggregation
FIND users
  GROUP BY age
  SHOW age, COUNT(*) as total;

-- Joins
FIND orders
  JOIN users ON orders.user_id = users.id
  WHERE users.age > 18
  SHOW orders.id, users.name, orders.total;
```

### Vector Search

```nexusql
-- Similarity search (L2 distance)
FIND embeddings
  WHERE vector NEAR $query_vector
  LIMIT 10
  SHOW id, content, DISTANCE(vector, $query_vector) as dist;

-- Cosine similarity
FIND embeddings
  WHERE vector SIMILAR TO $query_vector
  USING cosine
  THRESHOLD 0.8
  LIMIT 10;
```

### AI-Native Queries

```nexusql
-- Semantic search with ColBERT
FIND documents
  WHERE content MATCHES "artificial intelligence"
  USING ENGINE colbert
  LIMIT 10
  SHOW id, title, SCORE() as relevance;

-- HDC pattern recognition
FIND sensors
  WHERE signal PATTERN LIKE $target_pattern
  USING ENGINE hdc
  SHOW id, timestamp, SIMILARITY() as match_score;
```

### Updates

```nexusql
-- Simple update
UPDATE users
  SET age: 26
  WHERE id = 1;

-- Multi-field update
UPDATE products
  SET {
    price: 999.99,
    stock: 50,
    updated_at: NOW()
  }
  WHERE category = "electronics";
```

### Deletion

```nexusql
DELETE FROM users WHERE id = 1;
```

### Transactions

```nexusql
BEGIN TRANSACTION;
  INSERT INTO accounts { user_id: 1, balance: 1000 };
  UPDATE accounts SET balance: balance - 100 WHERE user_id: 1;
COMMIT;
```

### Indexes

```nexusql
-- Create index
CREATE INDEX idx_users_email ON users(email);

-- Vector index (HNSW)
CREATE VECTOR INDEX idx_embeddings_vec 
  ON embeddings(vector)
  USING hnsw
  WITH {
    m: 16,
    ef_construction: 200
  };
```

---

## Parameterized Queries

### Positional Parameters
```nexusql
FIND users WHERE age > $1 AND city = $2;
-- Execute with: params = [18, "New York"]
```

### Named Parameters
```nexusql
FIND users WHERE age > :min_age AND city = :city;
-- Execute with: { min_age: 18, city: "New York" }
```

---

## Type System

### Primitive Types
- `Int` - 64-bit integer
- `Float` - 64-bit floating point
- `Text` - UTF-8 string
- `Bool` - true/false
- `Blob` - Binary data

### Composite Types
- `Vector[n]` - n-dimensional vector
- `Array[T]` - Array of type T
- `Map[K, V]` - Key-value map
- `Json` - JSON object
- `DateTime` - Date and time
- `Timestamp` - Unix timestamp

---

## Backward Compatibility

NexusQL can parse standard SQL and convert it internally:

```nexusql
-- Standard SQL works too!
SELECT * FROM users WHERE age > 18;

-- Automatically converted to:
FIND users WHERE age > 18 SHOW *;
```

Enable compatibility mode:
```nexusql
SET COMPATIBILITY MODE sql;      -- Accept SQL syntax
SET COMPATIBILITY MODE nexusql;  -- Pure NexusQL only
SET COMPATIBILITY MODE auto;     -- Auto-detect and convert (default)
```

---

## Comparison: SQL vs NexusQL

| Feature | SQL | NexusQL |
|---------|-----|---------|
| **Query Order** | `SELECT ... FROM ... WHERE` | `FIND ... WHERE ... SHOW` |
| **Auto-Increment** | `SERIAL` or `AUTO_INCREMENT` | `Int AUTO` |
| **Insert Syntax** | `INSERT INTO t VALUES (...)` | `INSERT INTO t { key: value }` |
| **Vector Type** | Extension (pgvector) | Built-in `Vector[n]` |
| **Semantic Search** | Not supported | `MATCHES "text"` |
| **JSON** | `JSONB` type | Native `Json` type |
| **Parameters** | `$1` or `?` | `$1` or `:name` |

---

## Complete Example: E-Commerce Database

```nexusql
-- Create tables
CREATE TABLE users {
  id: Int AUTO,
  name: Text NOT NULL,
  email: Text UNIQUE,
  created_at: DateTime DEFAULT NOW()
};

CREATE TABLE products {
  id: Int AUTO,
  name: Text INDEX,
  description: Text,
  price: Float,
  embedding: Vector[384] INDEX USING hnsw,
  metadata: Json
};

CREATE TABLE orders {
  id: Int AUTO,
  user_id: Int REFERENCES users(id),
  product_id: Int REFERENCES products(id),
  quantity: Int,
  total: Float,
  created_at: DateTime DEFAULT NOW()
};

-- Insert data
INSERT INTO users { name: "Alice", email: "alice@example.com" };

INSERT INTO products {
  name: "Laptop",
  description: "High-performance laptop for AI/ML",
  price: 1299.99,
  embedding: [0.1, 0.2, ...],
  metadata: { brand: "TechCo", warranty: "2 years" }
};

-- Query with joins and vector search
FIND orders
  JOIN users ON orders.user_id = users.id
  JOIN products ON orders.product_id = products.id
  WHERE {
    products.embedding SIMILAR TO $query_vector
    AND products.price < 2000
    AND users.created_at > DATE_SUB(NOW(), INTERVAL 30 DAYS)
  }
  SORT BY SIMILARITY(products.embedding, $query_vector) DESC
  LIMIT 10
  SHOW {
    orders.id,
    users.name,
    products.name,
    orders.total,
    SIMILARITY(products.embedding, $query_vector) as relevance
  };
```

---

## Implementation Roadmap

1. ✅ Syntax design (this document)
2. 🔄 Lexer implementation (tokenizer for NexusQL)
3. 🔄 Parser implementation (recursive descent + Pratt)
4. 🔄 AST optimization
5. 🔄 Backward compatibility layer (SQL → NexusQL translator)
6. 🔄 Complete test suite
7. 🔄 TypeScript/Python bindings update
8. 🔄 Documentation and examples

---

## Grammar (EBNF Notation)

```ebnf
(* NexusQL Grammar *)

statement = create_table | insert | find | update | delete | transaction ;

create_table = "CREATE" "TABLE" identifier "{" column_def { "," column_def } "}" ";" ;

column_def = identifier ":" type [ "AUTO" | "NOT NULL" | "UNIQUE" | "INDEX" | "DEFAULT" expr ] ;

insert = "INSERT" "INTO" identifier ( insert_object | insert_array ) ";" ;

find = "FIND" identifier 
       [ "JOIN" join_clause ]
       [ "WHERE" expr ]
       [ "GROUP" "BY" expr_list ]
       [ "SORT" "BY" sort_list ]
       [ "LIMIT" number ]
       [ "SHOW" ( "*" | expr_list ) ]
       [ "USING" "ENGINE" identifier ] ";" ;

update = "UPDATE" identifier "SET" ( set_expr | set_object ) "WHERE" expr ";" ;

delete = "DELETE" "FROM" identifier "WHERE" expr ";" ;

transaction = "BEGIN" "TRANSACTION" ";" statement_list ( "COMMIT" | "ROLLBACK" ) ";" ;

type = "Int" | "Float" | "Text" | "Bool" | "Blob" 
     | "Vector" "[" number "]"
     | "Array" "[" type "]"
     | "Map" "[" type "," type "]"
     | "Json" | "DateTime" | "Timestamp" ;

expr = literal | identifier | function_call | binary_op | unary_op ;
```

---

## References

- **Pratt Parsing**: "Top Down Operator Precedence" by Vaughan Pratt (1973)
- **Recursive Descent**: "Compilers: Principles, Techniques, and Tools" (Dragon Book)
- **Modern Query Languages**: GraphQL, MongoDB Query Language, Prisma
- **Vector Databases**: Pinecone, Weaviate, Qdrant query syntax
- **AI-Native Design**: ColBERT, FAISS, HNSW algorithm papers

---

*NexusQL Syntax Specification v1.0*  
*The Future of AI-Native Databases*
