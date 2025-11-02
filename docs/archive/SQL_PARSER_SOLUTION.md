# 🔧 SQL Parser Solution - Handle USING Clause

## Problem
`SELECT * FROM docs USING colbert` fails because sqlparser-rs doesn't recognize "USING" after FROM.

## Research Findings:

### From PostgreSQL:
- USING is reserved for:
  - JOINs: `JOIN table USING (column)`
  - Extensions: Custom syntax allowed
  - PL/pgSQL: EXECUTE ... USING for parameters

### From sqlparser-rs:
- Supports custom dialects
- Extensible architecture
- Can add custom syntax

## Solution Options:

### Option 1: Use SQL Comments (Hints) ⭐ EASIEST
```sql
-- Engine hint in comment
SELECT * FROM docs /* ENGINE: colbert */;

-- Or table alias as hint
SELECT * FROM docs AS _colbert_;
```

### Option 2: Custom Function Syntax
```sql
-- Use function call instead
SELECT * FROM SEARCH_WITH('colbert', 'docs');
SELECT * FROM COLBERT_SEARCH('docs', 'query text');
```

### Option 3: WHERE Clause Extension
```sql
-- Put engine selection in WHERE
SELECT * FROM docs WHERE ENGINE = 'colbert';
SELECT * FROM docs WHERE _engine = 'colbert';
```

### Option 4: Extend sqlparser-rs (HARD but proper)
```rust
// Create custom dialect
pub struct NexusSQLDialect;

impl Dialect for NexusSQLDialect {
    fn is_identifier_start(&self, ch: char) -> bool {
        // Custom rules
    }
    
    fn supports_filter_during_aggregation(&self) -> bool {
        true
    }
}

// Modify parser to handle USING after FROM
```

---

## RECOMMENDED: Use SQL Comments

**Why:**
- ✅ Standard SQL (no parsing changes needed)
- ✅ Works with any SQL tool
- ✅ Easy to implement (5 minutes)
- ✅ Clean syntax

**Implementation:**
```rust
fn extract_engine_hint(sql: &str) -> Option<String> {
    // Parse comment: /* ENGINE: colbert */
    if let Some(start) = sql.find("/* ENGINE:") {
        if let Some(end) = sql[start..].find("*/") {
            let hint = &sql[start+10..start+end];
            return Some(hint.trim().to_string());
        }
    }
    None
}
```

**New syntax:**
```sql
SELECT * FROM docs /* ENGINE: colbert */;
SELECT * FROM sensors /* ENGINE: hdc */;
SELECT * FROM users;  -- default engine
```

---

**Want me to implement this?** It's quick and works! 🚀
