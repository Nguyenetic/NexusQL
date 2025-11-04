// Developer Tools
// SQL formatter, linter, schema diff, import/export utilities

use crate::types::Row;
use crate::{NexusError, Result};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

/// SQL Formatter
pub struct SqlFormatter;

impl SqlFormatter {
    /// Format SQL query for readability
    pub fn format(sql: &str) -> String {
        let mut formatted = String::new();
        let mut indent_level: usize = 0;

        for line in sql.lines() {
            let trimmed = line.trim();

            // Decrease indent for closing keywords
            if trimmed.starts_with(')') || trimmed.to_uppercase().starts_with("END") {
                indent_level = indent_level.saturating_sub(1);
            }

            // Add indentation
            formatted.push_str(&"  ".repeat(indent_level));
            formatted.push_str(trimmed);
            formatted.push('\n');

            // Increase indent for opening keywords
            if trimmed.ends_with('(') || trimmed.to_uppercase().starts_with("BEGIN") {
                indent_level += 1;
            }
        }

        formatted
    }

    /// Validate SQL syntax (basic)
    pub fn lint(sql: &str) -> Vec<String> {
        let mut warnings = Vec::new();
        let sql_upper = sql.to_uppercase();

        // Check for SELECT *
        if sql_upper.contains("SELECT *") {
            warnings.push("Avoid SELECT * - specify columns explicitly".to_string());
        }

        // Check for missing WHERE in DELETE/UPDATE
        if (sql_upper.contains("DELETE FROM") || sql_upper.contains("UPDATE"))
            && !sql_upper.contains("WHERE")
        {
            warnings.push("DELETE/UPDATE without WHERE - this affects all rows!".to_string());
        }

        // Check for missing indexes hint
        if sql_upper.contains("WHERE") && !sql_upper.contains("INDEX") {
            warnings.push("Consider adding an index for WHERE clause columns".to_string());
        }

        warnings
    }
}

/// Schema Diff Tool
pub struct SchemaDiff;

impl SchemaDiff {
    /// Compare two table schemas
    pub fn diff_tables(table1: &TableSchema, table2: &TableSchema) -> Vec<SchemaDifference> {
        let mut diffs = Vec::new();

        // Check for added columns
        for col in &table2.columns {
            if !table1.columns.contains(col) {
                diffs.push(SchemaDifference::ColumnAdded(col.clone()));
            }
        }

        // Check for removed columns
        for col in &table1.columns {
            if !table2.columns.contains(col) {
                diffs.push(SchemaDifference::ColumnRemoved(col.clone()));
            }
        }

        diffs
    }

    /// Generate migration SQL from diff
    pub fn generate_migration(diffs: &[SchemaDifference], table_name: &str) -> String {
        let mut sql = String::new();

        for diff in diffs {
            match diff {
                SchemaDifference::ColumnAdded(col) => {
                    sql.push_str(&format!("ALTER TABLE {} ADD COLUMN {};\n", table_name, col));
                }
                SchemaDifference::ColumnRemoved(col) => {
                    sql.push_str(&format!(
                        "ALTER TABLE {} DROP COLUMN {};\n",
                        table_name, col
                    ));
                }
                SchemaDifference::ColumnModified(col, change) => {
                    sql.push_str(&format!(
                        "ALTER TABLE {} ALTER COLUMN {} {};\n",
                        table_name, col, change
                    ));
                }
            }
        }

        sql
    }
}

#[derive(Debug, Clone)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SchemaDifference {
    ColumnAdded(String),
    ColumnRemoved(String),
    ColumnModified(String, String),
}

/// Data Import/Export
pub struct DataPortability;

impl DataPortability {
    /// Export table to CSV
    pub fn export_to_csv(rows: &[Row], path: &Path) -> Result<()> {
        let file = File::create(path).map_err(|e| NexusError::IoError(e))?;
        let mut writer = BufWriter::new(file);

        if rows.is_empty() {
            return Ok(());
        }

        // Write header
        let header = rows[0].columns.join(",");
        writeln!(writer, "{}", header).map_err(|e| NexusError::IoError(e))?;

        // Write rows
        for row in rows {
            let values: Vec<String> = row.values.iter().map(|v| format!("{}", v)).collect();
            writeln!(writer, "{}", values.join(",")).map_err(|e| NexusError::IoError(e))?;
        }

        writer.flush().map_err(|e| NexusError::IoError(e))?;
        Ok(())
    }

    /// Import from CSV
    pub fn import_from_csv(path: &Path) -> Result<Vec<Row>> {
        let file = File::open(path).map_err(|e| NexusError::IoError(e))?;
        let reader = BufReader::new(file);

        // In production: parse CSV properly
        // For now, return empty
        Ok(Vec::new())
    }

    /// Export to JSON
    pub fn export_to_json(rows: &[Row]) -> Result<String> {
        let json_rows: Vec<String> = rows
            .iter()
            .map(|row| {
                let fields: Vec<String> = row
                    .columns
                    .iter()
                    .zip(row.values.iter())
                    .map(|(col, val)| format!("\"{}\": \"{}\"", col, val))
                    .collect();
                format!("{{{}}}", fields.join(", "))
            })
            .collect();

        Ok(format!("[{}]", json_rows.join(", ")))
    }

    /// Import from JSON
    pub fn import_from_json(json: &str) -> Result<Vec<Row>> {
        // In production: parse JSON properly with serde_json
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sql_formatter() {
        let sql = "SELECT id,name FROM users WHERE id>10";
        let formatted = SqlFormatter::format(sql);
        assert!(formatted.contains("SELECT"));
    }

    #[test]
    fn test_sql_linter() {
        let warnings = SqlFormatter::lint("SELECT * FROM users");
        assert!(warnings.len() > 0);
        assert!(warnings[0].contains("SELECT *"));
    }

    #[test]
    fn test_sql_injection_prevention() {
        assert!(SqlInjectionPrevention::validate_input("SELECT * FROM users").is_ok());
        assert!(SqlInjectionPrevention::validate_input("'; DROP TABLE users--").is_err());
    }

    #[test]
    fn test_input_sanitization() {
        let input = "O'Brien";
        let sanitized = SqlInjectionPrevention::sanitize(input);
        assert_eq!(sanitized, "O''Brien");
    }

    #[test]
    fn test_schema_diff() {
        let schema1 = TableSchema {
            name: "users".to_string(),
            columns: vec!["id".to_string(), "name".to_string()],
        };

        let schema2 = TableSchema {
            name: "users".to_string(),
            columns: vec!["id".to_string(), "name".to_string(), "email".to_string()],
        };

        let diffs = SchemaDiff::diff_tables(&schema1, &schema2);
        assert_eq!(diffs.len(), 1);
        assert!(matches!(diffs[0], SchemaDifference::ColumnAdded(_)));
    }

    #[test]
    fn test_migration_generation() {
        let diffs = vec![SchemaDifference::ColumnAdded("email".to_string())];

        let migration = SchemaDiff::generate_migration(&diffs, "users");
        assert!(migration.contains("ALTER TABLE users ADD COLUMN email"));
    }

    #[test]
    fn test_csv_export() {
        let temp_dir = tempdir().unwrap();
        let csv_path = temp_dir.path().join("export.csv");

        let rows = vec![Row::new(
            vec!["id".to_string(), "name".to_string()],
            vec![Value::Integer(1), Value::Text("Alice".to_string())],
        )];

        DataPortability::export_to_csv(&rows, &csv_path).unwrap();
        assert!(csv_path.exists());
    }

    #[test]
    fn test_json_export() {
        let rows = vec![Row::new(
            vec!["id".to_string(), "name".to_string()],
            vec![Value::Integer(1), Value::Text("Alice".to_string())],
        )];

        let json = DataPortability::export_to_json(&rows).unwrap();
        assert!(json.contains("id"));
        assert!(json.contains("Alice"));
    }

    #[test]
    fn test_password_policy() {
        let policy = PasswordPolicy::default();

        assert!(policy.validate("weak").is_err());
        assert!(policy.validate("StrongPass123!").is_ok());
    }
}
