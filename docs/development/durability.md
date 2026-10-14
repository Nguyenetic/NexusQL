# Disk durability

## Audit of base 57068ef (before the WAL change)

The public SQL path is `NexusSQL` -> `NexusKernel` -> `DefaultEngine` ->
`Executor`, with `DiskStorage` persisting tables in sled under `<path>/default`.
It does **not** use `LSMStorage` or `TransactionManager`. The latter has a
separate MVCC implementation and JSON operation log, but no integration with
SQL table recovery. `src/storage/wal.rs` contains only a comment at this base.

| Operation | Memory | Disk at the base |
| --- | --- | --- |
| INSERT | Executor appends rows; literal INSERT can bypass it through direct bulk insert. | DefaultEngine calls `save_table` immediately, even inside BEGIN. |
| UPDATE / DELETE | Executor modifies rows and rebuilds GiST entries. | DefaultEngine saves the resulting table immediately, even inside BEGIN. |
| BEGIN | Executor clones every table, including indexes, as a rollback snapshot. | No transaction record or disk boundary. |
| COMMIT | Executor discards that snapshot. | No save, log append, flush, or fsync. |
| ROLLBACK | Executor restores the snapshot. | Previously saved changes remain in sled; reopening can resurrect rolled-back data. |
| Process kill | In-memory tables and rollback snapshot disappear. | Any DML sled happened to flush can survive, including uncommitted DML. Recently acknowledged commits can disappear. |

`DiskStorage::save_table` builds one atomic sled batch per table, replacing its
metadata, compressed row chunks, and geometry/index schema. This protects that
individual batch against mixed old/new chunks; it provides no SQL transaction
atomicity across tables. It does not call sled `flush`. Sled's periodic flushing
and clean drop determine what reaches durable storage. `DiskStorage::sync` and
`close` explicitly flush sled, but SQL COMMIT does not call them.

Bulk mode defers saves into a dirty-table set. `flush` saves each table separately
and flushes sled, including any active transaction's changes. Disabling bulk mode
only changes a flag. Shutdown calls flush and discards its error before invoking
engine shutdown; the default engine has no shutdown override at the base.
DROP TABLE changes memory without removing persisted keys.

The independent `transaction::WALManager::flush` flushes a BufWriter into the OS,
without `sync_all`/`sync_data`. Its length-prefixed JSON records have no checksum,
salt, or torn-record recovery. It is not the SQL durability mechanism.

Evidence: `src/engine/default.rs` (`execute`, `bulk_insert`, `flush`),
`src/engine/executor.rs` (`Statement::Transaction`, DML), `src/storage/disk.rs`
(`save_table`, `sync`), `src/lib.rs` (`execute_with_params` fast INSERT),
`src/kernel.rs` (`shutdown`), `src/transaction/wal.rs` (`flush`, `read_all`).

## Implementation reference

The new WAL follows the frame/checksum/salt/commit/checkpoint design documented
in SQLite's public-domain `src/wal.c`, especially its file-format and checkpoint
ordering comments. See [SQLite WAL](https://www.sqlite.org/wal.html) and
[WAL format](https://www.sqlite.org/walformat.html). The supplied
`sqlite-duckdb-findings.md` sections 5 and 11 identified the integration gap;
`mapping-findings.md` establishes the need to recover geometry and GiST together.
No PostGIS or GEOS source is copied.

## Guarantees of the default SQL disk engine

In the default FULL mode, a successful SQL COMMIT has written all dirty tables
to `default/nexusql.wal`, appended a commit marker, and completed `File::sync_all`
(fsync/FlushFileBuffers). A successful ordinary autocommit DDL/DML statement has
the same guarantee. This includes literal fast INSERT, parameterized INSERT,
direct bulk insert, UPDATE, DELETE, CREATE TABLE/INDEX, and DROP TABLE. Changes
to multiple tables in one explicit transaction commit together.

Until COMMIT, changes and the rollback snapshot live in the executor's memory.
Neither sled's main files nor the committed WAL overlay receives them.
ROLLBACK discards them. Statement errors restore the affected table; direct bulk
insert validates all geometry rows before inserting any. COMMIT failure retains
the active transaction and its rollback snapshot. An I/O failure during WAL write
or sync can leave an **uncertain commit**: reopening determines whether the full
commit record survived. The failed writer rejects further writes until reopen.

Recovery accepts only the ordered, checksummed prefix ending in a commit marker.
A torn payload, header, checksum, or missing commit marker discards the final
incomplete transaction, even when it spans multiple frames/tables. The last
committed prefix is replayed into a committed key overlay before tables load.
No recovery of arbitrary corruption of earlier acknowledged commits is promised;
an invalid complete WAL file header is an error, not silently erased data.

Geometry values, column types, GiST definitions, and auto-increment metadata use
the same redo transaction as row chunks. GiST trees rebuild from recovered rows,
so row-offset changes cannot leave persisted stale index entries. Other index
types retain their existing persistence limitations; this change does not add
persistence for B-Tree/HNSW/GIN/BRIN definitions. Existing pre-WAL sled table keys
remain readable without migration.

## Configuration and bulk loading

Configuration is per open database and resets to FULL / 4 MiB on reopen.

```sql
PRAGMA synchronous;                      -- 2 by default
PRAGMA synchronous = FULL;               -- FULL/2, NORMAL/1, OFF/0
PRAGMA nexusql_wal_checkpoint_bytes = 4194304;
PRAGMA nexusql_wal_checkpoint_bytes = 0;  -- disable automatic checkpoint
PRAGMA wal_checkpoint;                   -- checkpoint committed data only
```

| Mode | Commit | Checkpoint | Guarantee |
| --- | --- | --- | --- |
| FULL (default) | Write frames + marker and sync WAL | Sync WAL, apply and flush sled, truncate and sync WAL, write and sync fresh header | Acknowledged commits survive process crash and power loss when the filesystem/device honors sync barriers. |
| NORMAL | Write frames + marker into the OS | Same barriers as FULL | Process-kill recovery; recent commits may be lost on power failure/hard reset. |
| OFF | Write frames + marker, no WAL sync | Sled still flushes before log reset; WAL barriers omitted | No crash/power-loss durability guarantee. |

Changing synchronous inside BEGIN is rejected. Disk `journal_mode` is always
WAL; unsupported modes and invalid sync settings are rejected. POSIX creation
also syncs the WAL's directory and its ancestors. Guarantees assume a
local filesystem supporting sled's exclusive lock and the OS sync operations.
Network filesystems, storage corruption, and hardware ignoring barriers are
outside this contract. These tests inject process crashes, not physical power
cuts or faulty hardware.

`set_bulk_mode(true)` deliberately defers **autocommits** in memory. Use
`flush()` or explicit `shutdown()` to persist that batch; turning bulk mode off
only changes the flag. BEGIN first commits any preceding deferred batch.
An explicit COMMIT always writes its transaction, even in bulk mode. Flush and
manual checkpoint inside BEGIN checkpoint already committed data only; they do
not commit the active transaction. Explicit shutdown rolls back an active
transaction, persists deferred autocommits, checkpoints, and reports errors.
Drop attempts a checkpoint of already committed data, but cannot report errors
and does not persist deferred bulk writes. In-memory databases have no durability.

## Format and checkpoint ordering

The file header is 32 bytes: magic `NQLWAL01` (8), version (4), maximum frame
payload size (4), random generation salt (8), and checksum pair (8).
Each frame has a 40-byte header: frame sequence (8), salt (8), payload length (4),
kind (4: data=0, commit=1), transaction sequence (8), checksum pair (8).
All integers are little-endian. Data payloads contain at most 64 KiB and are
zero-padded to 8 bytes. A transaction's concatenated payload is a bincode map
from sled key to optional value (after-image or tombstone). Row chunks remain
zstd-compressed using the existing 16 KiB target chunking policy.

Two rolling 32-bit Fibonacci-weighted sums cover the header and then each frame's
identity, flags, length, and padded data. The zero-payload commit frame is itself
checksummed. Salts and consecutive frame/transaction numbers reject stale or
reordered frames. Lengths are checked before allocating frame payloads. Recovery
truncates the invalid/uncommitted suffix before new appends.

The checkpoint holds the disk-state mutex, syncs the WAL, copies the committed
overlay in one atomic sled batch, flushes sled, and only then truncates the log.
Truncation is synced before a fresh salt/header is written and synced. A crash
before reset replays idempotent after-images; a crash during reset uses the
already synced main data. A short header is therefore safe to reinitialize.
Automatic checkpoint runs on the commit that crosses the byte threshold.
Maintenance failure after a successful WAL commit is logged; it does not report
a false rollback. Explicit flush/close can report checkpoint errors.

The executor lock serializes SQL reads, writes, and checkpoint calls. The disk
mutex blocks direct table readers during checkpoint; returned Tables/ResultSets
own their data and remain valid. Sled's exclusive database lock excludes another
process opening the same database. This is one shared SQL session per engine,
**not SQL MVCC or independent concurrent transaction snapshots**. Callers sharing
one NexusSQL instance must serialize their BEGIN-to-COMMIT sequences. The separate
`TransactionManager`/MVCC API is not wired to SQL; its legacy journal now syncs
when flushed, but is not this recovery mechanism. HDC/ColBERT and the standalone
LSM APIs are outside the disk SQL guarantees.

This implementation logs complete replacements of dirty tables' keys at commit,
retaining the existing storage representation. It is not an incremental row/page
engine: commit costs scale with dirty-table size, and SQL snapshots/overlay/replay
consume memory. Streaming recovery and incremental table updates are future work.

## Validation

`tests/durability_test.rs` kills child processes after acknowledged commits with
subsequent uncommitted INSERT/UPDATE/DELETE/DDL, verifies the main sled image before
recovery, and compares recovered GiST probes against the recovered table. It
checks every byte truncation of the final multi-table transaction, corrupts every
byte of a final WAL transaction, tests stale salts, multi-frame payloads, failed
geometry statements, sync configuration, automatic checkpoint, bulk mode, and
clean shutdown with an active transaction.

`storage::disk::durability_tests` terminates child processes at five checkpoint
barriers (WAL synced, batch applied, main data synced, WAL truncated, new header
written). It also verifies the main file stays unchanged until checkpoint and
coordinates reader/checkpointer threads to test blocking and detached snapshots.
`storage::wal::tests` injects an I/O failure and verifies writer poisoning.

Local verification on Windows x86-64, debug build, default features:

- Changed Rust files: `cargo fmt --check` passed using a temporary manifest
  listing only those files, with `skip_children=true` (the repository has
  unrelated example-formatting debt).
- `cargo build -j 2` and `cargo test --all-targets -j 2 -- --nocapture` passed:
  **550 tests**, including 13 added durability tests, versus 537 at the base.
- Exhaustive final-transaction truncation: **705 byte boundaries**, **2,376-byte
  WAL**, **11.705 seconds**. Five deterministic checkpoint interruption points
  and all 33 possible short/full header lengths also passed.
- `cargo clippy -j 2` completed with **147 warnings**, report-only. The new
  checksum loop receives the `chunks_exact_to_as_chunks` suggestion; no lint
  cleanup outside this change was attempted.
- Python: `maturin develop` succeeded; `pytest -q` in `bindings/python` passed
  **22 tests in 1.93 seconds**, including DuckDB spatial parity.

Integration footprint: `transaction::durable` owns SQL transaction persistence.
The necessary `DefaultEngine` hooks delegate execute, direct bulk insert, flush,
and shutdown to it; the old premature table saves are removed. A single additive
dialect hook in `query/parser.rs` accepts bare synchronous/journal-mode values
that sqlparser previously rejected. The executor and `storage/mod.rs` are
unchanged. NOTICE attributes the SQLite WAL design.
