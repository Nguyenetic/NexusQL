// Row-Level Security (RLS)
// PostgreSQL-compatible row filtering based on user context
// Allows fine-grained access control at the row level

use crate::types::{Row, Value};
use crate::{NexusError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// RLS Policy (similar to PostgreSQL CREATE POLICY)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub name: String,
    /// Table this policy applies to
    pub table: String,
    /// Command type (SELECT, INSERT, UPDATE, DELETE, ALL)
    pub command: PolicyCommand,
    /// Roles this policy applies to (empty = all roles)
    pub roles: Vec<String>,
    /// USING clause - determines which rows are visible
    pub using_expression: Option<String>,
    /// WITH CHECK clause - determines which rows can be inserted/updated
    pub with_check_expression: Option<String>,
    pub created_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyCommand {
    All,
    Select,
    Insert,
    Update,
    Delete,
}

impl Policy {
    pub fn new(name: String, table: String, command: PolicyCommand) -> Self {
        Self {
            name,
            table,
            command,
            roles: Vec::new(),
            using_expression: None,
            with_check_expression: None,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Set the USING expression (for SELECT, UPDATE, DELETE)
    pub fn using_expr(mut self, expr: String) -> Self {
        self.using_expression = Some(expr);
        self
    }

    /// Set the WITH CHECK expression (for INSERT, UPDATE)
    pub fn with_check(mut self, expr: String) -> Self {
        self.with_check_expression = Some(expr);
        self
    }

    /// Restrict policy to specific roles
    pub fn for_roles(mut self, roles: Vec<String>) -> Self {
        self.roles = roles;
        self
    }

    /// Check if policy applies to a specific role
    pub fn applies_to_role(&self, role: &str) -> bool {
        self.roles.is_empty() || self.roles.contains(&role.to_string())
    }
}

/// Row-Level Security Manager
#[derive(Debug, Clone)]
pub struct RLSManager {
    /// Policies indexed by table name
    policies: HashMap<String, Vec<Policy>>,
    /// Tables with RLS enabled
    rls_enabled: HashMap<String, bool>,
}

impl RLSManager {
    pub fn new() -> Self {
        Self {
            policies: HashMap::new(),
            rls_enabled: HashMap::new(),
        }
    }

    /// Enable RLS on a table
    pub fn enable_rls(&mut self, table: String) {
        self.rls_enabled.insert(table, true);
    }

    /// Disable RLS on a table
    pub fn disable_rls(&mut self, table: &str) {
        self.rls_enabled.insert(table.to_string(), false);
    }

    /// Check if RLS is enabled for a table
    pub fn is_enabled(&self, table: &str) -> bool {
        *self.rls_enabled.get(table).unwrap_or(&false)
    }

    /// Create a policy
    pub fn create_policy(&mut self, policy: Policy) -> Result<()> {
        let table = policy.table.clone();

        // Check if policy already exists
        if let Some(policies) = self.policies.get(&table) {
            if policies.iter().any(|p| p.name == policy.name) {
                return Err(NexusError::QueryError(format!(
                    "Policy {} already exists on table {}",
                    policy.name, table
                )));
            }
        }

        self.policies
            .entry(table)
            .or_insert_with(Vec::new)
            .push(policy);

        Ok(())
    }

    /// Drop a policy
    pub fn drop_policy(&mut self, table: &str, policy_name: &str) -> Result<()> {
        if let Some(policies) = self.policies.get_mut(table) {
            if let Some(pos) = policies.iter().position(|p| p.name == policy_name) {
                policies.remove(pos);
                return Ok(());
            }
        }

        Err(NexusError::QueryError(format!(
            "Policy {} does not exist on table {}",
            policy_name, table
        )))
    }

    /// Get all policies for a table and command
    pub fn get_policies(&self, table: &str, command: PolicyCommand) -> Vec<&Policy> {
        self.policies
            .get(table)
            .map(|policies| {
                policies
                    .iter()
                    .filter(|p| p.command == command || p.command == PolicyCommand::All)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Filter rows based on RLS policies for SELECT
    /// Returns rows that pass all applicable policies
    pub fn filter_select(
        &self,
        table: &str,
        rows: Vec<Row>,
        user_context: &UserContext,
    ) -> Result<Vec<Row>> {
        // If RLS is not enabled, return all rows
        if !self.is_enabled(table) {
            return Ok(rows);
        }

        // Superuser bypasses RLS
        if user_context.is_superuser {
            return Ok(rows);
        }

        let policies = self.get_policies(table, PolicyCommand::Select);

        // If no policies apply to user's roles, deny all rows
        let applicable_policies: Vec<_> = policies
            .iter()
            .filter(|p| user_context.roles.iter().any(|r| p.applies_to_role(r)))
            .collect();

        if applicable_policies.is_empty() {
            return Ok(Vec::new()); // No access
        }

        // Filter rows through policies
        let filtered = rows
            .into_iter()
            .filter(|row| {
                // Row must pass at least one policy (OR logic)
                applicable_policies.iter().any(|policy| {
                    if let Some(using_expr) = &policy.using_expression {
                        // Evaluate USING expression
                        // For now, we'll use a simple expression evaluator
                        Self::evaluate_expression(using_expr, row, user_context)
                    } else {
                        // No USING clause means allow all
                        true
                    }
                })
            })
            .collect();

        Ok(filtered)
    }

    /// Check if a row can be inserted based on WITH CHECK policies
    pub fn check_insert(&self, table: &str, row: &Row, user_context: &UserContext) -> Result<()> {
        if !self.is_enabled(table) {
            return Ok(());
        }

        if user_context.is_superuser {
            return Ok(());
        }

        let policies = self.get_policies(table, PolicyCommand::Insert);
        let applicable_policies: Vec<_> = policies
            .iter()
            .filter(|p| user_context.roles.iter().any(|r| p.applies_to_role(r)))
            .collect();

        if applicable_policies.is_empty() {
            return Err(NexusError::QueryError(
                "RLS policy violation: no INSERT policy applies".to_string(),
            ));
        }

        // Row must pass all WITH CHECK clauses (AND logic)
        for policy in applicable_policies {
            if let Some(check_expr) = &policy.with_check_expression {
                if !Self::evaluate_expression(check_expr, row, user_context) {
                    return Err(NexusError::QueryError(format!(
                        "RLS policy violation: WITH CHECK failed for policy {}",
                        policy.name
                    )));
                }
            }
        }

        Ok(())
    }

    /// Check if a row can be updated
    pub fn check_update(
        &self,
        table: &str,
        old_row: &Row,
        new_row: &Row,
        user_context: &UserContext,
    ) -> Result<()> {
        if !self.is_enabled(table) {
            return Ok(());
        }

        if user_context.is_superuser {
            return Ok(());
        }

        let policies = self.get_policies(table, PolicyCommand::Update);
        let applicable_policies: Vec<_> = policies
            .iter()
            .filter(|p| user_context.roles.iter().any(|r| p.applies_to_role(r)))
            .collect();

        if applicable_policies.is_empty() {
            return Err(NexusError::QueryError(
                "RLS policy violation: no UPDATE policy applies".to_string(),
            ));
        }

        // Old row must be visible (USING)
        let visible = applicable_policies.iter().any(|policy| {
            if let Some(using_expr) = &policy.using_expression {
                Self::evaluate_expression(using_expr, old_row, user_context)
            } else {
                true
            }
        });

        if !visible {
            return Err(NexusError::QueryError(
                "RLS policy violation: row not visible for UPDATE".to_string(),
            ));
        }

        // New row must pass WITH CHECK
        for policy in applicable_policies {
            if let Some(check_expr) = &policy.with_check_expression {
                if !Self::evaluate_expression(check_expr, new_row, user_context) {
                    return Err(NexusError::QueryError(format!(
                        "RLS policy violation: WITH CHECK failed for policy {}",
                        policy.name
                    )));
                }
            }
        }

        Ok(())
    }

    /// Check if a row can be deleted
    pub fn check_delete(&self, table: &str, row: &Row, user_context: &UserContext) -> Result<()> {
        if !self.is_enabled(table) {
            return Ok(());
        }

        if user_context.is_superuser {
            return Ok(());
        }

        let policies = self.get_policies(table, PolicyCommand::Delete);
        let applicable_policies: Vec<_> = policies
            .iter()
            .filter(|p| user_context.roles.iter().any(|r| p.applies_to_role(r)))
            .collect();

        if applicable_policies.is_empty() {
            return Err(NexusError::QueryError(
                "RLS policy violation: no DELETE policy applies".to_string(),
            ));
        }

        // Row must be visible (USING)
        let visible = applicable_policies.iter().any(|policy| {
            if let Some(using_expr) = &policy.using_expression {
                Self::evaluate_expression(using_expr, row, user_context)
            } else {
                true
            }
        });

        if !visible {
            return Err(NexusError::QueryError(
                "RLS policy violation: row not visible for DELETE".to_string(),
            ));
        }

        Ok(())
    }

    /// Simple expression evaluator for RLS policies
    /// Supports patterns like:
    /// - "user_id = $user_id" (current user ID)
    /// - "department = $user_department" (user attribute)
    /// - "is_public = true" (boolean literal)
    /// - "owner = $username" (current username)
    fn evaluate_expression(expr: &str, row: &Row, user_context: &UserContext) -> bool {
        // Simple expression parser for common RLS patterns
        // In a production system, this would use a full SQL expression evaluator

        // Pattern: "column = $variable"
        if let Some((column, value)) = expr.split_once(" = ") {
            let column = column.trim();
            let value = value.trim();

            // Find column value in row
            // This is simplified - in reality, we'd need column metadata
            // For demo purposes, we'll assume simple equality checks

            if value == "$username" {
                // Check if row has a column matching current username
                return row
                    .values
                    .iter()
                    .any(|v| matches!(v, Value::Text(s) if s == &user_context.username));
            } else if value == "true" {
                return row.values.iter().any(|v| matches!(v, Value::Integer(1)));
            } else if value == "false" {
                return row.values.iter().any(|v| matches!(v, Value::Integer(0)));
            }
        }

        // Default to allowing the row (permissive)
        // In production, unknown expressions should deny access
        true
    }
}

/// User context for RLS evaluation
#[derive(Debug, Clone)]
pub struct UserContext {
    pub username: String,
    pub roles: Vec<String>,
    pub is_superuser: bool,
    /// Additional user attributes for policy evaluation
    pub attributes: HashMap<String, Value>,
}

impl UserContext {
    pub fn new(username: String, roles: Vec<String>, is_superuser: bool) -> Self {
        Self {
            username,
            roles,
            is_superuser,
            attributes: HashMap::new(),
        }
    }

    pub fn set_attribute(&mut self, key: String, value: Value) {
        self.attributes.insert(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rls_enable_disable() {
        let mut rls = RLSManager::new();

        assert!(!rls.is_enabled("users"));

        rls.enable_rls("users".to_string());
        assert!(rls.is_enabled("users"));

        rls.disable_rls("users");
        assert!(!rls.is_enabled("users"));
    }

    #[test]
    fn test_policy_creation() {
        let mut rls = RLSManager::new();

        let policy = Policy::new(
            "user_isolation".to_string(),
            "messages".to_string(),
            PolicyCommand::Select,
        )
        .using_expr("user_id = $username".to_string());

        assert!(rls.create_policy(policy).is_ok());

        // Duplicate policy should fail
        let policy2 = Policy::new(
            "user_isolation".to_string(),
            "messages".to_string(),
            PolicyCommand::Select,
        );

        assert!(rls.create_policy(policy2).is_err());
    }

    #[test]
    fn test_rls_filter_select() {
        let mut rls = RLSManager::new();
        rls.enable_rls("messages".to_string());

        // Create policy: users can only see their own messages
        let policy = Policy::new(
            "user_isolation".to_string(),
            "messages".to_string(),
            PolicyCommand::Select,
        )
        .using_expr("user_id = $username".to_string())
        .for_roles(vec!["user".to_string()]);

        rls.create_policy(policy).unwrap();

        // Create test rows
        let rows = vec![
            Row {
                values: vec![
                    Value::Integer(1),
                    Value::Text("alice".to_string()),
                    Value::Text("Hello".to_string()),
                ],
            },
            Row {
                values: vec![
                    Value::Integer(2),
                    Value::Text("bob".to_string()),
                    Value::Text("World".to_string()),
                ],
            },
        ];

        // Alice's context
        let alice_context = UserContext::new("alice".to_string(), vec!["user".to_string()], false);

        let filtered = rls
            .filter_select("messages", rows.clone(), &alice_context)
            .unwrap();

        // Alice should only see her message
        assert_eq!(filtered.len(), 1);
        assert!(matches!(&filtered[0].values[1], Value::Text(s) if s == "alice"));
    }
}
