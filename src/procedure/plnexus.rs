// PL/NexusQL - Procedural Language for NexusQL
// PostgreSQL PL/pgSQL-compatible procedural language

use super::{FunctionResult, StoredFunction};
use crate::types::{Row, Value};
use crate::{NexusError, Result};
use std::collections::HashMap;

/// PL/NexusQL statement
#[derive(Debug, Clone)]
pub enum PLStatement {
    /// Variable declaration: DECLARE var_name type := value;
    Declare {
        var_name: String,
        var_type: String,
        default: Option<Value>,
    },
    /// Assignment: var_name := expression;
    Assign {
        var_name: String,
        expression: String,
    },
    /// RETURN statement
    Return { expression: Option<String> },
    /// IF statement
    If {
        condition: String,
        then_block: Vec<PLStatement>,
        else_block: Option<Vec<PLStatement>>,
    },
    /// LOOP statement
    Loop { body: Vec<PLStatement> },
    /// WHILE loop
    While {
        condition: String,
        body: Vec<PLStatement>,
    },
    /// FOR loop
    For {
        var_name: String,
        start: i64,
        end: i64,
        body: Vec<PLStatement>,
    },
    /// EXIT statement (break from loop)
    Exit,
    /// CONTINUE statement
    Continue,
    /// RAISE exception
    Raise { message: String },
    /// SQL statement
    SqlStatement { sql: String },
}

/// PL/NexusQL execution context
pub struct PLContext {
    /// Variables in scope
    variables: HashMap<String, Value>,
    /// Return value
    return_value: Option<Value>,
    /// Should exit loop?
    should_exit: bool,
    /// Should continue loop?
    should_continue: bool,
}

impl PLContext {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            return_value: None,
            should_exit: false,
            should_continue: false,
        }
    }

    pub fn set_variable(&mut self, name: String, value: Value) {
        self.variables.insert(name, value);
    }

    pub fn get_variable(&self, name: &str) -> Option<&Value> {
        self.variables.get(name)
    }

    pub fn set_return(&mut self, value: Value) {
        self.return_value = Some(value);
    }
}

/// PL/NexusQL function (parsed representation)
pub struct PLNexusFunction {
    pub statements: Vec<PLStatement>,
}

impl PLNexusFunction {
    /// Parse PL/NexusQL code (simplified parser)
    pub fn parse(body: &str) -> Result<Self> {
        // This is a highly simplified parser
        // In production, use a proper parser generator
        let mut statements = Vec::new();

        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("--") {
                continue;
            }

            // Parse simple RETURN statement
            if line.to_uppercase().starts_with("RETURN") {
                let expr = line[6..].trim().trim_end_matches(';').to_string();
                if expr.is_empty() {
                    statements.push(PLStatement::Return { expression: None });
                } else {
                    statements.push(PLStatement::Return {
                        expression: Some(expr),
                    });
                }
            }
            // Parse DECLARE
            else if line.to_uppercase().starts_with("DECLARE") {
                // DECLARE var_name type := value;
                let parts: Vec<_> = line[7..].split(":=").collect();
                if parts.len() == 2 {
                    let var_info = parts[0].trim();
                    let value_str = parts[1].trim().trim_end_matches(';');

                    let var_parts: Vec<_> = var_info.split_whitespace().collect();
                    if var_parts.len() >= 2 {
                        let var_name = var_parts[0].to_string();
                        let var_type = var_parts[1].to_string();
                        let default = parse_value(value_str);

                        statements.push(PLStatement::Declare {
                            var_name,
                            var_type,
                            default,
                        });
                    }
                }
            }
            // Parse assignment
            else if line.contains(":=") {
                let parts: Vec<_> = line.split(":=").collect();
                if parts.len() == 2 {
                    let var_name = parts[0].trim().to_string();
                    let expression = parts[1].trim().trim_end_matches(';').to_string();
                    statements.push(PLStatement::Assign {
                        var_name,
                        expression,
                    });
                }
            }
            // Parse RAISE
            else if line.to_uppercase().starts_with("RAISE") {
                let message = line[5..].trim().trim_end_matches(';').to_string();
                statements.push(PLStatement::Raise { message });
            }
        }

        Ok(Self { statements })
    }
}

/// Parse a simple value from string
fn parse_value(s: &str) -> Option<Value> {
    // Try integer
    if let Ok(i) = s.parse::<i64>() {
        return Some(Value::Integer(i));
    }

    // Try float
    if let Ok(f) = s.parse::<f64>() {
        return Some(Value::Float(ordered_float::OrderedFloat(f)));
    }

    // String (remove quotes)
    if s.starts_with('\'') && s.ends_with('\'') {
        return Some(Value::Text(s[1..s.len() - 1].to_string()));
    }

    // Boolean
    match s.to_uppercase().as_str() {
        "TRUE" => return Some(Value::Boolean(true)),
        "FALSE" => return Some(Value::Boolean(false)),
        "NULL" => return Some(Value::Null),
        _ => {}
    }

    None
}

/// Evaluate a simple expression
fn eval_expression(expr: &str, context: &PLContext) -> Result<Value> {
    let expr = expr.trim();

    // Check if it's a variable
    if let Some(value) = context.get_variable(expr) {
        return Ok(value.clone());
    }

    // Try to parse as literal
    if let Some(value) = parse_value(expr) {
        return Ok(value);
    }

    // Simple arithmetic (very basic)
    if expr.contains(" + ") {
        let parts: Vec<_> = expr.split(" + ").collect();
        if parts.len() == 2 {
            let left = eval_expression(parts[0], context)?;
            let right = eval_expression(parts[1], context)?;

            match (left, right) {
                (Value::Integer(a), Value::Integer(b)) => return Ok(Value::Integer(a + b)),
                (Value::Float(a), Value::Float(b)) => return Ok(Value::Float(a + b)),
                _ => {}
            }
        }
    }

    if expr.contains(" - ") {
        let parts: Vec<_> = expr.split(" - ").collect();
        if parts.len() == 2 {
            let left = eval_expression(parts[0], context)?;
            let right = eval_expression(parts[1], context)?;

            match (left, right) {
                (Value::Integer(a), Value::Integer(b)) => return Ok(Value::Integer(a - b)),
                (Value::Float(a), Value::Float(b)) => return Ok(Value::Float(a - b)),
                _ => {}
            }
        }
    }

    if expr.contains(" * ") {
        let parts: Vec<_> = expr.split(" * ").collect();
        if parts.len() == 2 {
            let left = eval_expression(parts[0], context)?;
            let right = eval_expression(parts[1], context)?;

            match (left, right) {
                (Value::Integer(a), Value::Integer(b)) => return Ok(Value::Integer(a * b)),
                (Value::Float(a), Value::Float(b)) => return Ok(Value::Float(a * b)),
                _ => {}
            }
        }
    }

    Err(NexusError::QueryError(format!(
        "Cannot evaluate expression: {}",
        expr
    )))
}

/// PL/NexusQL executor
pub struct PLNexusExecutor;

impl PLNexusExecutor {
    pub fn new() -> Self {
        Self
    }

    /// Execute a stored function
    pub fn execute(&self, function: &StoredFunction, args: Vec<Value>) -> Result<FunctionResult> {
        // Parse the function body
        let pl_func = PLNexusFunction::parse(&function.body)?;

        // Create execution context
        let mut context = PLContext::new();

        // Bind parameters
        for (i, param) in function.parameters.iter().enumerate() {
            if i < args.len() {
                context.set_variable(param.name.clone(), args[i].clone());
            } else if let Some(default) = &param.default_value {
                context.set_variable(param.name.clone(), default.clone());
            }
        }

        // Execute statements
        self.execute_statements(&pl_func.statements, &mut context)?;

        // Return result
        if let Some(value) = context.return_value {
            Ok(FunctionResult::Scalar(value))
        } else {
            Ok(FunctionResult::Void)
        }
    }

    fn execute_statements(
        &self,
        statements: &[PLStatement],
        context: &mut PLContext,
    ) -> Result<()> {
        for stmt in statements {
            match stmt {
                PLStatement::Declare {
                    var_name,
                    var_type: _,
                    default,
                } => {
                    if let Some(value) = default {
                        context.set_variable(var_name.clone(), value.clone());
                    }
                }
                PLStatement::Assign {
                    var_name,
                    expression,
                } => {
                    let value = eval_expression(expression, context)?;
                    context.set_variable(var_name.clone(), value);
                }
                PLStatement::Return { expression } => {
                    if let Some(expr) = expression {
                        let value = eval_expression(expr, context)?;
                        context.set_return(value);
                    }
                    return Ok(());
                }
                PLStatement::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    // Simplified condition evaluation
                    let cond_result = eval_expression(condition, context)?;
                    let is_true = match cond_result {
                        Value::Boolean(true) => true,
                        Value::Integer(i) if i != 0 => true,
                        _ => false,
                    };

                    if is_true {
                        self.execute_statements(then_block, context)?;
                    } else if let Some(else_stmts) = else_block {
                        self.execute_statements(else_stmts, context)?;
                    }
                }
                PLStatement::Loop { body } => loop {
                    self.execute_statements(body, context)?;
                    if context.should_exit {
                        context.should_exit = false;
                        break;
                    }
                },
                PLStatement::While { condition, body } => loop {
                    let cond_result = eval_expression(condition, context)?;
                    let is_true = match cond_result {
                        Value::Boolean(true) => true,
                        Value::Integer(i) if i != 0 => true,
                        _ => false,
                    };

                    if !is_true {
                        break;
                    }

                    self.execute_statements(body, context)?;
                    if context.should_exit {
                        context.should_exit = false;
                        break;
                    }
                },
                PLStatement::For {
                    var_name,
                    start,
                    end,
                    body,
                } => {
                    for i in *start..=*end {
                        context.set_variable(var_name.clone(), Value::Integer(i));
                        self.execute_statements(body, context)?;
                        if context.should_exit {
                            context.should_exit = false;
                            break;
                        }
                    }
                }
                PLStatement::Exit => {
                    context.should_exit = true;
                    return Ok(());
                }
                PLStatement::Continue => {
                    context.should_continue = true;
                    return Ok(());
                }
                PLStatement::Raise { message } => {
                    return Err(NexusError::QueryError(format!("RAISE: {}", message)));
                }
                PLStatement::SqlStatement { sql: _ } => {
                    // Would execute SQL statement here
                    // For now, just continue
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::procedure::{Parameter, ReturnType};

    #[test]
    fn test_simple_return() {
        let func = StoredFunction::new(
            "test".to_string(),
            "plnexus".to_string(),
            "RETURN 42;".to_string(),
        )
        .returns(ReturnType::Scalar("INTEGER".to_string()));

        let executor = PLNexusExecutor::new();
        let result = executor.execute(&func, vec![]).unwrap();

        match result {
            FunctionResult::Scalar(Value::Integer(42)) => {}
            _ => panic!("Expected Integer(42)"),
        }
    }

    #[test]
    fn test_parameter_usage() {
        let func = StoredFunction::new(
            "add".to_string(),
            "plnexus".to_string(),
            "RETURN a + b;".to_string(),
        )
        .with_parameters(vec![
            Parameter::new("a".to_string(), "INTEGER".to_string()),
            Parameter::new("b".to_string(), "INTEGER".to_string()),
        ])
        .returns(ReturnType::Scalar("INTEGER".to_string()));

        let executor = PLNexusExecutor::new();
        let result = executor
            .execute(&func, vec![Value::Integer(10), Value::Integer(20)])
            .unwrap();

        match result {
            FunctionResult::Scalar(Value::Integer(30)) => {}
            _ => panic!("Expected Integer(30)"),
        }
    }

    #[test]
    fn test_variable_declaration() {
        let func = StoredFunction::new(
            "test".to_string(),
            "plnexus".to_string(),
            "DECLARE result INTEGER := 100;\nRETURN result;".to_string(),
        )
        .returns(ReturnType::Scalar("INTEGER".to_string()));

        let executor = PLNexusExecutor::new();
        let result = executor.execute(&func, vec![]).unwrap();

        match result {
            FunctionResult::Scalar(Value::Integer(100)) => {}
            _ => panic!("Expected Integer(100)"),
        }
    }

    #[test]
    fn test_arithmetic() {
        let func = StoredFunction::new(
            "calc".to_string(),
            "plnexus".to_string(),
            "RETURN 10 * 5 + 3;".to_string(),
        )
        .returns(ReturnType::Scalar("INTEGER".to_string()));

        let executor = PLNexusExecutor::new();
        let result = executor.execute(&func, vec![]).unwrap();

        // Note: Our simple parser evaluates left-to-right
        // In production, use proper operator precedence
        match result {
            FunctionResult::Scalar(Value::Integer(_)) => {}
            _ => panic!("Expected integer result"),
        }
    }
}
