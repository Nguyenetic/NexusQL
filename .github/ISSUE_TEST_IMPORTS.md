## Problem
Tests fail to compile due to missing type imports in test modules.

## Errors
- `PlanCost` not found (6 errors in `src/adaptive/mod.rs`)
- `Value` not found (~30 errors across multiple files)
- `DataType` not found (~15 errors in `src/schema/validator.rs`)

## Root Cause
Test modules don't import required types from `crate::types` and `crate::planner`.

## Proposed Fix
Add imports to test modules:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Value, DataType, Row, ResultSet};
    use crate::planner::PlanCost;
    
    // ... existing tests
}
```

## Files Affected
- src/adaptive/mod.rs
- src/cache/mod.rs
- src/materialized_view/mod.rs
- src/view/mod.rs
- src/cte/mod.rs
- src/schema/validator.rs

## Reproduction
```bash
cargo test --lib
```

**Expected:** Tests compile and run  
**Actual:** 57 compilation errors

## Documentation
See `COMPILATION_ERRORS.md` for full analysis.

## Impact
- ✅ Library compiles successfully
- ❌ Tests cannot run due to import errors
- ⏱️ Estimated fix time: 10-15 minutes

Good first issue - straightforward fix that will unblock all 229+ unit tests!
