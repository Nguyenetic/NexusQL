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
