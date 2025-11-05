# Add parameterized query support for SQL injection prevention

## Description
Currently NexusQL requires manual string escaping for SQL queries. Parameterized queries (prepared statements) are industry best practice for preventing SQL injection and should be implemented.

## Current State
Users must manually escape values:
```javascript
// ❌ Vulnerable to SQL injection
const name = getUserInput();
await db.execute(`INSERT INTO users (name) VALUES ('${name}')`);

// ⚠️ Manual escaping required
const escaped = name.replace(/'/g, "''");
await db.execute(`INSERT INTO users (name) VALUES ('${escaped}')`);
```

## Proposed Solution
Add parameterized query support:
```javascript
// ✅ Safe from SQL injection
await db.execute(
  'INSERT INTO users (name, email) VALUES (?, ?)',
  [name, email]
);

// ✅ Named parameters
await db.execute(
  'INSERT INTO users (name, email) VALUES (:name, :email)',
  { name: name, email: email }
);
```

## Implementation Options

### Option 1: Positional Parameters
```javascript
db.execute('SELECT * FROM users WHERE id = ? AND status = ?', [123, 'active'])
```

### Option 2: Named Parameters
```javascript
db.execute('SELECT * FROM users WHERE id = :id AND status = :status', {
  id: 123,
  status: 'active'
})
```

### Option 3: Both (Recommended)
Support both positional and named parameters for flexibility.

## API Design

```typescript
interface NexusDb {
  // Current API (keep for backward compatibility)
  execute(sql: string): Promise<void>;
  query(sql: string): Promise<ResultSet>;

  // New parameterized API
  execute(sql: string, params: any[] | Record<string, any>): Promise<void>;
  query(sql: string, params: any[] | Record<string, any>): Promise<ResultSet>;

  // Optional: Prepared statements
  prepare(sql: string): PreparedStatement;
}

interface PreparedStatement {
  execute(params: any[] | Record<string, any>): Promise<void>;
  query(params: any[] | Record<string, any>): Promise<ResultSet>;
  finalize(): void;
}
```

## Security Benefits
- ✅ **Prevents SQL injection** - values never interpreted as SQL
- ✅ **Automatic escaping** - no manual string manipulation
- ✅ **Type safety** - values maintain their types
- ✅ **Performance** - query plan caching (with prepared statements)

## Implementation Details

### Parser Changes
1. Detect parameter placeholders (`?` or `:name`)
2. Replace with parsed values during execution
3. Ensure values bypass SQL parsing

### Type Mapping
- JavaScript strings → TEXT
- JavaScript numbers → INTEGER/REAL
- JavaScript arrays (numeric) → VECTOR
- JavaScript objects → TEXT (JSON)
- JavaScript booleans → BOOLEAN
- null/undefined → NULL

### Vector Support
```javascript
// Vector parameters
await db.execute(
  'INSERT INTO memories (content, embedding) VALUES (?, ?)',
  ['User prefers TypeScript', [0.1, 0.2, 0.3, ...]]
);
```

## Examples from Test Suite

Current workaround (from `test-mcp-sage-real-data.js`):
```javascript
const contentEscaped = mem.content.replace(/'/g, "''");
const metadataStr = JSON.stringify(mem.metadata || {}).replace(/'/g, "''");

await db.execute(`
  INSERT INTO memories (content, metadata)
  VALUES ('${contentEscaped}', '${metadataStr}')
`);
```

With parameterized queries:
```javascript
await db.execute(
  'INSERT INTO memories (content, metadata) VALUES (?, ?)',
  [mem.content, JSON.stringify(mem.metadata || {})]
);
```

## References
- OWASP: Parameterized Queries - https://cheatsheetseries.owasp.org/cheatsheets/Query_Parameterization_Cheat_Sheet.html
- Node.js SQL Injection Guide - https://www.stackhawk.com/blog/node-js-sql-injection-guide-examples-and-prevention/
- SQLite Prepared Statements - https://www.sqlite.org/c3ref/prepare.html

## Priority
**High** - Security best practice, cited in test documentation (`TESTING_AND_SECURITY_REFERENCES.md`)

## Related Files
- Parser: `src/query/parser.rs`
- Test file using workarounds: `bindings/typescript/test-mcp-sage-real-data.js`
- TypeScript bindings: `bindings/typescript/src/lib.rs`

## Breaking Changes
None - this is purely additive. Existing string-based queries continue to work.
