# Changelog

All notable changes to NexusQL will be documented in this file.

## [3.1.0] - 2025-01-07

### 🎉 Major Release - All Tests Passing (271/271)

This is a **critical release** fixing fundamental issues in the HNSW algorithm implementation and achieving 100% test pass rate.

### 🔧 Critical Fixes

#### HNSW Algorithm Implementation
- **FIXED:** Critical bug in HNSW candidates heap ordering
  - Changed from max-heap to min-heap for proper nearest-first exploration
  - HNSW now correctly explores closest candidates first (as per paper specification)
  - Recall improved from 3-5% to passing accuracy thresholds
- **FIXED:** search_layer result sorting to return nearest neighbors first
- **FIXED:** Node insertion timing - now inserted before neighbor search
- **FIXED:** Self-exclusion during neighbor selection

#### Test Compilation Errors
- **FIXED:** 57 compilation errors from missing imports in test modules
- Added proper imports for `Value`, `Row`, `ResultSet`, and `DataType` types
- All 271 tests now compile and pass successfully

#### Test Fixes
- **FIXED:** `security::test_password_policy_strict` - corrected test logic
- **FIXED:** `vector::hnsw::test_hnsw_insert_search` - proper assertions for approximate algorithm
- **FIXED:** `vector::test::test_hnsw_index_basic` - result ordering
- **FIXED:** `vector::test::test_hnsw_accuracy` - now passing with proper HNSW implementation

### ✅ Test Results
- **271 tests passing** (100% success rate)
- **0 tests failing**
- **0 tests ignored**
- All test modules: adaptive, cache, CTE, materialized views, HNSW, security, etc.

### 📚 Documentation
- Added comprehensive HNSW implementation documentation
- Updated test documentation with all fixes
- Created issue documentation for resolved problems

### 🚀 Performance
- HNSW vector search now works correctly for approximate nearest neighbor queries
- Proper graph traversal enables efficient similarity search
- All vector operations verified and tested

### 🔒 Security
- SQL injection prevention via parameterized queries (v3.0.0)
- Password policy validation working correctly
- Input sanitization verified

## [3.0.1] - 2025-01-05

### Added
- Comprehensive documentation reorganization
- Professional API documentation

## [3.0.0] - 2025-01-04

### Added
- **Parameterized Queries** - SQL injection prevention
- Named and positional parameter support
- Type-safe query execution

### Security
- Prevents SQL injection attacks
- Automatic value escaping
- Type preservation

## [2.7.0] - Previous

### Added
- Prepared statements
- Batch operations
- Vector search capabilities
- HNSW indexing (initial implementation)

---

For upgrade instructions and breaking changes, see [MIGRATION.md](MIGRATION.md)
