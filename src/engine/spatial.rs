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

/// Ascending <-> LIMIT on a single indexed geometry column. Keep at most k
/// exact distances, using row offsets to preserve the stable full sort's ties.
pub(super) fn knn_rows(
    table: &Table,
    from: &crate::query::ast::FromClause,
    expr: &Expr,
    limit: usize,
    filter: Option<&Expr>,
    params: &QueryParams,
) -> Option<Vec<Row>> {
    use crate::types::Value;
    use geo::{Distance, Euclidean};
    use ordered_float::OrderedFloat;
    use std::collections::BinaryHeap;

    let Expr::BinaryOp {
        left,
        op: BinaryOperator::L2Distance,
        right,
    } = expr
    else {
        return None;
    };
    let outer = Row::new(vec![], vec![]);
    let (column, target, reversed) = match (&**left, &**right) {
        (Expr::Column(column), target) if available(target, &outer) => (column, target, false),
        (target, Expr::Column(column)) if available(target, &outer) => (column, target, true),
        _ => return None,
    };
    let target = evaluate_expr(&outer, target, params);
    let query = geometry(&target)?;
    let bounds = query.bounds()?;
    let qualifier = from.alias.as_deref().unwrap_or(&from.table);
    let index = table.typed_indexes.values().find_map(|index| {
        let IndexType::GiST(index) = index else {
            return None;
        };
        ((column == &index.column_name || column == &format!("{qualifier}.{}", index.column_name))
            && index.supports_knn(query.srid, table.rows.len()))
        .then_some(index)
    })?;

    // rstar's iterator accepts a point. A circle around the query's bounding
    // rectangle covers any query shape: distance(box, center) - radius is a
    // conservative, monotonic lower bound on the exact geometry distance.
    let center = [
        bounds.min().x * 0.5 + bounds.max().x * 0.5,
        bounds.min().y * 0.5 + bounds.max().y * 0.5,
    ];
    let radius = (bounds.max().x - center[0])
        .abs()
        .max((bounds.min().x - center[0]).abs())
        .hypot(
            (bounds.max().y - center[1])
                .abs()
                .max((bounds.min().y - center[1]).abs()),
        );
    if !radius.is_finite() {
        return None;
    }
    let neighbors = index.nearest_neighbors(center);
    if limit == 0 {
        return Some(vec![]);
    }
    let mut best: BinaryHeap<(OrderedFloat<f64>, usize)> = BinaryHeap::new();
    for (id, distance_2) in neighbors {
        if best.len() == limit {
            let worst = best.peek()?.0.into_inner();
            let bound = distance_2.sqrt();
            // Roundoff may only cause extra work, never exclude a boundary tie.
            // Overflowed squared bounds cannot justify pruning either.
            let tolerance = 32.0
                * f64::EPSILON
                * center[0]
                    .abs()
                    .max(center[1].abs())
                    .max(radius)
                    .max(bound)
                    .max(worst)
                    .max(1.0);
            if bound.is_finite() && bound > worst + radius + tolerance {
                break;
            }
        }
        let row = table.rows.get(id)?;
        if let Some(filter) = filter {
            let qualified;
            let filter_row = if from.alias.is_some() {
                qualified = qualify(row, qualifier);
                &qualified
            } else {
                row
            };
            if !crate::query::evaluator::evaluate_where(filter_row, filter, params) {
                continue;
            }
        }
        let Value::Geometry(g) = row.get(&index.column_name)? else {
            return None;
        };
        // Keep the SQL operand order even for floating-point geometry algorithms.
        let distance = if reversed {
            Euclidean.distance(query.shape(), g.shape())
        } else {
            Euclidean.distance(g.shape(), query.shape())
        };
        if !distance.is_finite() {
            return None;
        }
        let candidate = (OrderedFloat(distance), id);
        if best.len() < limit {
            best.push(candidate);
        } else if best.peek().is_some_and(|worst| candidate < *worst) {
            best.pop();
            best.push(candidate);
        }
    }
    Some(
        best.into_sorted_vec()
            .into_iter()
            .map(|(_, id)| table.rows[id].clone())
            .collect(),
    )
}
