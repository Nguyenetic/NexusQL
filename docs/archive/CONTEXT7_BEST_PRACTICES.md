# Context7 Best Practices for Insert Performance

## Research Summary

Based on research from ClickHouse, RocksDB, and PostgreSQL production databases, here are the proven techniques to improve insert performance WITHOUT losing data quality.

---

## 🎯 Key Findings

### Current Performance
- **Baseline:** 52,532 rows/sec (19.04s for 1M rows)
- **Hyper-optimized (client-side):** 40,671 rows/sec (24.59s) - **SLOWER**
- **Bottleneck:** 95.7% of time spent in execution (disk I/O, parsing, locking)

### Root Cause
**Client-side optimizations don't help** when the bottleneck is in the database engine itself (disk writes, WAL, locking).

---

## ✅ Production-Grade Solutions (Context7 Research)

### 1. **Group Commit** (PostgreSQL Best Practice)

**What it does:**
- Batches multiple transactions into a single fsync operation
- Reduces I/O overhead by 2-5x

**Configuration:**
```rust
group_commit_delay_us: 100,      // 100 microseconds
group_commit_min_siblings: 5,    // Wait if 5+ transactions pending
```

**How it works:**
1. First transaction starts commit
2. Wait for `commit_delay` microseconds
3. If `commit_siblings` other transactions are pending, batch them together
4. Single fsync for all transactions

**Benefits:**
- ✅ 2-5x throughput improvement in high-concurrency
- ✅ ACID compliant - all commits are durable
- ✅ Minimal latency impact (100μs)

**Source:** PostgreSQL Documentation 17-18

---

### 2. **Async Write-Behind** (RocksDB Best Practice)

**What it does:**
- Uses `bytes_per_sync` to trigger async OS write-back
- OS writes to disk in background while app continues

**Configuration:**
```rust
bytes_per_sync: 1_048_576,  // 1MB - triggers async write-back
```

**How it works:**
1. Data is written to OS buffer
2. When buffer reaches `bytes_per_sync`, OS starts async write to disk
3. Application doesn't wait for disk write
4. WAL still ensures durability

**Benefits:**
- ✅ **40% improvement** (proven in MyRocks benchmark)
- ✅ Data is in OS buffer, protected by WAL
- ✅ Reduces wait time for fsync/fdatasync

**Source:** RocksDB WAL Performance Wiki, FlushWAL Blog Post

---

### 3. **Pipelined Writes** (RocksDB Best Practice)

**What it does:**
- Separates WAL write queue from memtable write queue
- Next transaction can start WAL write before previous finishes memtable write

**Configuration:**
```rust
enable_pipelined_write: true
```

**How it works:**
1. WAL writer queue handles WAL writes
2. Memtable writer queue handles memtable writes
3. Threads on WAL queue only wait for WAL write, not memtable write
4. Parallel processing reduces latency

**Benefits:**
- ✅ Reduced latency
- ✅ Higher throughput
- ✅ Better CPU utilization

**Source:** RocksDB Options Documentation

---

### 4. **Async Insert Buffering** (ClickHouse Best Practice)

**What it does:**
- Buffers inserts in memory
- Flushes based on size/time/count triggers

**Configuration:**
```rust
max_buffer_size: 5_000_000,      // 5MB
max_buffer_time_ms: 2000,        // 2 seconds
max_buffer_ops: 200,             // 200 operations
```

**Critical Setting:**
```sql
SET async_insert = 1, wait_for_async_insert = 1
```

**Why `wait_for_async_insert=1` is REQUIRED:**
- Without it, you're in "fire-and-forget" mode
- Application won't know if inserts fail
- Can lose data silently due to disk issues, locks, etc.

**How it works:**
1. Inserts are buffered in memory
2. Flush triggers:
   - Buffer reaches 5MB (size trigger)
   - 2 seconds elapsed (time trigger)
   - 200 operations accumulated (count trigger)
3. Single batch write to disk

**Benefits:**
- ✅ **10-100x improvement** for small inserts
- ✅ Reduces number of parts created
- ✅ With `wait_for_async_insert=1`, client waits for flush (safe)

**When to use:**
- Hundreds/thousands of clients doing small inserts
- Client-side batching not feasible
- Observability workloads with continuous small payloads

**Source:** ClickHouse Async Inserts Documentation, Altinity Blog

---

## 📊 Comparative Analysis

### Buffer Tables vs Async Inserts (ClickHouse)

**Async Inserts (RECOMMENDED):**
- ✅ Properly applies sparse primary key during reading
- ✅ Better read performance
- ✅ Adaptive timeout (24.3+)

**Buffer Tables:**
- ❌ Full scan for all data in memory
- ❌ Worse read performance
- ⚠️ Being phased out

---

## 🚀 Implementation Recommendations

### Priority 1: Async Write-Behind (RocksDB)
**Expected gain:** 40% improvement
**Effort:** Low
**Risk:** Low

```rust
// Add to WAL configuration
bytes_per_sync: 1_048_576,  // 1MB
```

### Priority 2: Group Commit (PostgreSQL)
**Expected gain:** 2-5x in high concurrency
**Effort:** Medium
**Risk:** Low

```rust
// Implement in transaction commit path
if pending_commits >= 5 {
    tokio::time::sleep(Duration::from_micros(100)).await;
    flush_all_pending();
}
```

### Priority 3: Pipelined Writes (RocksDB)
**Expected gain:** Reduced latency, higher throughput
**Effort:** Medium
**Risk:** Medium

```rust
// Separate queues for WAL and memtable
wal_queue: Arc<Mutex<VecDeque<WriteOp>>>,
memtable_queue: Arc<Mutex<VecDeque<WriteOp>>>,
```

### Priority 4: Async Insert Buffering (ClickHouse)
**Expected gain:** 10-100x for small inserts
**Effort:** High
**Risk:** Medium

```rust
// Buffer configuration
AsyncWriteConfig {
    max_buffer_size: 5_000_000,
    max_buffer_time_ms: 2000,
    max_buffer_ops: 200,
}
```

---

## ⚠️ Data Safety Guarantees

All optimizations maintain ACID properties:

1. **Atomicity:** ✅ All or nothing commits
2. **Consistency:** ✅ Data constraints maintained
3. **Isolation:** ✅ Transaction isolation preserved
4. **Durability:** ✅ WAL ensures no data loss

### Critical Rules:
- ✅ **Always use `wait_for_async_insert=1`** (ClickHouse)
- ✅ **WAL must be written before acknowledging** (All)
- ✅ **fsync before commit confirmation** (All)
- ✅ **Group commit doesn't skip fsync, just batches it** (PostgreSQL)

---

## 📚 Sources

1. **ClickHouse**
   - Async Inserts Documentation (2024-2025)
   - Altinity Blog: "Using Async Inserts for Peak Data Loading Rates"
   - Best Practices: Bulk Inserts

2. **RocksDB**
   - WAL Performance Wiki
   - FlushWAL Blog Post (2017)
   - Options Documentation
   - Tuning Guide

3. **PostgreSQL**
   - Write-Ahead Log Documentation (v13-18)
   - Group Commit Implementation

---

## 🎯 Next Steps

### For Immediate 40% Improvement:
1. Implement `bytes_per_sync` in WAL (RocksDB approach)
2. Test with 1M row benchmark
3. Expect: ~73k rows/sec (up from 52k)

### For 2-3x Improvement:
1. Implement Group Commit (PostgreSQL approach)
2. Implement Pipelined Writes (RocksDB approach)
3. Combine with async write-behind
4. Expect: ~120-150k rows/sec

### For 10-100x (Small Inserts Only):
1. Implement Async Insert Buffering (ClickHouse approach)
2. **CRITICAL:** Use `wait_for_async_insert=1`
3. Only for workloads with many small inserts

---

## 🏆 Production Validation

These optimizations are proven in:
- **PostgreSQL:** Billions of transactions/day in production
- **RocksDB:** Facebook, LinkedIn, Netflix production
- **ClickHouse:** Yandex, Cloudflare, eBay production

**All maintain ACID guarantees. Zero data loss.**

---

## Code Implementation

See:
- `src/storage/lsm/async_writer.rs` - Full implementation
- Contains:
  - `AsyncWriteBuffer` (ClickHouse pattern)
  - `group_commit_flush()` (PostgreSQL pattern)
  - `PipelinedWriter` (RocksDB pattern)
  - Full configuration options
  - Comprehensive tests

**Status:** ✅ Implementation complete, ready for integration
