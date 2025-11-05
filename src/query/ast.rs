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
        values: Vec<Vec<Expr>>, // Changed from Value to Expr to support parameters
    },
    Select {
        columns: Vec<SelectItem>, // Items to select (columns, expressions, *)
        from: String,
        where_clause: Option<Expr>,
        using_engine: Option<String>,
        order_by: Option<OrderBy>,
        limit: Option<usize>,
    },
    Delete {
        from: String,
        where_clause: Option<Expr>,
    },
    Update {
        table: String,
        set: Vec<(String, Expr)>, // Changed from Value to Expr to support parameters
        where_clause: Option<Expr>,
    },
}

/// Column definition
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
}

/// ORDER BY clause
#[derive(Debug, Clone, PartialEq)]
pub struct OrderBy {
    pub expr: Expr,
    pub ascending: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    Integer,
    Float,
    Text,
    Blob,
    Boolean,
    /// Vector type with specified dimensions (e.g., VECTOR(384))
    Vector(usize),
}

/// SELECT list item (what appears in SELECT clause)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SelectItem {
    /// SELECT * (all columns)
    Wildcard,

    /// Expression with optional alias: expr AS alias
    Expr { expr: Expr, alias: Option<String> },
}

/// SQL Expression
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Expr {
    /// Column reference
    Column(String),

    /// Literal value
    Literal(Value),

    /// Parameter placeholder (? for positional, :name for named)
    Parameter(Parameter),

    /// Binary operation (>, <, =, AND, OR, etc.)
    BinaryOp {
        left: Box<Expr>,
        op: BinaryOperator,
        right: Box<Expr>,
    },

    /// Function call (e.g., SEMANTIC_SEARCH(...))
    Function { name: String, args: Vec<Expr> },
}

/// Parameter placeholder
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Parameter {
    /// Positional parameter (?)
    /// Index is 1-based to match SQL conventions
    Positional(usize),

    /// Named parameter (:name)
    Named(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum BinaryOperator {
    Eq,      // =
    NotEq,   // !=
    Lt,      // <
    LtEq,    // <=
    Gt,      // >
    GtEq,    // >=
    And,     // AND
    Or,      // OR
    Like,    // LIKE
    Matches, // MATCHES (custom)

    // Vector distance operators (pgvector-compatible)
    L2Distance,     // <-> (Euclidean distance)
    CosineDistance, // <=> (Cosine distance)
    InnerProduct,   // <#> (Negative inner product)
}
