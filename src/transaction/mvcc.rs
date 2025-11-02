// Multi-Version Concurrency Control (MVCC)
// PostgreSQL-compatible MVCC for non-blocking reads and writes

use super::TransactionId;
use crate::types::Row;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

/// Row version with transaction visibility information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowVersion {
    /// The row data
    pub row: Row,
    /// Transaction that created this version
    pub xmin: TransactionId,
    /// Transaction that deleted/updated this version (None if current)
    pub xmax: Option<TransactionId>,
    /// Is this version committed?
    pub committed: bool,
    /// Row key for identifying the logical row
    pub key: String,
}

impl RowVersion {
    pub fn new(row: Row, xmin: TransactionId, key: String) -> Self {
        Self {
            row,
            xmin,
            xmax: None,
            committed: false,
            key,
        }
    }

    /// Check if this version is visible to a transaction
    pub fn is_visible_to(&self, snapshot: &TransactionSnapshot) -> bool {
        // Version created by a transaction that started after our snapshot
        if self.xmin > snapshot.xmax {
            return false;
        }

        // Version created by a transaction that was still active in our snapshot
        if snapshot.active_txns.contains(&self.xmin) {
            return false;
        }

        // If version was deleted/updated
        if let Some(xmax) = self.xmax {
            // Deleted by a transaction that started after our snapshot
            if xmax > snapshot.xmax {
                return true; // Still visible
            }

            // Deleted by a transaction that was active in our snapshot
            if snapshot.active_txns.contains(&xmax) {
                return true; // Still visible
            }

            // Deleted by a committed transaction before our snapshot
            return false;
        }

        // Version is visible
        true
    }
}

/// Transaction snapshot for REPEATABLE READ and SERIALIZABLE
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSnapshot {
    /// Transaction ID when snapshot was taken
    pub xmin: TransactionId,
    /// Highest transaction ID + 1 when snapshot was taken
    pub xmax: TransactionId,
    /// Active transaction IDs at snapshot time
    pub active_txns: HashSet<TransactionId>,
}

impl TransactionSnapshot {
    pub fn new(
        xmin: TransactionId,
        xmax: TransactionId,
        active_txns: HashSet<TransactionId>,
    ) -> Self {
        Self {
            xmin,
            xmax,
            active_txns,
        }
    }
}

/// MVCC Manager
/// Manages multiple versions of rows for concurrent access
pub struct MVCCManager {
    /// All row versions, indexed by table and row key
    versions: Arc<Mutex<HashMap<String, Vec<RowVersion>>>>,
    /// Committed transaction IDs
    committed_txns: Arc<Mutex<HashSet<TransactionId>>>,
    /// Aborted transaction IDs
    aborted_txns: Arc<Mutex<HashSet<TransactionId>>>,
}

impl MVCCManager {
    pub fn new() -> Self {
        Self {
            versions: Arc::new(Mutex::new(HashMap::new())),
            committed_txns: Arc::new(Mutex::new(HashSet::new())),
            aborted_txns: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Create a snapshot for a transaction
    pub fn create_snapshot(&self, txn_id: TransactionId) -> TransactionSnapshot {
        // In a real implementation, we'd track active transactions
        // For now, simplified version
        TransactionSnapshot {
            xmin: 1,
            xmax: txn_id + 1,
            active_txns: HashSet::new(),
        }
    }

    /// Insert a new row version
    pub fn insert_version(&self, table: &str, key: String, row: Row, txn_id: TransactionId) {
        let mut versions = self.versions.lock().unwrap();
        let table_key = format!("{}:{}", table, key);

        let version = RowVersion::new(row, txn_id, key);

        versions
            .entry(table_key)
            .or_insert_with(Vec::new)
            .push(version);
    }

    /// Update a row (creates new version and marks old as deleted)
    pub fn update_version(
        &self,
        table: &str,
        key: String,
        old_row: Row,
        new_row: Row,
        txn_id: TransactionId,
    ) {
        let mut versions = self.versions.lock().unwrap();
        let table_key = format!("{}:{}", table, key);

        // Mark old version as deleted by this transaction
        if let Some(version_list) = versions.get_mut(&table_key) {
            // Find the current version (not deleted)
            for version in version_list.iter_mut() {
                if version.xmax.is_none() && version.row == old_row {
                    version.xmax = Some(txn_id);
                    break;
                }
            }
        }

        // Add new version
        let new_version = RowVersion::new(new_row, txn_id, key);
        versions
            .entry(table_key)
            .or_insert_with(Vec::new)
            .push(new_version);
    }

    /// Delete a row (marks current version as deleted)
    pub fn delete_version(&self, table: &str, key: String, txn_id: TransactionId) {
        let mut versions = self.versions.lock().unwrap();
        let table_key = format!("{}:{}", table, key);

        if let Some(version_list) = versions.get_mut(&table_key) {
            // Find current version and mark as deleted
            for version in version_list.iter_mut() {
                if version.xmax.is_none() {
                    version.xmax = Some(txn_id);
                    break;
                }
            }
        }
    }

    /// Get visible versions for a snapshot
    pub fn get_visible_versions(&self, table: &str, snapshot: &TransactionSnapshot) -> Vec<Row> {
        let versions = self.versions.lock().unwrap();
        let mut visible_rows = Vec::new();

        // Iterate through all versions for this table
        for (table_key, version_list) in versions.iter() {
            if !table_key.starts_with(&format!("{}:", table)) {
                continue;
            }

            // Find the visible version for each row
            for version in version_list {
                if version.is_visible_to(snapshot) {
                    visible_rows.push(version.row.clone());
                }
            }
        }

        visible_rows
    }

    /// Get current versions (for READ COMMITTED)
    pub fn get_current_versions(&self, table: &str) -> Vec<Row> {
        let versions = self.versions.lock().unwrap();
        let committed_txns = self.committed_txns.lock().unwrap();
        let aborted_txns = self.aborted_txns.lock().unwrap();

        let mut current_rows = Vec::new();

        for (table_key, version_list) in versions.iter() {
            if !table_key.starts_with(&format!("{}:", table)) {
                continue;
            }

            // Find the latest committed version
            for version in version_list.iter().rev() {
                // Skip versions from aborted transactions
                if aborted_txns.contains(&version.xmin) {
                    continue;
                }

                // Skip deleted versions
                if let Some(xmax) = version.xmax {
                    if committed_txns.contains(&xmax) {
                        continue;
                    }
                }

                // Only include committed versions
                if committed_txns.contains(&version.xmin) || version.committed {
                    current_rows.push(version.row.clone());
                    break;
                }
            }
        }

        current_rows
    }

    /// Mark transaction as committed
    pub fn commit_transaction(&self, txn_id: TransactionId) {
        let mut committed_txns = self.committed_txns.lock().unwrap();
        committed_txns.insert(txn_id);

        // Mark all versions created by this transaction as committed
        let mut versions = self.versions.lock().unwrap();
        for version_list in versions.values_mut() {
            for version in version_list.iter_mut() {
                if version.xmin == txn_id {
                    version.committed = true;
                }
            }
        }
    }

    /// Mark transaction as aborted
    pub fn abort_transaction(&self, txn_id: TransactionId) {
        let mut aborted_txns = self.aborted_txns.lock().unwrap();
        aborted_txns.insert(txn_id);
    }

    /// Vacuum old versions (garbage collection)
    pub fn vacuum(&self, table: &str) {
        let mut versions = self.versions.lock().unwrap();
        let committed_txns = self.committed_txns.lock().unwrap();
        let aborted_txns = self.aborted_txns.lock().unwrap();

        // Remove versions from aborted transactions
        for (table_key, version_list) in versions.iter_mut() {
            if !table_key.starts_with(&format!("{}:", table)) {
                continue;
            }

            version_list.retain(|v| !aborted_txns.contains(&v.xmin));
        }

        // Remove old versions that are no longer visible
        // (keep only the latest committed version for each row)
        for (table_key, version_list) in versions.iter_mut() {
            if !table_key.starts_with(&format!("{}:", table)) {
                continue;
            }

            // Find latest committed version
            let mut latest_committed_idx = None;
            for (i, version) in version_list.iter().enumerate().rev() {
                if committed_txns.contains(&version.xmin) && version.xmax.is_none() {
                    latest_committed_idx = Some(i);
                    break;
                }
            }

            // Keep only versions needed for active snapshots
            // For simplicity, keep last 2 committed versions
            if let Some(latest_idx) = latest_committed_idx {
                let keep_from = if latest_idx > 0 { latest_idx - 1 } else { 0 };
                version_list.drain(0..keep_from);
            }
        }
    }

    /// Get statistics
    pub fn get_stats(&self) -> MVCCStats {
        let versions = self.versions.lock().unwrap();
        let committed = self.committed_txns.lock().unwrap();
        let aborted = self.aborted_txns.lock().unwrap();

        let total_versions: usize = versions.values().map(|v| v.len()).sum();

        MVCCStats {
            total_versions,
            committed_txns: committed.len(),
            aborted_txns: aborted.len(),
        }
    }
}

/// MVCC statistics
#[derive(Debug, Clone)]
pub struct MVCCStats {
    pub total_versions: usize,
    pub committed_txns: usize,
    pub aborted_txns: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Value;

    #[test]
    fn test_mvcc_insert() {
        let mvcc = MVCCManager::new();

        let row = Row {
            columns: vec!["id".to_string(), "name".to_string()],
            values: vec![Value::Integer(1), Value::Text("Alice".to_string())],
        };

        mvcc.insert_version("users", "1".to_string(), row.clone(), 1);

        let snapshot = TransactionSnapshot {
            xmin: 1,
            xmax: 2,
            active_txns: HashSet::new(),
        };

        mvcc.commit_transaction(1);

        let visible = mvcc.get_visible_versions("users", &snapshot);
        assert_eq!(visible.len(), 1);
    }

    #[test]
    fn test_mvcc_update() {
        let mvcc = MVCCManager::new();

        let old_row = Row {
            columns: vec!["id".to_string(), "name".to_string()],
            values: vec![Value::Integer(1), Value::Text("Alice".to_string())],
        };

        let new_row = Row {
            columns: vec!["id".to_string(), "name".to_string()],
            values: vec![Value::Integer(1), Value::Text("Bob".to_string())],
        };

        mvcc.insert_version("users", "1".to_string(), old_row.clone(), 1);
        mvcc.commit_transaction(1);

        mvcc.update_version("users", "1".to_string(), old_row, new_row, 2);
        mvcc.commit_transaction(2);

        let current = mvcc.get_current_versions("users");
        assert_eq!(current.len(), 1);
        assert_eq!(current[0].values[1], Value::Text("Bob".to_string()));
    }
}
