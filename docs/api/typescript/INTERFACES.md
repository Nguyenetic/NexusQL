# TypeScript API - Interfaces

Complete reference for all interfaces and type definitions in the NexusQL TypeScript API.

---

## QueryResult

Result object returned from query operations.

### Definition

```typescript
interface QueryResult {
  rows: any;
  rowCount: number;
  fields: Array<Field>;
}
```

### Properties

#### rows: any

Query result rows as a JavaScript array of objects.

**Type:** `any` (typically `Array<Record<string, any>>`)

**Structure:**
```typescript
[
  { id: 1, name: 'Alice', email: 'alice@example.com' },
  { id: 2, name: 'Bob', email: 'bob@example.com' }
]
```

**Notes:**
- Automatically parsed from JSON (no manual parsing needed with wrapper)
- Each row is an object with column names as keys
- Values are typed according to the database column type:
  - `INTEGER` → `number`
  - `TEXT` → `string`
  - `FLOAT` / `REAL` → `number`
  - `BOOLEAN` → `boolean`
  - `NULL` → `null`
  - `BLOB` → `Uint8Array`
  - `VECTOR` → `number[]`

**Example:**
```typescript
const result = await db.query('SELECT id, name FROM users WHERE age > $1', [25]);

console.log(result.rows);
// [
//   { id: 1, name: 'Alice' },
//   { id: 3, name: 'Charlie' }
// ]

// Access individual rows
for (const row of result.rows) {
  console.log(`User ${row.id}: ${row.name}`);
}

// TypeScript type assertion for better type safety
interface User {
  id: number;
  name: string;
}

const users = result.rows as User[];
users.forEach(user => console.log(user.name.toUpperCase()));
```

---

#### rowCount: number

Number of rows returned or affected.

**Type:** `number`

**Behavior:**
- **SELECT:** Number of rows returned
- **INSERT:** Number of rows inserted
- **UPDATE:** Number of rows updated
- **DELETE:** Number of rows deleted
- **CREATE TABLE:** 0
- **CREATE INDEX:** 0

**Example:**
```typescript
// Query
const selectResult = await db.query('SELECT * FROM users');
console.log(`Found ${selectResult.rowCount} users`); // Found 10 users

// Insert
const insertCount = await db.execute('INSERT INTO users VALUES ($1, $2)', [11, 'Zoe']);
console.log(`Inserted ${insertCount} row(s)`); // Inserted 1 row(s)

// Update
const updateCount = await db.execute('UPDATE users SET active = true WHERE age > $1', [18]);
console.log(`Updated ${updateCount} row(s)`); // Updated 8 row(s)

// Delete
const deleteCount = await db.execute('DELETE FROM users WHERE age < $1', [18]);
console.log(`Deleted ${deleteCount} row(s)`); // Deleted 2 row(s)
```

---

#### fields: Array<Field>

Metadata about the columns in the result set.

**Type:** `Array<Field>`

**Structure:**
```typescript
[
  { name: 'id', fieldType: 'INTEGER' },
  { name: 'name', fieldType: 'TEXT' },
  { name: 'email', fieldType: 'TEXT' }
]
```

**Example:**
```typescript
const result = await db.query('SELECT id, name, email FROM users LIMIT 1');

console.log('Columns in result:');
result.fields.forEach(field => {
  console.log(`  ${field.name}: ${field.fieldType}`);
});

// Output:
// Columns in result:
//   id: INTEGER
//   name: TEXT
//   email: TEXT
```

**Use Cases:**
- Dynamic table rendering
- Column type validation
- Schema introspection
- Building generic data viewers

---

### Complete Example

```typescript
const result = await db.query('SELECT * FROM users WHERE age > $1', [25]);

// Access all properties
console.log('Query Results:');
console.log(`  Rows returned: ${result.rowCount}`);
console.log(`  Columns: ${result.fields.map(f => f.name).join(', ')}`);
console.log(`  Data:`, result.rows);

// Output:
// Query Results:
//   Rows returned: 3
//   Columns: id, name, email, age
//   Data: [
//     { id: 1, name: 'Alice', email: 'alice@example.com', age: 30 },
//     { id: 3, name: 'Charlie', email: 'charlie@example.com', age: 35 },
//     { id: 5, name: 'Eve', email: 'eve@example.com', age: 28 }
//   ]
```

---

## Field

Metadata about a single column in a query result.

### Definition

```typescript
interface Field {
  name: string;
  fieldType: string;
}
```

### Properties

#### name: string

The column name.

**Type:** `string`

**Examples:**
- `"id"`
- `"user_name"`
- `"created_at"`
- `"COUNT(*)"`  (for aggregate functions)
- `"u.email"` (for table-qualified names)

**Notes:**
- Matches the column name in the SELECT clause
- For aliases: `SELECT name AS user_name` → `name: "user_name"`
- For expressions: `SELECT age * 2` → `name: "age * 2"` (or use `AS` for clarity)

---

#### fieldType: string

The SQL data type of the column.

**Type:** `string`

**Common Values:**
- `"INTEGER"` - Integer numbers
- `"TEXT"` - String data
- `"FLOAT"` / `"REAL"` - Floating-point numbers
- `"BOOLEAN"` - True/false values
- `"BLOB"` - Binary data
- `"VECTOR"` - Vector embeddings (NexusQL-specific)
- `"NULL"` - NULL type

**Example:**
```typescript
const result = await db.query(`
  SELECT
    id,
    name,
    age,
    balance,
    active,
    embedding
  FROM users
  LIMIT 1
`);

result.fields.forEach(field => {
  console.log(`${field.name}: ${field.fieldType}`);
});

// Output:
// id: INTEGER
// name: TEXT
// age: INTEGER
// balance: FLOAT
// active: BOOLEAN
// embedding: VECTOR
```

---

### Use Cases

**1. Dynamic Table Rendering**

```typescript
function renderTable(result: QueryResult) {
  // Header
  const header = result.fields.map(f => f.name).join('\t');
  console.log(header);
  console.log('-'.repeat(header.length));

  // Rows
  result.rows.forEach(row => {
    const values = result.fields.map(f => row[f.name]);
    console.log(values.join('\t'));
  });
}

const result = await db.query('SELECT * FROM users LIMIT 5');
renderTable(result);
```

**2. Type Validation**

```typescript
function validateTypes(result: QueryResult, expected: Record<string, string>) {
  for (const field of result.fields) {
    if (expected[field.name] && expected[field.name] !== field.fieldType) {
      throw new Error(`Column ${field.name} has type ${field.fieldType}, expected ${expected[field.name]}`);
    }
  }
}

const result = await db.query('SELECT id, name FROM users LIMIT 1');
validateTypes(result, { id: 'INTEGER', name: 'TEXT' }); // ✅ Pass
```

**3. Schema Introspection**

```typescript
async function getTableSchema(db: NexusDb, tableName: string) {
  const result = await db.query(`SELECT * FROM ${tableName} LIMIT 0`);

  return result.fields.map(field => ({
    column: field.name,
    type: field.fieldType
  }));
}

const schema = await getTableSchema(db, 'users');
console.log(schema);
// [
//   { column: 'id', type: 'INTEGER' },
//   { column: 'name', type: 'TEXT' },
//   { column: 'email', type: 'TEXT' }
// ]
```

---

## Type Guards

Useful TypeScript type guards for working with query results:

```typescript
// Check if result has rows
function hasRows(result: QueryResult): boolean {
  return result.rowCount > 0 && result.rows.length > 0;
}

// Get single row or null
function getSingleRow<T>(result: QueryResult): T | null {
  return result.rowCount === 1 ? (result.rows[0] as T) : null;
}

// Type-safe row access
interface User {
  id: number;
  name: string;
  email: string;
}

const result = await db.query('SELECT * FROM users WHERE id = $1', [1]);
const user = getSingleRow<User>(result);

if (user) {
  console.log(user.name); // TypeScript knows user is User
}
```

---

## TypeScript Type Definitions

Complete type definitions for import:

```typescript
import {
  NexusDb,
  Transaction,
  PreparedStatement,
  QueryResult,
  Field
} from '@nguyenetic/nexusql';

// Use with full type safety
async function example() {
  const db: NexusDb = new NexusDb('./app.db');

  const result: QueryResult = await db.query('SELECT * FROM users');

  const fields: Field[] = result.fields;
  const rows: any[] = result.rows;
  const count: number = result.rowCount;

  await db.close();
}
```

---

## Next Steps

- [Class Reference](./CLASSES.md) - NexusDb, Transaction, PreparedStatement
- [Method Reference](./METHODS.md) - Complete method documentation
- [Examples](./EXAMPLES.md) - Real-world usage patterns
