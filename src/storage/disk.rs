//! Disk Storage Backend
//! PROPRIETARY

use crate::storage::{Table, TableMeta, TableMetaSer};
use crate::types::{Row, Value};
use crate::Result;
use sled::Db;
use std::collections::HashMap;
use std::io;
use std::path::Path;

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
#[allow(dead_code)]
pub struct DiskStorage {
    db: Option<Db>, // Optional for in-memory mode
    is_memory: bool,
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

        Ok(Self { db, is_memory })
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

    /// Explicitly close the database and flush all data
    pub fn close(&self) -> Result<()> {
        if let Some(ref db) = self.db {
            db.flush()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        }
        Ok(())
    }

    /// Persist a table across N+1 keys:
    ///   - `table:<name>:meta`                  — columns, chunk_count, auto-inc
    ///   - `table:<name>:chunk:<NNNNNNNNNN>`    — `Vec<Vec<Value>>` of rows
    ///
    /// All writes (plus removal of any pre-existing chunks for this table)
    /// happen in a single `sled::Batch`, so a crash mid-save leaves either
    /// the old table or the new one — never a mix.
    pub fn save_table(&self, table: &Table) -> Result<()> {
        let Some(db) = self.db.as_ref() else {
            return Ok(()); // in-memory mode: no-op
        };

        let meta_key = format!("table:{}:meta", table.name);
        let chunk_prefix = format!("table:{}:chunk:", table.name);

        let mut batch = sled::Batch::default();

        // Remove existing meta + all existing chunks for this table.
        batch.remove(meta_key.as_bytes());
        for item in db.scan_prefix(chunk_prefix.as_bytes()) {
            let (key, _) = item.map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
            batch.remove(&*key);
        }

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
            batch.insert(key.as_bytes(), compressed);
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
        batch.insert(meta_key.as_bytes(), meta_bytes);
        let schema_bytes = bincode::serialize(&table.schema)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        batch.insert(
            format!("table:{}:schema", table.name).as_bytes(),
            schema_bytes,
        );

        db.apply_batch(batch)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        // NOTE: sled auto-flushes on drop and buffers writes efficiently.
        // Callers that need strict durability should invoke `sync()`.
        Ok(())
    }

    /// Force all pending sled writes to disk. Call once at end of a bulk
    /// load instead of fsyncing after every table.
    pub fn sync(&self) -> Result<()> {
        if let Some(ref db) = self.db {
            db.flush()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        }
        Ok(())
    }

    pub fn load_table(&self, name: &str) -> Result<Option<Table>> {
        let Some(db) = self.db.as_ref() else {
            return Ok(None); // in-memory mode
        };

        let meta_key = format!("table:{}:meta", name);
        let Some(meta_bytes) = db
            .get(meta_key.as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?
        else {
            return Ok(None);
        };
        let meta: TableMeta = bincode::deserialize(&meta_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        let mut all_values: Vec<Vec<Value>> = Vec::new();
        for idx in 0..meta.chunk_count {
            let chunk_key = format!("table:{}:chunk:{:010}", name, idx);
            let chunk_bytes = db
                .get(chunk_key.as_bytes())
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?
                .ok_or_else(|| {
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

        Ok(Some(Table {
            name: meta.name,
            schema: db
                .get(format!("table:{}:schema", name))
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?
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
        }))
    }

    pub fn list_tables(&self) -> Result<Vec<String>> {
        let mut tables = Vec::new();
        if let Some(ref db) = self.db {
            // Each table emits one `:meta` key plus N `:chunk:` keys;
            // scan meta keys only so we return each table name once.
            for item in db.scan_prefix(b"table:") {
                let (key, _) =
                    item.map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
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
