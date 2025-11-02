//! LSM Tree Storage Engine - RocksDB-inspired
//! PROPRIETARY

pub mod async_writer;
pub mod memtable;
pub mod optimized_wal;
pub mod sst;
pub mod wal;

use crate::types::{Row, Value};
use crate::Result;
use std::collections::BTreeMap;

/// LSM Tree Storage
///
/// Architecture:
/// 1. MemTable (in-memory, sorted)
/// 2. WAL (Write-Ahead Log for durability)
/// 3. SST Files (Sorted String Tables on disk)
/// 4. Compaction (background merging)
pub struct LSMStorage {
    memtable: memtable::MemTable,
    wal: wal::WriteAheadLog,
    sequence: u64, // Monotonically increasing
}

impl LSMStorage {
    pub fn new(path: &str) -> Result<Self> {
        Ok(Self {
            memtable: memtable::MemTable::new(),
            wal: wal::WriteAheadLog::new(&format!("{}/wal", path))?,
            sequence: 0,
        })
    }

    /// Write key-value (goes to memtable + WAL)
    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        self.sequence += 1;

        // 1. Write to WAL first (durability)
        self.wal.append(self.sequence, &key, &value)?;

        // 2. Write to memtable (fast reads)
        self.memtable.put(key, value, self.sequence);

        // 3. Check if memtable is full → flush to SST
        if self.memtable.size() > 4 * 1024 * 1024 {
            // 4MB threshold
            tracing::info!("MemTable full, flushing to SST...");
            // TODO: Flush to SST file
        }

        Ok(())
    }

    /// Read key-value
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        // 1. Check memtable first (most recent)
        if let Some(val) = self.memtable.get(key) {
            return Ok(Some(val));
        }

        // 2. Check SST files (older data)
        // TODO: Search SST files

        Ok(None)
    }

    /// Delete key
    pub fn delete(&mut self, key: Vec<u8>) -> Result<()> {
        self.sequence += 1;

        // Write tombstone marker
        self.wal.append_delete(self.sequence, &key)?;
        self.memtable.delete(key, self.sequence);

        Ok(())
    }
}
