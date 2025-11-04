# Parser Auto-Converts TEXT Fields to VECTOR Type

## Bug Description
The SQL parser aggressively auto-converts any TEXT field value that starts with `[` and ends with `]` to a VECTOR type, even when the column schema explicitly defines it as TEXT. This breaks storing JSON arrays or any bracket-wrapped content in TEXT columns.

## Root Cause
**File**: `src/query/parser.rs`
**Lines**: 405-408

```rust
sql_ast::Value::SingleQuotedString(s) | sql_ast::Value::DoubleQuotedString(s) => {
    // Check if this is a vector literal like '[0.1, 0.2, 0.3]'
    if s.starts_with('[') && s.ends_with(']') {
        return self.parse_vector_literal(s);
    }
    Ok(Value::Text(s.clone()))
}
```

The parser checks for bracket syntax **before** checking the column's actual type from the schema.

## Steps to Reproduce
```sql
CREATE TABLE test (
    id INTEGER PRIMARY KEY,
    data TEXT NOT NULL
);

INSERT INTO test (id, data) VALUES (1, '["item1", "item2"]');
-- Error: Invalid vector element: "item1"
```

## Expected Behavior
- Parser should respect the column schema type
- TEXT columns should accept any string value, including bracket-wrapped content
- Only columns defined as VECTOR should trigger vector parsing

## Actual Behavior
- Parser tries to parse `["item1", "item2"]` as a VECTOR
- Fails with: `Parse error: Invalid vector element: "item1"`
- Cannot store JSON arrays in TEXT columns

## Workaround
Currently using comma-separated strings instead of JSON arrays:
```sql
-- Instead of: '["item1", "item2"]'
-- Use: 'item1, item2'
```

## Proposed Fix
1. Parser should look up the column type from the table schema
2. Only auto-convert to VECTOR if the column is defined as VECTOR type
3. For TEXT columns, keep the value as-is regardless of bracket syntax

## Impact
- **Severity**: Medium
- **Affects**: Any application storing JSON data in TEXT columns
- **Workaround exists**: Yes (use alternative string formats)

## Related Code
- Parser logic: `src/query/parser.rs:405-408`
- Vector literal parsing: `src/query/parser.rs:440-460`
- Test case: `bindings/typescript/test-mcp-sage-real-data.js:347-359`

## Environment
- NexusQL version: 2.5.0
- Platform: Windows/Linux/macOS (affects all platforms)
