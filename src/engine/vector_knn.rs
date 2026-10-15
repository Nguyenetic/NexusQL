//! `ORDER BY <distance to a constant vector> LIMIT k` through an HNSW index.
//!
//! Applies to a single-table SELECT whose ORDER BY is `col <-> q`, `col <=> q`,
//! `l2_distance(col, q)` / `vector_distance(col, q)` or `cosine_distance(col, q)`
//! (either operand order), where `col` has an HNSW index of the same metric and
//! `q` is a literal or parameter. The graph proposes candidates; each candidate
//! is checked against the full WHERE clause and ordered by its exact distance,
//! computed by the same expression the scan would evaluate. When the filter
//! rejects too many candidates the search widens, and if the graph cannot
//! supply `k` matches the query falls back to the exact scan. Like any HNSW
//! index the result is approximate: a true neighbor the graph misses is not
//! returned.

use super::spatial::{available, qualify};
use crate::query::ast::{BinaryOperator, Expr, FromClause};
use crate::query::evaluator::{evaluate_expr, evaluate_where, QueryParams};
use crate::storage::{IndexType, Table};
use crate::types::{Row, Value};
use crate::vector::sql::{indexable, VectorMetric};

/// The metric an ORDER BY expression measures and its two operands.
fn distance_call(expr: &Expr) -> Option<(VectorMetric, &Expr, &Expr)> {
    match expr {
        Expr::BinaryOp { left, op, right } => {
            let metric = match op {
                BinaryOperator::L2Distance => VectorMetric::L2,
                BinaryOperator::CosineDistance => VectorMetric::Cosine,
                _ => return None,
            };
            Some((metric, left, right))
        }
        Expr::Function { name, args } if args.len() == 2 => {
            let metric = match name.to_ascii_uppercase().as_str() {
                "L2_DISTANCE" | "VECTOR_DISTANCE" => VectorMetric::L2,
                "COSINE_DISTANCE" => VectorMetric::Cosine,
                _ => return None,
            };
            Some((metric, &args[0], &args[1]))
        }
        _ => None,
    }
}

pub(super) fn knn_rows(
    table: &Table,
    from: &FromClause,
    expr: &Expr,
    limit: usize,
    filter: Option<&Expr>,
    params: &QueryParams,
) -> Option<Vec<Row>> {
    let (metric, left, right) = distance_call(expr)?;
    let outer = Row::new(vec![], vec![]);
    let (column, target) = match (left, right) {
        (Expr::Column(column), target) if available(target, &outer) => (column, target),
        (target, Expr::Column(column)) if available(target, &outer) => (column, target),
        _ => return None,
    };
    let qualifier = from.alias.as_deref().unwrap_or(&from.table);
    let index = table.typed_indexes.values().find_map(|index| match index {
        IndexType::HNSW(index)
            if index.def.metric == metric
                && (column == &index.def.column
                    || column == &format!("{qualifier}.{}", index.def.column)) =>
        {
            Some(index)
        }
        _ => None,
    })?;
    let target = evaluate_expr(&outer, target, params);
    let query = indexable(Some(&target), metric)?;
    if limit == 0 {
        return Some(vec![]);
    }
    let total = table.rows.len();
    let mut fetch = limit.min(total);
    loop {
        let ef = index.def.ef_search.max(fetch);
        let hits = index.search(&table.rows, &query, fetch, ef)?;
        let mut found: Vec<(Value, usize)> = Vec::with_capacity(limit);
        for hit in &hits {
            let id = hit.row_id as usize;
            let row = table.rows.get(id)?;
            let qualified;
            let row = if from.alias.is_some() {
                qualified = qualify(row, qualifier);
                &qualified
            } else {
                row
            };
            if filter.is_some_and(|f| !evaluate_where(row, f, params)) {
                continue;
            }
            found.push((evaluate_expr(row, expr, params), id));
        }
        if found.len() >= limit || hits.len() == total {
            // Exact distances; ties keep table order, as the stable scan sort does.
            found.sort();
            found.truncate(limit);
            return Some(
                found
                    .into_iter()
                    .map(|(_, id)| table.rows[id].clone())
                    .collect(),
            );
        }
        if fetch >= total || hits.len() < fetch {
            // The graph cannot supply enough matching rows: scan exactly.
            return None;
        }
        fetch = (fetch * 4).min(total);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::ast::Statement;
    use crate::query::parser::QueryParser;
    use crate::vector::sql::{VectorColumnIndex, VectorIndexDefinition};

    fn table(metric: VectorMetric, with_null: bool) -> Table {
        let columns = vec!["id".to_string(), "e".to_string()];
        let mut t = Table::new("t".into(), columns.clone());
        let def = VectorIndexDefinition {
            name: "h".into(),
            column: "e".into(),
            metric,
            m: 8,
            ef_construction: 32,
            ef_search: 32,
        };
        t.schema.vector_indexes.push(def.clone());
        t.typed_indexes
            .insert("h".into(), IndexType::HNSW(VectorColumnIndex::new(def)));
        for i in 0..100 {
            let v = Value::Vector(vec![1.0 + i as f32, (i % 5) as f32]);
            t.insert(Row::new(columns.clone(), vec![Value::Integer(i), v]));
        }
        if with_null {
            t.insert(Row::new(columns, vec![Value::Integer(100), Value::Null]));
        }
        t
    }

    fn plan(sql: &str) -> (FromClause, Expr, usize, Option<Expr>) {
        let Statement::Select {
            from,
            order_by,
            limit,
            where_clause,
            ..
        } = QueryParser::parse(sql).unwrap()
        else {
            panic!("select");
        };
        (from, order_by.unwrap().expr, limit.unwrap(), where_clause)
    }

    fn ids(rows: &[Row]) -> Vec<i64> {
        rows.iter()
            .map(|r| match r.get("id") {
                Some(Value::Integer(i)) => *i,
                other => panic!("{other:?}"),
            })
            .collect()
    }

    #[test]
    fn uses_index_for_matching_metric_only() {
        let t = table(VectorMetric::L2, false);
        let params = QueryParams::with_positional(vec![Value::Vector(vec![42.0, 1.0])]);
        let (from, expr, k, filter) = plan("SELECT id FROM t ORDER BY e <-> $1 LIMIT 3");
        let rows = knn_rows(&t, &from, &expr, k, filter.as_ref(), &params).unwrap();
        assert_eq!(ids(&rows)[0], 41);
        // A cosine query cannot use an L2 graph.
        let (from, expr, k, filter) = plan("SELECT id FROM t ORDER BY e <=> $1 LIMIT 3");
        assert!(knn_rows(&t, &from, &expr, k, filter.as_ref(), &params).is_none());
    }

    #[test]
    fn filter_is_applied_and_selective_filters_widen_the_search() {
        let t = table(VectorMetric::L2, false);
        let params = QueryParams::with_positional(vec![Value::Vector(vec![10.0, 0.0])]);
        let (from, expr, k, filter) =
            plan("SELECT id FROM t WHERE id >= 90 ORDER BY l2_distance(e, $1) LIMIT 2");
        let rows = knn_rows(&t, &from, &expr, k, filter.as_ref(), &params).unwrap();
        assert_eq!(ids(&rows), [90, 91]);
    }

    #[test]
    fn unindexed_rows_force_a_scan() {
        let t = table(VectorMetric::Cosine, true);
        let params = QueryParams::with_positional(vec![Value::Vector(vec![1.0, 0.0])]);
        let (from, expr, k, filter) =
            plan("SELECT id FROM t ORDER BY cosine_distance(e, $1) LIMIT 1");
        assert!(knn_rows(&t, &from, &expr, k, filter.as_ref(), &params).is_none());
    }
}
