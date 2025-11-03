// v1.1.0: Extensions & Plugins
//
// Implements PostgreSQL-compatible extension system:
// - CREATE EXTENSION: Load and install extensions
// - Foreign Data Wrappers (FDW): Access external data sources
// - Custom data types: User-defined types with operators
// - Custom operators: Define new operators for types
// - Extension framework: Plugin architecture

use crate::types::Row;
use crate::{NexusError, Result};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Extension metadata
#[derive(Debug, Clone)]
pub struct Extension {
    pub name: String,
    pub version: String,
    pub description: String,
    pub schema: String,
    pub loaded: bool,
}

/// Extension registry
pub struct ExtensionRegistry {
    extensions: Arc<RwLock<HashMap<String, Extension>>>,
    custom_types: Arc<RwLock<HashMap<String, CustomDataType>>>,
    custom_operators: Arc<RwLock<HashMap<String, CustomOperator>>>,
    fdw_handlers: Arc<RwLock<HashMap<String, ForeignDataWrapper>>>,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self {
            extensions: Arc::new(RwLock::new(HashMap::new())),
            custom_types: Arc::new(RwLock::new(HashMap::new())),
            custom_operators: Arc::new(RwLock::new(HashMap::new())),
            fdw_handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// CREATE EXTENSION
    pub fn create_extension(&self, name: String, version: String) -> Result<()> {
        let mut extensions = self.extensions.write().unwrap();

        if extensions.contains_key(&name) {
            return Err(NexusError::ExecutionError(format!(
                "Extension '{}' already exists",
                name
            )));
        }

        let ext = Extension {
            name: name.clone(),
            version,
            description: format!("Extension: {}", name),
            schema: "public".to_string(),
            loaded: true,
        };

        extensions.insert(name, ext);
        Ok(())
    }

    /// DROP EXTENSION
    pub fn drop_extension(&self, name: &str, cascade: bool) -> Result<()> {
        let mut extensions = self.extensions.write().unwrap();

        if !extensions.contains_key(name) {
            return Err(NexusError::ExecutionError(format!(
                "Extension '{}' does not exist",
                name
            )));
        }

        extensions.remove(name);
        Ok(())
    }

    /// Register custom data type
    pub fn register_type(&self, custom_type: CustomDataType) -> Result<()> {
        let mut types = self.custom_types.write().unwrap();

        if types.contains_key(&custom_type.name) {
            return Err(NexusError::ExecutionError(format!(
                "Type '{}' already exists",
                custom_type.name
            )));
        }

        types.insert(custom_type.name.clone(), custom_type);
        Ok(())
    }

    /// Register custom operator
    pub fn register_operator(&self, operator: CustomOperator) -> Result<()> {
        let mut ops = self.custom_operators.write().unwrap();

        let key = format!(
            "{}_{}_{}",
            operator.operator_symbol, operator.left_type, operator.right_type
        );

        if ops.contains_key(&key) {
            return Err(NexusError::ExecutionError(format!(
                "Operator '{}' already exists for types {} and {}",
                operator.operator_symbol, operator.left_type, operator.right_type
            )));
        }

        ops.insert(key, operator);
        Ok(())
    }

    /// Register FDW handler
    pub fn register_fdw(&self, name: String, fdw: ForeignDataWrapper) -> Result<()> {
        let mut handlers = self.fdw_handlers.write().unwrap();

        if handlers.contains_key(&name) {
            return Err(NexusError::ExecutionError(format!(
                "FDW '{}' already exists",
                name
            )));
        }

        handlers.insert(name, fdw);
        Ok(())
    }

    /// Get extension list
    pub fn list_extensions(&self) -> Vec<Extension> {
        self.extensions.read().unwrap().values().cloned().collect()
    }

    /// Get custom type
    pub fn get_type(&self, name: &str) -> Option<CustomDataType> {
        self.custom_types.read().unwrap().get(name).cloned()
    }

    /// Get FDW handler
    pub fn get_fdw(&self, name: &str) -> Option<ForeignDataWrapper> {
        self.fdw_handlers.read().unwrap().get(name).cloned()
    }
}

/// Custom data type definition
#[derive(Debug, Clone)]
pub struct CustomDataType {
    pub name: String,
    pub internal_length: i32, // -1 for variable length
    pub alignment: String,    // "char", "int2", "int4", "double"
    pub storage: String,      // "plain", "extended", "external", "main"
}

impl CustomDataType {
    /// Create new custom type
    pub fn new(name: String) -> Self {
        Self {
            name,
            internal_length: -1, // Variable length by default
            alignment: "int4".to_string(),
            storage: "extended".to_string(),
        }
    }

    /// Set fixed length
    pub fn with_length(mut self, length: i32) -> Self {
        self.internal_length = length;
        self
    }

    /// Set alignment
    pub fn with_alignment(mut self, alignment: String) -> Self {
        self.alignment = alignment;
        self
    }
}

/// Custom operator definition
#[derive(Debug, Clone)]
pub struct CustomOperator {
    pub operator_symbol: String,
    pub left_type: String,
    pub right_type: String,
    pub return_type: String,
    pub commutator: Option<String>,
}

impl CustomOperator {
    /// Create new custom operator
    pub fn new(
        operator_symbol: String,
        left_type: String,
        right_type: String,
        return_type: String,
    ) -> Self {
        Self {
            operator_symbol,
            left_type,
            right_type,
            return_type,
            commutator: None,
        }
    }

    /// Set commutator operator
    pub fn with_commutator(mut self, commutator: String) -> Self {
        self.commutator = Some(commutator);
        self
    }
}

/// Foreign Data Wrapper
#[derive(Debug, Clone)]
pub struct ForeignDataWrapper {
    pub name: String,
    pub handler: String,
    pub validator: Option<String>,
}

impl ForeignDataWrapper {
    pub fn new(name: String, handler: String) -> Self {
        Self {
            name,
            handler,
            validator: None,
        }
    }

    pub fn with_validator(mut self, validator: String) -> Self {
        self.validator = Some(validator);
        self
    }
}

/// Foreign server
#[derive(Debug, Clone)]
pub struct ForeignServer {
    pub name: String,
    pub fdw_name: String,
    pub options: HashMap<String, String>,
}

impl ForeignServer {
    pub fn new(name: String, fdw_name: String) -> Self {
        Self {
            name,
            fdw_name,
            options: HashMap::new(),
        }
    }

    pub fn with_option(mut self, key: String, value: String) -> Self {
        self.options.insert(key, value);
        self
    }
}

/// Foreign table
#[derive(Debug, Clone)]
pub struct ForeignTable {
    pub name: String,
    pub server_name: String,
    pub columns: Vec<String>,
    pub options: HashMap<String, String>,
}

impl ForeignTable {
    pub fn new(name: String, server_name: String, columns: Vec<String>) -> Self {
        Self {
            name,
            server_name,
            columns,
            options: HashMap::new(),
        }
    }

    /// Query foreign table (simulated)
    pub fn query(&self) -> Result<Vec<Row>> {
        // In real implementation, this would call the FDW handler
        // For demo, return empty result
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_extension() {
        let registry = ExtensionRegistry::new();

        registry
            .create_extension("pg_trgm".to_string(), "1.0".to_string())
            .unwrap();

        let extensions = registry.list_extensions();
        assert_eq!(extensions.len(), 1);
        assert_eq!(extensions[0].name, "pg_trgm");
    }

    #[test]
    fn test_drop_extension() {
        let registry = ExtensionRegistry::new();

        registry
            .create_extension("test_ext".to_string(), "1.0".to_string())
            .unwrap();
        assert_eq!(registry.list_extensions().len(), 1);

        registry.drop_extension("test_ext", false).unwrap();
        assert_eq!(registry.list_extensions().len(), 0);
    }

    #[test]
    fn test_custom_data_type() {
        let registry = ExtensionRegistry::new();

        let custom_type = CustomDataType::new("complex".to_string())
            .with_length(16)
            .with_alignment("double".to_string());

        registry.register_type(custom_type).unwrap();

        let retrieved = registry.get_type("complex").unwrap();
        assert_eq!(retrieved.internal_length, 16);
        assert_eq!(retrieved.alignment, "double");
    }

    #[test]
    fn test_custom_operator() {
        let registry = ExtensionRegistry::new();

        let op = CustomOperator::new(
            "~>".to_string(),
            "text".to_string(),
            "text".to_string(),
            "pair".to_string(),
        );

        registry.register_operator(op).unwrap();
    }

    #[test]
    fn test_foreign_data_wrapper() {
        let registry = ExtensionRegistry::new();

        let fdw = ForeignDataWrapper::new("file_fdw".to_string(), "file_fdw_handler".to_string())
            .with_validator("file_fdw_validator".to_string());

        registry.register_fdw("file_fdw".to_string(), fdw).unwrap();

        let retrieved = registry.get_fdw("file_fdw").unwrap();
        assert_eq!(retrieved.handler, "file_fdw_handler");
        assert!(retrieved.validator.is_some());
    }

    #[test]
    fn test_foreign_server() {
        let server = ForeignServer::new("csv_server".to_string(), "file_fdw".to_string())
            .with_option("filename".to_string(), "/data/file.csv".to_string());

        assert_eq!(server.options.get("filename").unwrap(), "/data/file.csv");
    }

    #[test]
    fn test_foreign_table() {
        let table = ForeignTable::new(
            "remote_users".to_string(),
            "postgres_server".to_string(),
            vec!["id".to_string(), "name".to_string()],
        );

        assert_eq!(table.columns.len(), 2);
        assert!(table.query().is_ok());
    }
}
