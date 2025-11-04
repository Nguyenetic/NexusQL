//! Disk Storage Backend
//! PROPRIETARY

use crate::storage::Table;
use crate::Result;
use sled::Db;
use std::io;

/// Persistent disk storage using sled
pub struct DiskStorage {
    db: Option<Db>, // Optional for in-memory mode
    is_memory: bool,
}

impl DiskStorage {
    pub fn new(path: &str) -> Result<Self> {
        // Check if this is in-memory mode
        let is_memory = path.starts_with(":memory:");

        let db = if is_memory {
            // For in-memory mode, don't create actual sled database
            None
        } else {
            Some(
                sled::open(path)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?,
            )
        };

        Ok(Self { db, is_memory })
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
