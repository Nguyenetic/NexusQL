//! Schema Management System
//!
//! Based on PostgreSQL schema system with:
//! - Typed columns with data type enforcement
//! - Constraints (PRIMARY KEY, FOREIGN KEY, UNIQUE, CHECK, NOT NULL)
//! - Default values
//! - Schema ownership
//!
//! Context7 Research: PostgreSQL information_schema
//! PROPRIETARY & PATENTABLE

pub mod validator;

pub use validator::{ForeignKeyValidator, Validator};

use crate::types::{Row, Value};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Database schema containing multiple tables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub name: String,
    pub owner: String,
    pub tables: HashMap<String, TableSchema>,
    pub created_at: u64,
}

impl Schema {
    pub fn new(name: String, owner: String) -> Self {
        Self {
            name,
            owner,
            tables: HashMap::new(),
            created_at: current_timestamp(),
        }
    }

    pub fn add_table(&mut self, table: TableSchema) -> Result<()> {
        if self.tables.contains_key(&table.name) {
            return Err(crate::NexusError::QueryError(format!(
                "Table {} already exists",
                table.name
            )));
        }
        self.tables.insert(table.name.clone(), table);
        Ok(())
    }

    pub fn get_table(&self, name: &str) -> Option<&TableSchema> {
        self.tables.get(name)
    }

    pub fn drop_table(&mut self, name: &str) -> Result<()> {
        self.tables
            .remove(name)
            .ok_or_else(|| crate::NexusError::QueryError(format!("Table {} not found", name)))?;
        Ok(())
    }
}

/// Table schema definition with columns and constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnDef>,
    pub constraints: Vec<Constraint>,
    pub created_at: u64,
}

impl TableSchema {
    pub fn new(name: String) -> Self {
        Self {
            name,
            columns: Vec::new(),
            constraints: Vec::new(),
            created_at: current_timestamp(),
        }
    }

    pub fn add_column(&mut self, column: ColumnDef) -> Result<()> {
        if self.columns.iter().any(|c| c.name == column.name) {
            return Err(crate::NexusError::QueryError(format!(
                "Column {} already exists",
                column.name
            )));
        }
        self.columns.push(column);
        Ok(())
    }

    pub fn add_constraint(&mut self, constraint: Constraint) -> Result<()> {
        // Validate constraint references valid columns
        match &constraint {
            Constraint::PrimaryKey { columns, .. } | Constraint::Unique { columns, .. } => {
                for col in columns {
                    if !self.has_column(col) {
                        return Err(crate::NexusError::QueryError(format!(
                            "Column {} not found in table",
                            col
                        )));
                    }
                }
            }
            Constraint::ForeignKey { columns, .. } => {
                for col in columns {
                    if !self.has_column(col) {
                        return Err(crate::NexusError::QueryError(format!(
                            "Column {} not found in table",
                            col
                        )));
                    }
                }
            }
            Constraint::Check { .. } => {
                // Expression validation would go here
            }
        }

        self.constraints.push(constraint);
        Ok(())
    }

    pub fn has_column(&self, name: &str) -> bool {
        self.columns.iter().any(|c| c.name == name)
    }

    pub fn get_column(&self, name: &str) -> Option<&ColumnDef> {
        self.columns.iter().find(|c| c.name == name)
    }

    pub fn get_primary_key(&self) -> Option<&Constraint> {
        self.constraints
            .iter()
            .find(|c| matches!(c, Constraint::PrimaryKey { .. }))
    }
}

/// Column definition with type and constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub default_value: Option<Value>,
    pub constraints: Vec<ColumnConstraint>,
}

impl ColumnDef {
    pub fn new(name: String, data_type: DataType) -> Self {
        Self {
            name,
            data_type,
            nullable: true,
            default_value: None,
            constraints: Vec::new(),
        }
    }

    pub fn not_null(mut self) -> Self {
        self.nullable = false;
        self.constraints.push(ColumnConstraint::NotNull);
        self
    }

    pub fn default(mut self, value: Value) -> Self {
        self.default_value = Some(value.clone());
        self.constraints.push(ColumnConstraint::Default(value));
        self
    }

    pub fn unique(mut self) -> Self {
        self.constraints.push(ColumnConstraint::Unique);
        self
    }

    pub fn primary_key(mut self) -> Self {
        self.nullable = false;
        self.constraints.push(ColumnConstraint::PrimaryKey);
        self
    }
}

/// SQL data types (based on PostgreSQL)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    // Integer types
    SmallInt, // 2 bytes
    Integer,  // 4 bytes
    BigInt,   // 8 bytes

    // Floating point
    Float,  // 4 bytes
    Double, // 8 bytes
    Decimal { precision: u8, scale: u8 },

    // String types
    Text,
    Varchar(usize),
    Char(usize),

    // Boolean
    Boolean,

    // Date/Time
    Date,
    Time,
    Timestamp,
    TimestampTz,

    // Binary
    Blob,
    Bytea,

    // JSON
    Json,
    Jsonb,

    // UUID
    Uuid,
}

impl DataType {
    /// Check if a value matches this data type
    pub fn validate(&self, value: &Value) -> bool {
        match (self, value) {
            (DataType::Integer | DataType::BigInt | DataType::SmallInt, Value::Integer(_)) => true,
            (DataType::Float | DataType::Double, Value::Float(_)) => true,
            (DataType::Text | DataType::Varchar(_) | DataType::Char(_), Value::Text(_)) => true,
            (DataType::Boolean, Value::Boolean(_)) => true,
            (DataType::Blob | DataType::Bytea, Value::Blob(_)) => true,
            (_, Value::Null) => true, // NULL is valid for any type if column is nullable
            _ => false,
        }
    }

    /// Coerce a value to this data type if possible
    pub fn coerce(&self, value: Value) -> Result<Value> {
        match (self, &value) {
            // Already correct type
            (DataType::Integer | DataType::BigInt | DataType::SmallInt, Value::Integer(_)) => {
                Ok(value)
            }
            (DataType::Float | DataType::Double, Value::Float(_)) => Ok(value),
            (DataType::Text, Value::Text(_)) => Ok(value),
            (DataType::Boolean, Value::Boolean(_)) => Ok(value),

            // Integer to Float
            (DataType::Float | DataType::Double, Value::Integer(i)) => {
                Ok(Value::Float(ordered_float::OrderedFloat(*i as f64)))
            }

            // Text with length check for VARCHAR/CHAR
            (DataType::Varchar(max_len), Value::Text(s))
            | (DataType::Char(max_len), Value::Text(s)) => {
                if s.len() > *max_len {
                    return Err(crate::NexusError::QueryError(format!(
                        "Value too long for VARCHAR({}): got {} characters",
                        max_len,
                        s.len()
                    )));
                }
                Ok(value)
            }

            // NULL is always valid
            (_, Value::Null) => Ok(Value::Null),

            // Type mismatch
            _ => Err(crate::NexusError::QueryError(format!(
                "Type mismatch: expected {:?}, got {:?}",
                self, value
            ))),
        }
    }
}

/// Table-level constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Constraint {
    PrimaryKey {
        name: String,
        columns: Vec<String>,
    },
    ForeignKey {
        name: String,
        columns: Vec<String>,
        ref_table: String,
        ref_columns: Vec<String>,
        on_delete: ReferentialAction,
        on_update: ReferentialAction,
    },
    Unique {
        name: String,
        columns: Vec<String>,
        nulls_distinct: bool, // PostgreSQL: NULLS DISTINCT vs NULLS NOT DISTINCT
    },
    Check {
        name: String,
        expression: String,
        no_inherit: bool,
    },
}

/// Column-level constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColumnConstraint {
    NotNull,
    Null,
    Unique,
    PrimaryKey,
    Check(String),
    Default(Value),
    References {
        table: String,
        column: String,
        on_delete: ReferentialAction,
        on_update: ReferentialAction,
    },
}

/// Referential actions for foreign keys (ON DELETE, ON UPDATE)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferentialAction {
    NoAction,   // Default: raise error if referenced rows exist
    Restrict,   // Similar to NoAction but checked immediately
    Cascade,    // Delete/update referencing rows too
    SetNull,    // Set referencing columns to NULL
    SetDefault, // Set referencing columns to their default value
}

impl Default for ReferentialAction {
    fn default() -> Self {
        ReferentialAction::NoAction
    }
}

/// Get current Unix timestamp
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_creation() {
        let mut schema = Schema::new("public".to_string(), "admin".to_string());

        let mut table = TableSchema::new("users".to_string());
        table
            .add_column(ColumnDef::new("id".to_string(), DataType::Integer).primary_key())
            .unwrap();
        table
            .add_column(
                ColumnDef::new("email".to_string(), DataType::Varchar(100))
                    .not_null()
                    .unique(),
            )
            .unwrap();

        schema.add_table(table).unwrap();
        assert!(schema.get_table("users").is_some());
    }

    #[test]
    fn test_data_type_validation() {
        assert!(DataType::Integer.validate(&Value::Integer(42)));
        assert!(!DataType::Integer.validate(&Value::Text("foo".to_string())));
        assert!(DataType::Text.validate(&Value::Null)); // NULL is valid
    }

    #[test]
    fn test_varchar_length() {
        let dt = DataType::Varchar(10);
        assert!(dt.coerce(Value::Text("short".to_string())).is_ok());
        assert!(dt
            .coerce(Value::Text("this is way too long".to_string()))
            .is_err());
    }

    #[test]
    fn test_primary_key_constraint() {
        let mut table = TableSchema::new("users".to_string());
        table
            .add_column(ColumnDef::new("id".to_string(), DataType::Integer))
            .unwrap();

        table
            .add_constraint(Constraint::PrimaryKey {
                name: "users_pkey".to_string(),
                columns: vec!["id".to_string()],
            })
            .unwrap();

        assert!(table.get_primary_key().is_some());
    }
}
