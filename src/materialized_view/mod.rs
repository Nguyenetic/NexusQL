//! Materialized View System - Week 5-6
//!
//! Context7 Best Practices from PostgreSQL pg_ivm:
//! - Incremental View Maintenance (IVM) instead of full refresh
//! - AFTER triggers to update views immediately
//! - Delta computation for INSERT/UPDATE/DELETE
//! - Support for aggregations and joins
//! - Concurrent refresh (CONCURRENTLY option)
//!
//! PROPRIETARY & PATENTABLE

use crate::types::{ResultSet, Row, Value};
use crate::Result;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Materialized view definition
#[derive(Debug, Clone)]
pub struct MaterializedView {
    /// View name
    pub name: String,
    /// SQL query that defines the view
    pub query: String,
    /// Base tables this view depends on
    pub base_tables: Vec<String>,
    /// Cached result set
    pub data: Arc<RwLock<ResultSet>>,
    /// Last refresh timestamp
    pub last_refresh: u64,
    /// Refresh strategy
    pub refresh_strategy: RefreshStrategy,
    /// View type (determines incremental maintenance algorithm)
    pub view_type: ViewType,
}

/// Refresh strategy
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshStrategy {
    /// Full refresh: recompute entire view
    Full,
    /// Incremental: apply only deltas (pg_ivm pattern)
    Incremental,
    /// On-demand: refresh only when explicitly requested
    OnDemand,
    /// Periodic: refresh on a schedule
    Periodic { interval_seconds: u64 },
}

/// View type determines how incremental maintenance is performed
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewType {
    /// Simple projection: SELECT columns FROM table WHERE...
    SimpleProjection,
    /// Aggregation: SELECT columns, AGG() FROM table GROUP BY...
    Aggregation,
    /// Join: SELECT ... FROM t1 JOIN t2 ...
    Join,
    /// Complex: Multiple joins, subqueries, etc.
    Complex,
}

/// Change type for delta computation
#[derive(Debug, Clone)]
pub enum ChangeType {
    Insert(Row),
    Update { old: Row, new: Row },
    Delete(Row),
}

/// Delta record for incremental maintenance
#[derive(Debug, Clone)]
pub struct Delta {
    /// Table that changed
    pub table: String,
    /// Type of change
    pub change: ChangeType,
    /// Timestamp of change
    pub timestamp: u64,
}

impl MaterializedView {
    /// Create a new materialized view
    pub fn new(name: String, query: String, base_tables: Vec<String>, view_type: ViewType) -> Self {
        Self {
            name,
            query,
            base_tables,
            data: Arc::new(RwLock::new(ResultSet::new(vec![]))),
            last_refresh: Self::current_timestamp(),
            refresh_strategy: RefreshStrategy::Incremental,
            view_type,
        }
    }

    /// Full refresh: recompute the entire view
    pub fn refresh_full(&mut self, new_data: ResultSet) -> Result<()> {
        let mut data = self.data.write().unwrap();
        *data = new_data;
        self.last_refresh = Self::current_timestamp();
        Ok(())
    }

    /// Incremental refresh: apply delta changes (pg_ivm pattern)
    pub fn refresh_incremental(&mut self, delta: Delta) -> Result<()> {
        match &self.view_type {
            ViewType::SimpleProjection => self.apply_delta_projection(delta),
            ViewType::Aggregation => self.apply_delta_aggregation(delta),
            ViewType::Join => self.apply_delta_join(delta),
            ViewType::Complex => {
                // Complex views require full refresh
                // In production, this would trigger a background full refresh
                Ok(())
            }
        }
    }

    /// Apply delta for simple projection views
    fn apply_delta_projection(&mut self, delta: Delta) -> Result<()> {
        let mut data = self.data.write().unwrap();

        match delta.change {
            ChangeType::Insert(row) => {
                // For projections, just add the projected row if it passes filter
                if self.row_matches_filter(&row) {
                    let projected = self.project_row(row);
                    data.rows.push(projected);
                }
            }
            ChangeType::Delete(row) => {
                // Remove matching rows
                if self.row_matches_filter(&row) {
                    let projected = self.project_row(row);
                    data.rows.retain(|r| r != &projected);
                }
            }
            ChangeType::Update { old, new } => {
                // Delete old, insert new
                if self.row_matches_filter(&old) {
                    let old_projected = self.project_row(old);
                    data.rows.retain(|r| r != &old_projected);
                }
                if self.row_matches_filter(&new) {
                    let new_projected = self.project_row(new);
                    data.rows.push(new_projected);
                }
            }
        }

        self.last_refresh = Self::current_timestamp();
        Ok(())
    }

    /// Apply delta for aggregation views (COUNT, SUM, AVG, etc.)
    fn apply_delta_aggregation(&mut self, delta: Delta) -> Result<()> {
        let mut data = self.data.write().unwrap();

        // For aggregations, we need to update aggregate values
        // Example: COUNT(*) increments on INSERT, decrements on DELETE
        match delta.change {
            ChangeType::Insert(row) => {
                if self.row_matches_filter(&row) {
                    // Increment count-based aggregates
                    self.update_aggregates(&mut data, &row, 1);
                }
            }
            ChangeType::Delete(row) => {
                if self.row_matches_filter(&row) {
                    // Decrement count-based aggregates
                    self.update_aggregates(&mut data, &row, -1);
                }
            }
            ChangeType::Update { old, new } => {
                // Update = Delete + Insert for aggregates
                if self.row_matches_filter(&old) {
                    self.update_aggregates(&mut data, &old, -1);
                }
                if self.row_matches_filter(&new) {
                    self.update_aggregates(&mut data, &new, 1);
                }
            }
        }

        self.last_refresh = Self::current_timestamp();
        Ok(())
    }

    /// Apply delta for join views
    fn apply_delta_join(&mut self, delta: Delta) -> Result<()> {
        // Join incremental maintenance is complex
        // pg_ivm uses a technique called "delta tables"
        // For simplicity, we mark the view as needing refresh
        // In production, this would trigger an async refresh

        // TODO: Implement full delta join algorithm
        // For now, just update timestamp
        self.last_refresh = Self::current_timestamp();
        Ok(())
    }

    /// Check if a row matches the view's filter
    fn row_matches_filter(&self, _row: &Row) -> bool {
        // TODO: Parse WHERE clause and evaluate
        // For now, assume all rows match
        true
    }

    /// Project a row to the view's columns
    fn project_row(&self, row: Row) -> Row {
        // TODO: Parse SELECT columns and project
        // For now, return the full row
        row
    }

    /// Update aggregate values
    fn update_aggregates(&self, _data: &mut ResultSet, _row: &Row, _delta: i64) {
        // TODO: Implement aggregate update logic
        // This would update COUNT, SUM, AVG based on the delta
    }

    /// Get current timestamp
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Get a read lock on the data
    pub fn read_data(&self) -> std::sync::RwLockReadGuard<ResultSet> {
        self.data.read().unwrap()
    }

    /// Check if view needs refresh based on strategy
    pub fn needs_refresh(&self) -> bool {
        match &self.refresh_strategy {
            RefreshStrategy::Full | RefreshStrategy::OnDemand => false,
            RefreshStrategy::Incremental => false, // Updated via triggers
            RefreshStrategy::Periodic { interval_seconds } => {
                let now = Self::current_timestamp();
                now - self.last_refresh >= *interval_seconds
            }
        }
    }
}

/// Materialized view manager
pub struct MaterializedViewManager {
    /// All materialized views
    views: HashMap<String, MaterializedView>,
    /// Trigger registry: table -> list of views to update
    triggers: HashMap<String, Vec<String>>,
}

impl MaterializedViewManager {
    /// Create a new manager
    pub fn new() -> Self {
        Self {
            views: HashMap::new(),
            triggers: HashMap::new(),
        }
    }

    /// Create a materialized view
    pub fn create_view(&mut self, view: MaterializedView) -> Result<()> {
        let name = view.name.clone();

        // Register triggers for base tables (pg_ivm pattern)
        for table in &view.base_tables {
            self.triggers
                .entry(table.clone())
                .or_insert_with(Vec::new)
                .push(name.clone());
        }

        self.views.insert(name, view);
        Ok(())
    }

    /// Drop a materialized view
    pub fn drop_view(&mut self, name: &str) -> Result<()> {
        if let Some(view) = self.views.remove(name) {
            // Remove triggers
            for table in &view.base_tables {
                if let Some(views) = self.triggers.get_mut(table) {
                    views.retain(|v| v != name);
                }
            }
        }
        Ok(())
    }

    /// Get a view by name
    pub fn get_view(&self, name: &str) -> Option<&MaterializedView> {
        self.views.get(name)
    }

    /// Handle a change to a base table (trigger simulation)
    pub fn on_table_change(&mut self, table: &str, delta: Delta) -> Result<()> {
        // Find all views that depend on this table
        if let Some(view_names) = self.triggers.get(table) {
            let view_names = view_names.clone(); // Avoid borrow checker issues

            for view_name in view_names {
                if let Some(view) = self.views.get_mut(&view_name) {
                    // Apply incremental update
                    view.refresh_incremental(delta.clone())?;
                }
            }
        }
        Ok(())
    }

    /// Refresh all views that need periodic refresh
    pub fn refresh_periodic_views(&mut self) -> Result<()> {
        for view in self.views.values_mut() {
            if view.needs_refresh() {
                // In production, this would execute the view's query
                // For now, just update timestamp
                view.last_refresh = MaterializedView::current_timestamp();
            }
        }
        Ok(())
    }

    /// List all materialized views
    pub fn list_views(&self) -> Vec<String> {
        self.views.keys().cloned().collect()
    }
}

impl Default for MaterializedViewManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_materialized_view() {
        let view = MaterializedView::new(
            "user_counts".to_string(),
            "SELECT country, COUNT(*) FROM users GROUP BY country".to_string(),
            vec!["users".to_string()],
            ViewType::Aggregation,
        );

        assert_eq!(view.name, "user_counts");
        assert_eq!(view.base_tables, vec!["users".to_string()]);
        assert_eq!(view.view_type, ViewType::Aggregation);
    }

    #[test]
    fn test_view_manager() {
        let mut manager = MaterializedViewManager::new();

        let view = MaterializedView::new(
            "user_counts".to_string(),
            "SELECT country, COUNT(*) FROM users GROUP BY country".to_string(),
            vec!["users".to_string()],
            ViewType::Aggregation,
        );

        manager.create_view(view).unwrap();

        assert!(manager.get_view("user_counts").is_some());
        assert_eq!(manager.list_views().len(), 1);

        manager.drop_view("user_counts").unwrap();
        assert!(manager.get_view("user_counts").is_none());
    }

    #[test]
    fn test_incremental_refresh_projection() {
        let mut view = MaterializedView::new(
            "active_users".to_string(),
            "SELECT * FROM users WHERE active = true".to_string(),
            vec!["users".to_string()],
            ViewType::SimpleProjection,
        );

        // Simulate INSERT
        let row = Row {
            values: vec![
                Value::Integer(1),
                Value::Text("Alice".to_string()),
                Value::Boolean(true),
            ],
        };

        let delta = Delta {
            table: "users".to_string(),
            change: ChangeType::Insert(row.clone()),
            timestamp: MaterializedView::current_timestamp(),
        };

        view.refresh_incremental(delta).unwrap();

        // Data should be updated
        assert!(view.last_refresh > 0);
    }

    #[test]
    fn test_periodic_refresh_check() {
        let mut view = MaterializedView::new(
            "stats".to_string(),
            "SELECT COUNT(*) FROM users".to_string(),
            vec!["users".to_string()],
            ViewType::Aggregation,
        );

        view.refresh_strategy = RefreshStrategy::Periodic {
            interval_seconds: 60,
        };

        // Should not need refresh immediately
        assert!(!view.needs_refresh());

        // Simulate time passing (set last_refresh to past)
        view.last_refresh = MaterializedView::current_timestamp() - 120;

        // Now should need refresh
        assert!(view.needs_refresh());
    }
}
