//! WHERE Clause Expression Evaluator
//! PROPRIETARY

use crate::engine::hdc::{BscVec, Hypervector, MapVec};
use crate::query::ast::{BinaryOperator, Expr, Parameter};
use crate::types::{Row, Value};
use std::collections::HashMap;

/// Parameter values for query execution
#[derive(Debug, Clone, Default)]
pub struct QueryParams {
    /// Positional parameters (1-indexed)
    pub positional: Vec<Value>,
    /// Named parameters
    pub named: HashMap<String, Value>,
}

impl QueryParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_positional(params: Vec<Value>) -> Self {
        Self {
            positional: params,
            named: HashMap::new(),
        }
    }

    pub fn with_named(params: HashMap<String, Value>) -> Self {
        Self {
            positional: Vec::new(),
            named: params,
        }
    }

    pub fn get_positional(&self, index: usize) -> Option<&Value> {
        // Convert from 1-based to 0-based index
        if index > 0 && index <= self.positional.len() {
            self.positional.get(index - 1)
        } else {
            None
        }
    }

    pub fn get_named(&self, name: &str) -> Option<&Value> {
        self.named.get(name)
    }
}

/// Evaluate WHERE clause expression against a row
pub fn evaluate_where(row: &Row, expr: &Expr, params: &QueryParams) -> bool {
    match evaluate_expr(row, expr, params) {
        Value::Boolean(b) => b,
        Value::Integer(i) => i != 0,
        Value::Null => false,
        _ => false,
    }
}

/// Evaluate any expression
pub fn evaluate_expr(row: &Row, expr: &Expr, params: &QueryParams) -> Value {
    match expr {
        Expr::Column(col_name) => row
            .get(col_name)
            .or_else(|| {
                // Single-table scans carry bare column names. Join rows are fully
                // qualified, so never use this fallback when qualifiers are present.
                if !row.columns.iter().any(|c| c.contains('.')) {
                    col_name
                        .rsplit_once('.')
                        .and_then(|(_, name)| row.get(name))
                } else {
                    None
                }
            })
            .cloned()
            .unwrap_or(Value::Null),

        Expr::Literal(val) => val.clone(),

        Expr::Parameter(param) => match param {
            Parameter::Positional(index) => params
                .get_positional(*index)
                .cloned()
                .unwrap_or(Value::Null),
            Parameter::Named(name) => params.get_named(name).cloned().unwrap_or(Value::Null),
        },

        Expr::BinaryOp { left, op, right } => {
            let left_val = evaluate_expr(row, left, params);
            let right_val = evaluate_expr(row, right, params);
            evaluate_binary_op(&left_val, op, &right_val)
        }

        Expr::Function { name, args } => evaluate_function(row, name, args, params),

        Expr::Wildcard => {
            // Wildcard (*) in expressions - typically used inside COUNT(*)
            // Return a special marker value that aggregate handlers can recognize
            Value::Integer(1) // Each row counts as 1
        }

        Expr::IsNull(inner) => {
            let val = evaluate_expr(row, inner, params);
            Value::Boolean(matches!(val, Value::Null))
        }

        Expr::IsNotNull(inner) => {
            let val = evaluate_expr(row, inner, params);
            Value::Boolean(!matches!(val, Value::Null))
        }

        Expr::Cast { expr, data_type } => {
            let val = evaluate_expr(row, expr, params);
            cast_value(val, data_type)
        }

        // Subquery expressions require executor context and are evaluated there
        // These placeholders should not be reached during normal row-level evaluation
        Expr::Subquery(_) => {
            // Scalar subqueries are evaluated by the executor before row evaluation
            Value::Null
        }

        Expr::InSubquery { .. } => {
            // IN subqueries are evaluated by the executor
            Value::Boolean(false)
        }

        Expr::ExistsSubquery { .. } => {
            // EXISTS subqueries are evaluated by the executor
            Value::Boolean(false)
        }

        Expr::WindowFunction { .. } => {
            // Window functions are evaluated by the executor with partition context
            Value::Null
        }
    }
}

/// Cast a value to the specified data type
fn cast_value(value: Value, target_type: &crate::query::ast::DataType) -> Value {
    use crate::query::ast::DataType;

    match (value, target_type) {
        // NULL stays NULL
        (Value::Null, _) => Value::Null,
        (Value::Geometry(g), DataType::Geometry) => Value::Geometry(g),
        (Value::Text(s), DataType::Geometry) => crate::geometry::GeometryValue::from_wkt(&s)
            .map(Value::Geometry)
            .unwrap_or(Value::Null),
        (Value::Geometry(g), DataType::Text) => Value::Text(g.as_text()),

        // Cast to Integer
        (Value::Integer(i), DataType::Integer) => Value::Integer(i),
        (Value::Float(f), DataType::Integer) => Value::Integer(f.into_inner() as i64),
        (Value::Text(s), DataType::Integer) => {
            s.parse::<i64>().map(Value::Integer).unwrap_or(Value::Null)
        }
        (Value::Boolean(b), DataType::Integer) => Value::Integer(if b { 1 } else { 0 }),

        // Cast to Float
        (Value::Integer(i), DataType::Float) => Value::Float(ordered_float::OrderedFloat(i as f64)),
        (Value::Float(f), DataType::Float) => Value::Float(f),
        (Value::Text(s), DataType::Float) => s
            .parse::<f64>()
            .map(|f| Value::Float(ordered_float::OrderedFloat(f)))
            .unwrap_or(Value::Null),
        (Value::Boolean(b), DataType::Float) => {
            Value::Float(ordered_float::OrderedFloat(if b { 1.0 } else { 0.0 }))
        }

        // Cast to Text
        (Value::Integer(i), DataType::Text) => Value::Text(i.to_string()),
        (Value::Float(f), DataType::Text) => Value::Text(f.to_string()),
        (Value::Text(s), DataType::Text) => Value::Text(s),
        (Value::Boolean(b), DataType::Text) => Value::Text(if b {
            "true".to_string()
        } else {
            "false".to_string()
        }),
        (Value::Vector(v), DataType::Text) => {
            let formatted = v
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(",");
            Value::Text(format!("[{}]", formatted))
        }

        // Cast to Boolean
        (Value::Integer(i), DataType::Boolean) => Value::Boolean(i != 0),
        (Value::Float(f), DataType::Boolean) => Value::Boolean(f.into_inner() != 0.0),
        (Value::Text(s), DataType::Boolean) => {
            let lower = s.to_lowercase();
            Value::Boolean(lower == "true" || lower == "1" || lower == "yes" || lower == "on")
        }
        (Value::Boolean(b), DataType::Boolean) => Value::Boolean(b),

        // Cast to Vector
        (Value::Text(s), DataType::Vector(_)) => {
            // Parse "[0.1, 0.2, 0.3]" format
            let trimmed = s.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let inner = &trimmed[1..trimmed.len() - 1];
                let result: Result<Vec<f32>, _> =
                    inner.split(',').map(|s| s.trim().parse::<f32>()).collect();
                match result {
                    Ok(v) => Value::Vector(v),
                    Err(_) => Value::Null,
                }
            } else {
                Value::Null
            }
        }
        (Value::Vector(v), DataType::Vector(_)) => Value::Vector(v),

        // Unsupported casts return NULL
        _ => Value::Null,
    }
}

/// Evaluate binary operations
fn evaluate_binary_op(left: &Value, op: &BinaryOperator, right: &Value) -> Value {
    match op {
        BinaryOperator::Eq => Value::Boolean(values_equal(left, right)),
        BinaryOperator::NotEq => Value::Boolean(!values_equal(left, right)),
        BinaryOperator::Gt => {
            Value::Boolean(compare_values(left, right) == Some(std::cmp::Ordering::Greater))
        }
        BinaryOperator::GtEq => {
            let cmp = compare_values(left, right);
            Value::Boolean(
                cmp == Some(std::cmp::Ordering::Greater) || cmp == Some(std::cmp::Ordering::Equal),
            )
        }
        BinaryOperator::Lt => {
            Value::Boolean(compare_values(left, right) == Some(std::cmp::Ordering::Less))
        }
        BinaryOperator::LtEq => {
            let cmp = compare_values(left, right);
            Value::Boolean(
                cmp == Some(std::cmp::Ordering::Less) || cmp == Some(std::cmp::Ordering::Equal),
            )
        }
        BinaryOperator::And => {
            let left_bool = to_bool(left);
            let right_bool = to_bool(right);
            Value::Boolean(left_bool && right_bool)
        }
        BinaryOperator::Or => {
            let left_bool = to_bool(left);
            let right_bool = to_bool(right);
            Value::Boolean(left_bool || right_bool)
        }
        BinaryOperator::Like => {
            // Simple LIKE implementation
            if let (Value::Text(text), Value::Text(pattern)) = (left, right) {
                let pattern_simple = pattern.replace('%', ".*").replace('_', ".");
                let re = regex::Regex::new(&pattern_simple).ok();
                Value::Boolean(re.map(|r| r.is_match(text)).unwrap_or(false))
            } else {
                Value::Boolean(false)
            }
        }
        BinaryOperator::Matches => {
            // NexusSQL custom MATCHES operator
            if let (Value::Text(text), Value::Text(pattern)) = (left, right) {
                Value::Boolean(text.contains(pattern.as_str()))
            } else {
                Value::Boolean(false)
            }
        }

        // Vector distance operators (pgvector-compatible)
        BinaryOperator::L2Distance => {
            if let Some(distance) = left.l2_distance(right) {
                Value::Float(ordered_float::OrderedFloat(distance as f64))
            } else {
                Value::Null
            }
        }
        BinaryOperator::CosineDistance => {
            if let Some(distance) = left.cosine_distance(right) {
                Value::Float(ordered_float::OrderedFloat(distance as f64))
            } else {
                Value::Null
            }
        }
        BinaryOperator::InnerProduct => {
            if let Some(distance) = left.inner_product(right) {
                Value::Float(ordered_float::OrderedFloat(distance as f64))
            } else {
                Value::Null
            }
        }
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Integer(a), Value::Integer(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Text(a), Value::Text(b)) => a == b,
        (Value::Geometry(a), Value::Geometry(b)) => a == b,
        (Value::Boolean(a), Value::Boolean(b)) => a == b,
        (Value::Null, Value::Null) => true,
        // Cross-type comparisons for SQL compatibility
        (Value::Integer(i), Value::Text(s)) | (Value::Text(s), Value::Integer(i)) => {
            // Try to parse the text as an integer for comparison
            s.parse::<i64>().map(|parsed| parsed == *i).unwrap_or(false)
        }
        (Value::Float(f), Value::Text(s)) | (Value::Text(s), Value::Float(f)) => {
            // Try to parse the text as a float for comparison
            s.parse::<f64>()
                .map(|parsed| ordered_float::OrderedFloat(parsed) == *f)
                .unwrap_or(false)
        }
        (Value::Integer(i), Value::Float(f)) | (Value::Float(f), Value::Integer(i)) => {
            // Compare integer to float
            ordered_float::OrderedFloat(*i as f64) == *f
        }
        _ => false,
    }
}

fn compare_values(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Integer(a), Value::Integer(b)) => Some(a.cmp(b)),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
        (Value::Text(a), Value::Text(b)) => Some(a.cmp(b)),
        // Cross-type comparisons for SQL compatibility
        (Value::Integer(i), Value::Text(s)) => s
            .parse::<i64>()
            .ok()
            .and_then(|parsed| Some(i.cmp(&parsed))),
        (Value::Text(s), Value::Integer(i)) => {
            s.parse::<i64>().ok().and_then(|parsed| Some(parsed.cmp(i)))
        }
        (Value::Integer(i), Value::Float(f)) => {
            ordered_float::OrderedFloat(*i as f64).partial_cmp(f)
        }
        (Value::Float(f), Value::Integer(i)) => {
            f.partial_cmp(&ordered_float::OrderedFloat(*i as f64))
        }
        _ => None,
    }
}

fn to_bool(val: &Value) -> bool {
    match val {
        Value::Boolean(b) => *b,
        Value::Integer(i) => *i != 0,
        Value::Float(f) => *f != 0.0,
        Value::Text(s) => !s.is_empty(),
        Value::Null => false,
        _ => false,
    }
}

/// Evaluate function calls (built-in functions)
fn evaluate_function(row: &Row, name: &str, args: &[Expr], params: &QueryParams) -> Value {
    let name_upper = name.to_uppercase();
    if name_upper.starts_with("ST_") {
        let values: Vec<_> = args
            .iter()
            .map(|arg| evaluate_expr(row, arg, params))
            .collect();
        return crate::geometry::evaluate(&name_upper, &values);
    }

    match name_upper.as_str() {
        "COLBERT_SCORE" if matches!(args.len(), 2 | 3) => {
            if args.len() == 3
                && !evaluate_expr(row, &args[2], params)
                    .as_str()
                    .is_some_and(|mode| mode.eq_ignore_ascii_case("lightweight"))
            {
                return Value::Null;
            }
            let query = evaluate_expr(row, &args[0], params);
            let candidate = evaluate_expr(row, &args[1], params);
            match (query.as_str(), candidate.as_str()) {
                (Some(query), Some(candidate)) => {
                    crate::engine::colbert::lightweight::score(query, candidate)
                        .map(Value::from)
                        .unwrap_or(Value::Null)
                }
                _ => Value::Null,
            }
        }
        // Vector distance functions (pgvector-compatible)
        "VECTOR_DISTANCE" | "L2_DISTANCE" => {
            if args.len() != 2 {
                return Value::Null;
            }
            let arg1 = evaluate_expr(row, &args[0], params);
            let arg2 = evaluate_expr(row, &args[1], params);

            if let Some(distance) = arg1.l2_distance(&arg2) {
                Value::Float(ordered_float::OrderedFloat(distance as f64))
            } else {
                Value::Null
            }
        }

        "COSINE_DISTANCE" => {
            if args.len() != 2 {
                return Value::Null;
            }
            let arg1 = evaluate_expr(row, &args[0], params);
            let arg2 = evaluate_expr(row, &args[1], params);

            if let Some(distance) = arg1.cosine_distance(&arg2) {
                Value::Float(ordered_float::OrderedFloat(distance as f64))
            } else {
                Value::Null
            }
        }

        "INNER_PRODUCT" => {
            if args.len() != 2 {
                return Value::Null;
            }
            let arg1 = evaluate_expr(row, &args[0], params);
            let arg2 = evaluate_expr(row, &args[1], params);

            if let Some(distance) = arg1.inner_product(&arg2) {
                Value::Float(ordered_float::OrderedFloat(distance as f64))
            } else {
                Value::Null
            }
        }

        // String functions
        "UPPER" => {
            if args.len() != 1 {
                return Value::Null;
            }
            let arg = evaluate_expr(row, &args[0], params);
            if let Value::Text(s) = arg {
                Value::Text(s.to_uppercase())
            } else {
                Value::Null
            }
        }

        "LOWER" => {
            if args.len() != 1 {
                return Value::Null;
            }
            let arg = evaluate_expr(row, &args[0], params);
            if let Value::Text(s) = arg {
                Value::Text(s.to_lowercase())
            } else {
                Value::Null
            }
        }

        "LENGTH" | "LEN" => {
            if args.len() != 1 {
                return Value::Null;
            }
            let arg = evaluate_expr(row, &args[0], params);
            match arg {
                Value::Text(s) => Value::Integer(s.len() as i64),
                Value::Vector(v) => Value::Integer(v.len() as i64),
                Value::Blob(b) => Value::Integer(b.len() as i64),
                _ => Value::Null,
            }
        }

        // Math functions
        "ABS" => {
            if args.len() != 1 {
                return Value::Null;
            }
            let arg = evaluate_expr(row, &args[0], params);
            match arg {
                Value::Integer(i) => Value::Integer(i.abs()),
                Value::Float(f) => Value::Float(ordered_float::OrderedFloat(f.abs())),
                _ => Value::Null,
            }
        }

        "ROUND" => {
            if args.len() != 1 {
                return Value::Null;
            }
            let arg = evaluate_expr(row, &args[0], params);
            if let Value::Float(f) = arg {
                Value::Integer(f.round() as i64)
            } else {
                Value::Null
            }
        }

        // Null handling functions
        "NULLIF" => {
            // NULLIF(x, y) returns NULL if x = y, otherwise returns x
            if args.len() != 2 {
                return Value::Null;
            }
            let arg1 = evaluate_expr(row, &args[0], params);
            let arg2 = evaluate_expr(row, &args[1], params);

            if arg1 == arg2 {
                Value::Null
            } else {
                arg1
            }
        }

        "COALESCE" => {
            // COALESCE(x1, x2, ...) returns the first non-NULL argument
            for arg in args {
                let value = evaluate_expr(row, arg, params);
                if value != Value::Null {
                    return value;
                }
            }
            Value::Null
        }

        "IFNULL" | "NVL" => {
            // IFNULL(x, y) / NVL(x, y) returns x if x is not NULL, otherwise y
            // This is equivalent to COALESCE(x, y)
            if args.len() != 2 {
                return Value::Null;
            }
            let arg1 = evaluate_expr(row, &args[0], params);
            if arg1 != Value::Null {
                arg1
            } else {
                evaluate_expr(row, &args[1], params)
            }
        }

        // --- HDC (Hyperdimensional Computing) functions ---
        //
        // Hypervectors are carried through SQL as Value::Blob holding the
        // variant-tagged byte format produced by Hypervector::to_bytes().
        // Callers can store them in any Blob-typed column.
        "HV_ATOM" => {
            // hv_atom(kind TEXT, key TEXT, dim INT, variant TEXT)
            //   -> Blob of a deterministic hypervector derived from
            //      a FNV-1a 64-bit hash of "kind::key".
            //
            // This is the missing piece for SQL-side record encoding:
            // every distinct categorical value needs a stable hypervector
            // that's the SAME across queries but DIFFERENT per (kind, key).
            // Using the byte-string as a seed is the natural shape — no
            // caller has to pre-compute numeric seeds.
            if args.len() != 4 {
                return Value::Null;
            }
            let kind = match evaluate_expr(row, &args[0], params) {
                Value::Text(s) => s,
                _ => return Value::Null,
            };
            let key = match evaluate_expr(row, &args[1], params) {
                Value::Text(s) => s,
                _ => return Value::Null,
            };
            let dim = match evaluate_expr(row, &args[2], params) {
                Value::Integer(n) if n > 0 => n as usize,
                _ => return Value::Null,
            };
            let variant = match evaluate_expr(row, &args[3], params) {
                Value::Text(s) => s.to_uppercase(),
                _ => return Value::Null,
            };
            // FNV-1a 64-bit over "kind::key". Deterministic, cheap, no
            // external crypto-hash dep. The RNG seeded from this value
            // already produces quasi-orthogonal vectors for distinct keys.
            let combined = format!("{kind}::{key}");
            let mut h: u64 = 0xcbf29ce484222325;
            for b in combined.as_bytes() {
                h ^= *b as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
            let hv = match variant.as_str() {
                "BSC" => Hypervector::Bsc(BscVec::random_seeded(dim, h)),
                "MAP" => Hypervector::Map(MapVec::random_seeded(dim, h)),
                _ => return Value::Null,
            };
            Value::Blob(hv.to_bytes())
        }

        "HV_RANDOM" => {
            // hv_random(dim INT, variant TEXT[, seed BIGINT])
            //   -> Blob encoding a fresh hypervector
            if args.len() < 2 || args.len() > 3 {
                return Value::Null;
            }
            let dim = match evaluate_expr(row, &args[0], params) {
                Value::Integer(n) if n > 0 => n as usize,
                _ => return Value::Null,
            };
            let variant = match evaluate_expr(row, &args[1], params) {
                Value::Text(s) => s.to_uppercase(),
                _ => return Value::Null,
            };
            // Seed defaults to dim so callers writing hv_random(D, 'MAP')
            // get a deterministic-but-arbitrary result; explicit seed
            // overrides for reproducibility.
            let seed = if args.len() == 3 {
                match evaluate_expr(row, &args[2], params) {
                    Value::Integer(n) => n as u64,
                    Value::Null => dim as u64,
                    _ => return Value::Null,
                }
            } else {
                dim as u64
            };
            let hv = match variant.as_str() {
                "BSC" => Hypervector::Bsc(BscVec::random_seeded(dim, seed)),
                "MAP" => Hypervector::Map(MapVec::random_seeded(dim, seed)),
                _ => return Value::Null, // unknown variant
            };
            Value::Blob(hv.to_bytes())
        }

        "HV_BIND" => {
            // hv_bind(a BLOB, b BLOB) -> Blob of A ⊗ B
            if args.len() != 2 {
                return Value::Null;
            }
            let a = blob_as_hv(&evaluate_expr(row, &args[0], params));
            let b = blob_as_hv(&evaluate_expr(row, &args[1], params));
            match (a, b) {
                (Some(a), Some(b)) => match a.bind(&b) {
                    Ok(c) => Value::Blob(c.to_bytes()),
                    Err(_) => Value::Null,
                },
                _ => Value::Null,
            }
        }

        "HV_BUNDLE" => {
            // hv_bundle(a1, a2, ...) -> Blob of bundle (scalar form).
            // Aggregate form (hv_bundle over a grouping) lands in PR 6
            // together with SQL-side aggregation wiring.
            if args.is_empty() {
                return Value::Null;
            }
            let hvs: Vec<Hypervector> = args
                .iter()
                .filter_map(|arg| blob_as_hv(&evaluate_expr(row, arg, params)))
                .collect();
            if hvs.len() != args.len() {
                return Value::Null; // one of the inputs didn't decode
            }
            let refs: Vec<&Hypervector> = hvs.iter().collect();
            match Hypervector::bundle(&refs) {
                Ok(c) => Value::Blob(c.to_bytes()),
                Err(_) => Value::Null,
            }
        }

        "HV_PERMUTE" => {
            // hv_permute(a BLOB, shift INT) -> Blob of ρ(A, shift)
            if args.len() != 2 {
                return Value::Null;
            }
            let a = blob_as_hv(&evaluate_expr(row, &args[0], params));
            let shift = match evaluate_expr(row, &args[1], params) {
                Value::Integer(n) => n as isize,
                _ => return Value::Null,
            };
            a.map(|hv| Value::Blob(hv.permute(shift).to_bytes()))
                .unwrap_or(Value::Null)
        }

        "HV_SIMILARITY" => {
            // hv_similarity(a BLOB, b BLOB) -> REAL in [-1, +1]
            if args.len() != 2 {
                return Value::Null;
            }
            let a = blob_as_hv(&evaluate_expr(row, &args[0], params));
            let b = blob_as_hv(&evaluate_expr(row, &args[1], params));
            match (a, b) {
                (Some(a), Some(b)) => match a.similarity(&b) {
                    Ok(s) => Value::Float(ordered_float::OrderedFloat(s as f64)),
                    Err(_) => Value::Null,
                },
                _ => Value::Null,
            }
        }

        "HV_VARIANT" => {
            // hv_variant(a BLOB) -> TEXT — "BSC" | "MAP" | ...
            if args.len() != 1 {
                return Value::Null;
            }
            blob_as_hv(&evaluate_expr(row, &args[0], params))
                .map(|hv| Value::Text(hv.variant_name().to_string()))
                .unwrap_or(Value::Null)
        }

        "HV_DIM" => {
            // hv_dim(a BLOB) -> INTEGER — conceptual dimension
            if args.len() != 1 {
                return Value::Null;
            }
            blob_as_hv(&evaluate_expr(row, &args[0], params))
                .map(|hv| Value::Integer(hv.dim() as i64))
                .unwrap_or(Value::Null)
        }

        // Unknown function
        _ => Value::Null,
    }
}

/// Decode a `Value::Blob` as a `Hypervector`. Returns `None` for any
/// other `Value` kind or if the bytes don't parse (corrupt / wrong
/// variant tag). Centralises the conversion so every hv_* function
/// handles NULL and type-mismatch identically.
fn blob_as_hv(v: &Value) -> Option<Hypervector> {
    match v {
        Value::Blob(b) => Hypervector::from_bytes(b).ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- HDC SQL function surface ----
    //
    // These tests exercise hv_* via the `evaluate_expr` entry point —
    // the same path the executor takes. No schema / table needed; each
    // test builds an Expr::Function AST literal and calls the evaluator
    // directly. If these pass, the executor will too.

    fn hv_call(name: &str, args: Vec<Expr>) -> Expr {
        Expr::Function {
            name: name.to_string(),
            args,
        }
    }

    fn lit(v: Value) -> Expr {
        Expr::Literal(v)
    }

    fn empty_row() -> Row {
        Row::new(vec![], vec![])
    }

    #[test]
    fn hv_random_bsc_returns_blob_with_correct_dim() {
        let row = empty_row();
        let params = QueryParams::default();
        let expr = hv_call(
            "HV_RANDOM",
            vec![
                lit(Value::Integer(10_000)),
                lit(Value::Text("BSC".into())),
                lit(Value::Integer(42)),
            ],
        );
        let result = evaluate_expr(&row, &expr, &params);
        let Value::Blob(bytes) = &result else {
            panic!("expected Blob, got {:?}", result);
        };
        // Round-trip: decode + verify dim
        let dim_expr = hv_call("HV_DIM", vec![lit(Value::Blob(bytes.clone()))]);
        assert_eq!(
            evaluate_expr(&row, &dim_expr, &params),
            Value::Integer(10_000)
        );
        let variant_expr = hv_call("HV_VARIANT", vec![lit(Value::Blob(bytes.clone()))]);
        assert_eq!(
            evaluate_expr(&row, &variant_expr, &params),
            Value::Text("BSC".into())
        );
    }

    #[test]
    fn hv_random_map_matches_explicit_variant() {
        let row = empty_row();
        let params = QueryParams::default();
        let expr = hv_call(
            "HV_RANDOM",
            vec![
                lit(Value::Integer(1000)),
                lit(Value::Text("MAP".into())),
                lit(Value::Integer(7)),
            ],
        );
        let Value::Blob(bytes) = evaluate_expr(&row, &expr, &params) else {
            panic!("expected Blob");
        };
        assert_eq!(
            evaluate_expr(
                &row,
                &hv_call("HV_VARIANT", vec![lit(Value::Blob(bytes))]),
                &params
            ),
            Value::Text("MAP".into())
        );
    }

    #[test]
    fn hv_random_unknown_variant_returns_null() {
        let row = empty_row();
        let params = QueryParams::default();
        let expr = hv_call(
            "HV_RANDOM",
            vec![
                lit(Value::Integer(1000)),
                lit(Value::Text("QUANTUM".into())), // not a real variant
                lit(Value::Integer(1)),
            ],
        );
        assert_eq!(evaluate_expr(&row, &expr, &params), Value::Null);
    }

    #[test]
    fn hv_similarity_of_same_blob_is_one() {
        let row = empty_row();
        let params = QueryParams::default();
        let Value::Blob(bytes) = evaluate_expr(
            &row,
            &hv_call(
                "HV_RANDOM",
                vec![
                    lit(Value::Integer(10_000)),
                    lit(Value::Text("MAP".into())),
                    lit(Value::Integer(99)),
                ],
            ),
            &params,
        ) else {
            panic!("expected Blob");
        };
        let sim = evaluate_expr(
            &row,
            &hv_call(
                "HV_SIMILARITY",
                vec![lit(Value::Blob(bytes.clone())), lit(Value::Blob(bytes))],
            ),
            &params,
        );
        assert_eq!(sim, Value::Float(ordered_float::OrderedFloat(1.0)));
    }

    #[test]
    fn hv_bind_is_self_inverse_through_sql() {
        let row = empty_row();
        let params = QueryParams::default();
        let mk = |seed: i64| -> Vec<u8> {
            match evaluate_expr(
                &row,
                &hv_call(
                    "HV_RANDOM",
                    vec![
                        lit(Value::Integer(10_000)),
                        lit(Value::Text("MAP".into())),
                        lit(Value::Integer(seed)),
                    ],
                ),
                &params,
            ) {
                Value::Blob(b) => b,
                _ => panic!("expected Blob"),
            }
        };
        let a = mk(11);
        let b = mk(22);
        let bound = match evaluate_expr(
            &row,
            &hv_call(
                "HV_BIND",
                vec![lit(Value::Blob(a.clone())), lit(Value::Blob(b.clone()))],
            ),
            &params,
        ) {
            Value::Blob(v) => v,
            _ => panic!("expected Blob"),
        };
        // (A ⊗ B) ⊗ B == A — verified via hv_similarity == 1.0
        let recovered = match evaluate_expr(
            &row,
            &hv_call(
                "HV_BIND",
                vec![lit(Value::Blob(bound)), lit(Value::Blob(b))],
            ),
            &params,
        ) {
            Value::Blob(v) => v,
            _ => panic!("expected Blob"),
        };
        let sim = evaluate_expr(
            &row,
            &hv_call(
                "HV_SIMILARITY",
                vec![lit(Value::Blob(a)), lit(Value::Blob(recovered))],
            ),
            &params,
        );
        assert_eq!(sim, Value::Float(ordered_float::OrderedFloat(1.0)));
    }

    #[test]
    fn hv_similarity_across_variants_returns_null() {
        let row = empty_row();
        let params = QueryParams::default();
        let bsc = match evaluate_expr(
            &row,
            &hv_call(
                "HV_RANDOM",
                vec![
                    lit(Value::Integer(1000)),
                    lit(Value::Text("BSC".into())),
                    lit(Value::Integer(1)),
                ],
            ),
            &params,
        ) {
            Value::Blob(b) => b,
            _ => panic!(),
        };
        let map = match evaluate_expr(
            &row,
            &hv_call(
                "HV_RANDOM",
                vec![
                    lit(Value::Integer(1000)),
                    lit(Value::Text("MAP".into())),
                    lit(Value::Integer(1)),
                ],
            ),
            &params,
        ) {
            Value::Blob(b) => b,
            _ => panic!(),
        };
        assert_eq!(
            evaluate_expr(
                &row,
                &hv_call(
                    "HV_SIMILARITY",
                    vec![lit(Value::Blob(bsc)), lit(Value::Blob(map))]
                ),
                &params,
            ),
            Value::Null
        );
    }

    #[test]
    fn hv_permute_round_trip() {
        let row = empty_row();
        let params = QueryParams::default();
        let a = match evaluate_expr(
            &row,
            &hv_call(
                "HV_RANDOM",
                vec![
                    lit(Value::Integer(10_000)),
                    lit(Value::Text("MAP".into())),
                    lit(Value::Integer(100)),
                ],
            ),
            &params,
        ) {
            Value::Blob(b) => b,
            _ => panic!(),
        };
        // ρ(ρ(A, 123), -123) == A
        let forward = match evaluate_expr(
            &row,
            &hv_call(
                "HV_PERMUTE",
                vec![lit(Value::Blob(a.clone())), lit(Value::Integer(123))],
            ),
            &params,
        ) {
            Value::Blob(b) => b,
            _ => panic!(),
        };
        let back = match evaluate_expr(
            &row,
            &hv_call(
                "HV_PERMUTE",
                vec![lit(Value::Blob(forward)), lit(Value::Integer(-123))],
            ),
            &params,
        ) {
            Value::Blob(b) => b,
            _ => panic!(),
        };
        let sim = evaluate_expr(
            &row,
            &hv_call(
                "HV_SIMILARITY",
                vec![lit(Value::Blob(a)), lit(Value::Blob(back))],
            ),
            &params,
        );
        assert_eq!(sim, Value::Float(ordered_float::OrderedFloat(1.0)));
    }

    #[test]
    fn hv_bundle_scalar_form_is_similar_to_each_input() {
        let row = empty_row();
        let params = QueryParams::default();
        let mk = |seed: i64| -> Vec<u8> {
            match evaluate_expr(
                &row,
                &hv_call(
                    "HV_RANDOM",
                    vec![
                        lit(Value::Integer(10_000)),
                        lit(Value::Text("MAP".into())),
                        lit(Value::Integer(seed)),
                    ],
                ),
                &params,
            ) {
                Value::Blob(b) => b,
                _ => panic!(),
            }
        };
        let a = mk(1);
        let b = mk(2);
        let c = mk(3);
        let bundle = match evaluate_expr(
            &row,
            &hv_call(
                "HV_BUNDLE",
                vec![
                    lit(Value::Blob(a.clone())),
                    lit(Value::Blob(b.clone())),
                    lit(Value::Blob(c.clone())),
                ],
            ),
            &params,
        ) {
            Value::Blob(v) => v,
            _ => panic!(),
        };
        for seed_bytes in [a, b, c] {
            let Value::Float(sim) = evaluate_expr(
                &row,
                &hv_call(
                    "HV_SIMILARITY",
                    vec![
                        lit(Value::Blob(bundle.clone())),
                        lit(Value::Blob(seed_bytes)),
                    ],
                ),
                &params,
            ) else {
                panic!("expected Float");
            };
            // Each component should remain recognizable in the bundle
            // (theoretical ≈ 1/√3 ≈ 0.577 for 3-way MAP bundle).
            assert!(
                sim.into_inner() > 0.3,
                "bundle component similarity too low: {}",
                sim.into_inner()
            );
        }
    }

    #[test]
    fn hv_atom_is_deterministic_and_quasi_orthogonal() {
        let row = empty_row();
        let params = QueryParams::default();
        let mk = |kind: &str, key: &str| -> Vec<u8> {
            match evaluate_expr(
                &row,
                &hv_call(
                    "HV_ATOM",
                    vec![
                        lit(Value::Text(kind.into())),
                        lit(Value::Text(key.into())),
                        lit(Value::Integer(10_000)),
                        lit(Value::Text("MAP".into())),
                    ],
                ),
                &params,
            ) {
                Value::Blob(b) => b,
                _ => panic!("expected Blob"),
            }
        };

        // Determinism: same (kind, key) -> same bytes
        assert_eq!(mk("role", "status"), mk("role", "status"));

        // Different keys under the same kind are quasi-orthogonal
        let a = mk("role", "status");
        let b = mk("role", "reason");
        let sim = evaluate_expr(
            &row,
            &hv_call(
                "HV_SIMILARITY",
                vec![lit(Value::Blob(a)), lit(Value::Blob(b))],
            ),
            &params,
        );
        let Value::Float(s) = sim else {
            panic!("expected Float");
        };
        assert!(
            s.into_inner().abs() < 0.05,
            "different atoms should be quasi-orthogonal: sim={}",
            s.into_inner()
        );

        // Different kinds under the same key are also distinct
        let c = mk("val", "status");
        let sim2 = evaluate_expr(
            &row,
            &hv_call(
                "HV_SIMILARITY",
                vec![lit(Value::Blob(mk("role", "status"))), lit(Value::Blob(c))],
            ),
            &params,
        );
        let Value::Float(s2) = sim2 else { panic!() };
        assert!(s2.into_inner().abs() < 0.05);
    }

    #[test]
    fn hv_functions_return_null_on_non_blob_input() {
        let row = empty_row();
        let params = QueryParams::default();
        let bogus = lit(Value::Text("not a hypervector".into()));
        for name in ["HV_BIND", "HV_SIMILARITY"] {
            assert_eq!(
                evaluate_expr(
                    &row,
                    &hv_call(name, vec![bogus.clone(), bogus.clone()]),
                    &params,
                ),
                Value::Null,
                "{name} should return NULL on text input"
            );
        }
        for name in ["HV_VARIANT", "HV_DIM"] {
            assert_eq!(
                evaluate_expr(&row, &hv_call(name, vec![bogus.clone()]), &params),
                Value::Null,
                "{name} should return NULL on text input"
            );
        }
    }

    // ---- Original evaluator tests continue ----

    #[test]
    fn test_evaluate_eq() {
        let row = Row::new(vec!["age".to_string()], vec![Value::Integer(25)]);

        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Column("age".to_string())),
            op: BinaryOperator::Eq,
            right: Box::new(Expr::Literal(Value::Integer(25))),
        };

        let params = QueryParams::default();
        assert!(evaluate_where(&row, &expr, &params));
    }

    #[test]
    fn test_evaluate_gt() {
        let row = Row::new(vec!["age".to_string()], vec![Value::Integer(30)]);

        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Column("age".to_string())),
            op: BinaryOperator::Gt,
            right: Box::new(Expr::Literal(Value::Integer(18))),
        };

        let params = QueryParams::default();
        assert!(evaluate_where(&row, &expr, &params));
    }

    #[test]
    fn test_cast_text_to_integer() {
        use crate::query::ast::DataType;

        let row = Row::new(vec!["val".to_string()], vec![Value::Text("42".to_string())]);
        let expr = Expr::Cast {
            expr: Box::new(Expr::Column("val".to_string())),
            data_type: DataType::Integer,
        };
        let params = QueryParams::default();
        let result = evaluate_expr(&row, &expr, &params);
        assert_eq!(result, Value::Integer(42));
    }

    #[test]
    fn test_cast_integer_to_text() {
        use crate::query::ast::DataType;

        let row = Row::new(vec!["val".to_string()], vec![Value::Integer(123)]);
        let expr = Expr::Cast {
            expr: Box::new(Expr::Column("val".to_string())),
            data_type: DataType::Text,
        };
        let params = QueryParams::default();
        let result = evaluate_expr(&row, &expr, &params);
        assert_eq!(result, Value::Text("123".to_string()));
    }

    #[test]
    fn test_cast_float_to_integer() {
        use crate::query::ast::DataType;

        let row = Row::new(
            vec!["val".to_string()],
            vec![Value::Float(ordered_float::OrderedFloat(3.7))],
        );
        let expr = Expr::Cast {
            expr: Box::new(Expr::Column("val".to_string())),
            data_type: DataType::Integer,
        };
        let params = QueryParams::default();
        let result = evaluate_expr(&row, &expr, &params);
        assert_eq!(result, Value::Integer(3));
    }

    #[test]
    fn test_cast_bool_to_integer() {
        use crate::query::ast::DataType;

        let row = Row::new(vec!["val".to_string()], vec![Value::Boolean(true)]);
        let expr = Expr::Cast {
            expr: Box::new(Expr::Column("val".to_string())),
            data_type: DataType::Integer,
        };
        let params = QueryParams::default();
        let result = evaluate_expr(&row, &expr, &params);
        assert_eq!(result, Value::Integer(1));
    }

    #[test]
    fn test_cast_null_remains_null() {
        use crate::query::ast::DataType;

        let row = Row::new(vec!["val".to_string()], vec![Value::Null]);
        let expr = Expr::Cast {
            expr: Box::new(Expr::Column("val".to_string())),
            data_type: DataType::Integer,
        };
        let params = QueryParams::default();
        let result = evaluate_expr(&row, &expr, &params);
        assert_eq!(result, Value::Null);
    }
}
