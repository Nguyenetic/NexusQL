// Security & Audit Module
// Enterprise-grade security features

use crate::{NexusError, Result};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::SystemTime;

/// Audit log entry
#[derive(Debug, Clone)]
pub struct AuditLogEntry {
    pub timestamp: SystemTime,
    pub user: String,
    pub action: String,
    pub table: Option<String>,
    pub query: String,
    pub success: bool,
    pub error: Option<String>,
}

/// Audit logger
pub struct AuditLogger {
    log_file: PathBuf,
    enabled: bool,
}

impl AuditLogger {
    pub fn new(log_file: PathBuf) -> Result<Self> {
        Ok(Self {
            log_file,
            enabled: true,
        })
    }

    /// Log a query execution
    pub fn log_query(&self, entry: AuditLogEntry) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file)
            .map_err(|e| NexusError::IoError(e))?;

        let log_line = format!(
            "{:?}|{}|{}|{}|{}|{}\n",
            entry.timestamp,
            entry.user,
            entry.action,
            entry.table.unwrap_or_default(),
            entry.success,
            entry.error.unwrap_or_default()
        );

        file.write_all(log_line.as_bytes())
            .map_err(|e| NexusError::IoError(e))?;

        Ok(())
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }
}

/// SQL Injection Prevention
pub struct SqlInjectionPrevention;

impl SqlInjectionPrevention {
    /// Check for common SQL injection patterns
    pub fn validate_input(input: &str) -> Result<()> {
        let input_lower = input.to_lowercase();

        // Check for dangerous patterns
        let dangerous_patterns = vec![
            "'; drop table",
            "' or '1'='1",
            "' or 1=1",
            "union select",
            "exec(",
            "execute(",
            "../",
            "<script",
        ];

        for pattern in dangerous_patterns {
            if input_lower.contains(pattern) {
                return Err(NexusError::ExecutionError(format!(
                    "Potential SQL injection detected: {}",
                    pattern
                )));
            }
        }

        Ok(())
    }

    /// Sanitize user input
    pub fn sanitize(input: &str) -> String {
        input
            .replace("'", "''") // Escape single quotes
            .replace("\\", "\\\\") // Escape backslashes
            .replace("\0", "") // Remove null bytes
    }

    /// Validate identifier (table/column name)
    pub fn validate_identifier(name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(NexusError::ExecutionError(
                "Identifier cannot be empty".to_string(),
            ));
        }

        // Must start with letter or underscore
        if !name.chars().next().unwrap().is_alphabetic() && !name.starts_with('_') {
            return Err(NexusError::ExecutionError(
                "Identifier must start with letter or underscore".to_string(),
            ));
        }

        // Only alphanumeric and underscores
        if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(NexusError::ExecutionError(
                "Identifier can only contain alphanumeric characters and underscores".to_string(),
            ));
        }

        Ok(())
    }
}

/// Password policy enforcement
pub struct PasswordPolicy {
    min_length: usize,
    require_uppercase: bool,
    require_lowercase: bool,
    require_digit: bool,
    require_special: bool,
}

impl PasswordPolicy {
    pub fn default() -> Self {
        Self {
            min_length: 8,
            require_uppercase: true,
            require_lowercase: true,
            require_digit: true,
            require_special: true,
        }
    }

    pub fn strict() -> Self {
        Self {
            min_length: 12,
            require_uppercase: true,
            require_lowercase: true,
            require_digit: true,
            require_special: true,
        }
    }

    /// Validate password against policy
    pub fn validate(&self, password: &str) -> Result<()> {
        if password.len() < self.min_length {
            return Err(NexusError::ExecutionError(format!(
                "Password must be at least {} characters",
                self.min_length
            )));
        }

        if self.require_uppercase && !password.chars().any(|c| c.is_uppercase()) {
            return Err(NexusError::ExecutionError(
                "Password must contain at least one uppercase letter".to_string(),
            ));
        }

        if self.require_lowercase && !password.chars().any(|c| c.is_lowercase()) {
            return Err(NexusError::ExecutionError(
                "Password must contain at least one lowercase letter".to_string(),
            ));
        }

        if self.require_digit && !password.chars().any(|c| c.is_numeric()) {
            return Err(NexusError::ExecutionError(
                "Password must contain at least one digit".to_string(),
            ));
        }

        if self.require_special {
            let special_chars = "!@#$%^&*()_+-=[]{}|;:,.<>?";
            if !password.chars().any(|c| special_chars.contains(c)) {
                return Err(NexusError::ExecutionError(
                    "Password must contain at least one special character".to_string(),
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_audit_logging() {
        let temp_dir = tempdir().unwrap();
        let log_file = temp_dir.path().join("audit.log");

        let logger = AuditLogger::new(log_file.clone()).unwrap();

        logger
            .log_query(AuditLogEntry {
                timestamp: SystemTime::now(),
                user: "admin".to_string(),
                action: "SELECT".to_string(),
                table: Some("users".to_string()),
                query: "SELECT * FROM users".to_string(),
                success: true,
                error: None,
            })
            .unwrap();

        assert!(log_file.exists());
    }

    #[test]
    fn test_sql_injection_detection() {
        assert!(SqlInjectionPrevention::validate_input("SELECT * FROM users").is_ok());
        assert!(SqlInjectionPrevention::validate_input("'; DROP TABLE users--").is_err());
        assert!(SqlInjectionPrevention::validate_input("' OR '1'='1").is_err());
    }

    #[test]
    fn test_input_sanitization() {
        let input = "O'Brien";
        let sanitized = SqlInjectionPrevention::sanitize(input);
        assert_eq!(sanitized, "O''Brien");
    }

    #[test]
    fn test_identifier_validation() {
        assert!(SqlInjectionPrevention::validate_identifier("users").is_ok());
        assert!(SqlInjectionPrevention::validate_identifier("user_table").is_ok());
        assert!(SqlInjectionPrevention::validate_identifier("123table").is_err());
        assert!(SqlInjectionPrevention::validate_identifier("user-table").is_err());
    }

    #[test]
    fn test_password_policy_default() {
        let policy = PasswordPolicy::default();

        assert!(policy.validate("Weak1!").is_err()); // Too short
        assert!(policy.validate("weakpassword1!").is_err()); // No uppercase
        assert!(policy.validate("WEAKPASSWORD1!").is_err()); // No lowercase
        assert!(policy.validate("WeakPassword!").is_err()); // No digit
        assert!(policy.validate("WeakPassword1").is_err()); // No special
        assert!(policy.validate("StrongPass123!").is_ok()); // Valid
    }

    #[test]
    fn test_password_policy_strict() {
        let policy = PasswordPolicy::strict();

        assert!(policy.validate("StrongPass123!").is_err()); // Too short (< 12)
        assert!(policy.validate("VeryStrongPass123!").is_ok()); // Valid
    }
}
