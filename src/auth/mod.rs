// Authentication & Authorization Module
// Week 8: Database Fundamentals Phase 2
// PostgreSQL-compatible authentication with bcrypt and RBAC

pub mod rls;

use crate::types::Value;
use crate::{NexusError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub use rls::{Policy, PolicyCommand, RLSManager, UserContext};

/// Password hashing using bcrypt (PostgreSQL standard)
pub mod password {
    use super::*;

    /// Hash a password using bcrypt with default cost (12)
    /// PostgreSQL uses bcrypt for password encryption in pg_authid
    pub fn hash_password(password: &str) -> Result<String> {
        bcrypt::hash(password, bcrypt::DEFAULT_COST)
            .map_err(|e| NexusError::QueryError(format!("Password hashing failed: {}", e)))
    }

    /// Verify a password against a bcrypt hash
    pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
        bcrypt::verify(password, hash)
            .map_err(|e| NexusError::QueryError(format!("Password verification failed: {}", e)))
    }
}

/// Database user (similar to PostgreSQL pg_authid)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    /// bcrypt-hashed password
    pub password_hash: String,
    /// Superuser can bypass all permission checks
    pub is_superuser: bool,
    /// Can create databases
    pub can_create_db: bool,
    /// Can create roles
    pub can_create_role: bool,
    /// Can login (vs. group roles)
    pub can_login: bool,
    /// Member of these roles
    pub roles: HashSet<String>,
    /// Unix timestamp when created
    pub created_at: u64,
}

impl User {
    /// Create a new user with hashed password
    pub fn new(username: String, password: &str) -> Result<Self> {
        Ok(Self {
            username,
            password_hash: password::hash_password(password)?,
            is_superuser: false,
            can_create_db: false,
            can_create_role: false,
            can_login: true,
            roles: HashSet::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }

    /// Create a superuser
    pub fn new_superuser(username: String, password: &str) -> Result<Self> {
        let mut user = Self::new(username, password)?;
        user.is_superuser = true;
        user.can_create_db = true;
        user.can_create_role = true;
        Ok(user)
    }

    /// Verify password
    pub fn verify_password(&self, password: &str) -> Result<bool> {
        password::verify_password(password, &self.password_hash)
    }

    /// Grant a role to this user
    pub fn grant_role(&mut self, role: String) {
        self.roles.insert(role);
    }

    /// Revoke a role from this user
    pub fn revoke_role(&mut self, role: &str) {
        self.roles.remove(role);
    }

    /// Check if user has a specific role (directly or inherited)
    pub fn has_role(&self, role: &str, role_registry: &RoleRegistry) -> bool {
        if self.is_superuser {
            return true;
        }

        // Check direct membership
        if self.roles.contains(role) {
            return true;
        }

        // Check inherited roles
        for user_role in &self.roles {
            if let Some(r) = role_registry.get_role(user_role) {
                if r.inherits_from(role, role_registry) {
                    return true;
                }
            }
        }

        false
    }
}

/// Database role (PostgreSQL-compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub name: String,
    /// Roles this role inherits from
    pub inherited_roles: HashSet<String>,
    /// Permissions granted to this role
    pub permissions: HashMap<String, TablePermissions>,
    pub created_at: u64,
}

impl Role {
    pub fn new(name: String) -> Self {
        Self {
            name,
            inherited_roles: HashSet::new(),
            permissions: HashMap::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Grant a permission on a table
    pub fn grant(&mut self, table: String, permission: Permission) {
        self.permissions
            .entry(table)
            .or_insert_with(TablePermissions::new)
            .grant(permission);
    }

    /// Revoke a permission on a table
    pub fn revoke(&mut self, table: &str, permission: Permission) {
        if let Some(perms) = self.permissions.get_mut(table) {
            perms.revoke(permission);
        }
    }

    /// Inherit from another role
    pub fn inherit_from(&mut self, role: String) {
        self.inherited_roles.insert(role);
    }

    /// Check if this role inherits from another (recursive)
    pub fn inherits_from(&self, role: &str, role_registry: &RoleRegistry) -> bool {
        if self.inherited_roles.contains(role) {
            return true;
        }

        for inherited in &self.inherited_roles {
            if let Some(r) = role_registry.get_role(inherited) {
                if r.inherits_from(role, role_registry) {
                    return true;
                }
            }
        }

        false
    }

    /// Check if role has permission on table (including inherited)
    pub fn has_permission(
        &self,
        table: &str,
        permission: Permission,
        role_registry: &RoleRegistry,
    ) -> bool {
        // Check direct permissions
        if let Some(perms) = self.permissions.get(table) {
            if perms.has(permission) {
                return true;
            }
        }

        // Check inherited permissions
        for inherited in &self.inherited_roles {
            if let Some(role) = role_registry.get_role(inherited) {
                if role.has_permission(table, permission, role_registry) {
                    return true;
                }
            }
        }

        false
    }
}

/// Individual permissions (PostgreSQL ACL)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    /// SELECT privilege
    Select,
    /// INSERT privilege
    Insert,
    /// UPDATE privilege
    Update,
    /// DELETE privilege
    Delete,
    /// TRUNCATE privilege
    Truncate,
    /// REFERENCES privilege (for foreign keys)
    References,
    /// TRIGGER privilege
    Trigger,
}

/// Permissions on a specific table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TablePermissions {
    permissions: HashSet<Permission>,
}

impl TablePermissions {
    pub fn new() -> Self {
        Self {
            permissions: HashSet::new(),
        }
    }

    pub fn grant(&mut self, permission: Permission) {
        self.permissions.insert(permission);
    }

    pub fn revoke(&mut self, permission: Permission) {
        self.permissions.remove(&permission);
    }

    pub fn has(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }

    pub fn grant_all(&mut self) {
        self.permissions.insert(Permission::Select);
        self.permissions.insert(Permission::Insert);
        self.permissions.insert(Permission::Update);
        self.permissions.insert(Permission::Delete);
        self.permissions.insert(Permission::Truncate);
        self.permissions.insert(Permission::References);
        self.permissions.insert(Permission::Trigger);
    }
}

/// Registry for managing roles
#[derive(Debug, Clone)]
pub struct RoleRegistry {
    roles: HashMap<String, Role>,
}

impl RoleRegistry {
    pub fn new() -> Self {
        Self {
            roles: HashMap::new(),
        }
    }

    pub fn create_role(&mut self, name: String) -> Result<()> {
        if self.roles.contains_key(&name) {
            return Err(NexusError::QueryError(format!(
                "Role {} already exists",
                name
            )));
        }
        self.roles.insert(name.clone(), Role::new(name));
        Ok(())
    }

    pub fn drop_role(&mut self, name: &str) -> Result<()> {
        if !self.roles.contains_key(name) {
            return Err(NexusError::QueryError(format!(
                "Role {} does not exist",
                name
            )));
        }
        self.roles.remove(name);
        Ok(())
    }

    pub fn get_role(&self, name: &str) -> Option<&Role> {
        self.roles.get(name)
    }

    pub fn get_role_mut(&mut self, name: &str) -> Option<&mut Role> {
        self.roles.get_mut(name)
    }
}

/// Authentication manager
#[derive(Debug, Clone)]
pub struct AuthManager {
    users: HashMap<String, User>,
    role_registry: RoleRegistry,
    /// Currently authenticated user
    current_user: Option<String>,
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            users: HashMap::new(),
            role_registry: RoleRegistry::new(),
            current_user: None,
        }
    }

    /// Create a new user
    pub fn create_user(&mut self, username: String, password: &str) -> Result<()> {
        if self.users.contains_key(&username) {
            return Err(NexusError::QueryError(format!(
                "User {} already exists",
                username
            )));
        }

        let user = User::new(username.clone(), password)?;
        self.users.insert(username, user);
        Ok(())
    }

    /// Create a superuser
    pub fn create_superuser(&mut self, username: String, password: &str) -> Result<()> {
        if self.users.contains_key(&username) {
            return Err(NexusError::QueryError(format!(
                "User {} already exists",
                username
            )));
        }

        let user = User::new_superuser(username.clone(), password)?;
        self.users.insert(username, user);
        Ok(())
    }

    /// Authenticate a user
    pub fn authenticate(&mut self, username: &str, password: &str) -> Result<()> {
        let user = self
            .users
            .get(username)
            .ok_or_else(|| NexusError::QueryError(format!("User {} does not exist", username)))?;

        if !user.can_login {
            return Err(NexusError::QueryError(format!(
                "User {} cannot login",
                username
            )));
        }

        if !user.verify_password(password)? {
            return Err(NexusError::QueryError("Invalid password".to_string()));
        }

        self.current_user = Some(username.to_string());
        Ok(())
    }

    /// Get current authenticated user
    pub fn current_user(&self) -> Option<&User> {
        self.current_user.as_ref().and_then(|u| self.users.get(u))
    }

    /// Logout current user
    pub fn logout(&mut self) {
        self.current_user = None;
    }

    /// Create a role
    pub fn create_role(&mut self, name: String) -> Result<()> {
        self.require_permission()?;
        self.role_registry.create_role(name)
    }

    /// Grant role to user
    pub fn grant_role(&mut self, username: &str, role: &str) -> Result<()> {
        self.require_permission()?;

        // Ensure role exists
        if !self.role_registry.roles.contains_key(role) {
            return Err(NexusError::QueryError(format!(
                "Role {} does not exist",
                role
            )));
        }

        let user = self
            .users
            .get_mut(username)
            .ok_or_else(|| NexusError::QueryError(format!("User {} does not exist", username)))?;

        user.grant_role(role.to_string());
        Ok(())
    }

    /// Grant permission to role
    pub fn grant_permission(
        &mut self,
        role: &str,
        table: String,
        permission: Permission,
    ) -> Result<()> {
        self.require_permission()?;

        let role_obj = self
            .role_registry
            .get_role_mut(role)
            .ok_or_else(|| NexusError::QueryError(format!("Role {} does not exist", role)))?;

        role_obj.grant(table, permission);
        Ok(())
    }

    /// Revoke permission from role
    pub fn revoke_permission(
        &mut self,
        role: &str,
        table: &str,
        permission: Permission,
    ) -> Result<()> {
        self.require_permission()?;

        let role_obj = self
            .role_registry
            .get_role_mut(role)
            .ok_or_else(|| NexusError::QueryError(format!("Role {} does not exist", role)))?;

        role_obj.revoke(table, permission);
        Ok(())
    }

    /// Check if current user has permission
    pub fn has_permission(&self, table: &str, permission: Permission) -> Result<bool> {
        let user = self
            .current_user()
            .ok_or_else(|| NexusError::QueryError("No user authenticated".to_string()))?;

        // Superuser bypasses all checks
        if user.is_superuser {
            return Ok(true);
        }

        // Check all user's roles
        for role_name in &user.roles {
            if let Some(role) = self.role_registry.get_role(role_name) {
                if role.has_permission(table, permission, &self.role_registry) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Require that a user is authenticated
    fn require_permission(&self) -> Result<()> {
        let user = self
            .current_user()
            .ok_or_else(|| NexusError::QueryError("No user authenticated".to_string()))?;

        if !user.is_superuser && !user.can_create_role {
            return Err(NexusError::QueryError(
                "Permission denied: only superusers can manage roles".to_string(),
            ));
        }

        Ok(())
    }

    /// Get user
    pub fn get_user(&self, username: &str) -> Option<&User> {
        self.users.get(username)
    }

    /// Get mutable access to role registry (for demos/advanced usage)
    pub fn role_registry_mut(&mut self) -> &mut RoleRegistry {
        &mut self.role_registry
    }

    /// Get role registry
    pub fn role_registry(&self) -> &RoleRegistry {
        &self.role_registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing() {
        let password = "secret123";
        let hash = password::hash_password(password).unwrap();

        assert!(password::verify_password(password, &hash).unwrap());
        assert!(!password::verify_password("wrong", &hash).unwrap());
    }

    #[test]
    fn test_user_authentication() {
        let mut auth = AuthManager::new();

        auth.create_user("alice".to_string(), "password123")
            .unwrap();

        // Correct password
        assert!(auth.authenticate("alice", "password123").is_ok());

        // Wrong password
        assert!(auth.authenticate("alice", "wrong").is_err());

        // Non-existent user
        assert!(auth.authenticate("bob", "password").is_err());
    }

    #[test]
    fn test_rbac() {
        let mut auth = AuthManager::new();

        // Create superuser
        auth.create_superuser("admin".to_string(), "admin123")
            .unwrap();
        auth.authenticate("admin", "admin123").unwrap();

        // Create role
        auth.create_role("reader".to_string()).unwrap();

        // Grant SELECT permission
        auth.grant_permission("reader", "users".to_string(), Permission::Select)
            .unwrap();

        // Create regular user
        auth.create_user("alice".to_string(), "password").unwrap();
        auth.grant_role("alice", "reader").unwrap();

        // Login as alice
        auth.authenticate("alice", "password").unwrap();

        // Check permissions
        assert!(auth.has_permission("users", Permission::Select).unwrap());
        assert!(!auth.has_permission("users", Permission::Insert).unwrap());
    }
}
