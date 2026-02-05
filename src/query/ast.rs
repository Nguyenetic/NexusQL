//! SQL Abstract Syntax Tree
//! PROPRIETARY

use crate::types::Value;

/// SQL Statement types
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    CreateTable {
        name: String,
        columns: Vec<ColumnDef>,
        if_not_exists: bool,
    },
    CreateIndex {
        name: String,
        table: String,
        columns: Vec<String>,
        unique: bool,
        if_not_exists: bool,
        /// Index method (e.g., "hnsw", "btree", "gin")
        using: Option<String>,
        /// WITH clause options (e.g., m=16, ef_construction=100 for HNSW)
        with_options: Vec<IndexOption>,
    },
    DropTable {
        name: String,
        if_exists: bool,
    },
    Insert {
        table: String,
        columns: Option<Vec<String>>,
        values: Vec<Vec<Expr>>, // Changed from Value to Expr to support parameters
        /// Conflict handling mode (e.g., for INSERT OR REPLACE)
        on_conflict: Option<OnConflict>,
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
    Transaction {
        operation: TransactionOp,
    },
    Pragma {
        name: String,
        value: Option<String>,
    },
}

/// Transaction operations
#[derive(Debug, Clone, PartialEq)]
pub enum TransactionOp {
    Begin,
    Commit,
    Rollback,
}

/// Conflict handling for INSERT statements
#[derive(Debug, Clone, PartialEq)]
pub enum OnConflict {
    /// REPLACE: Delete existing row and insert new one (SQLite INSERT OR REPLACE)
    Replace,
    /// IGNORE: Skip insert if conflict occurs (SQLite INSERT OR IGNORE)
    Ignore,
    /// ROLLBACK: Rollback transaction on conflict (SQLite INSERT OR ROLLBACK)
    Rollback,
    /// ABORT: Abort statement on conflict (SQLite INSERT OR ABORT, default)
    Abort,
    /// FAIL: Fail statement but keep previous changes (SQLite INSERT OR FAIL)
    Fail,
    /// DO UPDATE: Update existing row (PostgreSQL ON CONFLICT DO UPDATE)
    DoUpdate {
        /// Columns that define the conflict target (e.g., primary key)
        conflict_target: Vec<String>,
        /// SET clause for the update
        update_set: Vec<(String, Expr)>,
    },
    /// DO NOTHING: Skip insert if conflict (PostgreSQL ON CONFLICT DO NOTHING)
    DoNothing {
        /// Optional conflict target columns
        conflict_target: Option<Vec<String>>,
    },
}

/// Column definition
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
    pub is_primary_key: bool,
    pub is_autoincrement: bool,
}

/// ORDER BY clause
#[derive(Debug, Clone, PartialEq)]
pub struct OrderBy {
    pub expr: Expr,
    pub ascending: bool,
}

/// Index option for WITH clause (e.g., m = 16, ef_construction = 100)
#[derive(Debug, Clone, PartialEq)]
pub struct IndexOption {
    pub name: String,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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

    /// Wildcard (*) - used in COUNT(*) and similar
    Wildcard,

    /// IS NULL check
    IsNull(Box<Expr>),

    /// IS NOT NULL check
    IsNotNull(Box<Expr>),

    /// Type cast expression (e.g., value::INTEGER, CAST(value AS INTEGER))
    Cast {
        expr: Box<Expr>,
        data_type: DataType,
    },
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
