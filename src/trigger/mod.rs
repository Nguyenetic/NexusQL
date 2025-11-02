// Trigger System
// Week 10: Database Convenience Features
// PostgreSQL-compatible triggers with BEFORE/AFTER and ROW/STATEMENT level

use crate::types::{Row, Value};
use crate::{NexusError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Trigger timing - when the trigger fires
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerTiming {
    /// Fire before the operation
    Before,
    /// Fire after the operation
    After,
    /// Fire instead of the operation (for views)
    InsteadOf,
}

/// Trigger event - what operation causes the trigger
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TriggerEvent {
    Insert,
    Update,
    Delete,
    Truncate,
}

/// Trigger level - row or statement level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerLevel {
    /// Fire once for each row affected
    Row,
    /// Fire once for the entire statement
    Statement,
}

/// Trigger definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    pub name: String,
    pub table: String,
    pub timing: TriggerTiming,
    pub events: Vec<TriggerEvent>,
    pub level: TriggerLevel,
    /// Function to execute (name)
    pub function_name: String,
    /// Optional WHEN condition
    pub when_condition: Option<String>,
    /// Is trigger enabled?
    pub enabled: bool,
    /// Trigger priority (lower = earlier)
    pub priority: i32,
    pub created_at: u64,
}

impl Trigger {
    pub fn new(
        name: String,
        table: String,
        timing: TriggerTiming,
        events: Vec<TriggerEvent>,
        level: TriggerLevel,
        function_name: String,
    ) -> Self {
        Self {
            name,
            table,
            timing,
            events,
            level,
            function_name,
            when_condition: None,
            enabled: true,
            priority: 0,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Set WHEN condition
    pub fn when(mut self, condition: String) -> Self {
        self.when_condition = Some(condition);
        self
    }

    /// Set priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Check if trigger should fire for an event
    pub fn fires_for(&self, event: TriggerEvent) -> bool {
        self.enabled && self.events.contains(&event)
    }
}

/// Trigger execution context
#[derive(Debug, Clone)]
pub struct TriggerContext {
    /// OLD row (for UPDATE/DELETE)
    pub old: Option<Row>,
    /// NEW row (for INSERT/UPDATE)
    pub new: Option<Row>,
    /// Table name
    pub table: String,
    /// Event that triggered
    pub event: TriggerEvent,
    /// Transaction ID
    pub txn_id: Option<u64>,
}

impl TriggerContext {
    pub fn for_insert(table: String, new: Row) -> Self {
        Self {
            old: None,
            new: Some(new),
            table,
            event: TriggerEvent::Insert,
            txn_id: None,
        }
    }

    pub fn for_update(table: String, old: Row, new: Row) -> Self {
        Self {
            old: Some(old),
            new: Some(new),
            table,
            event: TriggerEvent::Update,
            txn_id: None,
        }
    }

    pub fn for_delete(table: String, old: Row) -> Self {
        Self {
            old: Some(old),
            new: None,
            table,
            event: TriggerEvent::Delete,
            txn_id: None,
        }
    }

    pub fn for_truncate(table: String) -> Self {
        Self {
            old: None,
            new: None,
            table,
            event: TriggerEvent::Truncate,
            txn_id: None,
        }
    }
}

/// Trigger function result
#[derive(Debug, Clone)]
pub enum TriggerResult {
    /// Proceed with the operation
    Proceed,
    /// Skip the operation (for BEFORE triggers)
    Skip,
    /// Modified NEW row (for BEFORE INSERT/UPDATE)
    Modified(Row),
    /// Error
    Error(String),
}

/// Trigger function trait
pub trait TriggerFunction: Send + Sync {
    /// Execute the trigger function
    fn execute(&self, context: &TriggerContext) -> Result<TriggerResult>;
}

/// Simple trigger function that just logs
pub struct LogTriggerFunction {
    pub message: String,
}

impl TriggerFunction for LogTriggerFunction {
    fn execute(&self, context: &TriggerContext) -> Result<TriggerResult> {
        println!(
            "[TRIGGER] {}: {:?} on {}",
            self.message, context.event, context.table
        );
        Ok(TriggerResult::Proceed)
    }
}

/// Timestamp update trigger function
pub struct UpdateTimestampFunction {
    pub column: String,
}

impl TriggerFunction for UpdateTimestampFunction {
    fn execute(&self, context: &TriggerContext) -> Result<TriggerResult> {
        if let Some(mut new_row) = context.new.clone() {
            // Find the column index
            if let Some(idx) = new_row.columns.iter().position(|c| c == &self.column) {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                new_row.values[idx] = Value::Integer(timestamp as i64);
                return Ok(TriggerResult::Modified(new_row));
            }
        }
        Ok(TriggerResult::Proceed)
    }
}

/// Audit log trigger function
pub struct AuditLogFunction {
    pub audit_table: String,
}

impl TriggerFunction for AuditLogFunction {
    fn execute(&self, context: &TriggerContext) -> Result<TriggerResult> {
        // In a real implementation, this would insert into the audit table
        println!(
            "[AUDIT] {:?} on {} - Old: {:?}, New: {:?}",
            context.event, context.table, context.old, context.new
        );
        Ok(TriggerResult::Proceed)
    }
}

/// Trigger Manager
pub struct TriggerManager {
    /// All triggers indexed by table name
    triggers: Arc<Mutex<HashMap<String, Vec<Trigger>>>>,
    /// Registered trigger functions
    functions: Arc<Mutex<HashMap<String, Arc<dyn TriggerFunction>>>>,
}

impl TriggerManager {
    pub fn new() -> Self {
        Self {
            triggers: Arc::new(Mutex::new(HashMap::new())),
            functions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a trigger function
    pub fn register_function(&self, name: String, function: Arc<dyn TriggerFunction>) {
        let mut functions = self.functions.lock().unwrap();
        functions.insert(name, function);
    }

    /// Create a trigger
    pub fn create_trigger(&self, trigger: Trigger) -> Result<()> {
        let table = trigger.table.clone();
        let mut triggers = self.triggers.lock().unwrap();

        // Check if trigger already exists
        if let Some(table_triggers) = triggers.get(&table) {
            if table_triggers.iter().any(|t| t.name == trigger.name) {
                return Err(NexusError::QueryError(format!(
                    "Trigger {} already exists",
                    trigger.name
                )));
            }
        }

        // Add trigger
        triggers.entry(table).or_insert_with(Vec::new).push(trigger);

        Ok(())
    }

    /// Drop a trigger
    pub fn drop_trigger(&self, table: &str, trigger_name: &str) -> Result<()> {
        let mut triggers = self.triggers.lock().unwrap();

        if let Some(table_triggers) = triggers.get_mut(table) {
            if let Some(pos) = table_triggers.iter().position(|t| t.name == trigger_name) {
                table_triggers.remove(pos);
                return Ok(());
            }
        }

        Err(NexusError::QueryError(format!(
            "Trigger {} does not exist on table {}",
            trigger_name, table
        )))
    }

    /// Enable a trigger
    pub fn enable_trigger(&self, table: &str, trigger_name: &str) -> Result<()> {
        let mut triggers = self.triggers.lock().unwrap();

        if let Some(table_triggers) = triggers.get_mut(table) {
            if let Some(trigger) = table_triggers.iter_mut().find(|t| t.name == trigger_name) {
                trigger.enabled = true;
                return Ok(());
            }
        }

        Err(NexusError::QueryError(format!(
            "Trigger {} does not exist",
            trigger_name
        )))
    }

    /// Disable a trigger
    pub fn disable_trigger(&self, table: &str, trigger_name: &str) -> Result<()> {
        let mut triggers = self.triggers.lock().unwrap();

        if let Some(table_triggers) = triggers.get_mut(table) {
            if let Some(trigger) = table_triggers.iter_mut().find(|t| t.name == trigger_name) {
                trigger.enabled = false;
                return Ok(());
            }
        }

        Err(NexusError::QueryError(format!(
            "Trigger {} does not exist",
            trigger_name
        )))
    }

    /// Fire triggers for an event
    pub fn fire_triggers(
        &self,
        table: &str,
        timing: TriggerTiming,
        context: &TriggerContext,
    ) -> Result<TriggerResult> {
        let triggers = self.triggers.lock().unwrap();
        let functions = self.functions.lock().unwrap();

        // Get triggers for this table
        let table_triggers = match triggers.get(table) {
            Some(t) => t,
            None => return Ok(TriggerResult::Proceed),
        };

        // Filter triggers by timing and event
        let mut applicable: Vec<_> = table_triggers
            .iter()
            .filter(|t| t.timing == timing && t.fires_for(context.event))
            .collect();

        // Sort by priority
        applicable.sort_by_key(|t| t.priority);

        // Execute triggers
        let mut result = TriggerResult::Proceed;
        for trigger in applicable {
            // Get the function
            let function = match functions.get(&trigger.function_name) {
                Some(f) => f,
                None => {
                    return Err(NexusError::QueryError(format!(
                        "Trigger function {} not found",
                        trigger.function_name
                    )))
                }
            };

            // Execute the function
            match function.execute(context)? {
                TriggerResult::Skip => {
                    // BEFORE trigger can skip the operation
                    if timing == TriggerTiming::Before {
                        return Ok(TriggerResult::Skip);
                    }
                }
                TriggerResult::Modified(new_row) => {
                    // BEFORE trigger can modify the row
                    if timing == TriggerTiming::Before {
                        result = TriggerResult::Modified(new_row.clone());
                        // Update context for next trigger
                        // Note: In real implementation, we'd need to pass modified context
                    }
                }
                TriggerResult::Error(msg) => {
                    return Err(NexusError::QueryError(format!("Trigger error: {}", msg)))
                }
                TriggerResult::Proceed => {}
            }
        }

        Ok(result)
    }

    /// Get all triggers for a table
    pub fn get_triggers(&self, table: &str) -> Vec<Trigger> {
        let triggers = self.triggers.lock().unwrap();
        triggers.get(table).map(|v| v.clone()).unwrap_or_default()
    }

    /// Get trigger count
    pub fn trigger_count(&self) -> usize {
        let triggers = self.triggers.lock().unwrap();
        triggers.values().map(|v| v.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigger_creation() {
        let tm = TriggerManager::new();

        let trigger = Trigger::new(
            "update_timestamp".to_string(),
            "users".to_string(),
            TriggerTiming::Before,
            vec![TriggerEvent::Update],
            TriggerLevel::Row,
            "update_ts".to_string(),
        );

        assert!(tm.create_trigger(trigger).is_ok());
        assert_eq!(tm.trigger_count(), 1);
    }

    #[test]
    fn test_trigger_firing() {
        let tm = TriggerManager::new();

        // Register function
        let func = Arc::new(LogTriggerFunction {
            message: "Test trigger".to_string(),
        });
        tm.register_function("log_func".to_string(), func);

        // Create trigger
        let trigger = Trigger::new(
            "log_insert".to_string(),
            "users".to_string(),
            TriggerTiming::After,
            vec![TriggerEvent::Insert],
            TriggerLevel::Row,
            "log_func".to_string(),
        );
        tm.create_trigger(trigger).unwrap();

        // Fire trigger
        let row = Row {
            columns: vec!["id".to_string(), "name".to_string()],
            values: vec![Value::Integer(1), Value::Text("Alice".to_string())],
        };
        let context = TriggerContext::for_insert("users".to_string(), row);

        let result = tm
            .fire_triggers("users", TriggerTiming::After, &context)
            .unwrap();
        assert!(matches!(result, TriggerResult::Proceed));
    }

    #[test]
    fn test_update_timestamp_trigger() {
        let tm = TriggerManager::new();

        // Register timestamp function
        let func = Arc::new(UpdateTimestampFunction {
            column: "updated_at".to_string(),
        });
        tm.register_function("update_ts".to_string(), func);

        // Create trigger
        let trigger = Trigger::new(
            "auto_timestamp".to_string(),
            "users".to_string(),
            TriggerTiming::Before,
            vec![TriggerEvent::Update],
            TriggerLevel::Row,
            "update_ts".to_string(),
        );
        tm.create_trigger(trigger).unwrap();

        // Fire trigger
        let old_row = Row {
            columns: vec![
                "id".to_string(),
                "name".to_string(),
                "updated_at".to_string(),
            ],
            values: vec![
                Value::Integer(1),
                Value::Text("Alice".to_string()),
                Value::Integer(1000),
            ],
        };
        let new_row = Row {
            columns: vec![
                "id".to_string(),
                "name".to_string(),
                "updated_at".to_string(),
            ],
            values: vec![
                Value::Integer(1),
                Value::Text("Alice Updated".to_string()),
                Value::Integer(1000),
            ],
        };
        let context = TriggerContext::for_update("users".to_string(), old_row, new_row);

        let result = tm
            .fire_triggers("users", TriggerTiming::Before, &context)
            .unwrap();

        // Should get modified row with updated timestamp
        match result {
            TriggerResult::Modified(row) => {
                assert!(matches!(row.values[2], Value::Integer(ts) if ts > 1000));
            }
            _ => panic!("Expected Modified result"),
        }
    }
}
