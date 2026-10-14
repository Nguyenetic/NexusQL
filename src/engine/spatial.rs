//! Conservative spatial access paths: bounding-box candidates followed by the
//! original SQL predicate in the executor. Never prune on just one arm of OR.

use crate::geometry::{geometry, number};
use crate::index::{GiSTIndexType, GiSTPredicate};
use crate::query::ast::{BinaryOperator, Expr};
use crate::query::evaluator::{evaluate_expr, QueryParams};
use crate::storage::{IndexType, Table};
use crate::types::Row;

pub(super) fn qualify(row: &Row, qualifier: &str) -> Row {
    Row::new(
        row.columns
            .iter()
            .map(|c| format!("{qualifier}.{c}"))
            .collect(),
        row.values.clone(),
    )
}

// Only constants, parameters and columns provided by an outer join row may be
// evaluated before scanning this table. Row-dependent expressions must fall back.
fn available(expr: &Expr, outer: &Row) -> bool {
    match expr {
        Expr::Literal(_) | Expr::Parameter(_) => true,
        Expr::Column(c) => outer.get(c).is_some(),
        Expr::Function { args, .. } => args.iter().all(|e| available(e, outer)),
        Expr::Cast { expr, .. } | Expr::IsNull(expr) | Expr::IsNotNull(expr) => {
            available(expr, outer)
        }
        Expr::BinaryOp { left, right, .. } => available(left, outer) && available(right, outer),
        _ => false,
    }
}

pub(super) fn candidates(
    table: &Table,
    qualifier: &str,
    expr: &Expr,
    outer: &Row,
    params: &QueryParams,
) -> Option<Vec<usize>> {
    if let Expr::BinaryOp { left, op, right } = expr {
        return match op {
            BinaryOperator::And => candidates(table, qualifier, left, outer, params)
                .or_else(|| candidates(table, qualifier, right, outer, params)),
            BinaryOperator::Or => {
                let mut ids = candidates(table, qualifier, left, outer, params)?;
                ids.extend(candidates(table, qualifier, right, outer, params)?);
                ids.sort_unstable();
                ids.dedup();
                Some(ids)
            }
            _ => None,
        };
    }
    let Expr::Function { name, args } = expr else {
        return None;
    };
    let name = name.to_ascii_uppercase();
    if !matches!(
        name.as_str(),
        "ST_CONTAINS" | "ST_WITHIN" | "ST_INTERSECTS" | "ST_DWITHIN" | "ST_COVERS" | "ST_COVEREDBY"
    ) || args.len() != if name == "ST_DWITHIN" { 3 } else { 2 }
    {
        return None;
    }
    for index in table.typed_indexes.values() {
        let IndexType::GiST(index) = index else {
            continue;
        };
        if index.index_type() != GiSTIndexType::RTree {
            continue;
        }
        for side in 0..2 {
            let Expr::Column(column) = &args[side] else {
                continue;
            };
            let indexed = column == &format!("{qualifier}.{}", index.column_name)
                || (column == &index.column_name && outer.get(column).is_none());
            if !indexed || !available(&args[1 - side], outer) {
                continue;
            }
            let other = evaluate_expr(outer, &args[1 - side], params);
            let g = geometry(&other)?;
            let bounds = g.bounds()?;
            let radius = if name == "ST_DWITHIN" {
                if !available(&args[2], outer) {
                    continue;
                }
                let radius = number(&evaluate_expr(outer, &args[2], params))?;
                if radius < 0.0 {
                    return Some(vec![]);
                }
                radius
            } else {
                0.0
            };
            let coords = [
                bounds.min().x - radius,
                bounds.min().y - radius,
                bounds.max().x + radius,
                bounds.max().y + radius,
            ];
            // Overflow must cause a scan, never a false-negative empty lookup.
            if coords.iter().any(|v| !v.is_finite()) {
                return None;
            }
            return Some(index.search(&GiSTPredicate::BoundingBox {
                min_x: coords[0],
                min_y: coords[1],
                max_x: coords[2],
                max_y: coords[3],
            }));
        }
    }
    None
}
