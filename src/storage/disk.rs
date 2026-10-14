//! Disk Storage Backend
//! PROPRIETARY

use crate::storage::wal::{SyncPolicy, WriteAheadLog};
use crate::storage::{Table, TableMeta, TableMetaSer};
use crate::types::{Row, Value};
use crate::Result;
use sled::Db;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io;
use std::path::Path;
use std::sync::Mutex;

pub const WAL_FILE_NAME: &str = "nexusql.wal";
pub const DEFAULT_CHECKPOINT_BYTES: u64 = 4 * 1024 * 1024;

/// Redo after-images of the existing sled keys. None is a deletion.
type Changes = BTreeMap<Vec<u8>, Option<Vec<u8>>>;

struct DiskState {
    wal: Option<WriteAheadLog>,
    committed: Changes,
    checkpoint_bytes: u64,
}

/// Target *uncompressed* serialized size per row-chunk. After zstd
/// compression (level 3) this typically shrinks to ~30-50% of the
/// uncompressed size for Excel/text-heavy workloads, keeping the
/// on-disk chunk well under sled's ~4 KB inlined-value threshold so
/// nothing spills to the blob heap.
const CHUNK_SIZE_TARGET: usize = 16 * 1024;

/// zstd compression level for chunks. Level 3 is the crate default —
/// fast enough that flush doesn't regress perceptibly and a good
/// ratio on redundant tabular data.
const ZSTD_LEVEL: i32 = 3;

/// Persistent disk storage using sled
pub struct DiskStorage {
    db: Option<Db>, // Optional for in-memory mode
    // Covers both the committed overlay and sled reads/checkpoints. Readers
    // never observe a partially copied checkpoint or an uncommitted table.
    state: Mutex<DiskState>,
}

impl DiskStorage {
    /// Create or open a NexusQL database at the given path.
    ///
    /// For disk mode, this will:
    /// - Create a new sled database if the path doesn't exist
    /// - Open an existing sled database if the path is a directory
    /// - Return an error if the path is a file (not a NexusQL database)
    pub fn new(path: &str) -> Result<Self> {
        // Check if this is in-memory mode
        let is_memory = path.starts_with(":memory:");

        let db = if is_memory {
            // For in-memory mode, don't create actual sled database
            None
        } else {
            // Check if path exists and what type it is
            let path_obj = Path::new(path);

            if path_obj.exists() {
                if path_obj.is_file() {
                    // Path exists as a file - this is not a NexusQL database
                    // Could be an SQLite file or other format
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "Cannot open '{}': path exists as a file, not a NexusQL database directory. \
                            NexusQL uses a directory-based storage format (sled). \
                            If you're trying to open an SQLite database, note that NexusQL is not SQLite-compatible. \
                            To create a new NexusQL database, use a different path or remove the existing file.",
                            path
                        )
                    ).into());
                }
                // Path is a directory - try to open existing database
                tracing::debug!("Opening existing NexusQL database at: {}", path);
            } else {
                // Path doesn't exist - will create new database
                tracing::debug!("Creating new NexusQL database at: {}", path);
            }

            Some(
                sled::open(path)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?,
            )
        };

        let mut committed = Changes::new();
        // Open sled first: its exclusive database lock also protects our WAL.
        let wal = if is_memory {
            None
        } else {
            let (wal, transactions) =
                WriteAheadLog::open(&Path::new(path).join(WAL_FILE_NAME), SyncPolicy::Full)?;
            for payload in transactions {
                let changes: Changes = bincode::deserialize(&payload)?;
                committed.extend(changes);
            }
            Some(wal)
        };
        Ok(Self {
            db,
            state: Mutex::new(DiskState {
                wal,
                committed,
                checkpoint_bytes: DEFAULT_CHECKPOINT_BYTES,
            }),
        })
    }

    /// Check if a path contains a valid NexusQL database
    pub fn is_nexusql_database(path: &str) -> bool {
        let path_obj = Path::new(path);
        if !path_obj.exists() || !path_obj.is_dir() {
            return false;
        }

        // Sled databases have a "conf" file and "db" file in the directory
        path_obj.join("conf").exists() || path_obj.join("db").exists()
    }

    pub fn is_memory(&self) -> bool {
        self.db.is_none()
    }

    /// Explicit close reports checkpoint errors; Drop can only log them.
    pub fn close(&self) -> Result<()> {
        self.checkpoint()
    }

    /// Persist a table across N+1 keys:
    ///   - `table:<name>:meta`                  — columns, chunk_count, auto-inc
    ///   - `table:<name>:chunk:<NNNNNNNNNN>`    — `Vec<Vec<Value>>` of rows
    ///
    /// This is an autocommit operation. SQL transactions use commit_tables
    /// once for ALL their dirty tables, after successful execution of DML.
    pub fn save_table(&self, table: &Table) -> Result<()> {
        self.commit_tables(&[table], &[])
    }

    /// Commit all table replacements/deletions as one durable WAL transaction.
    /// Main sled keys are modified only by checkpoint, after the commit marker.
    pub fn commit_tables(&self, tables: &[&Table], removed: &[String]) -> Result<()> {
        let Some(db) = self.db.as_ref() else {
            return Ok(());
        };
        let mut state = self.state.lock().unwrap();
        state.wal.as_ref().unwrap().ensure_healthy()?;
        let mut changes = Changes::new();
        for name in removed {
            changes.insert(format!("table:{}:meta", name).into_bytes(), None);
            changes.insert(format!("table:{}:schema", name).into_bytes(), None);
            for key in Self::keys(db, &state, format!("table:{}:chunk:", name).as_bytes())? {
                changes.insert(key, None);
            }
        }
        for table in tables {
            for key in Self::keys(
                db,
                &state,
                format!("table:{}:chunk:", table.name).as_bytes(),
            )? {
                changes.insert(key, None);
            }
            Self::encode_table(table, &mut changes)?;
        }
        if changes.is_empty() {
            return Ok(());
        }
        state
            .wal
            .as_mut()
            .unwrap()
            .append(&bincode::serialize(&changes)?)?;
        state.committed.extend(changes);
        let wal_len = state.wal.as_ref().unwrap().len();
        if state.checkpoint_bytes > 0 && wal_len >= state.checkpoint_bytes {
            // COMMIT has succeeded. A maintenance failure must not turn that
            // into a reported rollback; recovery can use the WAL or synced sled.
            if let Err(error) = Self::checkpoint_locked(db, &mut state, &mut |_| {}) {
                tracing::warn!(%error, "Automatic checkpoint failed; commit remains recoverable");
            }
        }
        Ok(())
    }

    fn encode_table(table: &Table, changes: &mut Changes) -> Result<()> {
        let meta_key = format!("table:{}:meta", table.name);
        let chunk_prefix = format!("table:{}:chunk:", table.name);

        // Group row-values into chunks that stay under CHUNK_SIZE_TARGET.
        // A row larger than the target (e.g. an image BLOB) ends up alone
        // in its own chunk — still a smaller sled value than the old
        // one-blob-per-table format, and sled GCs individual chunks as
        // they're replaced on re-save.
        let mut chunks: Vec<Vec<&Vec<Value>>> = Vec::new();
        let mut current: Vec<&Vec<Value>> = Vec::new();
        let mut current_size: usize = 0;

        for row in &table.rows {
            let row_est = estimate_row_size(&row.values);
            if !current.is_empty() && current_size + row_est > CHUNK_SIZE_TARGET {
                chunks.push(std::mem::take(&mut current));
                current_size = 0;
            }
            current.push(&row.values);
            current_size += row_est;
        }
        if !current.is_empty() {
            chunks.push(current);
        }

        let chunk_count = chunks.len() as u32;

        for (idx, chunk) in chunks.iter().enumerate() {
            let key = format!("{}{:010}", chunk_prefix, idx);
            let raw = bincode::serialize(chunk)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
            let compressed = zstd::encode_all(&raw[..], ZSTD_LEVEL)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
            changes.insert(key.into_bytes(), Some(compressed));
        }

        let meta = TableMetaSer {
            name: &table.name,
            columns: &table.columns,
            chunk_count,
            auto_increment_column: &table.auto_increment_column,
            next_auto_increment_id: table.next_auto_increment_id,
        };
        let meta_bytes = bincode::serialize(&meta)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        changes.insert(meta_key.into_bytes(), Some(meta_bytes));
        let schema_bytes = bincode::serialize(&table.schema)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        changes.insert(
            format!("table:{}:schema", table.name).into_bytes(),
            Some(schema_bytes),
        );
        Ok(())
    }

    /// Flush committed state only. Active SQL transaction changes stay private.
    pub fn sync(&self) -> Result<()> {
        self.checkpoint()
    }

    pub fn checkpoint(&self) -> Result<()> {
        if let Some(db) = &self.db {
            Self::checkpoint_locked(db, &mut self.state.lock().unwrap(), &mut |_| {})?;
        }
        Ok(())
    }

    fn checkpoint_locked(
        db: &Db,
        state: &mut DiskState,
        observe: &mut impl FnMut(&'static str),
    ) -> Result<()> {
        let wal = state.wal.as_mut().unwrap();
        wal.sync()?;
        if wal.is_empty() {
            return Ok(());
        }
        observe("wal_synced");
        let mut batch = sled::Batch::default();
        for (key, value) in &state.committed {
            if let Some(value) = value {
                batch.insert(key.as_slice(), value.as_slice());
            } else {
                batch.remove(key.as_slice());
            }
        }
        db.apply_batch(batch)?;
        observe("data_applied");
        // Even OFF lets sled enforce its own main-file consistency. NORMAL
        // and FULL additionally synced the redo log before touching sled.
        db.flush()?;
        observe("data_synced");
        wal.reset_with(observe)?;
        state.committed.clear();
        Ok(())
    }

    pub fn sync_policy(&self) -> SyncPolicy {
        self.state
            .lock()
            .unwrap()
            .wal
            .as_ref()
            .map_or(SyncPolicy::Off, WriteAheadLog::policy)
    }

    pub fn set_sync_policy(&self, policy: SyncPolicy) -> Result<()> {
        if let Some(wal) = self.state.lock().unwrap().wal.as_mut() {
            wal.set_policy(policy)?;
        }
        Ok(())
    }

    /// Zero disables automatic checkpoints. Explicit flush/close still checkpoint.
    pub fn set_checkpoint_bytes(&self, bytes: u64) {
        self.state.lock().unwrap().checkpoint_bytes = bytes;
    }

    pub fn checkpoint_bytes(&self) -> u64 {
        self.state.lock().unwrap().checkpoint_bytes
    }

    pub fn wal_size(&self) -> u64 {
        self.state
            .lock()
            .unwrap()
            .wal
            .as_ref()
            .map_or(0, WriteAheadLog::len)
    }

    #[cfg(test)]
    pub(crate) fn inject_wal_write_failure(&self, path: &Path) {
        self.state
            .lock()
            .unwrap()
            .wal
            .as_mut()
            .unwrap()
            .inject_write_failure(path);
    }

    fn get(db: &Db, state: &DiskState, key: &[u8]) -> Result<Option<Vec<u8>>> {
        if let Some(value) = state.committed.get(key) {
            return Ok(value.clone());
        }
        Ok(db.get(key)?.map(|bytes| bytes.to_vec()))
    }

    fn keys(db: &Db, state: &DiskState, prefix: &[u8]) -> Result<BTreeSet<Vec<u8>>> {
        let mut keys = BTreeSet::new();
        for entry in db.scan_prefix(prefix) {
            keys.insert(entry?.0.to_vec());
        }
        for (key, value) in state.committed.range(prefix.to_vec()..) {
            if !key.starts_with(prefix) {
                break;
            }
            if value.is_some() {
                keys.insert(key.clone());
            } else {
                keys.remove(key);
            }
        }
        Ok(keys)
    }

    pub fn load_table(&self, name: &str) -> Result<Option<Table>> {
        let Some(db) = self.db.as_ref() else {
            return Ok(None); // in-memory mode
        };

        let state = self.state.lock().unwrap();
        let meta_key = format!("table:{}:meta", name);
        let Some(meta_bytes) = Self::get(db, &state, meta_key.as_bytes())? else {
            return Ok(None);
        };
        let meta: TableMeta = bincode::deserialize(&meta_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        let mut all_values: Vec<Vec<Value>> = Vec::new();
        for idx in 0..meta.chunk_count {
            let chunk_key = format!("table:{}:chunk:{:010}", name, idx);
            let chunk_bytes = Self::get(db, &state, chunk_key.as_bytes())?.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "table '{}' meta claims {} chunks but chunk {} is missing",
                        name, meta.chunk_count, idx
                    ),
                )
            })?;
            let raw = zstd::decode_all(&chunk_bytes[..])
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
            let mut chunk: Vec<Vec<Value>> = bincode::deserialize(&raw)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
            all_values.append(&mut chunk);
        }

        let cols = meta.columns;
        let rows: Vec<Row> = all_values
            .into_iter()
            .map(|values| Row {
                columns: cols.clone(),
                values,
            })
            .collect();

        let mut table = Table {
            name: meta.name,
            schema: Self::get(db, &state, format!("table:{}:schema", name).as_bytes())?
                .map(|bytes| bincode::deserialize(&bytes))
                .transpose()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?
                .unwrap_or_default(),
            columns: cols,
            rows,
            indexes: HashMap::new(),
            typed_indexes: HashMap::new(),
            auto_increment_column: meta.auto_increment_column,
            next_auto_increment_id: meta.next_auto_increment_id.max(1),
        };
        table.rebuild_gist_indexes();
        Ok(Some(table))
    }

    pub fn list_tables(&self) -> Result<Vec<String>> {
        let mut tables = Vec::new();
        if let Some(ref db) = self.db {
            let state = self.state.lock().unwrap();
            // Each table emits one `:meta` key plus N `:chunk:` keys;
            // scan meta keys only so we return each table name once.
            for key in Self::keys(db, &state, b"table:")? {
                let key_str = String::from_utf8_lossy(&key);
                if let Some(rest) = key_str.strip_prefix("table:") {
                    if let Some(name) = rest.strip_suffix(":meta") {
                        tables.push(name.to_string());
                    }
                }
            }
        }
        Ok(tables)
    }
}

impl Drop for DiskStorage {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            tracing::warn!(%error, "Close checkpoint failed; reopen to recover the WAL");
        }
    }
}

/// Rough upper-bound on bincode-serialized size of a row, used for chunk
/// packing. Doesn't need to be exact — small over- or under-estimates just
/// affect how close we get to the 32 KB target. A few bytes of overhead
/// per value covers bincode's discriminant + length prefixes.
fn estimate_row_size(values: &[Value]) -> usize {
    const PER_VALUE_OVERHEAD: usize = 8;
    const ROW_OVERHEAD: usize = 8;
    values
        .iter()
        .map(|v| {
            PER_VALUE_OVERHEAD
                + match v {
                    Value::Null => 0,
                    Value::Integer(_) => 8,
                    Value::Float(_) => 8,
                    Value::Boolean(_) => 1,
                    Value::Text(s) => s.len(),
                    Value::Blob(b) => b.len(),
                    Value::Vector(v) => v.len() * 4,
                    Value::Geometry(g) => g.estimated_size(),
                }
        })
        .sum::<usize>()
        + ROW_OVERHEAD
}

#[cfg(test)]
mod durability_tests {
    use super::*;
    use std::process::Command;
    use std::sync::{mpsc, Arc};
    use std::time::Duration;
    use tempfile::TempDir;

    fn table(name: &str, value: i64) -> Table {
        let mut table = Table::new(name.into(), vec!["id".into()]);
        table.insert(Row::new(vec!["id".into()], vec![Value::Integer(value)]));
        table
    }

    #[test]
    fn main_files_change_only_at_checkpoint() {
        let root = TempDir::new().unwrap();
        let disk = DiskStorage::new(root.path().to_str().unwrap()).unwrap();
        disk.save_table(&table("items", 1)).unwrap();
        assert!(disk
            .db
            .as_ref()
            .unwrap()
            .get(b"table:items:meta")
            .unwrap()
            .is_none());
        assert_eq!(
            disk.load_table("items").unwrap().unwrap().rows[0].values,
            vec![Value::Integer(1)]
        );
        disk.checkpoint().unwrap();
        disk.save_table(&table("items", 2)).unwrap();
        let bytes = disk
            .db
            .as_ref()
            .unwrap()
            .get(b"table:items:chunk:0000000000")
            .unwrap()
            .unwrap();
        let raw = zstd::decode_all(&bytes[..]).unwrap();
        let main_rows: Vec<Vec<Value>> = bincode::deserialize(&raw).unwrap();
        assert_eq!(main_rows, vec![vec![Value::Integer(1)]]);
        assert_eq!(
            disk.load_table("items").unwrap().unwrap().rows[0].values,
            vec![Value::Integer(2)]
        );
    }

    #[test]
    fn checkpoint_recovers_at_every_persistence_barrier() {
        if let Ok(phase) = std::env::var("NEXUSQL_CHECKPOINT_CRASH") {
            let path = std::env::var("NEXUSQL_CHECKPOINT_PATH").unwrap();
            let disk = DiskStorage::new(&path).unwrap();
            disk.commit_tables(&[&table("a", 1), &table("b", 1)], &[])
                .unwrap();
            disk.checkpoint().unwrap();
            disk.commit_tables(&[&table("a", 2), &table("b", 2)], &[])
                .unwrap();
            DiskStorage::checkpoint_locked(
                disk.db.as_ref().unwrap(),
                &mut disk.state.lock().unwrap(),
                &mut |at| {
                    if at == phase {
                        std::process::exit(71);
                    }
                },
            )
            .unwrap();
            panic!("checkpoint phase {phase} was not reached");
        }
        for phase in [
            "wal_synced",
            "data_applied",
            "data_synced",
            "wal_truncated",
            "header_written",
        ] {
            let root = TempDir::new().unwrap();
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "storage::disk::durability_tests::checkpoint_recovers_at_every_persistence_barrier", "--nocapture"])
                .env("NEXUSQL_CHECKPOINT_CRASH", phase)
                .env("NEXUSQL_CHECKPOINT_PATH", root.path())
                .output().unwrap();
            assert_eq!(
                output.status.code(),
                Some(71),
                "{phase}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let disk = DiskStorage::new(root.path().to_str().unwrap()).unwrap();
            for name in ["a", "b"] {
                assert_eq!(
                    disk.load_table(name).unwrap().unwrap().rows[0].values,
                    vec![Value::Integer(2)],
                    "{phase}: {name}"
                );
            }
            disk.checkpoint().unwrap();
            assert_eq!(disk.wal_size(), crate::storage::wal::WAL_HEADER_LEN as u64);
        }
    }

    #[test]
    fn readers_wait_for_checkpoint_and_detached_snapshots_stay_valid() {
        let root = TempDir::new().unwrap();
        let disk = Arc::new(DiskStorage::new(root.path().to_str().unwrap()).unwrap());
        disk.save_table(&table("items", 1)).unwrap();
        let snapshot = disk.load_table("items").unwrap().unwrap();
        disk.save_table(&table("items", 2)).unwrap();
        let (paused_tx, paused_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let writer_disk = disk.clone();
        let writer = std::thread::spawn(move || {
            DiskStorage::checkpoint_locked(
                writer_disk.db.as_ref().unwrap(),
                &mut writer_disk.state.lock().unwrap(),
                &mut |phase| {
                    if phase == "data_applied" {
                        paused_tx.send(()).unwrap();
                        resume_rx.recv().unwrap();
                    }
                },
            )
            .unwrap();
        });
        paused_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let (read_tx, read_rx) = mpsc::channel();
        let reader_disk = disk.clone();
        let reader = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            read_tx
                .send(reader_disk.load_table("items").unwrap().unwrap())
                .unwrap();
        });
        started_rx.recv().unwrap();
        assert!(read_rx.recv_timeout(Duration::from_millis(50)).is_err());
        resume_tx.send(()).unwrap();
        let current = read_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        writer.join().unwrap();
        reader.join().unwrap();
        assert_eq!(snapshot.rows[0].values, vec![Value::Integer(1)]);
        assert_eq!(current.rows[0].values, vec![Value::Integer(2)]);
    }
}
