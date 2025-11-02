// Sequence System
// Week 10: Database Convenience Features
// PostgreSQL-compatible sequences for auto-increment and ID generation

use crate::{NexusError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

/// Sequence definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sequence {
    pub name: String,
    /// Current value (not atomic in serialized form)
    #[serde(skip)]
    current_value: Arc<AtomicI64>,
    /// Last value for serialization
    pub last_value: i64,
    /// Starting value
    pub start_value: i64,
    /// Increment by
    pub increment: i64,
    /// Minimum value
    pub min_value: Option<i64>,
    /// Maximum value
    pub max_value: Option<i64>,
    /// Cache size (for performance)
    pub cache: i64,
    /// Cycle when reaching max/min
    pub cycle: bool,
    /// Owned by table.column (for SERIAL)
    pub owned_by: Option<(String, String)>,
    pub created_at: u64,
}

impl Sequence {
    pub fn new(name: String, start: i64) -> Self {
        Self {
            name,
            current_value: Arc::new(AtomicI64::new(start - 1)),
            last_value: start - 1,
            start_value: start,
            increment: 1,
            min_value: None,
            max_value: None,
            cache: 1,
            cycle: false,
            owned_by: None,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Set increment
    pub fn with_increment(mut self, inc: i64) -> Self {
        if inc == 0 {
            panic!("Increment cannot be zero");
        }
        self.increment = inc;
        self
    }

    /// Set minimum value
    pub fn with_min(mut self, min: i64) -> Self {
        self.min_value = Some(min);
        self
    }

    /// Set maximum value
    pub fn with_max(mut self, max: i64) -> Self {
        self.max_value = Some(max);
        self
    }

    /// Set cache size
    pub fn with_cache(mut self, cache: i64) -> Self {
        self.cache = cache.max(1);
        self
    }

    /// Enable cycling
    pub fn cycle(mut self) -> Self {
        self.cycle = true;
        self
    }

    /// Set ownership
    pub fn owned_by(mut self, table: String, column: String) -> Self {
        self.owned_by = Some((table, column));
        self
    }

    /// Get next value
    pub fn nextval(&self) -> Result<i64> {
        loop {
            let current = self.current_value.load(Ordering::SeqCst);
            let next = current + self.increment;

            // Check bounds
            if self.increment > 0 {
                if let Some(max) = self.max_value {
                    if next > max {
                        if self.cycle {
                            let new_val = self.min_value.unwrap_or(self.start_value);
                            if self
                                .current_value
                                .compare_exchange(
                                    current,
                                    new_val,
                                    Ordering::SeqCst,
                                    Ordering::SeqCst,
                                )
                                .is_ok()
                            {
                                return Ok(new_val);
                            }
                            continue;
                        } else {
                            return Err(NexusError::QueryError(format!(
                                "Sequence {} reached maximum value",
                                self.name
                            )));
                        }
                    }
                }
            } else {
                if let Some(min) = self.min_value {
                    if next < min {
                        if self.cycle {
                            let new_val = self.max_value.unwrap_or(self.start_value);
                            if self
                                .current_value
                                .compare_exchange(
                                    current,
                                    new_val,
                                    Ordering::SeqCst,
                                    Ordering::SeqCst,
                                )
                                .is_ok()
                            {
                                return Ok(new_val);
                            }
                            continue;
                        } else {
                            return Err(NexusError::QueryError(format!(
                                "Sequence {} reached minimum value",
                                self.name
                            )));
                        }
                    }
                }
            }

            // Try to update
            if self
                .current_value
                .compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return Ok(next);
            }
        }
    }

    /// Get current value (without incrementing)
    pub fn currval(&self) -> Result<i64> {
        let val = self.current_value.load(Ordering::SeqCst);
        if val == self.start_value - 1 {
            return Err(NexusError::QueryError(format!(
                "Sequence {} has not been used in this session",
                self.name
            )));
        }
        Ok(val)
    }

    /// Set current value
    pub fn setval(&self, value: i64, is_called: bool) -> Result<i64> {
        if is_called {
            self.current_value.store(value, Ordering::SeqCst);
            Ok(value)
        } else {
            self.current_value
                .store(value - self.increment, Ordering::SeqCst);
            Ok(value)
        }
    }

    /// Reset sequence to start value
    pub fn restart(&self) -> Result<()> {
        self.current_value
            .store(self.start_value - self.increment, Ordering::SeqCst);
        Ok(())
    }

    /// Restart with new value
    pub fn restart_with(&self, value: i64) -> Result<()> {
        self.current_value
            .store(value - self.increment, Ordering::SeqCst);
        Ok(())
    }
}

/// Sequence Manager
pub struct SequenceManager {
    sequences: Arc<Mutex<HashMap<String, Arc<Sequence>>>>,
}

impl SequenceManager {
    pub fn new() -> Self {
        Self {
            sequences: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a sequence
    pub fn create_sequence(&self, sequence: Sequence) -> Result<()> {
        let mut sequences = self.sequences.lock().unwrap();

        if sequences.contains_key(&sequence.name) {
            return Err(NexusError::QueryError(format!(
                "Sequence {} already exists",
                sequence.name
            )));
        }

        sequences.insert(sequence.name.clone(), Arc::new(sequence));
        Ok(())
    }

    /// Drop a sequence
    pub fn drop_sequence(&self, name: &str, cascade: bool) -> Result<()> {
        let mut sequences = self.sequences.lock().unwrap();

        if !sequences.contains_key(name) {
            return Err(NexusError::QueryError(format!(
                "Sequence {} does not exist",
                name
            )));
        }

        // Check if owned by a column
        let seq = sequences.get(name).unwrap();
        if seq.owned_by.is_some() && !cascade {
            return Err(NexusError::QueryError(format!(
                "Sequence {} is owned by a column (use CASCADE)",
                name
            )));
        }

        sequences.remove(name);
        Ok(())
    }

    /// Get sequence
    pub fn get_sequence(&self, name: &str) -> Result<Arc<Sequence>> {
        let sequences = self.sequences.lock().unwrap();
        sequences
            .get(name)
            .cloned()
            .ok_or_else(|| NexusError::QueryError(format!("Sequence {} does not exist", name)))
    }

    /// nextval() function
    pub fn nextval(&self, name: &str) -> Result<i64> {
        let seq = self.get_sequence(name)?;
        seq.nextval()
    }

    /// currval() function
    pub fn currval(&self, name: &str) -> Result<i64> {
        let seq = self.get_sequence(name)?;
        seq.currval()
    }

    /// setval() function
    pub fn setval(&self, name: &str, value: i64, is_called: bool) -> Result<i64> {
        let seq = self.get_sequence(name)?;
        seq.setval(value, is_called)
    }

    /// Alter sequence
    pub fn alter_sequence(&self, name: &str, restart_value: Option<i64>) -> Result<()> {
        let seq = self.get_sequence(name)?;

        if let Some(value) = restart_value {
            seq.restart_with(value)?;
        } else {
            seq.restart()?;
        }

        Ok(())
    }

    /// List all sequences
    pub fn list_sequences(&self) -> Vec<String> {
        let sequences = self.sequences.lock().unwrap();
        sequences.keys().cloned().collect()
    }

    /// Get sequence count
    pub fn sequence_count(&self) -> usize {
        let sequences = self.sequences.lock().unwrap();
        sequences.len()
    }

    /// Create a SERIAL sequence for a table.column
    pub fn create_serial(&self, table: &str, column: &str) -> Result<String> {
        let seq_name = format!("{}_{}_seq", table, column);

        let sequence = Sequence::new(seq_name.clone(), 1)
            .with_increment(1)
            .with_min(1)
            .owned_by(table.to_string(), column.to_string());

        self.create_sequence(sequence)?;
        Ok(seq_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequence_basic() {
        let seq = Sequence::new("test_seq".to_string(), 1);

        assert_eq!(seq.nextval().unwrap(), 1);
        assert_eq!(seq.nextval().unwrap(), 2);
        assert_eq!(seq.nextval().unwrap(), 3);
        assert_eq!(seq.currval().unwrap(), 3);
    }

    #[test]
    fn test_sequence_increment() {
        let seq = Sequence::new("test_seq".to_string(), 100).with_increment(10);

        assert_eq!(seq.nextval().unwrap(), 100);
        assert_eq!(seq.nextval().unwrap(), 110);
        assert_eq!(seq.nextval().unwrap(), 120);
    }

    #[test]
    fn test_sequence_bounds() {
        let seq = Sequence::new("test_seq".to_string(), 1)
            .with_min(1)
            .with_max(3);

        assert_eq!(seq.nextval().unwrap(), 1);
        assert_eq!(seq.nextval().unwrap(), 2);
        assert_eq!(seq.nextval().unwrap(), 3);
        assert!(seq.nextval().is_err()); // Exceeds max
    }

    #[test]
    fn test_sequence_cycle() {
        let seq = Sequence::new("test_seq".to_string(), 1)
            .with_min(1)
            .with_max(3)
            .cycle();

        assert_eq!(seq.nextval().unwrap(), 1);
        assert_eq!(seq.nextval().unwrap(), 2);
        assert_eq!(seq.nextval().unwrap(), 3);
        assert_eq!(seq.nextval().unwrap(), 1); // Cycles back
    }

    #[test]
    fn test_sequence_setval() {
        let seq = Sequence::new("test_seq".to_string(), 1);

        assert_eq!(seq.nextval().unwrap(), 1);
        assert_eq!(seq.setval(100, true).unwrap(), 100);
        assert_eq!(seq.nextval().unwrap(), 101);
    }

    #[test]
    fn test_sequence_restart() {
        let seq = Sequence::new("test_seq".to_string(), 1);

        assert_eq!(seq.nextval().unwrap(), 1);
        assert_eq!(seq.nextval().unwrap(), 2);
        seq.restart().unwrap();
        assert_eq!(seq.nextval().unwrap(), 1);
    }

    #[test]
    fn test_sequence_manager() {
        let sm = SequenceManager::new();

        let seq = Sequence::new("test_seq".to_string(), 1000);
        sm.create_sequence(seq).unwrap();

        assert_eq!(sm.nextval("test_seq").unwrap(), 1000);
        assert_eq!(sm.nextval("test_seq").unwrap(), 1001);
        assert_eq!(sm.currval("test_seq").unwrap(), 1001);
    }

    #[test]
    fn test_serial_creation() {
        let sm = SequenceManager::new();

        let seq_name = sm.create_serial("users", "id").unwrap();
        assert_eq!(seq_name, "users_id_seq");

        assert_eq!(sm.nextval(&seq_name).unwrap(), 1);
        assert_eq!(sm.nextval(&seq_name).unwrap(), 2);
    }

    #[test]
    fn test_concurrent_access() {
        use std::thread;

        let seq = Arc::new(Sequence::new("test_seq".to_string(), 1));
        let mut handles = vec![];

        for _ in 0..10 {
            let seq_clone = seq.clone();
            handles.push(thread::spawn(move || {
                let mut values = vec![];
                for _ in 0..10 {
                    values.push(seq_clone.nextval().unwrap());
                }
                values
            }));
        }

        let mut all_values = vec![];
        for handle in handles {
            all_values.extend(handle.join().unwrap());
        }

        // All values should be unique
        all_values.sort();
        for i in 1..=100 {
            assert_eq!(all_values[i - 1], i as i64);
        }
    }
}
