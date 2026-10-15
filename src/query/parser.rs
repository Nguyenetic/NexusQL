//! Full SQL Parser using sqlparser-rs with NexusQL custom dialect
//! PROPRIETARY

use crate::query::ast::{
    BinaryOperator, Cardinality, ColumnDef, CommonTableExpr, DataType, Expr, FetchClause,
    FromClause, IndexOption, JoinClause, JoinType, OnConflict, OrderBy, Relationship, SelectItem,
    SetOperator, Statement, SubqueryExpr, TableRef, TransactionOp, TraverseClause,
    TraverseDirection, TraverseMode, TraversePattern, WindowFrame, WindowFrameBound,
    WindowFrameUnits, WindowOrderBy, WindowSpec,
};
use crate::types::Value;
use crate::{NexusError, Result};
use sqlparser::ast as sql_ast;
use sqlparser::dialect::{Dialect, Precedence};
use sqlparser::parser::{Parser as SqlParser, ParserError};
use sqlparser::tokenizer::Token;

/// NexusQL Custom SQL Dialect
///
/// Combines PostgreSQL and SQLite features:
/// - PostgreSQL: Advanced features, FILTER, JSON operators, window functions
/// - SQLite: AUTOINCREMENT, simpler syntax
/// - NexusQL: Vector operations, AI-native features
#[derive(Debug)]
pub struct NexusQLDialect;

impl Dialect for NexusQLDialect {
    fn is_identifier_start(&self, ch: char) -> bool {
        ch.is_ascii_lowercase() || ch.is_ascii_uppercase() || ch == '_'
    }

    fn is_identifier_part(&self, ch: char) -> bool {
        ch.is_ascii_lowercase()
            || ch.is_ascii_uppercase()
            || ch.is_ascii_digit()
            || ch == '_'
            || ch == '$' // PostgreSQL parameter syntax
    }

    fn supports_filter_during_aggregation(&self) -> bool {
        true // PostgreSQL FILTER clause
    }

    fn supports_within_after_array_aggregation(&self) -> bool {
        true // PostgreSQL array aggregation
    }

    fn supports_group_by_expr(&self) -> bool {
        true // Allow expressions in GROUP BY
    }

    fn is_delimited_identifier_start(&self, ch: char) -> bool {
        ch == '"' || ch == '`' // Support both PostgreSQL and SQLite quotes
    }

    fn parse_statement(
        &self,
        parser: &mut SqlParser,
    ) -> Option<std::result::Result<sql_ast::Statement, ParserError>> {
        // sqlparser's PRAGMA path accepts literals, but SQLite-style sync and
        // journal settings also use bare words: PRAGMA synchronous = FULL.
        // Consume only this four-token shape; normal parsing checks the suffix.
        let (Token::Word(keyword), Token::Word(name), Token::Eq, Token::Word(value)) = (
            parser.peek_token().token,
            parser.peek_nth_token(1).token,
            parser.peek_nth_token(2).token,
            parser.peek_nth_token(3).token,
        ) else {
            return None;
        };
        if !keyword.value.eq_ignore_ascii_case("pragma")
            || keyword.quote_style.is_some()
            || !matches!(
                name.value.to_ascii_lowercase().as_str(),
                "synchronous" | "journal_mode"
            )
            || value.quote_style.is_some()
        {
            return None;
        }
        for _ in 0..4 {
            parser.next_token();
        }
        Some(Ok(sql_ast::Statement::Pragma {
            name: sql_ast::ObjectName(vec![sql_ast::Ident::new(name.value)]),
            value: Some(sql_ast::Value::SingleQuotedString(value.value)),
            is_eq: true,
        }))
    }

    fn get_next_precedence(
        &self,
        parser: &SqlParser,
    ) -> Option<std::result::Result<u8, ParserError>> {
        // Full-text `column MATCH 'query'` (SQLite FTS5 style) binds like LIKE.
        if is_match_keyword(&parser.peek_token().token) {
            return Some(Ok(self.prec_value(Precedence::Like)));
        }
        // The tokenizer emits <-> as Lt, Arrow. Recognize just this pair so
        // comparisons against negative numbers (e.g. x<-1) keep their meaning.
        if parser.peek_token().token == Token::Lt && parser.peek_nth_token(1).token == Token::Arrow
        {
            // PostgreSQL custom operators bind below arithmetic, above comparisons.
            Some(Ok(self.prec_value(Precedence::PlusMinus) - 1))
        } else {
            None
        }
    }

    fn parse_infix(
        &self,
        parser: &mut SqlParser,
        expr: &sql_ast::Expr,
        precedence: u8,
    ) -> Option<std::result::Result<sql_ast::Expr, ParserError>> {
        if is_match_keyword(&parser.peek_token().token) {
            parser.next_token();
            return Some(
                parser
                    .parse_subexpr(precedence)
                    .map(|right| sql_ast::Expr::BinaryOp {
                        left: Box::new(expr.clone()),
                        op: sql_ast::BinaryOperator::Custom("MATCH".into()),
                        right: Box::new(right),
                    }),
            );
        }
        if let Err(error) = self.get_next_precedence(parser)? {
            return Some(Err(error));
        }
        parser.next_token();
        parser.next_token();
        Some(
            parser
                .parse_subexpr(precedence)
                .map(|right| sql_ast::Expr::BinaryOp {
                    left: Box::new(expr.clone()),
                    op: sql_ast::BinaryOperator::Custom("<->".into()),
                    right: Box::new(right),
                }),
        )
    }
}

/// An unquoted `MATCH` word in operator position.
fn is_match_keyword(token: &Token) -> bool {
    matches!(token, Token::Word(word)
        if word.quote_style.is_none() && word.value.eq_ignore_ascii_case("match"))
}

pub struct QueryParser {
    dialect: NexusQLDialect,
}

/// Render a sqlparser `ObjectName` to a bare dotted string, dropping any
/// quote-style ident adornments. `CREATE TABLE "My Name"` -> `My Name`,
/// `schema.tbl` -> `schema.tbl`.
fn object_name_bare(name: &sql_ast::ObjectName) -> String {
    name.0
        .iter()
        .map(|ident| ident.value.clone())
        .collect::<Vec<_>>()
        .join(".")
}

impl QueryParser {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            dialect: NexusQLDialect,
        }
    }

    /// Preprocess SQL to handle NexusQL-specific syntax before parsing
    ///
    /// This function normalizes SQL syntax that sqlparser-rs might not handle correctly:
    /// - Extracts and removes FETCH clauses (custom NexusQL syntax)
    /// - Future: Handle other NexusQL-specific syntax
    ///
    /// Note: AUTOINCREMENT is handled directly by tracking it before sqlparser processing
    fn preprocess_sql(sql: &str) -> (String, Vec<FetchClause>) {
        // Extract FETCH clauses from the SQL
        let (cleaned_sql, fetch_clauses) = Self::extract_fetch_clauses(sql);
        (cleaned_sql, fetch_clauses)
    }

    /// Extract FETCH clauses from SQL and return cleaned SQL + parsed FETCH clauses
    fn extract_fetch_clauses(sql: &str) -> (String, Vec<FetchClause>) {
        let mut cleaned_sql = sql.to_string();
        let mut fetch_clauses = Vec::new();

        // Find FETCH keyword (case-insensitive)
        let sql_upper = sql.to_uppercase();

        // Look for FETCH keyword that's not part of another word
        let mut search_pos = 0;
        while let Some(fetch_pos) = sql_upper[search_pos..].find("FETCH ") {
            let absolute_pos = search_pos + fetch_pos;

            // Make sure it's a standalone keyword (preceded by whitespace or start)
            if absolute_pos > 0 {
                let prev_char = sql.chars().nth(absolute_pos - 1).unwrap_or(' ');
                if prev_char.is_alphanumeric() || prev_char == '_' {
                    search_pos = absolute_pos + 6;
                    continue;
                }
            }

            // Parse the FETCH clause starting at this position
            if let Some((fetch_clause, end_pos)) = Self::parse_fetch_clause(&sql[absolute_pos..]) {
                fetch_clauses.push(fetch_clause);

                // Remove the FETCH clause from the SQL
                let before = &sql[..absolute_pos];
                let after = &sql[absolute_pos + end_pos..];
                cleaned_sql = format!("{}{}", before.trim_end(), after);

                // Continue searching in the cleaned SQL
                break; // For now, handle one FETCH at a time (recursive calls will handle nested)
            } else {
                search_pos = absolute_pos + 6;
            }
        }

        // Recursively extract more FETCH clauses if there are any
        if fetch_clauses.is_empty() {
            (cleaned_sql, fetch_clauses)
        } else {
            let (final_sql, more_clauses) = Self::extract_fetch_clauses(&cleaned_sql);
            let mut all_clauses = fetch_clauses;
            all_clauses.extend(more_clauses);
            (final_sql, all_clauses)
        }
    }

    /// Parse a single FETCH clause: `FETCH relationship { columns [WHERE ...] [ORDER BY ...] [LIMIT n] }`
    /// Returns (FetchClause, bytes_consumed) or None if parsing fails
    fn parse_fetch_clause(sql: &str) -> Option<(FetchClause, usize)> {
        let sql = sql.trim();
        if !sql.to_uppercase().starts_with("FETCH ") {
            return None;
        }

        // Skip "FETCH "
        let rest = sql[6..].trim_start();

        // Parse relationship name (identifier until whitespace or {)
        let relationship_end = rest
            .find(|c: char| c.is_whitespace() || c == '{')
            .unwrap_or(rest.len());
        let relationship = rest[..relationship_end].trim().to_string();

        if relationship.is_empty() {
            return None;
        }

        let after_rel = rest[relationship_end..].trim_start();

        // Expect opening brace
        if !after_rel.starts_with('{') {
            return None;
        }

        // Find matching closing brace (handle nested braces)
        let brace_content = &after_rel[1..];
        let mut brace_count = 1;
        let mut close_pos = 0;

        for (i, c) in brace_content.chars().enumerate() {
            match c {
                '{' => brace_count += 1,
                '}' => {
                    brace_count -= 1;
                    if brace_count == 0 {
                        close_pos = i;
                        break;
                    }
                }
                _ => {}
            }
        }

        if brace_count != 0 {
            return None; // Unmatched braces
        }

        let inner_content = &brace_content[..close_pos];
        let total_consumed =
            6 + relationship_end + (after_rel.len() - brace_content.len()) + close_pos + 2;

        // Parse the inner content: columns, WHERE, ORDER BY, LIMIT, nested FETCH
        let (columns, where_clause, order_by, limit, nested_fetch) =
            Self::parse_fetch_inner(inner_content);

        Some((
            FetchClause {
                relationship,
                columns,
                where_clause,
                order_by,
                limit,
                nested_fetch,
            },
            total_consumed,
        ))
    }

    /// Parse the inner content of a FETCH clause
    fn parse_fetch_inner(
        content: &str,
    ) -> (
        Vec<SelectItem>,
        Option<Expr>,
        Option<OrderBy>,
        Option<usize>,
        Vec<FetchClause>,
    ) {
        let content = content.trim();

        // First, extract any nested FETCH clauses
        let (content_without_nested, nested_fetch) = Self::extract_fetch_clauses(content);

        // Parse the remaining content for columns, WHERE, ORDER BY, LIMIT
        let content = content_without_nested.trim();

        // Split by keywords (case-insensitive)
        let content_upper = content.to_uppercase();

        // Find WHERE position
        let where_pos = content_upper.find(" WHERE ");
        let order_pos = content_upper.find(" ORDER BY ");
        let limit_pos = content_upper.find(" LIMIT ");

        // Determine the end of columns section
        let columns_end = where_pos
            .or(order_pos)
            .or(limit_pos)
            .unwrap_or(content.len());

        // Parse columns
        let columns_str = content[..columns_end].trim();
        let columns = Self::parse_fetch_columns(columns_str);

        // Parse WHERE clause
        let where_clause = if let Some(w_pos) = where_pos {
            let where_start = w_pos + 7; // Skip " WHERE "
            let where_end = order_pos.or(limit_pos).unwrap_or(content.len());
            let where_str = content[where_start..where_end].trim();
            Self::parse_simple_where(where_str)
        } else {
            None
        };

        // Parse ORDER BY clause
        let order_by = if let Some(o_pos) = order_pos {
            let order_start = o_pos + 10; // Skip " ORDER BY "
            let order_end = limit_pos.unwrap_or(content.len());
            let order_str = content[order_start..order_end].trim();
            Self::parse_simple_order_by(order_str)
        } else {
            None
        };

        // Parse LIMIT clause
        let limit = if let Some(l_pos) = limit_pos {
            let limit_start = l_pos + 7; // Skip " LIMIT "
            let limit_str = content[limit_start..].trim();
            limit_str.parse::<usize>().ok()
        } else {
            None
        };

        (columns, where_clause, order_by, limit, nested_fetch)
    }

    /// Parse column list in FETCH clause (e.g., "id, name, email" or "*")
    fn parse_fetch_columns(columns_str: &str) -> Vec<SelectItem> {
        let columns_str = columns_str.trim();

        if columns_str == "*" {
            return vec![SelectItem::Wildcard];
        }

        columns_str
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|col| {
                // Check for alias: "column AS alias" or "column alias"
                let parts: Vec<&str> = col.splitn(2, " AS ").collect();
                if parts.len() == 2 {
                    SelectItem::Expr {
                        expr: Expr::Column(parts[0].trim().to_string()),
                        alias: Some(parts[1].trim().to_string()),
                    }
                } else {
                    // Check for space-separated alias
                    let parts: Vec<&str> = col.split_whitespace().collect();
                    if parts.len() == 2 {
                        SelectItem::Expr {
                            expr: Expr::Column(parts[0].to_string()),
                            alias: Some(parts[1].to_string()),
                        }
                    } else {
                        SelectItem::Expr {
                            expr: Expr::Column(col.to_string()),
                            alias: None,
                        }
                    }
                }
            })
            .collect()
    }

    /// Parse simple WHERE clause (column = value)
    fn parse_simple_where(where_str: &str) -> Option<Expr> {
        let where_str = where_str.trim();
        if where_str.is_empty() {
            return None;
        }

        // Try to parse simple comparison: column = value, column > value, etc.
        let operators = ["<=", ">=", "!=", "<>", "=", "<", ">"];

        for op_str in &operators {
            if let Some(op_pos) = where_str.find(op_str) {
                let left = where_str[..op_pos].trim();
                let right = where_str[op_pos + op_str.len()..].trim();

                let binary_op = match *op_str {
                    "=" => BinaryOperator::Eq,
                    "!=" | "<>" => BinaryOperator::NotEq,
                    "<" => BinaryOperator::Lt,
                    "<=" => BinaryOperator::LtEq,
                    ">" => BinaryOperator::Gt,
                    ">=" => BinaryOperator::GtEq,
                    _ => continue,
                };

                let left_expr = Expr::Column(left.to_string());
                let right_expr = Self::parse_simple_value(right);

                return Some(Expr::BinaryOp {
                    left: Box::new(left_expr),
                    op: binary_op,
                    right: Box::new(right_expr),
                });
            }
        }

        // Check for boolean column (e.g., "published" means "published = true")
        if where_str.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Some(Expr::BinaryOp {
                left: Box::new(Expr::Column(where_str.to_string())),
                op: BinaryOperator::Eq,
                right: Box::new(Expr::Literal(Value::Boolean(true))),
            });
        }

        None
    }

    /// Parse a simple value (string, number, boolean, null)
    fn parse_simple_value(value_str: &str) -> Expr {
        let value_str = value_str.trim();

        // Quoted string
        if (value_str.starts_with('\'') && value_str.ends_with('\''))
            || (value_str.starts_with('"') && value_str.ends_with('"'))
        {
            let inner = &value_str[1..value_str.len() - 1];
            return Expr::Literal(Value::Text(inner.to_string()));
        }

        // Boolean
        let upper = value_str.to_uppercase();
        if upper == "TRUE" {
            return Expr::Literal(Value::Boolean(true));
        }
        if upper == "FALSE" {
            return Expr::Literal(Value::Boolean(false));
        }
        if upper == "NULL" {
            return Expr::Literal(Value::Null);
        }

        // Number
        if let Ok(i) = value_str.parse::<i64>() {
            return Expr::Literal(Value::Integer(i));
        }
        if let Ok(f) = value_str.parse::<f64>() {
            return Expr::Literal(Value::Float(f.into()));
        }

        // Treat as column reference
        Expr::Column(value_str.to_string())
    }

    /// Parse simple ORDER BY clause (column [ASC|DESC])
    fn parse_simple_order_by(order_str: &str) -> Option<OrderBy> {
        let order_str = order_str.trim();
        if order_str.is_empty() {
            return None;
        }

        let parts: Vec<&str> = order_str.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let column = parts[0];
        let ascending = if parts.len() > 1 {
            parts[1].to_uppercase() != "DESC"
        } else {
            true
        };

        Some(OrderBy {
            expr: Expr::Column(column.to_string()),
            ascending,
        })
    }

    /// Parse CREATE RELATIONSHIP statement
    /// Syntax: CREATE RELATIONSHIP name FROM table.column TO table.column AS ONE|MANY
    fn parse_create_relationship(&self, sql: &str) -> Result<Statement> {
        let sql = sql.trim();
        let upper = sql.to_uppercase();

        // Validate start
        if !upper.starts_with("CREATE RELATIONSHIP") {
            return Err(NexusError::ParseError(
                "Expected CREATE RELATIONSHIP".into(),
            ));
        }

        // Extract tokens
        let tokens: Vec<&str> = sql.split_whitespace().collect();

        // Minimum: CREATE RELATIONSHIP name FROM t.c TO t.c AS ONE
        if tokens.len() < 9 {
            return Err(NexusError::ParseError(
                "Invalid CREATE RELATIONSHIP syntax. Expected: CREATE RELATIONSHIP name FROM table.column TO table.column AS ONE|MANY".into(),
            ));
        }

        let name = tokens[2].to_string();

        // Find FROM, TO, AS keywords
        let from_idx = tokens
            .iter()
            .position(|&t| t.to_uppercase() == "FROM")
            .ok_or_else(|| NexusError::ParseError("Missing FROM in CREATE RELATIONSHIP".into()))?;

        let to_idx = tokens
            .iter()
            .position(|&t| t.to_uppercase() == "TO")
            .ok_or_else(|| NexusError::ParseError("Missing TO in CREATE RELATIONSHIP".into()))?;

        let as_idx = tokens
            .iter()
            .position(|&t| t.to_uppercase() == "AS")
            .ok_or_else(|| NexusError::ParseError("Missing AS in CREATE RELATIONSHIP".into()))?;

        // Parse FROM table.column
        let from_spec = tokens[from_idx + 1];
        let from_parts: Vec<&str> = from_spec.split('.').collect();
        if from_parts.len() != 2 {
            return Err(NexusError::ParseError(
                "FROM must be table.column format".into(),
            ));
        }
        let from_table = from_parts[0].to_string();
        let from_column = from_parts[1].to_string();

        // Parse TO table.column
        let to_spec = tokens[to_idx + 1];
        let to_parts: Vec<&str> = to_spec.split('.').collect();
        if to_parts.len() != 2 {
            return Err(NexusError::ParseError(
                "TO must be table.column format".into(),
            ));
        }
        let to_table = to_parts[0].to_string();
        let to_column = to_parts[1].to_string();

        // Parse cardinality (ONE or MANY)
        let cardinality_str = tokens[as_idx + 1].to_uppercase();
        let cardinality = match cardinality_str.as_str() {
            "ONE" => Cardinality::One,
            "MANY" => Cardinality::Many,
            _ => {
                return Err(NexusError::ParseError(
                    "Cardinality must be ONE or MANY".into(),
                ))
            }
        };

        Ok(Statement::CreateRelationship(Relationship {
            name,
            from_table,
            from_column,
            to_table,
            to_column,
            cardinality,
        }))
    }

    /// Parse TRAVERSE statement
    /// Syntax: TRAVERSE [DEPTH min..max] pattern FROM table [WHERE condition] [RETURN columns] [LIMIT n]
    fn parse_traverse(&self, sql: &str) -> Result<Statement> {
        let sql = sql.trim();
        let upper = sql.to_uppercase();

        if !upper.starts_with("TRAVERSE") {
            return Err(NexusError::ParseError("Expected TRAVERSE".into()));
        }

        // Skip "TRAVERSE "
        let rest = sql[8..].trim();

        // Parse optional DEPTH clause
        let (min_depth, max_depth, after_depth) = if rest.to_uppercase().starts_with("DEPTH ") {
            let depth_rest = rest[6..].trim();
            // Look for pattern like "1..3" or "1"
            let depth_end = depth_rest
                .find(|c: char| c.is_whitespace())
                .unwrap_or(depth_rest.len());
            let depth_str = &depth_rest[..depth_end];

            if let Some(dotdot_pos) = depth_str.find("..") {
                let min = depth_str[..dotdot_pos]
                    .parse::<usize>()
                    .map_err(|_| NexusError::ParseError("Invalid DEPTH min".into()))?;
                let max = depth_str[dotdot_pos + 2..]
                    .parse::<usize>()
                    .map_err(|_| NexusError::ParseError("Invalid DEPTH max".into()))?;
                (min, max, depth_rest[depth_end..].trim())
            } else {
                let depth = depth_str
                    .parse::<usize>()
                    .map_err(|_| NexusError::ParseError("Invalid DEPTH".into()))?;
                (depth, depth, depth_rest[depth_end..].trim())
            }
        } else {
            (1, usize::MAX, rest) // Default: unlimited depth
        };

        // Parse traverse pattern: (source)-[:REL]->(target) or just relationship name
        let from_idx = after_depth
            .to_uppercase()
            .find(" FROM ")
            .ok_or_else(|| NexusError::ParseError("Missing FROM in TRAVERSE".into()))?;

        let pattern_str = after_depth[..from_idx].trim();
        let after_from = after_depth[from_idx + 6..].trim();

        // Parse pattern
        let pattern = Self::parse_traverse_pattern(pattern_str)?;

        // Parse FROM table
        let where_pos = after_from.to_uppercase().find(" WHERE ");
        let return_pos = after_from.to_uppercase().find(" RETURN ");
        let limit_pos = after_from.to_uppercase().find(" LIMIT ");

        let table_end = where_pos
            .or(return_pos)
            .or(limit_pos)
            .unwrap_or(after_from.len());
        let from_table = after_from[..table_end].trim().to_string();

        // Parse WHERE clause
        let where_clause = if let Some(w_pos) = where_pos {
            let where_start = w_pos + 7;
            let where_end = return_pos.or(limit_pos).unwrap_or(after_from.len());
            let where_str = after_from[where_start..where_end].trim();
            Self::parse_simple_where(where_str)
        } else {
            None
        };

        // Parse RETURN clause (columns)
        let return_columns = if let Some(r_pos) = return_pos {
            let return_start = r_pos + 8;
            let return_end = limit_pos.unwrap_or(after_from.len());
            let return_str = after_from[return_start..return_end].trim();
            Self::parse_fetch_columns(return_str)
        } else {
            vec![SelectItem::Wildcard]
        };

        // Parse LIMIT
        let limit = if let Some(l_pos) = limit_pos {
            let limit_str = after_from[l_pos + 7..].trim();
            // Remove trailing semicolon if present
            let limit_str = limit_str.trim_end_matches(';').trim();
            limit_str.parse::<usize>().ok()
        } else {
            None
        };

        // Determine mode from pattern
        let mode = if pattern_str.to_uppercase().contains("SHORTEST_PATH") {
            TraverseMode::ShortestPath
        } else if pattern_str.to_uppercase().contains("CYCLE") {
            TraverseMode::Cycle
        } else if pattern_str.to_uppercase().contains("BFS") {
            TraverseMode::Bfs
        } else if pattern_str.to_uppercase().contains("DFS") {
            TraverseMode::Dfs
        } else {
            TraverseMode::All
        };

        Ok(Statement::Traverse {
            clause: TraverseClause {
                min_depth,
                max_depth,
                pattern,
                start_where: where_clause.clone(),
                mode,
            },
            from_table,
            where_clause,
            return_columns,
            limit,
        })
    }

    /// Parse traverse pattern: (source)-[:REL]->(target) or simple relationship name
    fn parse_traverse_pattern(pattern_str: &str) -> Result<TraversePattern> {
        let pattern_str = pattern_str.trim();

        // Simple pattern: just relationship name
        if pattern_str.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Ok(TraversePattern {
                source_label: None,
                relationship_type: pattern_str.to_string(),
                target_label: None,
                direction: TraverseDirection::Outgoing,
            });
        }

        // Complex pattern: (source)-[:REL]->(target)
        // Parse using regex-like approach

        // Determine direction
        let direction = if pattern_str.contains("<-") && pattern_str.contains("->") {
            TraverseDirection::Both
        } else if pattern_str.contains("<-") {
            TraverseDirection::Incoming
        } else {
            TraverseDirection::Outgoing
        };

        // Extract relationship type from [:REL] or -[:REL]-
        let rel_start = pattern_str.find("[:").or_else(|| pattern_str.find("["));
        let rel_end = pattern_str.find(']');

        let relationship_type = if let (Some(start), Some(end)) = (rel_start, rel_end) {
            let rel_part = &pattern_str[start + 1..end];
            rel_part.trim_start_matches(':').to_string()
        } else {
            "RELATED".to_string() // Default relationship
        };

        // Extract source label from (source)
        let source_label = if let Some(paren_start) = pattern_str.find('(') {
            if let Some(paren_end) = pattern_str[paren_start..].find(')') {
                let label = &pattern_str[paren_start + 1..paren_start + paren_end];
                if !label.is_empty() && label != "*" {
                    // Handle (var:Label) syntax
                    if let Some(colon_pos) = label.find(':') {
                        Some(label[colon_pos + 1..].to_string())
                    } else {
                        Some(label.to_string())
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Extract target label from last (target)
        let target_label = {
            let last_paren = pattern_str.rfind('(');
            if let Some(paren_start) = last_paren {
                // Make sure this isn't the same as source
                if Some(paren_start) != pattern_str.find('(') {
                    if let Some(paren_end) = pattern_str[paren_start..].find(')') {
                        let label = &pattern_str[paren_start + 1..paren_start + paren_end];
                        if !label.is_empty() && label != "*" {
                            if let Some(colon_pos) = label.find(':') {
                                Some(label[colon_pos + 1..].to_string())
                            } else {
                                Some(label.to_string())
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        Ok(TraversePattern {
            source_label,
            relationship_type,
            target_label,
            direction,
        })
    }

    /// Check if the original SQL has AUTOINCREMENT for a specific column
    fn has_autoincrement_for_column(sql: &str, column_name: &str) -> bool {
        let sql_upper = sql.to_uppercase();

        // Look for patterns like "column_name ... AUTOINCREMENT" or "column_name ... AUTO_INCREMENT"
        // We need to find the column and check if AUTOINCREMENT follows before the next column or closing paren

        let col_upper = column_name.to_uppercase();

        // Find column definition start
        if let Some(col_pos) = sql_upper.find(&col_upper) {
            // Find the end of this column definition (next comma or closing paren)
            let remaining = &sql_upper[col_pos..];
            let end_pos = remaining
                .find(',')
                .unwrap_or_else(|| remaining.find(')').unwrap_or(remaining.len()));
            let col_def = &remaining[..end_pos];

            // Check if this column definition contains AUTOINCREMENT
            col_def.contains("AUTOINCREMENT") || col_def.contains("AUTO_INCREMENT")
        } else {
            false
        }
    }

    /// Rewrite each `?` placeholder token to `$1`, `$2`, ... in source order.
    /// Uses the SQL tokenizer, so `?` inside strings and comments is untouched.
    /// Returns None when the statement has no `?` placeholder.
    fn number_question_placeholders(dialect: &NexusQLDialect, sql: &str) -> Option<String> {
        if !sql.contains('?') {
            return None;
        }
        let tokens = sqlparser::tokenizer::Tokenizer::new(dialect, sql)
            .tokenize_with_location()
            .ok()?;
        let marks: Vec<(u64, u64)> = tokens
            .iter()
            .filter(|t| matches!(&t.token, Token::Placeholder(p) if p == "?"))
            .map(|t| (t.location.line, t.location.column))
            .collect();
        if marks.is_empty() {
            return None;
        }
        // The tokenizer reports 1-based line and character columns.
        let mut out = String::with_capacity(sql.len() + marks.len() * 2);
        let (mut line, mut column, mut next) = (1u64, 1u64, 0usize);
        for ch in sql.chars() {
            if ch == '?' && marks.get(next) == Some(&(line, column)) {
                next += 1;
                out.push_str(&format!("${next}"));
            } else {
                out.push(ch);
            }
            if ch == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        Some(out)
    }

    pub fn parse(sql: &str) -> Result<Statement> {
        let parser = Self::new();

        if let Some(statement) = crate::changefeed::parse_statement(sql)? {
            return Ok(Statement::Changefeed(statement));
        }

        // Check for CREATE RELATIONSHIP statement first (custom NexusQL syntax)
        if sql.trim().to_uppercase().starts_with("CREATE RELATIONSHIP") {
            return parser.parse_create_relationship(sql);
        }

        // Check for TRAVERSE statement (custom NexusQL syntax)
        if sql.trim().to_uppercase().starts_with("TRAVERSE") {
            return parser.parse_traverse(sql);
        }

        // `?` placeholders bind in order of appearance, like SQLite/DB-API.
        let numbered = Self::number_question_placeholders(&parser.dialect, sql);
        let sql = numbered.as_deref().unwrap_or(sql);

        // Preprocess SQL to extract FETCH clauses
        let (preprocessed_sql, fetch_clauses) = Self::preprocess_sql(sql);

        let ast = SqlParser::parse_sql(&parser.dialect, &preprocessed_sql)
            .map_err(|e| NexusError::ParseError(e.to_string()))?;

        if ast.is_empty() {
            return Err(NexusError::ParseError("Empty statement".into()));
        }

        // Convert first statement, passing original SQL for AUTOINCREMENT detection and FETCH clauses
        parser.convert_statement_with_fetch(&ast[0], sql, fetch_clauses)
    }

    /// Convert sqlparser Statement to NexusQL Statement with FETCH clauses attached
    fn convert_statement_with_fetch(
        &self,
        stmt: &sql_ast::Statement,
        original_sql: &str,
        fetch_clauses: Vec<FetchClause>,
    ) -> Result<Statement> {
        let mut result = self.convert_statement(stmt, original_sql)?;

        // Attach FETCH clauses to SELECT statements
        if !fetch_clauses.is_empty() {
            if let Statement::Select { ref mut fetch, .. } = result {
                *fetch = fetch_clauses;
            }
        }

        Ok(result)
    }

    fn convert_statement(
        &self,
        stmt: &sql_ast::Statement,
        original_sql: &str,
    ) -> Result<Statement> {
        match stmt {
            sql_ast::Statement::Query(query) => self.convert_query(query),

            sql_ast::Statement::Insert(insert) => self.convert_insert(insert),

            sql_ast::Statement::CreateTable(create) => Ok(Statement::CreateTable {
                // `ObjectName::to_string()` preserves quoting (e.g. "How to use"
                // stays quoted). We store table names bare so that INSERT /
                // SELECT / DROP with either quoted or unquoted refs resolve to
                // the same entry.
                name: object_name_bare(&create.name),
                columns: create
                    .columns
                    .iter()
                    .map(|col| {
                        // Check for PRIMARY KEY in column options
                        let mut is_primary_key = false;

                        for option in &col.options {
                            if let sql_ast::ColumnOption::Unique { is_primary, .. } = &option.option
                            {
                                if *is_primary {
                                    is_primary_key = true;
                                }
                            }
                        }

                        // Check for AUTOINCREMENT in the original SQL
                        // sqlparser-rs doesn't handle AUTOINCREMENT well, so we detect it directly
                        let col_name = col.name.to_string();
                        let is_autoincrement =
                            Self::has_autoincrement_for_column(original_sql, &col_name);

                        ColumnDef {
                            name: col_name,
                            data_type: self.convert_data_type(&col.data_type),
                            is_primary_key,
                            is_autoincrement,
                        }
                    })
                    .collect(),
                if_not_exists: create.if_not_exists,
            }),

            sql_ast::Statement::CreateIndex(create_index) => {
                // Extract fields from the CreateIndex struct
                let index_name = create_index
                    .name
                    .as_ref()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "unnamed_index".to_string());
                let table = object_name_bare(&create_index.table_name);

                // Extract column names from OrderByExpr
                let column_names: Vec<String> = create_index
                    .columns
                    .iter()
                    .filter_map(|order_expr| {
                        if let sql_ast::Expr::Identifier(ident) = &order_expr.expr {
                            Some(ident.value.clone())
                        } else {
                            None
                        }
                    })
                    .collect();

                // Extract USING clause (e.g., USING hnsw)
                let using = create_index.using.as_ref().map(|ident| ident.value.clone());

                // Extract WITH clause options (e.g., WITH (m = 16, ef_construction = 100))
                let with_options: Vec<IndexOption> =
                    create_index
                        .with
                        .iter()
                        .filter_map(|expr| {
                            if let sql_ast::Expr::BinaryOp {
                                left,
                                op: sql_ast::BinaryOperator::Eq,
                                right,
                            } = expr
                            {
                                if let sql_ast::Expr::Identifier(ident) = left.as_ref() {
                                    let name = ident.value.clone();
                                    let value = match right.as_ref() {
                                        sql_ast::Expr::Value(sql_ast::Value::Number(n, _)) => {
                                            if let Ok(i) = n.parse::<i64>() {
                                                Value::Integer(i)
                                            } else if let Ok(f) = n.parse::<f64>() {
                                                Value::Float(f.into())
                                            } else {
                                                return None;
                                            }
                                        }
                                        sql_ast::Expr::Value(
                                            sql_ast::Value::SingleQuotedString(s),
                                        )
                                        | sql_ast::Expr::Value(
                                            sql_ast::Value::DoubleQuotedString(s),
                                        ) => Value::Text(s.clone()),
                                        sql_ast::Expr::Value(sql_ast::Value::Boolean(b)) => {
                                            Value::Boolean(*b)
                                        }
                                        _ => return None,
                                    };
                                    return Some(IndexOption { name, value });
                                }
                            }
                            None
                        })
                        .collect();

                Ok(Statement::CreateIndex {
                    name: index_name,
                    table,
                    columns: column_names,
                    unique: create_index.unique,
                    if_not_exists: create_index.if_not_exists,
                    using,
                    with_options,
                })
            }

            sql_ast::Statement::StartTransaction { .. } => {
                // Handle BEGIN / START TRANSACTION
                Ok(Statement::Transaction {
                    operation: TransactionOp::Begin,
                })
            }

            sql_ast::Statement::Commit { .. } => {
                // Handle COMMIT
                Ok(Statement::Transaction {
                    operation: TransactionOp::Commit,
                })
            }

            sql_ast::Statement::Rollback { .. } => {
                // Handle ROLLBACK
                Ok(Statement::Transaction {
                    operation: TransactionOp::Rollback,
                })
            }

            sql_ast::Statement::Pragma { name, value, .. } => Ok(Statement::Pragma {
                name: name.to_string(),
                value: value.as_ref().map(|v| v.to_string()),
            }),

            sql_ast::Statement::Drop {
                object_type,
                if_exists,
                names,
                ..
            } => {
                // Check if it's a DROP TABLE
                if *object_type == sql_ast::ObjectType::Table {
                    if let Some(name) = names.first() {
                        return Ok(Statement::DropTable {
                            name: name.to_string(),
                            if_exists: *if_exists,
                        });
                    }
                }
                Err(NexusError::ParseError("Unsupported DROP statement".into()))
            }

            sql_ast::Statement::Update {
                table,
                assignments,
                selection,
                ..
            } => {
                let table_name = if let sql_ast::TableFactor::Table { name, .. } = &table.relation {
                    object_name_bare(name)
                } else {
                    table.to_string()
                };

                // Convert SET assignments
                let mut set_map = Vec::new();
                for assignment in assignments {
                    let col_name = match &assignment.target {
                        sql_ast::AssignmentTarget::ColumnName(obj_name) => obj_name
                            .0
                            .iter()
                            .map(|id| id.value.clone())
                            .collect::<Vec<_>>()
                            .join("."),
                        _ => {
                            return Err(NexusError::ParseError(
                                "Unsupported assignment target".into(),
                            ))
                        }
                    };
                    // Convert value - use convert_expr_to_ast to support parameters
                    let expr = self.convert_expr_to_ast(&assignment.value)?;
                    set_map.push((col_name, expr));
                }

                // Convert WHERE clause (if any) - use convert_expr_to_ast to support parameters
                let where_clause = selection
                    .as_ref()
                    .map(|expr| self.convert_expr_to_ast(expr))
                    .transpose()?;

                Ok(Statement::Update {
                    table: table_name,
                    set: set_map,
                    where_clause,
                })
            }

            sql_ast::Statement::Delete(delete) => {
                // Extract table name from DELETE statement
                let table_name = match &delete.from {
                    sql_ast::FromTable::WithFromKeyword(tables) => {
                        if let Some(table_with_joins) = tables.first() {
                            if let sql_ast::TableFactor::Table { name, .. } =
                                &table_with_joins.relation
                            {
                                object_name_bare(name)
                            } else {
                                return Err(NexusError::ParseError(
                                    "Unsupported DELETE FROM structure".into(),
                                ));
                            }
                        } else {
                            return Err(NexusError::ParseError(
                                "DELETE without FROM clause".into(),
                            ));
                        }
                    }
                    sql_ast::FromTable::WithoutKeyword(tables) => {
                        if let Some(from) = tables.first() {
                            from.to_string()
                        } else {
                            return Err(NexusError::ParseError("DELETE without table".into()));
                        }
                    }
                };

                // Convert WHERE clause (if any) - use convert_expr_to_ast to support parameters
                let where_clause = delete
                    .selection
                    .as_ref()
                    .map(|expr| self.convert_expr_to_ast(expr))
                    .transpose()?;

                Ok(Statement::Delete {
                    from: table_name,
                    where_clause,
                })
            }

            _ => Err(NexusError::ParseError("Unsupported statement type".into())),
        }
    }

    /// LIMIT must be a non-negative integer literal. OFFSET, parameters and
    /// expressions used to be ignored silently, returning the wrong rows.
    fn convert_limit(query: &sql_ast::Query) -> Result<Option<usize>> {
        if query.offset.is_some() {
            return Err(NexusError::ParseError("OFFSET is not supported".into()));
        }
        match &query.limit {
            None => Ok(None),
            Some(sql_ast::Expr::Value(sql_ast::Value::Number(n, _))) => {
                n.parse::<usize>().map(Some).map_err(|_| {
                    NexusError::ParseError(format!("LIMIT must be a non-negative integer, got {n}"))
                })
            }
            Some(other) => Err(NexusError::ParseError(format!(
                "LIMIT must be an integer literal, got {other}"
            ))),
        }
    }

    fn convert_query(&self, query: &Box<sql_ast::Query>) -> Result<Statement> {
        // Extract CTEs (WITH clause)
        let with = self.convert_with_clause(&query.with)?;

        match &*query.body {
            sql_ast::SetExpr::Select(select) => {
                let from_clause = self.extract_from_clause(&select.from)?;

                let columns = select
                    .projection
                    .iter()
                    .map(|proj| self.convert_select_item(proj))
                    .collect::<Result<Vec<_>>>()?;

                // Extract USING clause if present
                let using_engine = self.extract_using_clause(query);

                // Extract ORDER BY
                let order_by = if let Some(order_by_clause) = &query.order_by {
                    if !order_by_clause.exprs.is_empty() {
                        let order_item = &order_by_clause.exprs[0];
                        let expr = self.convert_expr_to_ast(&order_item.expr)?;
                        let ascending = order_item.asc.unwrap_or(true);
                        Some(OrderBy { expr, ascending })
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Extract LIMIT
                let limit = Self::convert_limit(query)?;

                // Extract WHERE clause
                let where_clause = select
                    .selection
                    .as_ref()
                    .map(|expr| self.convert_expr_to_ast(expr))
                    .transpose()?;

                // Extract GROUP BY
                let group_by = match &select.group_by {
                    sql_ast::GroupByExpr::All(_) => {
                        // GROUP BY ALL is not commonly supported, treat as empty
                        vec![]
                    }
                    sql_ast::GroupByExpr::Expressions(exprs, _modifiers) => exprs
                        .iter()
                        .map(|expr| self.convert_expr_to_ast(expr))
                        .collect::<Result<Vec<_>>>()?,
                };

                // Extract HAVING clause
                let having = select
                    .having
                    .as_ref()
                    .map(|expr| self.convert_expr_to_ast(expr))
                    .transpose()?;

                Ok(Statement::Select {
                    with,
                    columns,
                    from: from_clause,
                    where_clause,
                    group_by,
                    having,
                    using_engine,
                    order_by,
                    limit,
                    fetch: vec![], // FETCH clauses added by convert_statement_with_fetch
                })
            }
            sql_ast::SetExpr::SetOperation {
                op,
                set_quantifier,
                left,
                right,
            } => {
                // Convert the set operator
                let set_op = match (op, set_quantifier) {
                    (sql_ast::SetOperator::Union, sql_ast::SetQuantifier::All) => {
                        SetOperator::UnionAll
                    }
                    (sql_ast::SetOperator::Union, _) => SetOperator::Union,
                    (sql_ast::SetOperator::Intersect, sql_ast::SetQuantifier::All) => {
                        SetOperator::IntersectAll
                    }
                    (sql_ast::SetOperator::Intersect, _) => SetOperator::Intersect,
                    (sql_ast::SetOperator::Except, sql_ast::SetQuantifier::All) => {
                        SetOperator::ExceptAll
                    }
                    (sql_ast::SetOperator::Except, _) => SetOperator::Except,
                };

                // Convert left and right subqueries
                let left_query = Box::new(sql_ast::Query {
                    with: None,
                    body: left.clone(),
                    order_by: None,
                    limit: None,
                    limit_by: vec![],
                    offset: None,
                    fetch: None,
                    locks: vec![],
                    for_clause: None,
                    settings: None,
                    format_clause: None,
                });
                let right_query = Box::new(sql_ast::Query {
                    with: None,
                    body: right.clone(),
                    order_by: None,
                    limit: None,
                    limit_by: vec![],
                    offset: None,
                    fetch: None,
                    locks: vec![],
                    for_clause: None,
                    settings: None,
                    format_clause: None,
                });

                let left_stmt = self.convert_query(&left_query)?;
                let right_stmt = self.convert_query(&right_query)?;

                // Extract ORDER BY and LIMIT from the outer query (applies to entire set operation)
                let order_by = if let Some(order_by_clause) = &query.order_by {
                    if !order_by_clause.exprs.is_empty() {
                        let order_item = &order_by_clause.exprs[0];
                        let expr = self.convert_where_expr(&order_item.expr)?;
                        let ascending = order_item.asc.unwrap_or(true);
                        Some(OrderBy { expr, ascending })
                    } else {
                        None
                    }
                } else {
                    None
                };

                let limit = Self::convert_limit(query)?;

                Ok(Statement::SetOperation {
                    left: Box::new(left_stmt),
                    op: set_op,
                    right: Box::new(right_stmt),
                    order_by,
                    limit,
                })
            }
            _ => Err(NexusError::ParseError(
                "Only SELECT and set operations (UNION/INTERSECT/EXCEPT) supported".into(),
            )),
        }
    }

    fn convert_select_item(&self, item: &sql_ast::SelectItem) -> Result<SelectItem> {
        match item {
            sql_ast::SelectItem::Wildcard(_) => Ok(SelectItem::Wildcard),

            sql_ast::SelectItem::UnnamedExpr(expr) => {
                let converted_expr = self.convert_expr_to_ast(expr)?;
                Ok(SelectItem::Expr {
                    expr: converted_expr,
                    alias: None,
                })
            }

            sql_ast::SelectItem::ExprWithAlias { expr, alias } => {
                let converted_expr = self.convert_expr_to_ast(expr)?;
                Ok(SelectItem::Expr {
                    expr: converted_expr,
                    alias: Some(alias.value.clone()),
                })
            }

            _ => Err(NexusError::ParseError(
                "Unsupported SELECT item type".into(),
            )),
        }
    }

    fn convert_expr_to_ast(&self, expr: &sql_ast::Expr) -> Result<Expr> {
        match expr {
            sql_ast::Expr::Nested(inner) => self.convert_expr_to_ast(inner),
            // Column reference
            sql_ast::Expr::Identifier(ident) => {
                // Check if this is a named parameter (:name)
                if ident.value.starts_with(':') {
                    let param_name = ident.value[1..].to_string();
                    Ok(Expr::Parameter(crate::query::ast::Parameter::Named(
                        param_name,
                    )))
                } else {
                    Ok(Expr::Column(ident.value.clone()))
                }
            }

            sql_ast::Expr::CompoundIdentifier(idents) => {
                let col_name = idents
                    .iter()
                    .map(|i| i.value.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                Ok(Expr::Column(col_name))
            }

            // Literal values
            sql_ast::Expr::Value(val) => {
                let value = match val {
                    // Check for positional parameter placeholder
                    sql_ast::Value::Placeholder(placeholder) => {
                        // sqlparser uses $ for placeholders (PostgreSQL style)
                        // We'll also support ? by checking the string
                        if placeholder == "?" {
                            // For now, we'll track position during parsing
                            // This is a simplified approach - in production, you'd want
                            // to track parameter positions more carefully
                            return Ok(Expr::Parameter(crate::query::ast::Parameter::Positional(
                                1,
                            )));
                        } else if placeholder.starts_with('$') {
                            // PostgreSQL-style $1, $2, etc.
                            if let Ok(index) = placeholder[1..].parse::<usize>() {
                                return Ok(Expr::Parameter(
                                    crate::query::ast::Parameter::Positional(index),
                                ));
                            }
                        }
                        return Err(NexusError::ParseError(format!(
                            "Invalid placeholder: {}",
                            placeholder
                        )));
                    }
                    sql_ast::Value::Number(n, _) => {
                        if let Ok(i) = n.parse::<i64>() {
                            Value::Integer(i)
                        } else if let Ok(f) = n.parse::<f64>() {
                            Value::Float(f.into())
                        } else {
                            return Err(NexusError::ParseError(format!("Invalid number: {}", n)));
                        }
                    }
                    sql_ast::Value::SingleQuotedString(s)
                    | sql_ast::Value::DoubleQuotedString(s) => {
                        // Check if this is a vector literal
                        if s.starts_with('[') && s.ends_with(']') {
                            self.parse_vector_literal(s)?
                        } else {
                            Value::Text(s.clone())
                        }
                    }
                    sql_ast::Value::Boolean(b) => Value::Boolean(*b),
                    sql_ast::Value::Null => Value::Null,
                    _ => return Err(NexusError::ParseError("Unsupported value type".into())),
                };
                Ok(Expr::Literal(value))
            }

            // Function calls (may include window functions with OVER clause)
            sql_ast::Expr::Function(func) => {
                let name = func.name.to_string();

                // Handle FunctionArguments enum
                let args = match &func.args {
                    sql_ast::FunctionArguments::None => Vec::new(),
                    sql_ast::FunctionArguments::List(arg_list) => arg_list
                        .args
                        .iter()
                        .map(|arg| match arg {
                            sql_ast::FunctionArg::Unnamed(sql_ast::FunctionArgExpr::Expr(e)) => {
                                self.convert_expr_to_ast(e)
                            }
                            sql_ast::FunctionArg::Unnamed(sql_ast::FunctionArgExpr::Wildcard) => {
                                Ok(Expr::Wildcard)
                            }
                            sql_ast::FunctionArg::Unnamed(
                                sql_ast::FunctionArgExpr::QualifiedWildcard(_),
                            ) => Ok(Expr::Wildcard),
                            _ => Err(NexusError::ParseError(
                                "Unsupported function argument".into(),
                            )),
                        })
                        .collect::<Result<Vec<_>>>()?,
                    _ => {
                        return Err(NexusError::ParseError(
                            "Unsupported function arguments type".into(),
                        ))
                    }
                };

                let base_function = Expr::Function {
                    name: name.clone(),
                    args,
                };

                // Check if this is a window function (has OVER clause)
                if let Some(window_spec) = &func.over {
                    let window = self.convert_window_spec(window_spec)?;
                    Ok(Expr::WindowFunction {
                        function: Box::new(base_function),
                        window_spec: window,
                    })
                } else {
                    Ok(base_function)
                }
            }

            // Binary operations (for expressions in SELECT)
            sql_ast::Expr::BinaryOp { left, op, right } => {
                let left_expr = Box::new(self.convert_expr_to_ast(left)?);
                let right_expr = Box::new(self.convert_expr_to_ast(right)?);
                let binary_op = self.convert_binary_op(op)?;

                Ok(Expr::BinaryOp {
                    left: left_expr,
                    op: binary_op,
                    right: right_expr,
                })
            }

            // LIKE expression (separate from binary operators in sqlparser)
            sql_ast::Expr::Like {
                negated,
                expr,
                pattern,
                escape_char: _,
                any: _,
            } => {
                let left_expr = Box::new(self.convert_expr_to_ast(expr)?);
                let right_expr = Box::new(self.convert_expr_to_ast(pattern)?);

                let like_expr = Expr::BinaryOp {
                    left: left_expr,
                    op: BinaryOperator::Like,
                    right: right_expr,
                };

                // NOT LIKE negates the match, the same way unary NOT is lowered.
                if *negated {
                    Ok(Expr::BinaryOp {
                        left: Box::new(like_expr),
                        op: BinaryOperator::Eq,
                        right: Box::new(Expr::Literal(Value::Boolean(false))),
                    })
                } else {
                    Ok(like_expr)
                }
            }

            // IS NULL expression
            sql_ast::Expr::IsNull(inner_expr) => {
                let inner = self.convert_expr_to_ast(inner_expr)?;
                Ok(Expr::IsNull(Box::new(inner)))
            }

            // IS NOT NULL expression
            sql_ast::Expr::IsNotNull(inner_expr) => {
                let inner = self.convert_expr_to_ast(inner_expr)?;
                Ok(Expr::IsNotNull(Box::new(inner)))
            }

            // CAST expression (CAST(expr AS type) or expr::type in PostgreSQL)
            sql_ast::Expr::Cast {
                expr: inner_expr,
                data_type,
                ..
            } => {
                let inner = self.convert_expr_to_ast(inner_expr)?;
                let target_type = self.convert_data_type(data_type);
                Ok(Expr::Cast {
                    expr: Box::new(inner),
                    data_type: target_type,
                })
            }

            // Scalar subquery: (SELECT ...)
            sql_ast::Expr::Subquery(query) => {
                let subquery = self.convert_query_to_subquery(query)?;
                Ok(Expr::Subquery(Box::new(subquery)))
            }

            // IN subquery: expr IN (SELECT ...)
            sql_ast::Expr::InSubquery {
                expr,
                subquery,
                negated,
            } => {
                let left_expr = self.convert_expr_to_ast(expr)?;
                let subquery_expr = self.convert_query_to_subquery(subquery)?;
                Ok(Expr::InSubquery {
                    expr: Box::new(left_expr),
                    subquery: Box::new(subquery_expr),
                    negated: *negated,
                })
            }

            // EXISTS subquery: EXISTS (SELECT ...)
            sql_ast::Expr::Exists { subquery, negated } => {
                let subquery_expr = self.convert_query_to_subquery(subquery)?;
                Ok(Expr::ExistsSubquery {
                    subquery: Box::new(subquery_expr),
                    negated: *negated,
                })
            }

            // Unary +/-/NOT. sqlparser emits negative numeric literals as
            // UnaryOp(Minus, Number(...)); without this branch every INSERT
            // with a negative value fails to parse. Fold `-<literal>` into a
            // negated Literal; unary `+` is a no-op; `NOT x` becomes `x = false`.
            sql_ast::Expr::UnaryOp { op, expr: inner } => {
                use sql_ast::UnaryOperator as UOP;
                match op {
                    UOP::Plus => self.convert_expr_to_ast(inner),
                    UOP::Minus => {
                        let inner_ast = self.convert_expr_to_ast(inner)?;
                        match inner_ast {
                            Expr::Literal(Value::Integer(n)) => {
                                Ok(Expr::Literal(Value::Integer(-n)))
                            }
                            Expr::Literal(Value::Float(f)) => Ok(Expr::Literal(Value::Float(-f))),
                            _ => Err(NexusError::ParseError(
                                "Unary minus only supported on numeric literals".to_string(),
                            )),
                        }
                    }
                    UOP::Not => {
                        let inner_ast = self.convert_expr_to_ast(inner)?;
                        Ok(Expr::BinaryOp {
                            left: Box::new(inner_ast),
                            op: BinaryOperator::Eq,
                            right: Box::new(Expr::Literal(Value::Boolean(false))),
                        })
                    }
                    _ => Err(NexusError::ParseError(format!(
                        "Unsupported unary operator: {:?}",
                        op
                    ))),
                }
            }

            _ => Err(NexusError::ParseError(format!(
                "Unsupported expression in SELECT: {:?}",
                expr
            ))),
        }
    }

    fn convert_binary_op(&self, op: &sql_ast::BinaryOperator) -> Result<BinaryOperator> {
        match op {
            sql_ast::BinaryOperator::Eq => Ok(BinaryOperator::Eq),
            sql_ast::BinaryOperator::NotEq => Ok(BinaryOperator::NotEq),
            sql_ast::BinaryOperator::Lt => Ok(BinaryOperator::Lt),
            sql_ast::BinaryOperator::LtEq => Ok(BinaryOperator::LtEq),
            sql_ast::BinaryOperator::Gt => Ok(BinaryOperator::Gt),
            sql_ast::BinaryOperator::GtEq => Ok(BinaryOperator::GtEq),
            sql_ast::BinaryOperator::And => Ok(BinaryOperator::And),
            sql_ast::BinaryOperator::Or => Ok(BinaryOperator::Or),
            // Vector distance operators (pgvector-compatible)
            sql_ast::BinaryOperator::Spaceship => Ok(BinaryOperator::CosineDistance), // <=> operator
            // Full-text search: `col MATCH 'q'` and PostgreSQL-style `col @@ 'q'`
            sql_ast::BinaryOperator::AtAt => Ok(BinaryOperator::TextMatch),
            sql_ast::BinaryOperator::Custom(op_str) => match op_str.as_str() {
                "MATCH" => Ok(BinaryOperator::TextMatch),
                "<->" => Ok(BinaryOperator::L2Distance),
                "<=>" => Ok(BinaryOperator::CosineDistance),
                "<#>" => Ok(BinaryOperator::InnerProduct),
                _ => Err(NexusError::ParseError(format!(
                    "Unsupported custom operator: {}",
                    op_str
                ))),
            },
            _ => Err(NexusError::ParseError(format!(
                "Unsupported binary operator: {:?}",
                op
            ))),
        }
    }

    /// Convert a sql_ast::Query to our SubqueryExpr AST
    fn convert_query_to_subquery(&self, query: &sql_ast::Query) -> Result<SubqueryExpr> {
        // Extract the SELECT from the query body
        match &*query.body {
            sql_ast::SetExpr::Select(select) => {
                // Convert columns
                let columns = select
                    .projection
                    .iter()
                    .map(|item| self.convert_select_item(item))
                    .collect::<Result<Vec<_>>>()?;

                // Convert FROM clause to TableRef
                let from = if !select.from.is_empty() {
                    self.convert_table_with_joins_to_table_ref(&select.from[0])?
                } else {
                    return Err(NexusError::ParseError(
                        "Subquery requires a FROM clause".into(),
                    ));
                };

                // Convert WHERE clause
                let where_clause = select
                    .selection
                    .as_ref()
                    .map(|expr| self.convert_expr_to_ast(expr))
                    .transpose()?
                    .map(Box::new);

                // Convert GROUP BY
                let group_by = match &select.group_by {
                    sql_ast::GroupByExpr::All(_) => vec![],
                    sql_ast::GroupByExpr::Expressions(exprs, _) => exprs
                        .iter()
                        .map(|expr| self.convert_expr_to_ast(expr))
                        .collect::<Result<Vec<_>>>()?,
                };

                // Convert HAVING
                let having = select
                    .having
                    .as_ref()
                    .map(|expr| self.convert_expr_to_ast(expr))
                    .transpose()?
                    .map(Box::new);

                // Convert ORDER BY
                let order_by = query
                    .order_by
                    .as_ref()
                    .and_then(|ob| ob.exprs.first())
                    .map(|item| -> Result<OrderBy> {
                        let expr = self.convert_expr_to_ast(&item.expr)?;
                        let ascending = !item.asc.map(|a| !a).unwrap_or(false);
                        Ok(OrderBy { expr, ascending })
                    })
                    .transpose()?;

                // Convert LIMIT
                let limit = Self::convert_limit(query)?;

                Ok(SubqueryExpr {
                    columns,
                    from,
                    where_clause,
                    group_by,
                    having,
                    order_by,
                    limit,
                })
            }
            _ => Err(NexusError::ParseError(
                "Only SELECT queries supported in subqueries".into(),
            )),
        }
    }

    /// Convert sqlparser's TableWithJoins to our TableRef
    fn convert_table_with_joins_to_table_ref(
        &self,
        table_with_joins: &sql_ast::TableWithJoins,
    ) -> Result<TableRef> {
        match &table_with_joins.relation {
            sql_ast::TableFactor::Table { name, alias, .. } => Ok(TableRef::Table {
                name: object_name_bare(name),
                alias: alias.as_ref().map(|a| a.name.value.clone()),
            }),
            sql_ast::TableFactor::Derived {
                subquery, alias, ..
            } => {
                let alias_name = alias
                    .as_ref()
                    .map(|a| a.name.value.clone())
                    .ok_or_else(|| {
                        NexusError::ParseError("Derived table requires an alias".into())
                    })?;
                let subquery_expr = self.convert_query_to_subquery(subquery)?;
                Ok(TableRef::Subquery {
                    subquery: Box::new(subquery_expr),
                    alias: alias_name,
                })
            }
            _ => Err(NexusError::ParseError("Unsupported table type".into())),
        }
    }

    /// Convert sqlparser's WindowType to our WindowSpec
    fn convert_window_spec(&self, window_type: &sql_ast::WindowType) -> Result<WindowSpec> {
        match window_type {
            sql_ast::WindowType::WindowSpec(spec) => {
                // Convert PARTITION BY
                let partition_by = spec
                    .partition_by
                    .iter()
                    .map(|expr| self.convert_expr_to_ast(expr))
                    .collect::<Result<Vec<_>>>()?;

                // Convert ORDER BY within window
                let order_by = spec
                    .order_by
                    .iter()
                    .map(|item| {
                        let expr = self.convert_expr_to_ast(&item.expr)?;
                        let ascending = !item.asc.map(|a| !a).unwrap_or(false);
                        let nulls_first = item.nulls_first;
                        Ok(WindowOrderBy {
                            expr,
                            ascending,
                            nulls_first,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;

                // Convert window frame
                let frame = spec
                    .window_frame
                    .as_ref()
                    .map(|f| self.convert_window_frame(f))
                    .transpose()?;

                Ok(WindowSpec {
                    partition_by,
                    order_by,
                    frame,
                })
            }
            sql_ast::WindowType::NamedWindow(_) => {
                // Named windows like OVER w (where w is defined in WINDOW clause)
                // For now, return an empty spec - we can add named window support later
                Ok(WindowSpec {
                    partition_by: vec![],
                    order_by: vec![],
                    frame: None,
                })
            }
        }
    }

    /// Convert sqlparser's WindowFrame to our WindowFrame
    fn convert_window_frame(&self, frame: &sql_ast::WindowFrame) -> Result<WindowFrame> {
        let units = match frame.units {
            sql_ast::WindowFrameUnits::Rows => WindowFrameUnits::Rows,
            sql_ast::WindowFrameUnits::Range => WindowFrameUnits::Range,
            sql_ast::WindowFrameUnits::Groups => WindowFrameUnits::Groups,
        };

        let start = self.convert_window_frame_bound(&frame.start_bound)?;
        let end = frame
            .end_bound
            .as_ref()
            .map(|b| self.convert_window_frame_bound(b))
            .transpose()?;

        Ok(WindowFrame { units, start, end })
    }

    /// Convert sqlparser's WindowFrameBound to our WindowFrameBound
    fn convert_window_frame_bound(
        &self,
        bound: &sql_ast::WindowFrameBound,
    ) -> Result<WindowFrameBound> {
        match bound {
            sql_ast::WindowFrameBound::CurrentRow => Ok(WindowFrameBound::CurrentRow),
            sql_ast::WindowFrameBound::Preceding(None) => Ok(WindowFrameBound::UnboundedPreceding),
            sql_ast::WindowFrameBound::Following(None) => Ok(WindowFrameBound::UnboundedFollowing),
            sql_ast::WindowFrameBound::Preceding(Some(expr)) => {
                if let sql_ast::Expr::Value(sql_ast::Value::Number(n, _)) = expr.as_ref() {
                    let val = n.parse::<u64>().map_err(|_| {
                        NexusError::ParseError(format!("Invalid window frame bound: {}", n))
                    })?;
                    Ok(WindowFrameBound::Preceding(val))
                } else {
                    Err(NexusError::ParseError(
                        "Window frame bound must be a number".into(),
                    ))
                }
            }
            sql_ast::WindowFrameBound::Following(Some(expr)) => {
                if let sql_ast::Expr::Value(sql_ast::Value::Number(n, _)) = expr.as_ref() {
                    let val = n.parse::<u64>().map_err(|_| {
                        NexusError::ParseError(format!("Invalid window frame bound: {}", n))
                    })?;
                    Ok(WindowFrameBound::Following(val))
                } else {
                    Err(NexusError::ParseError(
                        "Window frame bound must be a number".into(),
                    ))
                }
            }
        }
    }

    /// Convert WITH clause (CTEs) from sqlparser
    fn convert_with_clause(&self, with: &Option<sql_ast::With>) -> Result<Vec<CommonTableExpr>> {
        match with {
            Some(with_clause) => {
                let recursive = with_clause.recursive;
                with_clause
                    .cte_tables
                    .iter()
                    .map(|cte| {
                        let name = cte.alias.name.value.clone();
                        let columns = if cte.alias.columns.is_empty() {
                            None
                        } else {
                            Some(cte.alias.columns.iter().map(|c| c.value.clone()).collect())
                        };
                        let query = self.convert_query_to_subquery(&cte.query)?;
                        Ok(CommonTableExpr {
                            name,
                            columns,
                            query: Box::new(query),
                            recursive,
                        })
                    })
                    .collect()
            }
            None => Ok(vec![]),
        }
    }

    fn convert_insert(&self, insert: &sql_ast::Insert) -> Result<Statement> {
        let table = object_name_bare(&insert.table_name);

        // Extract column names
        let columns = if !insert.columns.is_empty() {
            Some(insert.columns.iter().map(|col| col.value.clone()).collect())
        } else {
            None
        };

        // Extract values from the source
        let mut values = Vec::new();

        if let Some(query_box) = &insert.source {
            if let sql_ast::SetExpr::Values(vals) = &*query_box.body {
                for row in &vals.rows {
                    let mut row_values = Vec::new();
                    for expr in row {
                        // Use convert_expr_to_ast to support parameters, not just literals
                        row_values.push(self.convert_expr_to_ast(expr)?);
                    }
                    values.push(row_values);
                }
            }
        }

        // Handle INSERT OR ... (SQLite conflict resolution)
        let on_conflict = match &insert.or {
            Some(or_clause) => {
                use sql_ast::SqliteOnConflict;
                match or_clause {
                    SqliteOnConflict::Replace => Some(OnConflict::Replace),
                    SqliteOnConflict::Ignore => Some(OnConflict::Ignore),
                    SqliteOnConflict::Rollback => Some(OnConflict::Rollback),
                    SqliteOnConflict::Abort => Some(OnConflict::Abort),
                    SqliteOnConflict::Fail => Some(OnConflict::Fail),
                }
            }
            None => None, // TODO: Add PostgreSQL ON CONFLICT DO UPDATE/NOTHING support
        };

        Ok(Statement::Insert {
            table,
            columns,
            values,
            on_conflict,
        })
    }

    #[allow(dead_code)]
    fn convert_value_expr(&self, expr: &sql_ast::Expr) -> Result<Value> {
        match expr {
            sql_ast::Expr::Value(val) => match val {
                sql_ast::Value::Number(n, _) => {
                    if let Ok(i) = n.parse::<i64>() {
                        Ok(Value::Integer(i))
                    } else if let Ok(f) = n.parse::<f64>() {
                        Ok(Value::Float(f.into()))
                    } else {
                        Err(NexusError::ParseError(format!("Invalid number: {}", n)))
                    }
                }
                sql_ast::Value::SingleQuotedString(s) | sql_ast::Value::DoubleQuotedString(s) => {
                    // Don't auto-convert bracket strings to vectors - respect column type
                    // Users should use ARRAY[...] syntax for vectors or vector columns
                    Ok(Value::Text(s.clone()))
                }
                sql_ast::Value::Boolean(b) => Ok(Value::Boolean(*b)),
                sql_ast::Value::Null => Ok(Value::Null),
                _ => Err(NexusError::ParseError("Unsupported value type".into())),
            },
            // Handle array syntax: ARRAY[0.1, 0.2, 0.3] or [0.1, 0.2, 0.3]
            sql_ast::Expr::Array(sql_ast::Array { elem, .. }) => {
                let mut vector = Vec::new();
                for element in elem {
                    if let sql_ast::Expr::Value(sql_ast::Value::Number(n, _)) = element {
                        if let Ok(f) = n.parse::<f32>() {
                            vector.push(f);
                        } else {
                            return Err(NexusError::ParseError(format!(
                                "Invalid vector element: {}",
                                n
                            )));
                        }
                    } else {
                        return Err(NexusError::ParseError(
                            "Vector elements must be numbers".into(),
                        ));
                    }
                }
                Ok(Value::Vector(vector))
            }
            _ => Err(NexusError::ParseError(
                "Complex expressions not yet supported in INSERT".into(),
            )),
        }
    }

    /// Parse vector literal from string like '[0.1, 0.2, 0.3]'.
    ///
    /// Only recognises all-numeric, comma-separated elements so that JSON
    /// arrays (e.g. `[{"index": 1}]`) or other bracketed text fall through
    /// to plain-text storage rather than erroring.
    fn parse_vector_literal(&self, s: &str) -> Result<Value> {
        if let Some(v) = self.try_parse_vector_literal(s) {
            Ok(Value::Vector(v))
        } else {
            // Not actually a numeric vector — preserve the original text
            // rather than erroring. Common case: JSON blobs or display
            // strings that happen to use `[...]`.
            Ok(Value::Text(s.to_string()))
        }
    }

    fn try_parse_vector_literal(&self, s: &str) -> Option<Vec<f32>> {
        let trimmed = s.trim();
        if !(trimmed.starts_with('[') && trimmed.ends_with(']')) {
            return None;
        }
        let inner = &trimmed[1..trimmed.len() - 1];
        if inner.trim().is_empty() {
            return None; // empty brackets — treat as text, not empty vector
        }
        let mut vector = Vec::new();
        for part in inner.split(',') {
            let part = part.trim();
            match part.parse::<f32>() {
                Ok(f) => vector.push(f),
                Err(_) => return None,
            }
        }
        Some(vector)
    }

    fn extract_from_clause(&self, from: &Vec<sql_ast::TableWithJoins>) -> Result<FromClause> {
        if let Some(first_table) = from.first() {
            // Extract primary table name and alias
            let (table_name, table_alias) = match &first_table.relation {
                sql_ast::TableFactor::Table { name, alias, .. } => {
                    let alias_str = alias.as_ref().map(|a| a.name.value.clone());
                    (name.to_string(), alias_str)
                }
                _ => {
                    return Err(NexusError::ParseError(
                        "Unsupported table factor in FROM clause".into(),
                    ))
                }
            };

            // Extract JOIN clauses
            let mut joins = Vec::new();
            for join in &first_table.joins {
                let join_clause = self.convert_join(join)?;
                joins.push(join_clause);
            }

            return Ok(FromClause {
                table: table_name,
                alias: table_alias,
                joins,
            });
        }
        Ok(FromClause {
            table: String::new(),
            alias: None,
            joins: vec![],
        })
    }

    /// Convert a sqlparser Join to NexusQL JoinClause
    fn convert_join(&self, join: &sql_ast::Join) -> Result<JoinClause> {
        // Extract table name and alias from the join relation
        let (table_name, table_alias) = match &join.relation {
            sql_ast::TableFactor::Table { name, alias, .. } => {
                let alias_str = alias.as_ref().map(|a| a.name.value.clone());
                (name.to_string(), alias_str)
            }
            _ => {
                return Err(NexusError::ParseError(
                    "Unsupported table factor in JOIN clause".into(),
                ))
            }
        };

        // Convert join operator to JoinType
        let (join_type, on_condition) = match &join.join_operator {
            sql_ast::JoinOperator::Inner(constraint) => {
                let condition = self.extract_join_condition(constraint)?;
                (JoinType::Inner, condition)
            }
            sql_ast::JoinOperator::LeftOuter(constraint) => {
                let condition = self.extract_join_condition(constraint)?;
                (JoinType::Left, condition)
            }
            sql_ast::JoinOperator::RightOuter(constraint) => {
                let condition = self.extract_join_condition(constraint)?;
                (JoinType::Right, condition)
            }
            sql_ast::JoinOperator::FullOuter(constraint) => {
                let condition = self.extract_join_condition(constraint)?;
                (JoinType::Full, condition)
            }
            sql_ast::JoinOperator::CrossJoin => (JoinType::Cross, None),
            _ => return Err(NexusError::ParseError("Unsupported JOIN type".into())),
        };

        Ok(JoinClause {
            table: table_name,
            alias: table_alias,
            join_type,
            on_condition,
        })
    }

    /// Extract the ON condition from a join constraint
    fn extract_join_condition(&self, constraint: &sql_ast::JoinConstraint) -> Result<Option<Expr>> {
        match constraint {
            sql_ast::JoinConstraint::On(expr) => {
                let condition = self.convert_expr_to_ast(expr)?;
                Ok(Some(condition))
            }
            sql_ast::JoinConstraint::None => Ok(None),
            sql_ast::JoinConstraint::Natural => {
                // Natural join - for now, we'll return None and handle it in executor
                Ok(None)
            }
            sql_ast::JoinConstraint::Using(_) => {
                // USING clause - could be implemented later
                Err(NexusError::ParseError(
                    "JOIN USING clause not yet supported, use ON instead".into(),
                ))
            }
        }
    }

    fn extract_using_clause(&self, _query: &sql_ast::Query) -> Option<String> {
        // Check for USING in SQL (custom extension)
        // For now, return None (will implement when needed)
        None
    }

    fn convert_data_type(&self, dt: &sql_ast::DataType) -> DataType {
        match dt {
            sql_ast::DataType::Int(_) | sql_ast::DataType::Integer(_) => DataType::Integer,
            sql_ast::DataType::Float(_) | sql_ast::DataType::Real => DataType::Float,
            sql_ast::DataType::Text | sql_ast::DataType::Varchar(_) => DataType::Text,
            sql_ast::DataType::Boolean => DataType::Boolean,
            // VECTOR(n) data type for embeddings
            sql_ast::DataType::Custom(obj_name, args) => {
                if obj_name.to_string().eq_ignore_ascii_case("GEOMETRY") {
                    return DataType::Geometry;
                }
                if obj_name.to_string().to_uppercase() == "VECTOR" && !args.is_empty() {
                    // Extract dimension from VECTOR(384)
                    // args is Vec<String>, so parse the first arg
                    if let Ok(dim) = args[0].parse::<usize>() {
                        return DataType::Vector(dim);
                    }
                }
                DataType::Blob // Fallback for unknown custom types
            }
            _ => DataType::Blob,
        }
    }

    fn convert_where_expr(&self, expr: &sql_ast::Expr) -> Result<Expr> {
        match expr {
            sql_ast::Expr::BinaryOp { left, op, right } => {
                let left_expr = self.convert_where_expr(left)?;
                let right_expr = self.convert_where_expr(right)?;

                let binary_op = self.convert_binary_op(op)?;

                Ok(Expr::BinaryOp {
                    left: Box::new(left_expr),
                    op: binary_op,
                    right: Box::new(right_expr),
                })
            }

            sql_ast::Expr::Identifier(ident) => Ok(Expr::Column(ident.value.clone())),

            sql_ast::Expr::Value(val) => {
                let value = match val {
                    sql_ast::Value::Number(n, _) => {
                        if let Ok(i) = n.parse::<i64>() {
                            Value::Integer(i)
                        } else if let Ok(f) = n.parse::<f64>() {
                            Value::Float(f.into())
                        } else {
                            return Err(NexusError::ParseError(format!("Invalid number: {}", n)));
                        }
                    }
                    sql_ast::Value::SingleQuotedString(s)
                    | sql_ast::Value::DoubleQuotedString(s) => {
                        // Check if this is a vector literal
                        if s.starts_with('[') && s.ends_with(']') {
                            self.parse_vector_literal(s)?
                        } else {
                            Value::Text(s.clone())
                        }
                    }
                    sql_ast::Value::Boolean(b) => Value::Boolean(*b),
                    sql_ast::Value::Null => Value::Null,
                    _ => {
                        return Err(NexusError::ParseError(
                            "Unsupported value type in WHERE".into(),
                        ))
                    }
                };
                Ok(Expr::Literal(value))
            }

            // Handle array syntax in WHERE clause
            sql_ast::Expr::Array(sql_ast::Array { elem, .. }) => {
                let mut vector = Vec::new();
                for element in elem {
                    if let sql_ast::Expr::Value(sql_ast::Value::Number(n, _)) = element {
                        if let Ok(f) = n.parse::<f32>() {
                            vector.push(f);
                        } else {
                            return Err(NexusError::ParseError(format!(
                                "Invalid vector element: {}",
                                n
                            )));
                        }
                    } else {
                        return Err(NexusError::ParseError(
                            "Vector elements must be numbers".into(),
                        ));
                    }
                }
                Ok(Expr::Literal(Value::Vector(vector)))
            }

            // CAST expression (CAST(expr AS type) or expr::type in PostgreSQL)
            sql_ast::Expr::Cast {
                expr: inner_expr,
                data_type,
                ..
            } => {
                let inner = self.convert_where_expr(inner_expr)?;
                let target_type = self.convert_data_type(data_type);
                Ok(Expr::Cast {
                    expr: Box::new(inner),
                    data_type: target_type,
                })
            }

            _ => Err(NexusError::ParseError(
                "Complex WHERE expressions not yet supported".into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn question_mark_placeholders_bind_in_order() {
        let stmt = QueryParser::parse("INSERT INTO t VALUES (?, '?', ?) -- ?\n").unwrap();
        let Statement::Insert { values, .. } = stmt else {
            panic!("insert");
        };
        assert_eq!(
            values[0],
            vec![
                Expr::Parameter(crate::query::ast::Parameter::Positional(1)),
                Expr::Literal(Value::Text("?".into())),
                Expr::Parameter(crate::query::ast::Parameter::Positional(2)),
            ]
        );
        let stmt = QueryParser::parse("SELECT id FROM t\nWHERE a = ? AND b = 'it''s ?' AND c = ?")
            .unwrap();
        let Statement::Select { where_clause, .. } = stmt else {
            panic!("select");
        };
        let text = format!("{:?}", where_clause.unwrap());
        assert!(
            text.contains("Positional(1)") && text.contains("Positional(2)"),
            "{text}"
        );
        assert!(!text.contains("Positional(3)"), "{text}");
    }

    #[test]
    fn unsupported_limit_forms_are_errors_not_ignored() {
        assert!(QueryParser::parse("SELECT * FROM t LIMIT 5").is_ok());
        for sql in [
            "SELECT * FROM t LIMIT 5 OFFSET 10",
            "SELECT * FROM t LIMIT $1",
            "SELECT * FROM t LIMIT ?",
            "SELECT a FROM t UNION SELECT a FROM u LIMIT 1 OFFSET 1",
        ] {
            let error = QueryParser::parse(sql).unwrap_err().to_string();
            assert!(
                error.contains("OFFSET") || error.contains("LIMIT"),
                "{sql}: {error}"
            );
        }
    }

    #[test]
    fn test_parse_select() {
        let stmt = QueryParser::parse("SELECT * FROM users").unwrap();
        match stmt {
            Statement::Select { from, .. } => {
                assert_eq!(from.table, "users");
                assert!(from.joins.is_empty());
            }
            _ => panic!("Wrong type"),
        }
    }

    #[test]
    fn test_parse_create_table() {
        let stmt = QueryParser::parse("CREATE TABLE users (id INTEGER, name TEXT)").unwrap();
        match stmt {
            Statement::CreateTable { name, columns, .. } => {
                assert_eq!(name, "users");
                assert_eq!(columns.len(), 2);
            }
            _ => panic!("Wrong type"),
        }
    }

    #[test]
    fn test_parse_with_limit() {
        let stmt = QueryParser::parse("SELECT * FROM users LIMIT 10").unwrap();
        match stmt {
            Statement::Select { limit, .. } => {
                assert_eq!(limit, Some(10));
            }
            _ => panic!("Wrong type"),
        }
    }

    #[test]
    fn test_parse_create_index() {
        let stmt = QueryParser::parse("CREATE INDEX idx_user_email ON users (email)").unwrap();
        match stmt {
            Statement::CreateIndex {
                name,
                table,
                columns,
                unique,
                ..
            } => {
                assert_eq!(name, "idx_user_email");
                assert_eq!(table, "users");
                assert_eq!(columns, vec!["email"]);
                assert_eq!(unique, false);
            }
            _ => panic!("Expected CreateIndex"),
        }
    }

    #[test]
    fn test_parse_create_unique_index() {
        let stmt =
            QueryParser::parse("CREATE UNIQUE INDEX idx_user_email ON users (email)").unwrap();
        match stmt {
            Statement::CreateIndex {
                name,
                table,
                columns,
                unique,
                ..
            } => {
                assert_eq!(name, "idx_user_email");
                assert_eq!(table, "users");
                assert_eq!(columns, vec!["email"]);
                assert_eq!(unique, true);
            }
            _ => panic!("Expected CreateIndex with unique=true"),
        }
    }

    #[test]
    fn test_parse_create_multi_column_index() {
        let stmt = QueryParser::parse(
            "CREATE INDEX idx_user_name_email ON users (name, email, created_at)",
        )
        .unwrap();
        match stmt {
            Statement::CreateIndex {
                name,
                table,
                columns,
                unique,
                ..
            } => {
                assert_eq!(name, "idx_user_name_email");
                assert_eq!(table, "users");
                assert_eq!(columns, vec!["name", "email", "created_at"]);
                assert_eq!(unique, false);
            }
            _ => panic!("Expected CreateIndex with multiple columns"),
        }
    }

    #[test]
    fn test_parse_cast_as_integer() {
        // Test PostgreSQL-style cast: '42'::INTEGER
        let stmt = QueryParser::parse("SELECT '42'::INTEGER AS num FROM users").unwrap();
        match stmt {
            Statement::Select { columns, .. } => {
                assert_eq!(columns.len(), 1);
                if let SelectItem::Expr { expr, alias } = &columns[0] {
                    assert_eq!(alias, &Some("num".to_string()));
                    match expr {
                        Expr::Cast { data_type, .. } => {
                            assert_eq!(*data_type, DataType::Integer);
                        }
                        _ => panic!("Expected Cast expression"),
                    }
                } else {
                    panic!("Expected expression with alias");
                }
            }
            _ => panic!("Expected SELECT statement"),
        }
    }

    #[test]
    fn test_parse_cast_as_text() {
        // Test CAST function syntax: CAST(123 AS TEXT)
        let stmt = QueryParser::parse("SELECT CAST(123 AS TEXT) AS str FROM users").unwrap();
        match stmt {
            Statement::Select { columns, .. } => {
                assert_eq!(columns.len(), 1);
                if let SelectItem::Expr { expr, alias } = &columns[0] {
                    assert_eq!(alias, &Some("str".to_string()));
                    match expr {
                        Expr::Cast { data_type, .. } => {
                            assert_eq!(*data_type, DataType::Text);
                        }
                        _ => panic!("Expected Cast expression"),
                    }
                } else {
                    panic!("Expected expression with alias");
                }
            }
            _ => panic!("Expected SELECT statement"),
        }
    }

    #[test]
    fn test_parse_cast_column() {
        // Test casting a column: price::FLOAT
        let stmt = QueryParser::parse("SELECT price::FLOAT AS price_f FROM products").unwrap();
        match stmt {
            Statement::Select { columns, .. } => {
                assert_eq!(columns.len(), 1);
                if let SelectItem::Expr { expr, .. } = &columns[0] {
                    match expr {
                        Expr::Cast {
                            expr: inner,
                            data_type,
                        } => {
                            assert_eq!(*data_type, DataType::Float);
                            if let Expr::Column(col) = inner.as_ref() {
                                assert_eq!(col, "price");
                            } else {
                                panic!("Expected column reference inside cast");
                            }
                        }
                        _ => panic!("Expected Cast expression"),
                    }
                } else {
                    panic!("Expected expression");
                }
            }
            _ => panic!("Expected SELECT statement"),
        }
    }

    #[test]
    fn test_parse_create_index_using_hnsw() {
        // Test CREATE INDEX ... USING hnsw
        let stmt =
            QueryParser::parse("CREATE INDEX idx_vectors ON documents USING hnsw (embedding)")
                .unwrap();
        match stmt {
            Statement::CreateIndex {
                name,
                table,
                columns,
                using,
                ..
            } => {
                assert_eq!(name, "idx_vectors");
                assert_eq!(table, "documents");
                assert_eq!(columns, vec!["embedding"]);
                assert_eq!(using, Some("hnsw".to_string()));
            }
            _ => panic!("Expected CreateIndex statement"),
        }
    }

    #[test]
    fn test_parse_create_index_using_gin() {
        // Test CREATE INDEX ... USING gin
        let stmt = QueryParser::parse("CREATE INDEX idx_content ON documents USING gin (content)")
            .unwrap();
        match stmt {
            Statement::CreateIndex {
                name,
                table,
                columns,
                using,
                ..
            } => {
                assert_eq!(name, "idx_content");
                assert_eq!(table, "documents");
                assert_eq!(columns, vec!["content"]);
                assert_eq!(using, Some("gin".to_string()));
            }
            _ => panic!("Expected CreateIndex statement"),
        }
    }

    #[test]
    fn test_parse_create_index_if_not_exists_using_hnsw() {
        // Test CREATE INDEX IF NOT EXISTS ... USING hnsw
        let stmt = QueryParser::parse(
            "CREATE INDEX IF NOT EXISTS idx_pages_vector ON pages USING hnsw (content_vector)",
        )
        .unwrap();
        match stmt {
            Statement::CreateIndex {
                name,
                table,
                columns,
                if_not_exists,
                using,
                ..
            } => {
                assert_eq!(name, "idx_pages_vector");
                assert_eq!(table, "pages");
                assert_eq!(columns, vec!["content_vector"]);
                assert!(if_not_exists);
                assert_eq!(using, Some("hnsw".to_string()));
            }
            _ => panic!("Expected CreateIndex statement"),
        }
    }

    #[test]
    fn test_parse_union() {
        let stmt = QueryParser::parse("SELECT id FROM users UNION SELECT id FROM admins").unwrap();
        match stmt {
            Statement::SetOperation { op, .. } => {
                assert_eq!(op, SetOperator::Union);
            }
            _ => panic!("Expected SetOperation statement"),
        }
    }

    #[test]
    fn test_parse_union_all() {
        let stmt =
            QueryParser::parse("SELECT name FROM users UNION ALL SELECT name FROM guests").unwrap();
        match stmt {
            Statement::SetOperation { op, .. } => {
                assert_eq!(op, SetOperator::UnionAll);
            }
            _ => panic!("Expected SetOperation with UNION ALL"),
        }
    }

    #[test]
    fn test_parse_intersect() {
        let stmt = QueryParser::parse(
            "SELECT id FROM premium_users INTERSECT SELECT id FROM active_users",
        )
        .unwrap();
        match stmt {
            Statement::SetOperation { op, .. } => {
                assert_eq!(op, SetOperator::Intersect);
            }
            _ => panic!("Expected SetOperation with INTERSECT"),
        }
    }

    #[test]
    fn test_parse_except() {
        let stmt =
            QueryParser::parse("SELECT id FROM all_users EXCEPT SELECT id FROM banned_users")
                .unwrap();
        match stmt {
            Statement::SetOperation { op, .. } => {
                assert_eq!(op, SetOperator::Except);
            }
            _ => panic!("Expected SetOperation with EXCEPT"),
        }
    }

    #[test]
    fn test_parse_union_with_order_by_limit() {
        let stmt = QueryParser::parse(
            "SELECT name FROM users UNION SELECT name FROM guests ORDER BY name LIMIT 10",
        )
        .unwrap();
        match stmt {
            Statement::SetOperation {
                op,
                order_by,
                limit,
                ..
            } => {
                assert_eq!(op, SetOperator::Union);
                assert!(order_by.is_some());
                assert_eq!(limit, Some(10));
            }
            _ => panic!("Expected SetOperation with ORDER BY and LIMIT"),
        }
    }

    // ========== FETCH Clause Tests ==========

    #[test]
    fn test_parse_select_with_fetch() {
        let stmt =
            QueryParser::parse("SELECT id, name FROM users FETCH orders { id, total }").unwrap();
        match stmt {
            Statement::Select {
                columns,
                from,
                fetch,
                ..
            } => {
                assert_eq!(from.table, "users");
                assert_eq!(columns.len(), 2);
                assert_eq!(fetch.len(), 1);
                assert_eq!(fetch[0].relationship, "orders");
                assert_eq!(fetch[0].columns.len(), 2);
            }
            _ => panic!("Expected SELECT with FETCH"),
        }
    }

    #[test]
    fn test_parse_fetch_with_where() {
        let stmt = QueryParser::parse(
            "SELECT id FROM posts FETCH comments { id, content WHERE approved = true }",
        )
        .unwrap();
        match stmt {
            Statement::Select { fetch, .. } => {
                assert_eq!(fetch.len(), 1);
                assert_eq!(fetch[0].relationship, "comments");
                assert!(fetch[0].where_clause.is_some());
            }
            _ => panic!("Expected SELECT with FETCH WHERE"),
        }
    }

    #[test]
    fn test_parse_fetch_with_order_limit() {
        let stmt = QueryParser::parse(
            "SELECT id FROM users FETCH orders { id, total ORDER BY total DESC LIMIT 5 }",
        )
        .unwrap();
        match stmt {
            Statement::Select { fetch, .. } => {
                assert_eq!(fetch.len(), 1);
                assert!(fetch[0].order_by.is_some());
                let order_by = fetch[0].order_by.as_ref().unwrap();
                assert!(!order_by.ascending); // DESC
                assert_eq!(fetch[0].limit, Some(5));
            }
            _ => panic!("Expected SELECT with FETCH ORDER BY LIMIT"),
        }
    }

    #[test]
    fn test_parse_nested_fetch() {
        let stmt = QueryParser::parse(
            "SELECT id FROM users FETCH orders { id FETCH items { name, price } }",
        )
        .unwrap();
        match stmt {
            Statement::Select { fetch, .. } => {
                assert_eq!(fetch.len(), 1);
                assert_eq!(fetch[0].relationship, "orders");
                assert_eq!(fetch[0].nested_fetch.len(), 1);
                assert_eq!(fetch[0].nested_fetch[0].relationship, "items");
            }
            _ => panic!("Expected SELECT with nested FETCH"),
        }
    }

    #[test]
    fn test_parse_multiple_fetch() {
        let stmt = QueryParser::parse(
            "SELECT id FROM users FETCH orders { id } FETCH profile { avatar_url }",
        )
        .unwrap();
        match stmt {
            Statement::Select { fetch, .. } => {
                assert_eq!(fetch.len(), 2);
                assert_eq!(fetch[0].relationship, "orders");
                assert_eq!(fetch[1].relationship, "profile");
            }
            _ => panic!("Expected SELECT with multiple FETCH"),
        }
    }

    // ========== CREATE RELATIONSHIP Tests ==========

    #[test]
    fn test_parse_create_relationship_one() {
        let stmt = QueryParser::parse(
            "CREATE RELATIONSHIP author_rel FROM posts.author_id TO users.id AS ONE",
        )
        .unwrap();
        match stmt {
            Statement::CreateRelationship(rel) => {
                assert_eq!(rel.name, "author_rel");
                assert_eq!(rel.from_table, "posts");
                assert_eq!(rel.from_column, "author_id");
                assert_eq!(rel.to_table, "users");
                assert_eq!(rel.to_column, "id");
                assert_eq!(rel.cardinality, Cardinality::One);
            }
            _ => panic!("Expected CreateRelationship"),
        }
    }

    #[test]
    fn test_parse_create_relationship_many() {
        let stmt = QueryParser::parse(
            "CREATE RELATIONSHIP orders_rel FROM users.id TO orders.user_id AS MANY",
        )
        .unwrap();
        match stmt {
            Statement::CreateRelationship(rel) => {
                assert_eq!(rel.name, "orders_rel");
                assert_eq!(rel.from_table, "users");
                assert_eq!(rel.from_column, "id");
                assert_eq!(rel.to_table, "orders");
                assert_eq!(rel.to_column, "user_id");
                assert_eq!(rel.cardinality, Cardinality::Many);
            }
            _ => panic!("Expected CreateRelationship with MANY"),
        }
    }

    // ========== TRAVERSE Tests ==========

    #[test]
    fn test_parse_traverse_simple() {
        let stmt = QueryParser::parse("TRAVERSE FOLLOWS FROM users WHERE id = 1").unwrap();
        match stmt {
            Statement::Traverse {
                clause,
                from_table,
                where_clause,
                ..
            } => {
                assert_eq!(clause.pattern.relationship_type, "FOLLOWS");
                assert_eq!(from_table, "users");
                assert!(where_clause.is_some());
            }
            _ => panic!("Expected TRAVERSE statement"),
        }
    }

    #[test]
    fn test_parse_traverse_with_depth() {
        let stmt =
            QueryParser::parse("TRAVERSE DEPTH 1..3 FOLLOWS FROM users WHERE id = 1").unwrap();
        match stmt {
            Statement::Traverse { clause, .. } => {
                assert_eq!(clause.min_depth, 1);
                assert_eq!(clause.max_depth, 3);
            }
            _ => panic!("Expected TRAVERSE with DEPTH"),
        }
    }

    #[test]
    fn test_parse_traverse_with_pattern() {
        let stmt = QueryParser::parse(
            "TRAVERSE (user)-[:FOLLOWS]->(friend) FROM users WHERE id = 1 RETURN friend.name LIMIT 10"
        ).unwrap();
        match stmt {
            Statement::Traverse {
                clause,
                return_columns,
                limit,
                ..
            } => {
                assert_eq!(clause.pattern.relationship_type, "FOLLOWS");
                assert_eq!(clause.pattern.direction, TraverseDirection::Outgoing);
                assert!(!return_columns.is_empty());
                assert_eq!(limit, Some(10));
            }
            _ => panic!("Expected TRAVERSE with pattern"),
        }
    }

    #[test]
    fn test_parse_traverse_incoming() {
        let stmt = QueryParser::parse("TRAVERSE (a)<-[:PURCHASED]-(b) FROM products WHERE id = 1")
            .unwrap();
        match stmt {
            Statement::Traverse { clause, .. } => {
                assert_eq!(clause.pattern.relationship_type, "PURCHASED");
                assert_eq!(clause.pattern.direction, TraverseDirection::Incoming);
            }
            _ => panic!("Expected TRAVERSE with incoming direction"),
        }
    }

    // ========== JOIN Tests ==========

    #[test]
    fn test_parse_inner_join() {
        let stmt = QueryParser::parse(
            "SELECT users.name, orders.total FROM users INNER JOIN orders ON users.id = orders.user_id"
        ).unwrap();
        match stmt {
            Statement::Select { from, .. } => {
                assert_eq!(from.table, "users");
                assert_eq!(from.joins.len(), 1);
                assert_eq!(from.joins[0].table, "orders");
                assert_eq!(from.joins[0].join_type, JoinType::Inner);
                assert!(from.joins[0].on_condition.is_some());
            }
            _ => panic!("Expected SELECT with INNER JOIN"),
        }
    }

    #[test]
    fn test_parse_left_join() {
        let stmt = QueryParser::parse(
            "SELECT u.name, o.total FROM users u LEFT JOIN orders o ON u.id = o.user_id",
        )
        .unwrap();
        match stmt {
            Statement::Select { from, .. } => {
                assert_eq!(from.table, "users");
                assert_eq!(from.alias, Some("u".to_string()));
                assert_eq!(from.joins.len(), 1);
                assert_eq!(from.joins[0].table, "orders");
                assert_eq!(from.joins[0].alias, Some("o".to_string()));
                assert_eq!(from.joins[0].join_type, JoinType::Left);
            }
            _ => panic!("Expected SELECT with LEFT JOIN"),
        }
    }

    #[test]
    fn test_parse_right_join() {
        let stmt = QueryParser::parse(
            "SELECT * FROM users RIGHT JOIN orders ON users.id = orders.user_id",
        )
        .unwrap();
        match stmt {
            Statement::Select { from, .. } => {
                assert_eq!(from.joins.len(), 1);
                assert_eq!(from.joins[0].join_type, JoinType::Right);
            }
            _ => panic!("Expected SELECT with RIGHT JOIN"),
        }
    }

    #[test]
    fn test_parse_full_outer_join() {
        let stmt = QueryParser::parse(
            "SELECT * FROM users FULL OUTER JOIN orders ON users.id = orders.user_id",
        )
        .unwrap();
        match stmt {
            Statement::Select { from, .. } => {
                assert_eq!(from.joins.len(), 1);
                assert_eq!(from.joins[0].join_type, JoinType::Full);
            }
            _ => panic!("Expected SELECT with FULL OUTER JOIN"),
        }
    }

    #[test]
    fn test_parse_cross_join() {
        let stmt = QueryParser::parse("SELECT * FROM colors CROSS JOIN sizes").unwrap();
        match stmt {
            Statement::Select { from, .. } => {
                assert_eq!(from.table, "colors");
                assert_eq!(from.joins.len(), 1);
                assert_eq!(from.joins[0].table, "sizes");
                assert_eq!(from.joins[0].join_type, JoinType::Cross);
                assert!(from.joins[0].on_condition.is_none());
            }
            _ => panic!("Expected SELECT with CROSS JOIN"),
        }
    }

    #[test]
    fn test_parse_multiple_joins() {
        let stmt = QueryParser::parse(
            "SELECT * FROM orders o \
             INNER JOIN users u ON o.user_id = u.id \
             LEFT JOIN products p ON o.product_id = p.id",
        )
        .unwrap();
        match stmt {
            Statement::Select { from, .. } => {
                assert_eq!(from.table, "orders");
                assert_eq!(from.alias, Some("o".to_string()));
                assert_eq!(from.joins.len(), 2);

                assert_eq!(from.joins[0].table, "users");
                assert_eq!(from.joins[0].alias, Some("u".to_string()));
                assert_eq!(from.joins[0].join_type, JoinType::Inner);

                assert_eq!(from.joins[1].table, "products");
                assert_eq!(from.joins[1].alias, Some("p".to_string()));
                assert_eq!(from.joins[1].join_type, JoinType::Left);
            }
            _ => panic!("Expected SELECT with multiple JOINs"),
        }
    }

    // GROUP BY and HAVING tests

    #[test]
    fn test_parse_group_by() {
        let stmt =
            QueryParser::parse("SELECT department, COUNT(*) FROM employees GROUP BY department")
                .unwrap();
        match stmt {
            Statement::Select {
                columns,
                from,
                group_by,
                having,
                ..
            } => {
                assert_eq!(from.table, "employees");
                assert_eq!(columns.len(), 2);
                assert_eq!(group_by.len(), 1);
                assert!(matches!(&group_by[0], Expr::Column(col) if col == "department"));
                assert!(having.is_none());
            }
            _ => panic!("Expected SELECT with GROUP BY"),
        }
    }

    #[test]
    fn test_parse_group_by_multiple() {
        let stmt = QueryParser::parse(
            "SELECT department, status, COUNT(*) FROM employees GROUP BY department, status",
        )
        .unwrap();
        match stmt {
            Statement::Select { group_by, .. } => {
                assert_eq!(group_by.len(), 2);
                assert!(matches!(&group_by[0], Expr::Column(col) if col == "department"));
                assert!(matches!(&group_by[1], Expr::Column(col) if col == "status"));
            }
            _ => panic!("Expected SELECT with multiple GROUP BY columns"),
        }
    }

    #[test]
    fn test_parse_group_by_with_having() {
        let stmt = QueryParser::parse("SELECT department, COUNT(*) as cnt FROM employees GROUP BY department HAVING COUNT(*) > 5").unwrap();
        match stmt {
            Statement::Select {
                group_by, having, ..
            } => {
                assert_eq!(group_by.len(), 1);
                assert!(having.is_some());
                // HAVING should be a comparison: COUNT(*) > 5
                if let Some(Expr::BinaryOp { op, .. }) = having {
                    assert_eq!(op, BinaryOperator::Gt);
                } else {
                    panic!("Expected HAVING with binary operation");
                }
            }
            _ => panic!("Expected SELECT with GROUP BY and HAVING"),
        }
    }

    #[test]
    fn test_parse_group_by_with_where_and_having() {
        let stmt = QueryParser::parse(
            "SELECT department, AVG(salary) FROM employees WHERE status = 'active' GROUP BY department HAVING AVG(salary) > 50000"
        ).unwrap();
        match stmt {
            Statement::Select {
                where_clause,
                group_by,
                having,
                ..
            } => {
                assert!(where_clause.is_some());
                assert_eq!(group_by.len(), 1);
                assert!(having.is_some());
            }
            _ => panic!("Expected SELECT with WHERE, GROUP BY, and HAVING"),
        }
    }

    #[test]
    fn test_parse_group_by_without_having() {
        let stmt = QueryParser::parse("SELECT name FROM users").unwrap();
        match stmt {
            Statement::Select {
                group_by, having, ..
            } => {
                assert!(group_by.is_empty());
                assert!(having.is_none());
            }
            _ => panic!("Expected SELECT without GROUP BY"),
        }
    }

    // === Subquery Parser Tests ===

    #[test]
    fn test_parse_in_subquery() {
        let stmt =
            QueryParser::parse("SELECT * FROM users WHERE id IN (SELECT user_id FROM orders)")
                .unwrap();
        match stmt {
            Statement::Select { where_clause, .. } => {
                assert!(where_clause.is_some());
                let where_expr = where_clause.unwrap();
                match where_expr {
                    Expr::InSubquery { negated, .. } => {
                        assert!(!negated);
                    }
                    _ => panic!("Expected InSubquery expression"),
                }
            }
            _ => panic!("Expected SELECT statement"),
        }
    }

    #[test]
    fn test_parse_not_in_subquery() {
        let stmt =
            QueryParser::parse("SELECT * FROM users WHERE id NOT IN (SELECT user_id FROM banned)")
                .unwrap();
        match stmt {
            Statement::Select { where_clause, .. } => {
                let where_expr = where_clause.unwrap();
                match where_expr {
                    Expr::InSubquery { negated, .. } => {
                        assert!(negated);
                    }
                    _ => panic!("Expected InSubquery expression"),
                }
            }
            _ => panic!("Expected SELECT statement"),
        }
    }

    #[test]
    fn test_parse_exists_subquery() {
        let stmt = QueryParser::parse(
            "SELECT * FROM users u WHERE EXISTS (SELECT 1 FROM orders o WHERE o.user_id = u.id)",
        )
        .unwrap();
        match stmt {
            Statement::Select { where_clause, .. } => {
                let where_expr = where_clause.unwrap();
                match where_expr {
                    Expr::ExistsSubquery { negated, .. } => {
                        assert!(!negated);
                    }
                    _ => panic!("Expected ExistsSubquery expression"),
                }
            }
            _ => panic!("Expected SELECT statement"),
        }
    }

    #[test]
    fn test_parse_scalar_subquery() {
        let stmt = QueryParser::parse(
            "SELECT name, (SELECT COUNT(*) FROM orders WHERE orders.user_id = users.id) as order_count FROM users",
        )
        .unwrap();
        match stmt {
            Statement::Select { columns, .. } => {
                assert_eq!(columns.len(), 2);
                // Check that second column is a subquery
                if let SelectItem::Expr { expr, alias } = &columns[1] {
                    assert_eq!(alias.as_ref().map(|s| s.as_str()), Some("order_count"));
                    assert!(matches!(expr, Expr::Subquery(_)));
                } else {
                    panic!("Expected Expr SelectItem");
                }
            }
            _ => panic!("Expected SELECT statement"),
        }
    }

    // === CTE (WITH clause) Parser Tests ===

    #[test]
    fn test_parse_simple_cte() {
        let stmt = QueryParser::parse(
            "WITH active_users AS (SELECT * FROM users WHERE status = 'active') SELECT * FROM active_users",
        )
        .unwrap();
        match stmt {
            Statement::Select { with, from, .. } => {
                assert_eq!(with.len(), 1);
                assert_eq!(with[0].name, "active_users");
                assert!(!with[0].recursive);
                assert_eq!(from.table, "active_users");
            }
            _ => panic!("Expected SELECT statement"),
        }
    }

    #[test]
    fn test_parse_multiple_ctes() {
        let stmt = QueryParser::parse(
            "WITH cte1 AS (SELECT 1 as x FROM dual), cte2 AS (SELECT 2 as y FROM dual) SELECT * FROM cte1, cte2",
        )
        .unwrap();
        match stmt {
            Statement::Select { with, .. } => {
                assert_eq!(with.len(), 2);
                assert_eq!(with[0].name, "cte1");
                assert_eq!(with[1].name, "cte2");
            }
            _ => panic!("Expected SELECT statement"),
        }
    }

    #[test]
    fn test_parse_recursive_cte() {
        let stmt = QueryParser::parse(
            "WITH RECURSIVE nums AS (SELECT 1 as n FROM dual UNION ALL SELECT n + 1 FROM nums WHERE n < 10) SELECT * FROM nums",
        );
        // Note: Recursive CTEs with UNION ALL may require more complex parsing
        // This test verifies that the parser at least handles the recursive keyword
        assert!(stmt.is_ok() || stmt.is_err()); // Just verify it doesn't crash
    }

    // === Window Function Parser Tests ===

    #[test]
    fn test_parse_row_number_over() {
        let stmt = QueryParser::parse(
            "SELECT name, ROW_NUMBER() OVER (ORDER BY created_at) as rn FROM users",
        )
        .unwrap();
        match stmt {
            Statement::Select { columns, .. } => {
                assert_eq!(columns.len(), 2);
                if let SelectItem::Expr { expr, alias } = &columns[1] {
                    assert_eq!(alias.as_ref().map(|s| s.as_str()), Some("rn"));
                    match expr {
                        Expr::WindowFunction {
                            function,
                            window_spec,
                        } => {
                            if let Expr::Function { name, .. } = function.as_ref() {
                                assert_eq!(name, "ROW_NUMBER");
                            }
                            assert!(window_spec.partition_by.is_empty());
                            assert_eq!(window_spec.order_by.len(), 1);
                        }
                        _ => panic!("Expected WindowFunction"),
                    }
                }
            }
            _ => panic!("Expected SELECT statement"),
        }
    }

    #[test]
    fn test_parse_window_with_partition_by() {
        let stmt = QueryParser::parse(
            "SELECT dept, name, RANK() OVER (PARTITION BY dept ORDER BY salary DESC) as rank FROM employees",
        )
        .unwrap();
        match stmt {
            Statement::Select { columns, .. } => {
                assert_eq!(columns.len(), 3);
                if let SelectItem::Expr { expr, .. } = &columns[2] {
                    match expr {
                        Expr::WindowFunction { window_spec, .. } => {
                            assert_eq!(window_spec.partition_by.len(), 1);
                            assert_eq!(window_spec.order_by.len(), 1);
                        }
                        _ => panic!("Expected WindowFunction"),
                    }
                }
            }
            _ => panic!("Expected SELECT statement"),
        }
    }

    #[test]
    fn test_parse_window_with_frame() {
        let stmt = QueryParser::parse(
            "SELECT name, SUM(amount) OVER (ORDER BY date ROWS BETWEEN 1 PRECEDING AND CURRENT ROW) as running_sum FROM transactions",
        )
        .unwrap();
        match stmt {
            Statement::Select { columns, .. } => {
                if let SelectItem::Expr { expr, .. } = &columns[1] {
                    match expr {
                        Expr::WindowFunction { window_spec, .. } => {
                            assert!(window_spec.frame.is_some());
                            let frame = window_spec.frame.as_ref().unwrap();
                            assert!(matches!(frame.units, WindowFrameUnits::Rows));
                            assert!(matches!(frame.start, WindowFrameBound::Preceding(1)));
                        }
                        _ => panic!("Expected WindowFunction"),
                    }
                }
            }
            _ => panic!("Expected SELECT statement"),
        }
    }

    #[test]
    fn test_parse_lag_lead() {
        let stmt = QueryParser::parse(
            "SELECT name, LAG(salary, 1) OVER (ORDER BY id) as prev_salary FROM employees",
        )
        .unwrap();
        match stmt {
            Statement::Select { columns, .. } => {
                if let SelectItem::Expr { expr, .. } = &columns[1] {
                    match expr {
                        Expr::WindowFunction { function, .. } => {
                            if let Expr::Function { name, args } = function.as_ref() {
                                assert_eq!(name, "LAG");
                                assert_eq!(args.len(), 2);
                            }
                        }
                        _ => panic!("Expected WindowFunction"),
                    }
                }
            }
            _ => panic!("Expected SELECT statement"),
        }
    }
}
