// Stored Procedures & Functions System
// Week 10: Database Convenience Features
// PostgreSQL-compatible stored procedures with PL/NexusQL language

pub mod plnexus;

use crate::types::{Row, Value};
use crate::{NexusError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub use plnexus::{PLNexusExecutor, PLNexusFunction};

/// Function volatility (affects caching and optimization)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FunctionVolatility {
    /// Always returns same result for same inputs (can be cached)
    Immutable,
    /// Returns same result within a statement (can be optimized)
    Stable,
    /// May return different results even within a statement
    Volatile,
}

/// Function parameter mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParameterMode {
    /// Input parameter
    In,
    /// Output parameter
    Out,
    /// Input and output parameter
    InOut,
}

/// Function parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub data_type: String,
    pub mode: ParameterMode,
    pub default_value: Option<Value>,
}

impl Parameter {
    pub fn new(name: String, data_type: String) -> Self {
        Self {
            name,
            data_type,
            mode: ParameterMode::In,
            default_value: None,
        }
    }

    pub fn out(mut self) -> Self {
        self.mode = ParameterMode::Out;
        self
    }

    pub fn inout(mut self) -> Self {
        self.mode = ParameterMode::InOut;
        self
    }

    pub fn with_default(mut self, value: Value) -> Self {
        self.default_value = Some(value);
        self
    }
}

/// Function return type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReturnType {
    /// Returns void (procedure)
    Void,
    /// Returns scalar value
    Scalar(String),
    /// Returns table (set of rows)
    Table(Vec<String>), // Column types
    /// Returns setof (multiple values of same type)
    SetOf(String),
}

/// Stored function or procedure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredFunction {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: ReturnType,
    /// Language (plnexus, sql, etc.)
    pub language: String,
    /// Function body (SQL or procedural code)
    pub body: String,
    /// Volatility
    pub volatility: FunctionVolatility,
    /// Security context (DEFINER or INVOKER)
    pub security_definer: bool,
    /// Is this a procedure (vs function)?
    pub is_procedure: bool,
    pub created_at: u64,
}

impl StoredFunction {
    pub fn new(name: String, language: String, body: String) -> Self {
        Self {
            name,
            parameters: Vec::new(),
            return_type: ReturnType::Void,
            language,
            body,
            volatility: FunctionVolatility::Volatile,
            security_definer: false,
            is_procedure: false,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    pub fn with_parameters(mut self, params: Vec<Parameter>) -> Self {
        self.parameters = params;
        self
    }

    pub fn returns(mut self, return_type: ReturnType) -> Self {
        self.return_type = return_type;
        self
    }

    pub fn immutable(mut self) -> Self {
        self.volatility = FunctionVolatility::Immutable;
        self
    }

    pub fn stable(mut self) -> Self {
        self.volatility = FunctionVolatility::Stable;
        self
    }

    pub fn security_definer(mut self) -> Self {
        self.security_definer = true;
        self
    }

    pub fn procedure(mut self) -> Self {
        self.is_procedure = true;
        self.return_type = ReturnType::Void;
        self
    }

    /// Get function signature (for overloading)
    pub fn signature(&self) -> String {
        let param_types: Vec<_> = self
            .parameters
            .iter()
            .map(|p| p.data_type.as_str())
            .collect();
        format!("{}({})", self.name, param_types.join(", "))
    }
}

/// Function execution result
#[derive(Debug, Clone)]
pub enum FunctionResult {
    /// Scalar value
    Scalar(Value),
    /// Table result
    Table(Vec<Row>),
    /// Void (procedure)
    Void,
}

/// Procedure/Function Manager
pub struct ProcedureManager {
    /// All functions indexed by signature (allows overloading)
    functions: Arc<Mutex<HashMap<String, StoredFunction>>>,
    /// PL/NexusQL executor
    plnexus_executor: Arc<PLNexusExecutor>,
}

impl ProcedureManager {
    pub fn new() -> Self {
        Self {
            functions: Arc::new(Mutex::new(HashMap::new())),
            plnexus_executor: Arc::new(PLNexusExecutor::new()),
        }
    }

    /// Create a function or procedure
    pub fn create_function(&self, function: StoredFunction) -> Result<()> {
        let mut functions = self.functions.lock().unwrap();

        let signature = function.signature();
        if functions.contains_key(&signature) {
            return Err(NexusError::QueryError(format!(
                "Function {} already exists",
                signature
            )));
        }

        functions.insert(signature, function);
        Ok(())
    }

    /// Drop a function
    pub fn drop_function(&self, name: &str, param_types: &[String]) -> Result<()> {
        let mut functions = self.functions.lock().unwrap();

        let signature = format!("{}({})", name, param_types.join(", "));
        if !functions.contains_key(&signature) {
            return Err(NexusError::QueryError(format!(
                "Function {} does not exist",
                signature
            )));
        }

        functions.remove(&signature);
        Ok(())
    }

    /// Get function
    pub fn get_function(&self, name: &str, param_types: &[String]) -> Option<StoredFunction> {
        let functions = self.functions.lock().unwrap();
        let signature = format!("{}({})", name, param_types.join(", "));
        functions.get(&signature).cloned()
    }

    /// Execute a function
    pub fn execute_function(&self, name: &str, args: Vec<Value>) -> Result<FunctionResult> {
        let functions = self.functions.lock().unwrap();

        // Find matching function (simple type matching)
        let func = functions
            .values()
            .find(|f| f.name == name && f.parameters.len() == args.len())
            .ok_or_else(|| {
                NexusError::QueryError(format!(
                    "Function {} with {} parameters not found",
                    name,
                    args.len()
                ))
            })?
            .clone();

        drop(functions);

        // Execute based on language
        match func.language.as_str() {
            "plnexus" => self.plnexus_executor.execute(&func, args),
            "sql" => {
                // For SQL functions, execute the body as SQL
                // This is simplified - would need full SQL executor
                Ok(FunctionResult::Void)
            }
            _ => Err(NexusError::QueryError(format!(
                "Unknown language: {}",
                func.language
            ))),
        }
    }

    /// List all functions
    pub fn list_functions(&self) -> Vec<String> {
        let functions = self.functions.lock().unwrap();
        functions.keys().cloned().collect()
    }

    /// Get function count
    pub fn function_count(&self) -> usize {
        let functions = self.functions.lock().unwrap();
        functions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_creation() {
        let pm = ProcedureManager::new();

        let func = StoredFunction::new(
            "add_numbers".to_string(),
            "plnexus".to_string(),
            "RETURN a + b;".to_string(),
        )
        .with_parameters(vec![
            Parameter::new("a".to_string(), "INTEGER".to_string()),
            Parameter::new("b".to_string(), "INTEGER".to_string()),
        ])
        .returns(ReturnType::Scalar("INTEGER".to_string()));

        assert!(pm.create_function(func).is_ok());
        assert_eq!(pm.function_count(), 1);
    }

    #[test]
    fn test_function_overloading() {
        let pm = ProcedureManager::new();

        // Function with 1 parameter
        let func1 = StoredFunction::new(
            "test_func".to_string(),
            "plnexus".to_string(),
            "RETURN a;".to_string(),
        )
        .with_parameters(vec![Parameter::new("a".to_string(), "INTEGER".to_string())])
        .returns(ReturnType::Scalar("INTEGER".to_string()));

        // Function with 2 parameters (overload)
        let func2 = StoredFunction::new(
            "test_func".to_string(),
            "plnexus".to_string(),
            "RETURN a + b;".to_string(),
        )
        .with_parameters(vec![
            Parameter::new("a".to_string(), "INTEGER".to_string()),
            Parameter::new("b".to_string(), "INTEGER".to_string()),
        ])
        .returns(ReturnType::Scalar("INTEGER".to_string()));

        assert!(pm.create_function(func1).is_ok());
        assert!(pm.create_function(func2).is_ok());
        assert_eq!(pm.function_count(), 2);
    }

    #[test]
    fn test_procedure_vs_function() {
        let func = StoredFunction::new(
            "my_func".to_string(),
            "plnexus".to_string(),
            "RETURN 42;".to_string(),
        )
        .returns(ReturnType::Scalar("INTEGER".to_string()));

        let proc = StoredFunction::new(
            "my_proc".to_string(),
            "plnexus".to_string(),
            "INSERT INTO log VALUES ('test');".to_string(),
        )
        .procedure();

        assert!(!func.is_procedure);
        assert!(proc.is_procedure);
        assert!(matches!(proc.return_type, ReturnType::Void));
    }
}
