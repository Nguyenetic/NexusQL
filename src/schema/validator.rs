//! Data Validation & Constraint Enforcement
//!
//! Based on PostgreSQL constraint checking:
//! - NOT NULL enforcement
//! - UNIQUE constraint validation
//! - PRIMARY KEY uniqueness
//! - FOREIGN KEY referential integrity
//! - CHECK constraint evaluation
//! - Data type validation
//!
//! Context7 Research: PostgreSQL constraint system
//! PROPRIETARY & PATENTABLE

use super::{Constraint, DataType, ReferentialAction, TableSchema};
use crate::types::{Row, Value};
use crate::Result;

/// Validates data against schema constraints
pub struct Validator {
    schema: TableSchema,
}

impl Validator {
    pub fn new(schema: TableSchema) -> Self {
        Self { schema }
    }

    /// Validate an INSERT operation
    pub fn validate_insert(&self, row: &Row, existing_rows: &[Row]) -> Result<Row> {
        let mut validated_row = row.clone();

        // 1. Check column count matches schema
        if validated_row.columns.len() != self.schema.columns.len() {
            return Err(crate::NexusError::QueryError(format!(
                "Column count mismatch: expected {}, got {}",
                self.schema.columns.len(),
                validated_row.columns.len()
            )));
        }

        // 2. Validate and coerce each column
        for (i, col_def) in self.schema.columns.iter().enumerate() {
            let value = &validated_row.values[i];

            // Apply default if NULL and default exists
            let value = if matches!(value, Value::Null) {
                if let Some(default) = &col_def.default_value {
                    default.clone()
                } else {
                    value.clone()
                }
            } else {
                value.clone()
            };

            // Check NOT NULL
            if !col_def.nullable && matches!(value, Value::Null) {
                return Err(crate::NexusError::QueryError(format!(
                    "NULL value in non-null column: {}",
                    col_def.name
                )));
            }

            // Validate data type
            if !matches!(value, Value::Null) && !col_def.data_type.validate(&value) {
                return Err(crate::NexusError::QueryError(format!(
                    "Type mismatch for column {}: expected {:?}",
                    col_def.name, col_def.data_type
                )));
            }

            // Coerce value to correct type
            validated_row.values[i] = col_def.data_type.coerce(value)?;
        }

        // 3. Check UNIQUE constraints
        self.check_unique_constraints(&validated_row, existing_rows)?;

        // 4. Check PRIMARY KEY uniqueness
        self.check_primary_key(&validated_row, existing_rows)?;

        // 5. Check CHECK constraints
        self.check_constraints(&validated_row)?;

        // Note: FOREIGN KEY validation requires access to referenced tables
        // This would be done at a higher level (in the executor)

        Ok(validated_row)
    }

    /// Validate an UPDATE operation
    pub fn validate_update(
        &self,
        old_row: &Row,
        new_row: &Row,
        existing_rows: &[Row],
    ) -> Result<Row> {
        let mut validated_row = new_row.clone();

        // 1. Validate data types and coerce
        for (i, col_def) in self.schema.columns.iter().enumerate() {
            let value = &validated_row.values[i];

            // Check NOT NULL
            if !col_def.nullable && matches!(value, Value::Null) {
                return Err(crate::NexusError::QueryError(format!(
                    "NULL value in non-null column: {}",
                    col_def.name
                )));
            }

            // Validate data type
            if !matches!(value, Value::Null) && !col_def.data_type.validate(value) {
                return Err(crate::NexusError::QueryError(format!(
                    "Type mismatch for column {}: expected {:?}",
                    col_def.name, col_def.data_type
                )));
            }

            // Coerce value
            validated_row.values[i] = col_def.data_type.coerce(value.clone())?;
        }

        // 2. Check if PRIMARY KEY is being modified (usually not allowed)
        if let Some(pk) = self.schema.get_primary_key() {
            if let Constraint::PrimaryKey { columns, .. } = pk {
                for col_name in columns {
                    let col_idx = self
                        .schema
                        .columns
                        .iter()
                        .position(|c| &c.name == col_name)
                        .unwrap();

                    if old_row.values[col_idx] != validated_row.values[col_idx] {
                        return Err(crate::NexusError::QueryError(format!(
                            "Cannot update PRIMARY KEY column: {}",
                            col_name
                        )));
                    }
                }
            }
        }

        // 3. Check UNIQUE constraints (excluding current row)
        let other_rows: Vec<Row> = existing_rows
            .iter()
            .filter(|r| r != &old_row)
            .cloned()
            .collect();
        self.check_unique_constraints(&validated_row, &other_rows)?;

        // 4. Check CHECK constraints
        self.check_constraints(&validated_row)?;

        Ok(validated_row)
    }

    /// Validate a DELETE operation
    pub fn validate_delete(&self, _row: &Row) -> Result<()> {
        // DELETE validation is mainly about checking if row is referenced by foreign keys
        // This would be done at a higher level (in the executor)
        Ok(())
    }

    /// Check UNIQUE constraints
    fn check_unique_constraints(&self, row: &Row, existing_rows: &[Row]) -> Result<()> {
        for constraint in &self.schema.constraints {
            if let Constraint::Unique {
                columns,
                nulls_distinct,
                ..
            } = constraint
            {
                // Get values for unique columns
                let mut values = Vec::new();
                for col_name in columns {
                    let col_idx = self
                        .schema
                        .columns
                        .iter()
                        .position(|c| &c.name == col_name)
                        .ok_or_else(|| {
                            crate::NexusError::QueryError(format!("Column {} not found", col_name))
                        })?;
                    values.push(row.values[col_idx].clone());
                }

                // Check if values are NULL
                let has_null = values.iter().any(|v| matches!(v, Value::Null));

                // If NULLS DISTINCT (default), NULLs are treated as distinct
                // If NULLS NOT DISTINCT, NULLs are treated as equal
                if has_null && *nulls_distinct {
                    // Multiple NULLs are allowed with NULLS DISTINCT
                    continue;
                }

                // Check against existing rows
                for existing_row in existing_rows {
                    let mut existing_values = Vec::new();
                    for col_name in columns {
                        let col_idx = self
                            .schema
                            .columns
                            .iter()
                            .position(|c| &c.name == col_name)
                            .unwrap();
                        existing_values.push(existing_row.values[col_idx].clone());
                    }

                    if values == existing_values {
                        return Err(crate::NexusError::QueryError(format!(
                            "UNIQUE constraint violation on columns: {:?}",
                            columns
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    /// Check PRIMARY KEY uniqueness
    fn check_primary_key(&self, row: &Row, existing_rows: &[Row]) -> Result<()> {
        if let Some(pk) = self.schema.get_primary_key() {
            if let Constraint::PrimaryKey { columns, .. } = pk {
                // Get primary key values
                let mut pk_values = Vec::new();
                for col_name in columns {
                    let col_idx = self
                        .schema
                        .columns
                        .iter()
                        .position(|c| &c.name == col_name)
                        .ok_or_else(|| {
                            crate::NexusError::QueryError(format!("Column {} not found", col_name))
                        })?;

                    let value = &row.values[col_idx];

                    // PRIMARY KEY cannot be NULL
                    if matches!(value, Value::Null) {
                        return Err(crate::NexusError::QueryError(format!(
                            "NULL value in PRIMARY KEY column: {}",
                            col_name
                        )));
                    }

                    pk_values.push(value.clone());
                }

                // Check uniqueness against existing rows
                for existing_row in existing_rows {
                    let mut existing_pk_values = Vec::new();
                    for col_name in columns {
                        let col_idx = self
                            .schema
                            .columns
                            .iter()
                            .position(|c| &c.name == col_name)
                            .unwrap();
                        existing_pk_values.push(existing_row.values[col_idx].clone());
                    }

                    if pk_values == existing_pk_values {
                        return Err(crate::NexusError::QueryError(
                            "PRIMARY KEY constraint violation: duplicate key".to_string(),
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    /// Check CHECK constraints
    fn check_constraints(&self, _row: &Row) -> Result<()> {
        // CHECK constraint evaluation would require expression parsing and evaluation
        // For now, we'll skip this (would use the query evaluator)

        // TODO: Implement CHECK constraint evaluation
        // Example: CHECK (age >= 18)
        // Would need to:
        // 1. Parse expression
        // 2. Evaluate against row values
        // 3. Return error if check fails

        Ok(())
    }
}

/// Foreign key validator (requires access to multiple tables)
pub struct ForeignKeyValidator;

impl ForeignKeyValidator {
    /// Validate that foreign key references exist
    pub fn validate_references(
        constraint: &Constraint,
        row: &Row,
        source_schema: &TableSchema,
        target_rows: &[Row],
        target_schema: &TableSchema,
    ) -> Result<()> {
        if let Constraint::ForeignKey {
            columns,
            ref_columns,
            ..
        } = constraint
        {
            // Get foreign key values from row
            let mut fk_values = Vec::new();
            for col_name in columns {
                let col_idx = source_schema
                    .columns
                    .iter()
                    .position(|c| &c.name == col_name)
                    .ok_or_else(|| {
                        crate::NexusError::QueryError(format!("Column {} not found", col_name))
                    })?;
                fk_values.push(row.values[col_idx].clone());
            }

            // NULL foreign keys are allowed
            if fk_values.iter().any(|v| matches!(v, Value::Null)) {
                return Ok(());
            }

            // Check if referenced values exist in target table
            let mut found = false;
            for target_row in target_rows {
                let mut ref_values = Vec::new();
                for ref_col in ref_columns {
                    let ref_idx = target_schema
                        .columns
                        .iter()
                        .position(|c| &c.name == ref_col)
                        .ok_or_else(|| {
                            crate::NexusError::QueryError(format!("Column {} not found", ref_col))
                        })?;
                    ref_values.push(target_row.values[ref_idx].clone());
                }

                if fk_values == ref_values {
                    found = true;
                    break;
                }
            }

            if !found {
                return Err(crate::NexusError::QueryError(format!(
                    "FOREIGN KEY constraint violation: referenced row does not exist in target table"
                )));
            }
        }

        Ok(())
    }

    /// Handle cascading deletes
    pub fn cascade_delete(_constraint: &Constraint, _deleted_row: &Row) -> Result<Vec<Row>> {
        // TODO: Implement CASCADE delete
        // Find all rows in referencing table that reference deleted row
        // Return them for deletion
        Ok(Vec::new())
    }

    /// Handle SET NULL on delete
    pub fn set_null_on_delete(
        constraint: &Constraint,
        deleted_row: &Row,
        referencing_rows: &mut [Row],
        source_schema: &TableSchema,
        target_schema: &TableSchema,
    ) -> Result<()> {
        if let Constraint::ForeignKey {
            columns,
            ref_columns,
            on_delete: ReferentialAction::SetNull,
            ..
        } = constraint
        {
            // Get deleted primary key values
            let mut deleted_values = Vec::new();
            for ref_col in ref_columns {
                let ref_idx = target_schema
                    .columns
                    .iter()
                    .position(|c| &c.name == ref_col)
                    .ok_or_else(|| {
                        crate::NexusError::QueryError(format!("Column {} not found", ref_col))
                    })?;
                deleted_values.push(deleted_row.values[ref_idx].clone());
            }

            // Set referencing columns to NULL
            for row in referencing_rows {
                let mut fk_values = Vec::new();
                for col_name in columns {
                    let col_idx = source_schema
                        .columns
                        .iter()
                        .position(|c| &c.name == col_name)
                        .unwrap();
                    fk_values.push(row.values[col_idx].clone());
                }

                if fk_values == deleted_values {
                    // Set FK columns to NULL
                    for col_name in columns {
                        let col_idx = source_schema
                            .columns
                            .iter()
                            .position(|c| &c.name == col_name)
                            .unwrap();
                        row.values[col_idx] = Value::Null;
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::ColumnDef;

    fn create_test_schema() -> TableSchema {
        let mut schema = TableSchema::new("users".to_string());

        schema
            .add_column(ColumnDef::new("id".to_string(), DataType::Integer).not_null())
            .unwrap();

        schema
            .add_column(ColumnDef::new("email".to_string(), DataType::Varchar(100)).not_null())
            .unwrap();

        schema
            .add_constraint(Constraint::PrimaryKey {
                name: "users_pkey".to_string(),
                columns: vec!["id".to_string()],
            })
            .unwrap();

        schema
            .add_constraint(Constraint::Unique {
                name: "users_email_unique".to_string(),
                columns: vec!["email".to_string()],
                nulls_distinct: true,
            })
            .unwrap();

        schema
    }

    #[test]
    fn test_insert_validation() {
        let schema = create_test_schema();
        let validator = Validator::new(schema);

        let row = Row::new(
            vec!["id".to_string(), "email".to_string()],
            vec![
                Value::Integer(1),
                Value::Text("test@example.com".to_string()),
            ],
        );

        let result = validator.validate_insert(&row, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_not_null_violation() {
        let schema = create_test_schema();
        let validator = Validator::new(schema);

        let row = Row::new(
            vec!["id".to_string(), "email".to_string()],
            vec![Value::Null, Value::Text("test@example.com".to_string())],
        );

        let result = validator.validate_insert(&row, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_unique_constraint() {
        let schema = create_test_schema();
        let validator = Validator::new(schema);

        let existing = Row::new(
            vec!["id".to_string(), "email".to_string()],
            vec![
                Value::Integer(1),
                Value::Text("test@example.com".to_string()),
            ],
        );

        let new_row = Row::new(
            vec!["id".to_string(), "email".to_string()],
            vec![
                Value::Integer(2),
                Value::Text("test@example.com".to_string()),
            ],
        );

        let result = validator.validate_insert(&new_row, &[existing]);
        assert!(result.is_err());
    }

    #[test]
    fn test_primary_key_violation() {
        let schema = create_test_schema();
        let validator = Validator::new(schema);

        let existing = Row::new(
            vec!["id".to_string(), "email".to_string()],
            vec![
                Value::Integer(1),
                Value::Text("test1@example.com".to_string()),
            ],
        );

        let new_row = Row::new(
            vec!["id".to_string(), "email".to_string()],
            vec![
                Value::Integer(1),
                Value::Text("test2@example.com".to_string()),
            ],
        );

        let result = validator.validate_insert(&new_row, &[existing]);
        assert!(result.is_err());
    }
}
