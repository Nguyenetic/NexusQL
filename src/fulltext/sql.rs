//! SQL surface for full-text search: `MATCH`, `fts_match()`, `bm25()`,
//! `fts_score()`, `snippet()`, `highlight()` and index candidate selection.
//!
//! Row-level functions only see one row, but BM25 needs corpus statistics.
//! The executor therefore calls [`prepare_select`] before evaluating a SELECT;
//! it validates constant queries and records statistics per (column, query)
//! in a statement-scoped thread local, the same pattern the spatial code uses
//! for prepared geometry. The executor is synchronous, so this is sound.

use super::index::{scan_stats, FullTextIndex};
use super::query::{self, CorpusStats, Query, DEFAULT_B, DEFAULT_K1};
use super::tokenizer::{tokenize, Mode};
use crate::query::ast::{BinaryOperator, Expr};
use crate::query::evaluator::{evaluate_expr, QueryParams};
use crate::storage::{IndexType, Table};
use crate::types::{Row, Value};
use crate::{NexusError, Result};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

type StatsKey = (String, String);

thread_local! {
    static STATS: RefCell<Option<HashMap<StatsKey, Arc<CorpusStats>>>> = const { RefCell::new(None) };
    static PARSED: RefCell<HashMap<String, Arc<Query>>> = RefCell::new(HashMap::new());
}

const PARSED_CACHE_LIMIT: usize = 256;

/// Parse with a small per-thread cache; queries repeat once per row.
pub fn parse_cached(text: &str) -> std::result::Result<Arc<Query>, query::QueryError> {
    if let Some(q) = PARSED.with(|cache| cache.borrow().get(text).cloned()) {
        return Ok(q);
    }
    let parsed = Arc::new(Query::parse(text)?);
    PARSED.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= PARSED_CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(text.to_string(), parsed.clone());
    });
    Ok(parsed)
}

/// Restores the enclosing statement's statistics when dropped.
pub struct FtsScope(Option<HashMap<StatsKey, Arc<CorpusStats>>>);

impl Drop for FtsScope {
    fn drop(&mut self) {
        let previous = self.0.take();
        STATS.with(|stats| *stats.borrow_mut() = previous);
    }
}

fn is_fts_function(name: &str) -> bool {
    matches!(
        name.to_ascii_uppercase().as_str(),
        "FTS_MATCH" | "BM25" | "FTS_SCORE" | "SNIPPET" | "HIGHLIGHT"
    )
}

/// Bare column name of `t.col` / `col`.
fn bare(column: &str) -> &str {
    column.rsplit_once('.').map_or(column, |(_, c)| c)
}

/// Literals and parameters only: values knowable before scanning rows.
fn constant(expr: &Expr) -> bool {
    match expr {
        Expr::Literal(_) | Expr::Parameter(_) => true,
        Expr::Cast { expr, .. } => constant(expr),
        _ => false,
    }
}

fn find_calls<'a>(expr: &'a Expr, out: &mut Vec<(&'a str, &'a [Expr])>) {
    match expr {
        Expr::BinaryOp { left, op, right } => {
            if *op == BinaryOperator::TextMatch {
                // Store as a two-argument call for uniform handling.
                if let (Expr::Column(_), true) = (left.as_ref(), constant(right)) {
                    out.push(("FTS_MATCH_OP", std::slice::from_ref(expr)));
                }
            }
            find_calls(left, out);
            find_calls(right, out);
        }
        Expr::Function { name, args } => {
            if is_fts_function(name) {
                out.push((name.as_str(), args.as_slice()));
            }
            args.iter().for_each(|a| find_calls(a, out));
        }
        Expr::Cast { expr, .. } | Expr::IsNull(expr) | Expr::IsNotNull(expr) => {
            find_calls(expr, out)
        }
        Expr::WindowFunction { function, .. } => find_calls(function, out),
        _ => {}
    }
}

fn fulltext_index<'t>(table: &'t Table, column: &str) -> Option<&'t FullTextIndex> {
    table.typed_indexes.values().find_map(|index| match index {
        IndexType::FullText(index) if index.column == column => Some(index),
        _ => None,
    })
}

fn query_text(expr: &Expr, params: &QueryParams) -> Option<String> {
    match evaluate_expr(&Row::new(vec![], vec![]), expr, params) {
        Value::Text(text) => Some(text),
        _ => None,
    }
}

/// Validate constant full-text queries in a SELECT over `table` and compute
/// BM25 statistics for each `bm25()` / `fts_score()` call.
pub fn prepare_select(table: &Table, exprs: &[&Expr], params: &QueryParams) -> Result<FtsScope> {
    let mut calls = Vec::new();
    for expr in exprs {
        find_calls(expr, &mut calls);
    }
    let mut stats: HashMap<StatsKey, Arc<CorpusStats>> = HashMap::new();
    for (name, args) in calls {
        let (column, query_expr) = match (name, args) {
            ("FTS_MATCH_OP", [Expr::BinaryOp { left, right, .. }]) => {
                (left.as_ref(), right.as_ref())
            }
            (_, [column, query_expr, ..]) => (column, query_expr),
            _ => continue,
        };
        if !constant(query_expr) {
            continue;
        }
        let Some(text) = query_text(query_expr, params) else {
            continue;
        };
        let parsed = parse_cached(&text).map_err(|e| NexusError::QueryError(e.to_string()))?;
        let ranking = matches!(name.to_ascii_uppercase().as_str(), "BM25" | "FTS_SCORE");
        let Expr::Column(column) = column else {
            continue;
        };
        let column = bare(column);
        if !ranking || table.name.is_empty() {
            continue;
        }
        let key = (column.to_string(), text);
        if stats.contains_key(&key) {
            continue;
        }
        let computed = match fulltext_index(table, column) {
            Some(index) => index.stats(&parsed),
            None => scan_stats(table.rows.iter().map(|r| r.get(column)), &parsed),
        };
        stats.insert(key, Arc::new(computed));
    }
    let previous = STATS.with(|s| s.replace(Some(stats)));
    Ok(FtsScope(previous))
}

fn statement_stats(column: &str, query: &str) -> Option<Arc<CorpusStats>> {
    STATS.with(|s| {
        s.borrow()
            .as_ref()
            .and_then(|m| m.get(&(column.to_string(), query.to_string())).cloned())
    })
}

/// `text MATCH query`: NULL text or query yields NULL; invalid queries are
/// rejected earlier by `prepare_select` when constant, otherwise yield NULL.
pub fn text_match(text: &Value, query_value: &Value) -> Value {
    let (Value::Text(text), Value::Text(q)) = (text, query_value) else {
        return Value::Null;
    };
    match parse_cached(q) {
        Ok(q) => Value::Boolean(q.matches(&tokenize(text, Mode::Document))),
        Err(_) => Value::Null,
    }
}

fn arg_text(row: &Row, args: &[Expr], i: usize, default: &str, params: &QueryParams) -> String {
    match args.get(i).map(|a| evaluate_expr(row, a, params)) {
        Some(Value::Text(s)) => s,
        Some(Value::Integer(n)) => n.to_string(),
        _ => default.to_string(),
    }
}

fn arg_number(row: &Row, args: &[Expr], i: usize, default: f64, params: &QueryParams) -> f64 {
    match args.get(i).map(|a| evaluate_expr(row, a, params)) {
        Some(Value::Integer(n)) => n as f64,
        Some(Value::Float(f)) => f.into_inner(),
        _ => default,
    }
}

/// Evaluate a full-text SQL function, or None if `name` is not one.
pub fn evaluate_function(
    row: &Row,
    name: &str,
    args: &[Expr],
    params: &QueryParams,
) -> Option<Value> {
    if !is_fts_function(name) {
        return None;
    }
    if args.len() < 2 {
        return Some(Value::Null);
    }
    let text_value = evaluate_expr(row, &args[0], params);
    let query_value = evaluate_expr(row, &args[1], params);
    let name = name.to_ascii_uppercase();
    if name == "FTS_MATCH" {
        return Some(text_match(&text_value, &query_value));
    }
    let (Value::Text(text), Value::Text(query_text)) = (&text_value, &query_value) else {
        return Some(Value::Null);
    };
    let Ok(parsed) = parse_cached(query_text) else {
        return Some(Value::Null);
    };
    Some(match name.as_str() {
        "BM25" | "FTS_SCORE" => {
            let stats = match &args[0] {
                Expr::Column(column) => statement_stats(bare(column), query_text),
                _ => None,
            };
            let k1 = arg_number(row, args, 2, DEFAULT_K1, params);
            let b = arg_number(row, args, 3, DEFAULT_B, params);
            let tokens = tokenize(text, Mode::Document);
            let score = query::bm25(&tokens, &parsed, stats.as_deref(), k1, b).unwrap_or(0.0);
            // bm25() follows SQLite FTS5: lower (more negative) is better, so
            // ORDER BY bm25(...) puts the best match first. fts_score() is the
            // positive score for ORDER BY ... DESC.
            Value::Float(ordered_float::OrderedFloat(
                if name == "BM25" && score != 0.0 {
                    -score
                } else {
                    score
                },
            ))
        }
        "SNIPPET" => {
            let open = arg_text(row, args, 2, "[", params);
            let close = arg_text(row, args, 3, "]", params);
            let ellipsis = arg_text(row, args, 4, "...", params);
            let tokens = arg_number(row, args, 5, 16.0, params).clamp(1.0, 64.0) as usize;
            Value::Text(query::snippet(
                text, &parsed, &open, &close, &ellipsis, tokens,
            ))
        }
        "HIGHLIGHT" => {
            let open = arg_text(row, args, 2, "[", params);
            let close = arg_text(row, args, 3, "]", params);
            Value::Text(query::highlight(text, &parsed, &open, &close))
        }
        _ => Value::Null,
    })
}

/// Index candidates for one WHERE leaf (`col MATCH q` / `fts_match(col, q)`),
/// or None when no full-text index applies. Candidates are exact; the WHERE
/// clause is still re-evaluated on them by the executor.
pub fn candidates(
    table: &Table,
    qualifier: &str,
    expr: &Expr,
    params: &QueryParams,
) -> Option<Vec<usize>> {
    let (column, query_expr) = match expr {
        Expr::BinaryOp {
            left,
            op: BinaryOperator::TextMatch,
            right,
        } => (left.as_ref(), right.as_ref()),
        Expr::Function { name, args }
            if name.eq_ignore_ascii_case("FTS_MATCH") && args.len() == 2 =>
        {
            (&args[0], &args[1])
        }
        _ => return None,
    };
    let Expr::Column(column) = column else {
        return None;
    };
    let column = match column.split_once('.') {
        Some((q, c)) if q == qualifier || q == table.name => c,
        Some(_) => return None,
        None => column.as_str(),
    };
    if !constant(query_expr) {
        return None;
    }
    let index = fulltext_index(table, column)?;
    let parsed = parse_cached(&query_text(query_expr, params)?).ok()?;
    Some(index.search(&parsed))
}

/// Statistics helper for callers that hold a table directly (tests, tools).
pub fn column_stats(table: &Table, column: &str, query: &Query) -> CorpusStats {
    match fulltext_index(table, column) {
        Some(index) => index.stats(query),
        None => scan_stats(table.rows.iter().map(|r| r.get(column)), query),
    }
}
