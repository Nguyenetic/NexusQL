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
        /// Common Table Expressions (WITH clause)
        with: Vec<CommonTableExpr>,
        columns: Vec<SelectItem>, // Items to select (columns, expressions, *)
        from: FromClause,
        where_clause: Option<Expr>,
        /// GROUP BY columns/expressions
        group_by: Vec<Expr>,
        /// HAVING clause (filter on aggregated results)
        having: Option<Expr>,
        using_engine: Option<String>,
        order_by: Option<OrderBy>,
        limit: Option<usize>,
        /// FETCH clauses for nested data retrieval (GraphQL-like)
        fetch: Vec<FetchClause>,
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
    /// Set operations (UNION, INTERSECT, EXCEPT)
    SetOperation {
        left: Box<Statement>,
        op: SetOperator,
        right: Box<Statement>,
        /// For final ORDER BY and LIMIT
        order_by: Option<OrderBy>,
        limit: Option<usize>,
    },
    /// Create relationship for FETCH operations
    CreateRelationship(Relationship),
    /// TRAVERSE for graph operations
    Traverse {
        clause: TraverseClause,
        from_table: String,
        where_clause: Option<Expr>,
        return_columns: Vec<SelectItem>,
        limit: Option<usize>,
    },
}

/// SQL Set operators
#[derive(Debug, Clone, PartialEq)]
pub enum SetOperator {
    /// UNION - combines and removes duplicates
    Union,
    /// UNION ALL - combines and keeps duplicates
    UnionAll,
    /// INTERSECT - only rows in both
    Intersect,
    /// INTERSECT ALL - keeps duplicates of rows in both
    IntersectAll,
    /// EXCEPT - rows in left but not in right
    Except,
    /// EXCEPT ALL - keeps duplicates, removes matching count
    ExceptAll,
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum DataType {
    Integer,
    Float,
    Text,
    Blob,
    Boolean,
    /// Vector type with specified dimensions (e.g., VECTOR(384))
    Vector(usize),
    Geometry,
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

    /// Scalar subquery - returns a single value
    /// e.g., SELECT (SELECT MAX(salary) FROM employees) as max_sal
    Subquery(Box<SubqueryExpr>),

    /// IN subquery - checks if value is in subquery result
    /// e.g., WHERE id IN (SELECT user_id FROM orders)
    InSubquery {
        expr: Box<Expr>,
        subquery: Box<SubqueryExpr>,
        negated: bool,
    },

    /// EXISTS subquery - checks if subquery returns any rows
    /// e.g., WHERE EXISTS (SELECT 1 FROM orders WHERE user_id = users.id)
    ExistsSubquery {
        subquery: Box<SubqueryExpr>,
        negated: bool,
    },

    /// Window function with OVER clause
    /// e.g., ROW_NUMBER() OVER (PARTITION BY dept ORDER BY salary DESC)
    WindowFunction {
        function: Box<Expr>,
        window_spec: WindowSpec,
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

/// Subquery expression wrapper
/// Contains a SELECT statement that can be used in expressions
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SubqueryExpr {
    /// The SELECT columns
    pub columns: Vec<SelectItem>,
    /// FROM clause (can be a table or another subquery)
    pub from: TableRef,
    /// WHERE clause
    pub where_clause: Option<Box<Expr>>,
    /// GROUP BY
    pub group_by: Vec<Expr>,
    /// HAVING
    pub having: Option<Box<Expr>>,
    /// ORDER BY
    pub order_by: Option<OrderBy>,
    /// LIMIT
    pub limit: Option<usize>,
}

/// Table reference - can be a table name or a derived table (subquery)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TableRef {
    /// Simple table reference
    Table { name: String, alias: Option<String> },
    /// Derived table (subquery in FROM clause)
    Subquery {
        subquery: Box<SubqueryExpr>,
        alias: String, // Alias is required for derived tables
    },
}

/// Window specification for window functions
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WindowSpec {
    /// PARTITION BY columns
    pub partition_by: Vec<Expr>,
    /// ORDER BY within the window
    pub order_by: Vec<WindowOrderBy>,
    /// Window frame (ROWS/RANGE BETWEEN ... AND ...)
    pub frame: Option<WindowFrame>,
}

/// ORDER BY item within a window specification
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WindowOrderBy {
    pub expr: Expr,
    pub ascending: bool,
    pub nulls_first: Option<bool>,
}

/// Window frame specification
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WindowFrame {
    pub units: WindowFrameUnits,
    pub start: WindowFrameBound,
    pub end: Option<WindowFrameBound>,
}

/// Window frame units (ROWS vs RANGE)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum WindowFrameUnits {
    Rows,
    Range,
    Groups,
}

/// Window frame bound
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum WindowFrameBound {
    /// CURRENT ROW
    CurrentRow,
    /// UNBOUNDED PRECEDING
    UnboundedPreceding,
    /// UNBOUNDED FOLLOWING
    UnboundedFollowing,
    /// N PRECEDING
    Preceding(u64),
    /// N FOLLOWING
    Following(u64),
}

/// Common Table Expression (CTE) for WITH clause
#[derive(Debug, Clone, PartialEq)]
pub struct CommonTableExpr {
    /// CTE name
    pub name: String,
    /// Optional column list
    pub columns: Option<Vec<String>>,
    /// The query definition
    pub query: Box<SubqueryExpr>,
    /// Whether this is a recursive CTE
    pub recursive: bool,
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

/// JOIN type for SQL joins
#[derive(Debug, Clone, PartialEq)]
pub enum JoinType {
    /// INNER JOIN - only matching rows
    Inner,
    /// LEFT [OUTER] JOIN - all left rows + matching right
    Left,
    /// RIGHT [OUTER] JOIN - all right rows + matching left
    Right,
    /// FULL [OUTER] JOIN - all rows from both tables
    Full,
    /// CROSS JOIN - cartesian product
    Cross,
}

/// JOIN clause representing a single join operation
#[derive(Debug, Clone, PartialEq)]
pub struct JoinClause {
    /// Table to join with
    pub table: String,
    /// Optional table alias
    pub alias: Option<String>,
    /// Type of join
    pub join_type: JoinType,
    /// ON condition (None for CROSS JOIN)
    pub on_condition: Option<Expr>,
}

/// FROM clause with optional joins
#[derive(Debug, Clone, PartialEq)]
pub struct FromClause {
    /// Primary table
    pub table: String,
    /// Optional table alias
    pub alias: Option<String>,
    /// JOIN clauses
    pub joins: Vec<JoinClause>,
}

/// FETCH clause for GraphQL-like nested data retrieval
#[derive(Debug, Clone, PartialEq)]
pub struct FetchClause {
    /// Relationship name or target table
    pub relationship: String,
    /// Columns to fetch from the related table
    pub columns: Vec<SelectItem>,
    /// Optional filter on related rows
    pub where_clause: Option<Expr>,
    /// Ordering of related rows
    pub order_by: Option<OrderBy>,
    /// Limit on related rows
    pub limit: Option<usize>,
    /// Nested FETCH clauses (for multi-level nesting)
    pub nested_fetch: Vec<FetchClause>,
}

/// TRAVERSE clause for graph operations
#[derive(Debug, Clone, PartialEq)]
pub struct TraverseClause {
    /// Depth range (min..max)
    pub min_depth: usize,
    pub max_depth: usize,
    /// Relationship pattern to traverse
    pub pattern: TraversePattern,
    /// Starting condition
    pub start_where: Option<Expr>,
    /// Traversal mode
    pub mode: TraverseMode,
}

/// Pattern for graph traversal
#[derive(Debug, Clone, PartialEq)]
pub struct TraversePattern {
    /// Source node label (optional)
    pub source_label: Option<String>,
    /// Relationship type
    pub relationship_type: String,
    /// Target node label (optional)
    pub target_label: Option<String>,
    /// Direction
    pub direction: TraverseDirection,
}

/// Direction for graph traversal
#[derive(Debug, Clone, PartialEq)]
pub enum TraverseDirection {
    /// ->
    Outgoing,
    /// <-
    Incoming,
    /// --
    Both,
}

/// Mode for traversal
#[derive(Debug, Clone, PartialEq)]
pub enum TraverseMode {
    /// Find all matching paths
    All,
    /// Find shortest path only
    ShortestPath,
    /// Detect cycles
    Cycle,
    /// Breadth-first search
    Bfs,
    /// Depth-first search
    Dfs,
}

/// Relationship definition
#[derive(Debug, Clone, PartialEq)]
pub struct Relationship {
    /// Relationship name
    pub name: String,
    /// Source table.column
    pub from_table: String,
    pub from_column: String,
    /// Target table.column
    pub to_table: String,
    pub to_column: String,
    /// Cardinality
    pub cardinality: Cardinality,
}

/// Relationship cardinality
#[derive(Debug, Clone, PartialEq)]
pub enum Cardinality {
    /// One-to-one
    One,
    /// One-to-many
    Many,
}
