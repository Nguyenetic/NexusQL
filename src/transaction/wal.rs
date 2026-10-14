// Write-Ahead Log (WAL)
// Standalone TransactionManager operation journal (not used by SQL storage).
// SQL crash recovery uses storage::wal and transaction::durable instead.

use super::TransactionId;
use crate::types::Row;
use crate::{NexusError, Result};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// WAL record types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WALRecord {
    /// Transaction begin
    Begin {
        txn_id: TransactionId,
        timestamp: u64,
    },
    /// Insert operation
    Insert {
        txn_id: TransactionId,
        table: String,
        row: Row,
    },
    /// Update operation
    Update {
        txn_id: TransactionId,
        table: String,
        old_row: Row,
        new_row: Row,
    },
    /// Delete operation
    Delete {
        txn_id: TransactionId,
        table: String,
        row: Row,
    },
    /// Transaction commit
    Commit {
        txn_id: TransactionId,
        timestamp: u64,
    },
    /// Transaction rollback
    Rollback {
        txn_id: TransactionId,
        timestamp: u64,
    },
    /// Checkpoint marker
    Checkpoint { timestamp: u64 },
}

impl WALRecord {
    pub fn txn_id(&self) -> Option<TransactionId> {
        match self {
            WALRecord::Begin { txn_id, .. }
            | WALRecord::Insert { txn_id, .. }
            | WALRecord::Update { txn_id, .. }
            | WALRecord::Delete { txn_id, .. }
            | WALRecord::Commit { txn_id, .. }
            | WALRecord::Rollback { txn_id, .. } => Some(*txn_id),
            WALRecord::Checkpoint { .. } => None,
        }
    }
}

/// WAL Manager
/// Manages write-ahead logging for durability
pub struct WALManager {
    /// Path to WAL file
    path: String,
    /// WAL file writer
    writer: BufWriter<File>,
    /// Number of records written since last flush
    unflushed_records: usize,
    /// Flush every N records
    flush_threshold: usize,
}

impl WALManager {
    /// Create a new WAL manager
    pub fn new(path: &str) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new().create(true).append(true).open(path)?;

        Ok(Self {
            path: path.to_string(),
            writer: BufWriter::new(file),
            unflushed_records: 0,
            flush_threshold: 10, // Flush every 10 records
        })
    }

    /// Write a record to the WAL
    fn write_record(&mut self, record: &WALRecord) -> Result<()> {
        // Serialize record to JSON (in production, use binary format)
        let json = serde_json::to_string(record).map_err(|e| {
            NexusError::QueryError(format!("Failed to serialize WAL record: {}", e))
        })?;

        // Write length prefix (4 bytes) followed by JSON
        let len = json.len() as u32;
        self.writer.write_all(&len.to_le_bytes())?;

        self.writer.write_all(json.as_bytes())?;

        // Write newline for readability
        self.writer.write_all(b"\n")?;

        self.unflushed_records += 1;

        // Auto-flush if threshold reached
        if self.unflushed_records >= self.flush_threshold {
            self.flush()?;
        }

        Ok(())
    }

    /// Log BEGIN transaction
    pub fn log_begin(&mut self, txn_id: TransactionId) -> Result<()> {
        let record = WALRecord::Begin {
            txn_id,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        self.write_record(&record)
    }

    /// Log INSERT operation
    pub fn log_insert(&mut self, txn_id: TransactionId, table: &str, row: &Row) -> Result<()> {
        let record = WALRecord::Insert {
            txn_id,
            table: table.to_string(),
            row: row.clone(),
        };
        self.write_record(&record)
    }

    /// Log UPDATE operation
    pub fn log_update(
        &mut self,
        txn_id: TransactionId,
        table: &str,
        old_row: &Row,
        new_row: &Row,
    ) -> Result<()> {
        let record = WALRecord::Update {
            txn_id,
            table: table.to_string(),
            old_row: old_row.clone(),
            new_row: new_row.clone(),
        };
        self.write_record(&record)
    }

    /// Log DELETE operation
    pub fn log_delete(&mut self, txn_id: TransactionId, table: &str, row: &Row) -> Result<()> {
        let record = WALRecord::Delete {
            txn_id,
            table: table.to_string(),
            row: row.clone(),
        };
        self.write_record(&record)
    }

    /// Log COMMIT transaction
    pub fn log_commit(&mut self, txn_id: TransactionId) -> Result<()> {
        let record = WALRecord::Commit {
            txn_id,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        self.write_record(&record)
    }

    /// Log ROLLBACK transaction
    pub fn log_rollback(&mut self, txn_id: TransactionId) -> Result<()> {
        let record = WALRecord::Rollback {
            txn_id,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        self.write_record(&record)
    }

    /// Log CHECKPOINT
    pub fn checkpoint(&mut self) -> Result<()> {
        let record = WALRecord::Checkpoint {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        self.write_record(&record)
    }

    /// Flush WAL to disk
    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        self.writer.get_ref().sync_all()?;

        self.unflushed_records = 0;
        Ok(())
    }

    /// Read all WAL records (for recovery)
    pub fn read_all(&self) -> Result<Vec<WALRecord>> {
        let file = File::open(&self.path)?;

        let mut reader = BufReader::new(file);
        let mut records = Vec::new();

        loop {
            // Read length prefix
            let mut len_bytes = [0u8; 4];
            match reader.read_exact(&mut len_bytes) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.into()),
            }

            let len = u32::from_le_bytes(len_bytes) as usize;

            // Read JSON record
            let mut json_bytes = vec![0u8; len];
            reader.read_exact(&mut json_bytes)?;

            // Read newline
            let mut newline = [0u8; 1];
            reader.read_exact(&mut newline)?;

            // Deserialize
            let json_str = String::from_utf8(json_bytes).map_err(|e| {
                NexusError::QueryError(format!("Invalid UTF-8 in WAL record: {}", e))
            })?;

            let record: WALRecord = serde_json::from_str(&json_str).map_err(|e| {
                NexusError::QueryError(format!("Failed to deserialize WAL record: {}", e))
            })?;

            records.push(record);
        }

        Ok(records)
    }

    /// Truncate WAL after checkpoint
    pub fn truncate(&mut self) -> Result<()> {
        // Close current writer
        self.flush()?;
        // Writer will be dropped when recreated below

        // Truncate file
        let _file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.path)?;

        // Reopen writer
        self.writer = BufWriter::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?,
        );

        Ok(())
    }

    /// Get WAL file size
    pub fn file_size(&self) -> Result<u64> {
        let metadata = std::fs::metadata(&self.path)?;
        Ok(metadata.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Value;
    use tempfile::TempDir;

    #[test]
    fn test_wal_write_read() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");

        {
            let mut wal = WALManager::new(wal_path.to_str().unwrap()).unwrap();

            wal.log_begin(1).unwrap();

            let row = Row {
                columns: vec!["id".to_string(), "name".to_string()],
                values: vec![Value::Integer(1), Value::Text("Alice".to_string())],
            };

            wal.log_insert(1, "users", &row).unwrap();
            wal.log_commit(1).unwrap();
            wal.flush().unwrap();
        }

        // Read back
        let wal = WALManager::new(wal_path.to_str().unwrap()).unwrap();
        let records = wal.read_all().unwrap();

        assert_eq!(records.len(), 3); // BEGIN, INSERT, COMMIT

        match &records[0] {
            WALRecord::Begin { txn_id, .. } => assert_eq!(*txn_id, 1),
            _ => panic!("Expected Begin record"),
        }

        match &records[1] {
            WALRecord::Insert { txn_id, table, .. } => {
                assert_eq!(*txn_id, 1);
                assert_eq!(table, "users");
            }
            _ => panic!("Expected Insert record"),
        }

        match &records[2] {
            WALRecord::Commit { txn_id, .. } => assert_eq!(*txn_id, 1),
            _ => panic!("Expected Commit record"),
        }
    }

    #[test]
    fn test_wal_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("test.wal");

        let mut wal = WALManager::new(wal_path.to_str().unwrap()).unwrap();

        wal.log_begin(1).unwrap();
        wal.log_commit(1).unwrap();
        wal.checkpoint().unwrap();
        wal.flush().unwrap();

        let records = wal.read_all().unwrap();
        assert_eq!(records.len(), 3);

        match &records[2] {
            WALRecord::Checkpoint { .. } => {}
            _ => panic!("Expected Checkpoint record"),
        }
    }
}
