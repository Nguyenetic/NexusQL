//! Disk Storage Backend
//! PROPRIETARY

use sled::Db;
use crate::storage::Table;
use crate::Result;
use std::io;

/// Persistent disk storage using sled
pub struct DiskStorage {
    db: Db,
}

impl DiskStorage {
    pub fn new(path: &str) -> Result<Self> {
        let db = sled::open(path).map_err(|e| {
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;
        Ok(Self { db })
    }
    
    pub fn save_table(&self, table: &Table) -> Result<()> {
        let key = format!("table:{}", table.name);
        let value = bincode::serialize(table).map_err(|e| {
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;
        self.db.insert(key.as_bytes(), value).map_err(|e| {
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;
        self.db.flush().map_err(|e| {
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })?;
        Ok(())
    }
    
    pub fn load_table(&self, name: &str) -> Result<Option<Table>> {
        let key = format!("table:{}", name);
        if let Some(bytes) = self.db.get(key.as_bytes()).map_err(|e| {
            io::Error::new(io::ErrorKind::Other, e.to_string())
        })? {
            let table: Table = bincode::deserialize(&bytes).map_err(|e| {
                io::Error::new(io::ErrorKind::Other, e.to_string())
            })?;
            Ok(Some(table))
        } else {
            Ok(None)
        }
    }
    
    pub fn list_tables(&self) -> Result<Vec<String>> {
        let mut tables = Vec::new();
        for item in self.db.scan_prefix(b"table:") {
            let (key, _) = item.map_err(|e| {
                io::Error::new(io::ErrorKind::Other, e.to_string())
            })?;
            let key_str = String::from_utf8_lossy(&key);
            if let Some(name) = key_str.strip_prefix("table:") {
                tables.push(name.to_string());
            }
        }
        Ok(tables)
    }
}
