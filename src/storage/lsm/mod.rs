//! LSM Tree Storage Engine - RocksDB-inspired
//! PROPRIETARY

pub mod async_writer;
pub mod memtable;
pub mod optimized_wal;
pub mod sst;
pub mod wal;

use crate::Result;

/// LSM Tree Storage
///
/// Architecture:
/// 1. MemTable (in-memory, sorted)
/// 2. WAL (Write-Ahead Log for durability) - optional for :memory: mode
/// 3. SST Files (Sorted String Tables on disk)
/// 4. Compaction (background merging)
pub struct LSMStorage {
    memtable: memtable::MemTable,
    wal: Option<wal::WriteAheadLog>, // Optional for :memory: mode
    sequence: u64,                   // Monotonically increasing
    is_memory: bool,
}

impl LSMStorage {
    pub fn new(path: &str, is_memory: bool) -> Result<Self> {
        // Skip WAL for in-memory databases
        let wal = if is_memory {
            None
        } else {
            Some(wal::WriteAheadLog::new(&format!("{}/wal", path))?)
        };

        Ok(Self {
            memtable: memtable::MemTable::new(),
            wal,
            sequence: 0,
            is_memory,
        })
    }

    /// Write key-value (goes to memtable + WAL)
    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        self.sequence += 1;

        // 1. Write to WAL first (durability) - skip for in-memory
        if let Some(ref mut wal) = self.wal {
            wal.append(self.sequence, &key, &value)?;
        }

        // 2. Write to memtable (fast reads)
        self.memtable.put(key, value, self.sequence);

        // 3. Check if memtable is full → flush to SST (skip for in-memory)
        if !self.is_memory && self.memtable.size() > 4 * 1024 * 1024 {
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

        // Write tombstone marker - skip WAL for in-memory
        if let Some(ref mut wal) = self.wal {
            wal.append_delete(self.sequence, &key)?;
        }
        self.memtable.delete(key, self.sequence);

        Ok(())
    }
}
