//! Conservative spatial access paths: bounding-box candidates followed by the
//! original SQL predicate in the executor. Never prune on just one arm of OR.

use crate::geometry::prepared::{Predicate, PreparedGeometryCache};
use crate::geometry::{geometry, number};
use crate::index::{GiSTIndexType, GiSTPredicate};
use crate::query::ast::{BinaryOperator, Expr};
use crate::query::evaluator::{evaluate_expr, QueryParams};
use crate::storage::{IndexType, Table};
use crate::types::{Row, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;

#[derive(Default)]
struct StatementState {
    prepared: PreparedGeometryCache,
    constants: BTreeMap<Expr, Value>,
    constant_bytes: usize,
}

thread_local! {
    // Executor::execute is synchronous. geo's prepared graph is !Send; keeping
    // it here avoids changing Executor/QueryParams' Send/Sync or public layout.
    static STATEMENT: RefCell<Option<StatementState>> = const { RefCell::new(None) };
}

pub(super) struct StatementScope(Option<StatementState>);

impl StatementScope {
    pub(super) fn enter() -> Self {
        Self(STATEMENT.with(|state| state.replace(Some(StatementState::default()))))
    }
}

impl Drop for StatementScope {
    fn drop(&mut self) {
        // Errors and unwinding release caches too. Nested statements get their
        // own constants/parameters and restore the enclosing statement's cache.
        STATEMENT.with(|state| state.replace(self.0.take()));
    }
}

fn value_bytes(value: &Value) -> usize {
    std::mem::size_of::<Value>()
        + match value {
            Value::Text(s) => s.len(),
            Value::Blob(b) => b.len(),
            Value::Vector(v) => v.len() * std::mem::size_of::<f32>(),
            Value::Geometry(g) => g.estimated_size(),
            _ => 0,
        }
}

// Unlike available(), this only accepts known deterministic expressions. Never
// memoize a row-dependent expression or a function such as RANDOM()/NOW().
fn constant_bytes(expr: &Expr) -> Option<usize> {
    let children = match expr {
        Expr::Literal(value) => value_bytes(value),
        Expr::Parameter(crate::query::ast::Parameter::Named(name)) => name.len(),
        Expr::Parameter(_) => 0,
        Expr::Cast { expr, .. } | Expr::IsNull(expr) | Expr::IsNotNull(expr) => {
            constant_bytes(expr)?
        }
        Expr::BinaryOp { left, right, .. } => constant_bytes(left)? + constant_bytes(right)?,
        Expr::Function { name, args } if name.to_ascii_uppercase().starts_with("ST_") => {
            name.len() + args.iter().map(constant_bytes).sum::<Option<usize>>()?
        }
        _ => return None,
    };
    Some(std::mem::size_of::<Expr>() + children)
}

fn predicate_argument(row: &Row, expr: &Expr, params: &QueryParams) -> Value {
    let size = constant_bytes(expr);
    if size.is_some() {
        if let Some(value) = STATEMENT.with(|state| {
            state
                .borrow()
                .as_ref()
                .and_then(|state| state.constants.get(expr).cloned())
        }) {
            return value;
        }
    }
    // Evaluation may recursively invoke another spatial predicate. Do not hold
    // a RefCell borrow across the evaluator call.
    let evaluated = evaluate_expr(row, expr, params);
    let value = if let Value::Text(text) = &evaluated {
        crate::geometry::GeometryValue::from_wkt(text)
            .ok()
            .map(Value::Geometry)
            .unwrap_or(Value::Null)
    } else {
        evaluated
    };
    if let Some(size) = size {
        STATEMENT.with(|state| {
            if let Some(state) = state.borrow_mut().as_mut() {
                let bytes = size + value_bytes(&value);
                if state.constants.len() < 128
                    && bytes <= 1_048_576_usize.saturating_sub(state.constant_bytes)
                {
                    state.constant_bytes += bytes;
                    state.constants.insert(expr.clone(), value.clone());
                }
            }
        });
    }
    value
}

impl super::Executor {
    pub(crate) fn evaluate_spatial_predicate(
        row: &Row,
        predicate: Predicate,
        args: &[Expr],
        params: &QueryParams,
    ) -> Value {
        if args.len() != 2 {
            return Value::Null;
        }
        let a = predicate_argument(row, &args[0], params);
        let b = predicate_argument(row, &args[1], params);
        let (Some(a), Some(b)) = (geometry(&a), geometry(&b)) else {
            return Value::Null;
        };
        STATEMENT.with(|state| {
            let result = if let Some(state) = state.borrow_mut().as_mut() {
                state.prepared.evaluate(predicate, &a, &b)
            } else {
                predicate.unprepared(&a, &b)
            };
            result.map(Value::Boolean).unwrap_or(Value::Null)
        })
    }
}

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

#[cfg(test)]
mod prepared_tests {
    use super::*;
    use crate::geometry::GeometryValue;
    use crate::query::ast::Parameter;

    fn evaluate(expr: Expr, params: &QueryParams) {
        let row = Row::new(
            vec!["p".into()],
            vec![Value::Geometry(
                GeometryValue::from_wkt("POINT(1 1)").unwrap(),
            )],
        );
        assert_eq!(
            super::super::Executor::evaluate_spatial_predicate(
                &row,
                Predicate::Contains,
                &[expr, Expr::Column("p".into())],
                params,
            ),
            Value::Boolean(true)
        );
    }

    #[test]
    fn constants_prepare_once_and_nested_scopes_release_state() {
        assert!(STATEMENT.with(|s| s.borrow().is_none()));
        let params = QueryParams::with_positional(vec![Value::Text(
            "POLYGON((0 0,10 0,10 10,0 10,0 0))".into(),
        )]);
        let expr = Expr::Parameter(Parameter::Positional(1));
        {
            let _statement = StatementScope::enter();
            for _ in 0..100 {
                evaluate(expr.clone(), &params);
            }
            STATEMENT.with(|s| {
                let s = s.borrow();
                let s = s.as_ref().unwrap();
                assert_eq!(s.prepared.len(), 1);
                assert_eq!(s.constants.len(), 1);
            });
            {
                let _nested = StatementScope::enter();
                STATEMENT.with(|s| assert!(s.borrow().as_ref().unwrap().prepared.is_empty()));
                evaluate(expr.clone(), &params);
            }
            STATEMENT.with(|s| assert_eq!(s.borrow().as_ref().unwrap().prepared.len(), 1));
            std::thread::spawn(|| assert!(STATEMENT.with(|s| s.borrow().is_none())))
                .join()
                .unwrap();
        }
        assert!(STATEMENT.with(|s| s.borrow().is_none()));
        // A standalone expression evaluation must not create a persistent cache.
        evaluate(expr, &params);
        assert!(STATEMENT.with(|s| s.borrow().is_none()));
    }

    #[test]
    fn errors_and_unwinding_release_statement_cache() {
        let mut executor = super::super::Executor::new();
        assert!(executor
            .execute(
                &crate::QueryParser::parse("SELECT * FROM missing").unwrap(),
                &QueryParams::new()
            )
            .is_err());
        assert!(STATEMENT.with(|s| s.borrow().is_none()));
        let _ = std::panic::catch_unwind(|| {
            let _statement = StatementScope::enter();
            panic!("test unwinding");
        });
        assert!(STATEMENT.with(|s| s.borrow().is_none()));
    }

    #[test]
    fn constant_cache_has_entry_and_byte_limits() {
        let _statement = StatementScope::enter();
        let row = Row::new(vec![], vec![]);
        for i in 0..256 {
            predicate_argument(&row, &Expr::Literal(Value::Integer(i)), &QueryParams::new());
        }
        STATEMENT.with(|s| assert_eq!(s.borrow().as_ref().unwrap().constants.len(), 128));
        let _nested = StatementScope::enter();
        predicate_argument(
            &row,
            &Expr::Literal(Value::Text("x".repeat(1_048_576))),
            &QueryParams::new(),
        );
        STATEMENT.with(|s| assert!(s.borrow().as_ref().unwrap().constants.is_empty()));
        assert!(constant_bytes(&Expr::Function {
            name: "RANDOM".into(),
            args: vec![]
        })
        .is_none());
        assert!(constant_bytes(&Expr::Column("geom".into())).is_none());
    }
}
