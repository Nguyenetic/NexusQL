//! SQL Abstract Syntax Tree
//! PROPRIETARY

use crate::types::Value;

/// SQL Statement types
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    CreateTable {
        name: String,
        columns: Vec<ColumnDef>,
    },
    Insert {
        table: String,
        columns: Option<Vec<String>>,
        values: Vec<Vec<Value>>,
    },
    Select {
        columns: Vec<String>,  // Empty = SELECT *
        from: String,
        where_clause: Option<Expr>,
        using_engine: Option<String>,
        limit: Option<usize>,
    },
    Delete {
        from: String,
        where_clause: Option<Expr>,
    },
    Update {
        table: String,
        set: Vec<(String, Value)>,
        where_clause: Option<Expr>,
    },
}

/// Column definition
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    Integer,
    Float,
    Text,
    Blob,
    Boolean,
}

/// SQL Expression
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Column reference
    Column(String),
    
    /// Literal value
    Literal(Value),
    
    /// Binary operation (>, <, =, AND, OR, etc.)
    BinaryOp {
        left: Box<Expr>,
        op: BinaryOperator,
        right: Box<Expr>,
    },
    
    /// Function call (e.g., SEMANTIC_SEARCH(...))
    Function {
        name: String,
        args: Vec<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOperator {
    Eq,          // =
    NotEq,       // !=
    Lt,          // <
    LtEq,        // <=
    Gt,          // >
    GtEq,        // >=
    And,         // AND
    Or,          // OR
    Like,        // LIKE
    Matches,     // MATCHES (custom)
}
