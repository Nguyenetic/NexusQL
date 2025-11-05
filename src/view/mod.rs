// View System
// Week 10: Database Convenience Features
// PostgreSQL-compatible views with query rewriting and updateable views

use crate::types::Row;
use crate::{NexusError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// View definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct View {
    pub name: String,
    /// SQL query that defines the view
    pub query: String,
    /// Column names (if specified)
    pub columns: Option<Vec<String>>,
    /// Is this view updateable?
    pub updateable: bool,
    /// WITH CHECK OPTION (for updateable views)
    pub with_check_option: bool,
    /// Materialized view (cached results)
    pub materialized: bool,
    /// Dependencies (tables/views this view depends on)
    pub dependencies: Vec<String>,
    pub created_at: u64,
}

impl View {
    pub fn new(name: String, query: String) -> Self {
        Self {
            name,
            query,
            columns: None,
            updateable: false,
            with_check_option: false,
            materialized: false,
            dependencies: Vec::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Set column names
    pub fn with_columns(mut self, columns: Vec<String>) -> Self {
        self.columns = Some(columns);
        self
    }

    /// Mark as updateable
    pub fn updateable(mut self) -> Self {
        self.updateable = true;
        self
    }

    /// Add WITH CHECK OPTION
    pub fn with_check_option(mut self) -> Self {
        self.with_check_option = true;
        self
    }

    /// Mark as materialized
    pub fn materialized(mut self) -> Self {
        self.materialized = true;
        self
    }

    /// Add dependency
    pub fn add_dependency(mut self, table: String) -> Self {
        if !self.dependencies.contains(&table) {
            self.dependencies.push(table);
        }
        self
    }
}

/// View Manager
pub struct ViewManager {
    /// All views indexed by name
    views: Arc<Mutex<HashMap<String, View>>>,
    /// Materialized view data cache
    materialized_data: Arc<Mutex<HashMap<String, Vec<Row>>>>,
}

impl ViewManager {
    pub fn new() -> Self {
        Self {
            views: Arc::new(Mutex::new(HashMap::new())),
            materialized_data: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a view
    pub fn create_view(&self, view: View) -> Result<()> {
        let mut views = self.views.lock().unwrap();

        if views.contains_key(&view.name) {
            return Err(NexusError::QueryError(format!(
                "View {} already exists",
                view.name
            )));
        }

        views.insert(view.name.clone(), view);
        Ok(())
    }

    /// Drop a view
    pub fn drop_view(&self, name: &str, cascade: bool) -> Result<()> {
        let mut views = self.views.lock().unwrap();

        if !views.contains_key(name) {
            return Err(NexusError::QueryError(format!(
                "View {} does not exist",
                name
            )));
        }

        // Check for dependent views
        if !cascade {
            for view in views.values() {
                if view.dependencies.contains(&name.to_string()) {
                    return Err(NexusError::QueryError(format!(
                        "Cannot drop view {}: view {} depends on it (use CASCADE)",
                        name, view.name
                    )));
                }
            }
        }

        // Remove the view
        views.remove(name);

        // Remove materialized data if exists
        let mut mat_data = self.materialized_data.lock().unwrap();
        mat_data.remove(name);

        // If cascade, remove dependent views
        if cascade {
            let dependent: Vec<_> = views
                .values()
                .filter(|v| v.dependencies.contains(&name.to_string()))
                .map(|v| v.name.clone())
                .collect();

            for dep in dependent {
                views.remove(&dep);
                mat_data.remove(&dep);
            }
        }

        Ok(())
    }

    /// Get view definition
    pub fn get_view(&self, name: &str) -> Option<View> {
        let views = self.views.lock().unwrap();
        views.get(name).cloned()
    }

    /// Check if a name is a view
    pub fn is_view(&self, name: &str) -> bool {
        let views = self.views.lock().unwrap();
        views.contains_key(name)
    }

    /// Rewrite query to expand views
    pub fn rewrite_query(&self, query: &str) -> Result<String> {
        let views = self.views.lock().unwrap();
        let mut rewritten = query.to_string();

        // Simple view expansion (in production, use a proper SQL parser)
        // This is a simplified example
        for (name, view) in views.iter() {
            if !view.materialized {
                // Replace view references with subquery
                let pattern = format!("FROM {}", name);
                if rewritten.contains(&pattern) {
                    rewritten =
                        rewritten.replace(&pattern, &format!("FROM ({}) AS {}", view.query, name));
                }
            }
        }

        Ok(rewritten)
    }

    /// Refresh a materialized view
    pub fn refresh_materialized_view(&self, name: &str, data: Vec<Row>) -> Result<()> {
        let views = self.views.lock().unwrap();

        if !views.contains_key(name) {
            return Err(NexusError::QueryError(format!(
                "View {} does not exist",
                name
            )));
        }

        let view = views.get(name).unwrap();
        if !view.materialized {
            return Err(NexusError::QueryError(format!(
                "View {} is not materialized",
                name
            )));
        }

        drop(views);

        let mut mat_data = self.materialized_data.lock().unwrap();
        mat_data.insert(name.to_string(), data);

        Ok(())
    }

    /// Get materialized view data
    pub fn get_materialized_data(&self, name: &str) -> Option<Vec<Row>> {
        let mat_data = self.materialized_data.lock().unwrap();
        mat_data.get(name).cloned()
    }

    /// List all views
    pub fn list_views(&self) -> Vec<String> {
        let views = self.views.lock().unwrap();
        views.keys().cloned().collect()
    }

    /// Get view count
    pub fn view_count(&self) -> usize {
        let views = self.views.lock().unwrap();
        views.len()
    }

    /// Check if view is updateable
    pub fn is_updateable(&self, name: &str) -> bool {
        let views = self.views.lock().unwrap();
        views.get(name).map(|v| v.updateable).unwrap_or(false)
    }

    /// Validate update against view's WITH CHECK OPTION
    pub fn validate_update(&self, view_name: &str, _new_row: &Row) -> Result<()> {
        let views = self.views.lock().unwrap();

        if let Some(view) = views.get(view_name) {
            if view.with_check_option {
                // In a real implementation, this would evaluate the view's WHERE clause
                // against the new row to ensure it still matches the view criteria
                // For now, we'll just return Ok
                // TODO: Implement actual check option validation
            }
        }

        Ok(())
    }
}

/// Helper to detect if a view is automatically updateable
/// According to SQL standard, a view is automatically updateable if:
/// - It selects from a single table
/// - It doesn't use DISTINCT, GROUP BY, HAVING, LIMIT, OFFSET
/// - It doesn't use set operations (UNION, INTERSECT, EXCEPT)
/// - All columns are simple column references (not expressions)
pub fn is_auto_updateable(query: &str) -> bool {
    let query_lower = query.to_lowercase();

    // Simple heuristic checks (in production, use proper SQL parsing)
    if query_lower.contains("distinct")
        || query_lower.contains("group by")
        || query_lower.contains("having")
        || query_lower.contains("limit")
        || query_lower.contains("offset")
        || query_lower.contains("union")
        || query_lower.contains("intersect")
        || query_lower.contains("except")
    {
        return false;
    }

    // Check for aggregate functions
    if query_lower.contains("count(")
        || query_lower.contains("sum(")
        || query_lower.contains("avg(")
        || query_lower.contains("min(")
        || query_lower.contains("max(")
    {
        return false;
    }

    // Check for single FROM clause
    let from_count = query_lower.matches("from").count();
    if from_count != 1 {
        return false;
    }

    // Check for joins
    if query_lower.contains("join") {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::PlanCost;
    use crate::types::{ResultSet, Row, Value};

    #[test]
    fn test_view_creation() {
        let vm = ViewManager::new();

        let view = View::new(
            "active_users".to_string(),
            "SELECT * FROM users WHERE status = 'active'".to_string(),
        );

        assert!(vm.create_view(view).is_ok());
        assert_eq!(vm.view_count(), 1);
        assert!(vm.is_view("active_users"));
    }

    #[test]
    fn test_view_query_rewriting() {
        let vm = ViewManager::new();

        let view = View::new(
            "active_users".to_string(),
            "SELECT id, username FROM users WHERE status = 'active'".to_string(),
        );
        vm.create_view(view).unwrap();

        let query = "SELECT * FROM active_users";
        let rewritten = vm.rewrite_query(query).unwrap();

        assert!(rewritten.contains("SELECT id, username FROM users WHERE status = 'active'"));
    }

    #[test]
    fn test_materialized_view() {
        let vm = ViewManager::new();

        let view = View::new(
            "user_summary".to_string(),
            "SELECT COUNT(*) FROM users".to_string(),
        )
        .materialized();

        vm.create_view(view).unwrap();

        // Refresh with data
        let data = vec![Row {
            columns: vec!["count".to_string()],
            values: vec![Value::Integer(100)],
        }];
        vm.refresh_materialized_view("user_summary", data).unwrap();

        // Retrieve data
        let cached = vm.get_materialized_data("user_summary").unwrap();
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].values[0], Value::Integer(100));
    }

    #[test]
    fn test_view_dependencies() {
        let vm = ViewManager::new();

        let base_view = View::new("base".to_string(), "SELECT * FROM users".to_string());
        vm.create_view(base_view).unwrap();

        let dependent_view = View::new(
            "dependent".to_string(),
            "SELECT * FROM base WHERE id > 10".to_string(),
        )
        .add_dependency("base".to_string());
        vm.create_view(dependent_view).unwrap();

        // Cannot drop base without CASCADE
        assert!(vm.drop_view("base", false).is_err());

        // Can drop with CASCADE
        assert!(vm.drop_view("base", true).is_ok());
        assert_eq!(vm.view_count(), 0);
    }

    #[test]
    fn test_auto_updateable_detection() {
        assert!(is_auto_updateable("SELECT id, name FROM users"));
        assert!(is_auto_updateable(
            "SELECT * FROM users WHERE status = 'active'"
        ));

        assert!(!is_auto_updateable("SELECT DISTINCT name FROM users"));
        assert!(!is_auto_updateable("SELECT COUNT(*) FROM users"));
        assert!(!is_auto_updateable("SELECT * FROM users GROUP BY status"));
        assert!(!is_auto_updateable(
            "SELECT * FROM users JOIN orders ON users.id = orders.user_id"
        ));
    }
}
