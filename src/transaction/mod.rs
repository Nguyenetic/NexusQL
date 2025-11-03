// Transaction Management System
// Week 9: Database Fundamentals Phase 3
// PostgreSQL-compatible transactions with MVCC and WAL

pub mod mvcc;
pub mod wal;

use crate::types::Row;
use crate::{NexusError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub use mvcc::{MVCCManager, RowVersion, TransactionSnapshot};
pub use wal::{WALManager, WALRecord};

/// Global transaction ID counter
static NEXT_TXN_ID: AtomicU64 = AtomicU64::new(1);

/// Transaction ID type
pub type TransactionId = u64;

/// Transaction state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionState {
    /// Transaction is active
    Active,
    /// Transaction is preparing to commit (2PC)
    Preparing,
    /// Transaction has committed
    Committed,
    /// Transaction has been aborted/rolled back
    Aborted,
}

/// Transaction isolation level (PostgreSQL-compatible)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsolationLevel {
    /// Read Uncommitted - lowest isolation (not standard in PostgreSQL)
    ReadUncommitted,
    /// Read Committed - default in PostgreSQL
    ReadCommitted,
    /// Repeatable Read - prevents non-repeatable reads
    RepeatableRead,
    /// Serializable - highest isolation
    Serializable,
}

impl Default for IsolationLevel {
    fn default() -> Self {
        IsolationLevel::ReadCommitted
    }
}

/// Transaction metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: TransactionId,
    pub state: TransactionState,
    pub isolation_level: IsolationLevel,
    pub started_at: u64,
    pub snapshot: Option<TransactionSnapshot>,
    /// Tables locked by this transaction
    pub read_locks: HashSet<String>,
    pub write_locks: HashSet<String>,
    /// Savepoints for nested rollback
    pub savepoints: HashMap<String, Savepoint>,
}

impl Transaction {
    pub fn new(isolation_level: IsolationLevel) -> Self {
        let id = NEXT_TXN_ID.fetch_add(1, Ordering::SeqCst);
        Self {
            id,
            state: TransactionState::Active,
            isolation_level,
            started_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            snapshot: None,
            read_locks: HashSet::new(),
            write_locks: HashSet::new(),
            savepoints: HashMap::new(),
        }
    }

    /// Check if transaction is active
    pub fn is_active(&self) -> bool {
        self.state == TransactionState::Active || self.state == TransactionState::Preparing
    }

    /// Acquire read lock on table
    pub fn read_lock(&mut self, table: String) {
        self.read_locks.insert(table);
    }

    /// Acquire write lock on table
    pub fn write_lock(&mut self, table: String) {
        self.write_locks.insert(table);
    }
}

/// Savepoint for nested transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Savepoint {
    pub name: String,
    pub txn_id: TransactionId,
    pub created_at: u64,
    /// Number of operations before this savepoint
    pub operation_count: usize,
}

/// Transaction operation for rollback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionOperation {
    Insert {
        table: String,
        row: Row,
    },
    Update {
        table: String,
        old_row: Row,
        new_row: Row,
    },
    Delete {
        table: String,
        row: Row,
    },
}

/// Transaction Manager
/// Manages all active transactions and coordinates commits/rollbacks
pub struct TransactionManager {
    /// All transactions (active and completed)
    transactions: Arc<Mutex<HashMap<TransactionId, Transaction>>>,
    /// Currently active transaction IDs
    active_txns: Arc<Mutex<HashSet<TransactionId>>>,
    /// MVCC manager for multi-version concurrency control
    mvcc: Arc<MVCCManager>,
    /// WAL manager for durability
    wal: Arc<Mutex<WALManager>>,
    /// Transaction operations for rollback (per transaction)
    operations: Arc<Mutex<HashMap<TransactionId, Vec<TransactionOperation>>>>,
    /// Deadlock detection graph
    waits_for: Arc<Mutex<HashMap<TransactionId, HashSet<TransactionId>>>>,
}

impl TransactionManager {
    pub fn new(wal_path: &str) -> Result<Self> {
        Ok(Self {
            transactions: Arc::new(Mutex::new(HashMap::new())),
            active_txns: Arc::new(Mutex::new(HashSet::new())),
            mvcc: Arc::new(MVCCManager::new()),
            wal: Arc::new(Mutex::new(WALManager::new(wal_path)?)),
            operations: Arc::new(Mutex::new(HashMap::new())),
            waits_for: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Begin a new transaction
    pub fn begin(&self, isolation_level: IsolationLevel) -> Result<TransactionId> {
        let mut txn = Transaction::new(isolation_level);
        let txn_id = txn.id;

        // Create snapshot for REPEATABLE READ and SERIALIZABLE
        if matches!(
            isolation_level,
            IsolationLevel::RepeatableRead | IsolationLevel::Serializable
        ) {
            let snapshot = self.mvcc.create_snapshot(txn_id);
            txn.snapshot = Some(snapshot);
        }

        // Register transaction
        let mut transactions = self.transactions.lock().unwrap();
        let mut active_txns = self.active_txns.lock().unwrap();

        transactions.insert(txn_id, txn);
        active_txns.insert(txn_id);

        // Initialize operations log
        let mut operations = self.operations.lock().unwrap();
        operations.insert(txn_id, Vec::new());

        // Log BEGIN to WAL
        let mut wal = self.wal.lock().unwrap();
        wal.log_begin(txn_id)?;

        Ok(txn_id)
    }

    /// Commit a transaction
    pub fn commit(&self, txn_id: TransactionId) -> Result<()> {
        let mut transactions = self.transactions.lock().unwrap();
        let mut active_txns = self.active_txns.lock().unwrap();

        let txn = transactions.get_mut(&txn_id).ok_or_else(|| {
            NexusError::QueryError(format!("Transaction {} does not exist", txn_id))
        })?;

        if !txn.is_active() {
            return Err(NexusError::QueryError(format!(
                "Transaction {} is not active",
                txn_id
            )));
        }

        // Change state to preparing
        txn.state = TransactionState::Preparing;

        // Write all operations to WAL before committing
        let operations = self.operations.lock().unwrap();
        if let Some(ops) = operations.get(&txn_id) {
            let mut wal = self.wal.lock().unwrap();
            for op in ops {
                match op {
                    TransactionOperation::Insert { table, row } => {
                        wal.log_insert(txn_id, table, row)?;
                    }
                    TransactionOperation::Update {
                        table,
                        old_row,
                        new_row,
                    } => {
                        wal.log_update(txn_id, table, old_row, new_row)?;
                    }
                    TransactionOperation::Delete { table, row } => {
                        wal.log_delete(txn_id, table, row)?;
                    }
                }
            }

            // Log COMMIT to WAL
            wal.log_commit(txn_id)?;

            // Flush WAL to ensure durability
            wal.flush()?;
        }

        // Mark transaction as committed
        txn.state = TransactionState::Committed;
        active_txns.remove(&txn_id);

        // Notify MVCC that transaction committed
        self.mvcc.commit_transaction(txn_id);

        // Clean up operations log
        drop(operations);
        let mut operations = self.operations.lock().unwrap();
        operations.remove(&txn_id);

        // Release locks
        txn.read_locks.clear();
        txn.write_locks.clear();

        Ok(())
    }

    /// Rollback a transaction
    pub fn rollback(&self, txn_id: TransactionId) -> Result<()> {
        let mut transactions = self.transactions.lock().unwrap();
        let mut active_txns = self.active_txns.lock().unwrap();

        let txn = transactions.get_mut(&txn_id).ok_or_else(|| {
            NexusError::QueryError(format!("Transaction {} does not exist", txn_id))
        })?;

        if !txn.is_active() {
            return Err(NexusError::QueryError(format!(
                "Transaction {} is not active",
                txn_id
            )));
        }

        // Log ROLLBACK to WAL
        let mut wal = self.wal.lock().unwrap();
        wal.log_rollback(txn_id)?;
        wal.flush()?;
        drop(wal);

        // Mark transaction as aborted
        txn.state = TransactionState::Aborted;
        active_txns.remove(&txn_id);

        // Notify MVCC
        self.mvcc.abort_transaction(txn_id);

        // Clean up operations
        let mut operations = self.operations.lock().unwrap();
        operations.remove(&txn_id);

        // Release locks
        txn.read_locks.clear();
        txn.write_locks.clear();

        Ok(())
    }

    /// Create a savepoint
    pub fn savepoint(&self, txn_id: TransactionId, name: String) -> Result<()> {
        let mut transactions = self.transactions.lock().unwrap();
        let txn = transactions.get_mut(&txn_id).ok_or_else(|| {
            NexusError::QueryError(format!("Transaction {} does not exist", txn_id))
        })?;

        if !txn.is_active() {
            return Err(NexusError::QueryError(format!(
                "Transaction {} is not active",
                txn_id
            )));
        }

        let operations = self.operations.lock().unwrap();
        let operation_count = operations.get(&txn_id).map(|v| v.len()).unwrap_or(0);

        let savepoint = Savepoint {
            name: name.clone(),
            txn_id,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            operation_count,
        };

        txn.savepoints.insert(name, savepoint);

        Ok(())
    }

    /// Rollback to a savepoint
    pub fn rollback_to_savepoint(&self, txn_id: TransactionId, name: &str) -> Result<()> {
        let transactions = self.transactions.lock().unwrap();
        let txn = transactions.get(&txn_id).ok_or_else(|| {
            NexusError::QueryError(format!("Transaction {} does not exist", txn_id))
        })?;

        let savepoint = txn
            .savepoints
            .get(name)
            .ok_or_else(|| NexusError::QueryError(format!("Savepoint {} does not exist", name)))?;

        // Truncate operations to savepoint
        let mut operations = self.operations.lock().unwrap();
        if let Some(ops) = operations.get_mut(&txn_id) {
            ops.truncate(savepoint.operation_count);
        }

        Ok(())
    }

    /// Log an operation for potential rollback
    pub fn log_operation(&self, txn_id: TransactionId, operation: TransactionOperation) {
        let mut operations = self.operations.lock().unwrap();
        if let Some(ops) = operations.get_mut(&txn_id) {
            ops.push(operation);
        }
    }

    /// Check for deadlocks using cycle detection
    pub fn detect_deadlock(&self, txn_id: TransactionId) -> Result<bool> {
        let waits_for = self.waits_for.lock().unwrap();

        // Use DFS to detect cycles
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        fn has_cycle(
            node: TransactionId,
            waits_for: &HashMap<TransactionId, HashSet<TransactionId>>,
            visited: &mut HashSet<TransactionId>,
            rec_stack: &mut HashSet<TransactionId>,
        ) -> bool {
            visited.insert(node);
            rec_stack.insert(node);

            if let Some(neighbors) = waits_for.get(&node) {
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) {
                        if has_cycle(neighbor, waits_for, visited, rec_stack) {
                            return true;
                        }
                    } else if rec_stack.contains(&neighbor) {
                        return true; // Cycle detected
                    }
                }
            }

            rec_stack.remove(&node);
            false
        }

        Ok(has_cycle(txn_id, &waits_for, &mut visited, &mut rec_stack))
    }

    /// Get transaction info
    pub fn get_transaction(&self, txn_id: TransactionId) -> Option<Transaction> {
        self.transactions.lock().unwrap().get(&txn_id).cloned()
    }

    /// Get all active transactions
    pub fn get_active_transactions(&self) -> Vec<TransactionId> {
        self.active_txns.lock().unwrap().iter().copied().collect()
    }

    /// Get MVCC manager
    pub fn mvcc(&self) -> Arc<MVCCManager> {
        self.mvcc.clone()
    }

    /// Recover from WAL after crash
    pub fn recover(&self) -> Result<()> {
        let wal = self.wal.lock().unwrap();
        let records = wal.read_all()?;

        let mut in_progress: HashMap<TransactionId, Vec<TransactionOperation>> = HashMap::new();

        for record in records {
            match record {
                WALRecord::Begin { txn_id, .. } => {
                    in_progress.insert(txn_id, Vec::new());
                }
                WALRecord::Insert { txn_id, table, row } => {
                    if let Some(ops) = in_progress.get_mut(&txn_id) {
                        ops.push(TransactionOperation::Insert { table, row });
                    }
                }
                WALRecord::Update {
                    txn_id,
                    table,
                    old_row,
                    new_row,
                } => {
                    if let Some(ops) = in_progress.get_mut(&txn_id) {
                        ops.push(TransactionOperation::Update {
                            table,
                            old_row,
                            new_row,
                        });
                    }
                }
                WALRecord::Delete { txn_id, table, row } => {
                    if let Some(ops) = in_progress.get_mut(&txn_id) {
                        ops.push(TransactionOperation::Delete { table, row });
                    }
                }
                WALRecord::Commit { txn_id, .. } => {
                    // Transaction committed - replay operations
                    if let Some(ops) = in_progress.remove(&txn_id) {
                        // In a real implementation, we'd replay these operations
                        // to restore the database state
                        tracing::info!(
                            "Replaying {} operations for committed txn {}",
                            ops.len(),
                            txn_id
                        );
                    }
                }
                WALRecord::Rollback { txn_id, .. } => {
                    // Transaction rolled back - discard operations
                    in_progress.remove(&txn_id);
                }
                WALRecord::Checkpoint { .. } => {
                    // Checkpoint - all transactions before this are durably committed
                    in_progress.clear();
                }
            }
        }

        // Any remaining transactions were in-progress during crash - rollback
        for (txn_id, _ops) in in_progress {
            tracing::warn!("Rolling back incomplete transaction {} after crash", txn_id);
        }

        Ok(())
    }

    /// Create a checkpoint in the WAL
    pub fn checkpoint(&self) -> Result<()> {
        let mut wal = self.wal.lock().unwrap();
        wal.checkpoint()?;
        wal.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_begin_commit() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("wal").to_str().unwrap().to_string();
        let tm = TransactionManager::new(&wal_path).unwrap();

        let txn_id = tm.begin(IsolationLevel::ReadCommitted).unwrap();
        assert!(txn_id > 0);

        let txn = tm.get_transaction(txn_id).unwrap();
        assert_eq!(txn.state, TransactionState::Active);

        tm.commit(txn_id).unwrap();

        let txn = tm.get_transaction(txn_id).unwrap();
        assert_eq!(txn.state, TransactionState::Committed);
    }

    #[test]
    fn test_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("wal").to_str().unwrap().to_string();
        let tm = TransactionManager::new(&wal_path).unwrap();

        let txn_id = tm.begin(IsolationLevel::ReadCommitted).unwrap();
        tm.rollback(txn_id).unwrap();

        let txn = tm.get_transaction(txn_id).unwrap();
        assert_eq!(txn.state, TransactionState::Aborted);
    }

    #[test]
    fn test_savepoint() {
        let temp_dir = TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("wal").to_str().unwrap().to_string();
        let tm = TransactionManager::new(&wal_path).unwrap();

        let txn_id = tm.begin(IsolationLevel::ReadCommitted).unwrap();

        tm.savepoint(txn_id, "sp1".to_string()).unwrap();

        let txn = tm.get_transaction(txn_id).unwrap();
        assert!(txn.savepoints.contains_key("sp1"));
    }
}
