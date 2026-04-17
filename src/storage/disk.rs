//! Disk Storage Backend
//! PROPRIETARY

use crate::storage::Table;
use crate::Result;
use sled::Db;
use std::io;
use std::path::Path;

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

    pub fn save_table(&self, table: &Table) -> Result<()> {
        // Skip for in-memory mode
        if let Some(ref db) = self.db {
            let key = format!("table:{}", table.name);
            let value = bincode::serialize(table)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
            db.insert(key.as_bytes(), value)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
            // NOTE: sled auto-flushes on drop and buffers writes efficiently.
            // The explicit per-save flush was forcing an fsync on every call,
            // which dominated bulk-load flush time. Callers that need strict
            // durability should invoke `sync()` explicitly.
        }
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
        // For in-memory mode, return None (tables exist only in executor)
        if let Some(ref db) = self.db {
            let key = format!("table:{}", name);
            if let Some(bytes) = db
                .get(key.as_bytes())
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?
            {
                let table: Table = bincode::deserialize(&bytes)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
                return Ok(Some(table));
            }
        }
        Ok(None)
    }

    pub fn list_tables(&self) -> Result<Vec<String>> {
        let mut tables = Vec::new();
        // For in-memory mode, return empty list (tables exist only in executor)
        if let Some(ref db) = self.db {
            for item in db.scan_prefix(b"table:") {
                let (key, _) =
                    item.map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
                let key_str = String::from_utf8_lossy(&key);
                if let Some(name) = key_str.strip_prefix("table:") {
                    tables.push(name.to_string());
                }
            }
        }
        Ok(tables)
    }
}
