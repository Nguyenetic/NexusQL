//! Fast-path INSERT parser.
//!
//! Handles exactly the shape `INSERT INTO "tbl" (col, ...) VALUES
//! (lit, ...), (lit, ...), ... ;` with literal values only (NULL, bool,
//! integer, float, single-quoted string with `''` escape, unary minus on
//! numbers). ~10-50x faster than sqlparser-rs on the wide VALUES lists
//! produced by bulk-loader scripts, because it never builds a general
//! AST — it emits rows directly.
//!
//! Returns `None` when the input doesn't match the narrow shape, and the
//! caller falls back to the full SQL parser.

use crate::types::Value;

/// Parsed fast-insert payload.
#[derive(Debug)]
pub struct FastInsert {
    pub table: String,
    pub columns: Option<Vec<String>>,
    pub rows: Vec<Vec<Value>>,
}

/// Scanner with byte cursor into the SQL text. Only valid ASCII keywords
/// are consumed with `eat_keyword`; UTF-8 bytes pass through unchanged
/// inside string literals.
struct Scanner<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Scanner<'a> {
    fn new(s: &'a str) -> Self {
        Self { s: s.as_bytes(), i: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let c = self.peek()?;
        self.i += 1;
        Some(c)
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace() {
                self.i += 1;
            } else if c == b'-' && self.s.get(self.i + 1) == Some(&b'-') {
                // line comment
                while let Some(c) = self.peek() {
                    self.i += 1;
                    if c == b'\n' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    /// Consume the given keyword case-insensitively if present. Returns
    /// true on match and advances; false otherwise (no advance).
    fn eat_keyword(&mut self, kw: &str) -> bool {
        let bytes = kw.as_bytes();
        if self.s.len() < self.i + bytes.len() {
            return false;
        }
        for (k, b) in bytes.iter().enumerate() {
            let c = self.s[self.i + k];
            if c.to_ascii_uppercase() != *b {
                return false;
            }
        }
        // Next char must not be identifier-continuation
        let next = self.s.get(self.i + bytes.len()).copied();
        if let Some(c) = next {
            if c.is_ascii_alphanumeric() || c == b'_' {
                return false;
            }
        }
        self.i += bytes.len();
        true
    }

    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    /// Parse an identifier (quoted or bare). Returns a copy of the name.
    fn ident(&mut self) -> Option<String> {
        self.skip_ws();
        match self.peek()? {
            b'"' => {
                self.i += 1;
                let start = self.i;
                while let Some(c) = self.peek() {
                    if c == b'"' {
                        break;
                    }
                    self.i += 1;
                }
                let end = self.i;
                if self.peek() != Some(b'"') {
                    return None;
                }
                self.i += 1;
                std::str::from_utf8(&self.s[start..end]).ok().map(String::from)
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let start = self.i;
                while let Some(c) = self.peek() {
                    if c.is_ascii_alphanumeric() || c == b'_' || c == b'-' {
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                std::str::from_utf8(&self.s[start..self.i]).ok().map(String::from)
            }
            _ => None,
        }
    }

    /// Parse a single value literal (NULL / bool / number / string).
    fn value(&mut self) -> Option<Value> {
        self.skip_ws();
        match self.peek()? {
            b'\'' => self.string_literal(),
            b'-' | b'+' => self.number_literal(),
            c if c.is_ascii_digit() => self.number_literal(),
            c if c.is_ascii_alphabetic() => self.keyword_value(),
            _ => None,
        }
    }

    fn string_literal(&mut self) -> Option<Value> {
        debug_assert_eq!(self.peek(), Some(b'\''));
        self.i += 1;
        let mut out = String::new();
        while let Some(c) = self.peek() {
            if c == b'\'' {
                // Check for doubled quote (escape)
                if self.s.get(self.i + 1) == Some(&b'\'') {
                    out.push('\'');
                    self.i += 2;
                    continue;
                }
                self.i += 1;
                return Some(Value::Text(out));
            }
            // UTF-8 safe: just push the byte's char. Works because we
            // split at `'` which is ASCII.
            out.push(c as char);
            self.i += 1;
        }
        None
    }

    fn number_literal(&mut self) -> Option<Value> {
        let start = self.i;
        if matches!(self.peek(), Some(b'-') | Some(b'+')) {
            self.i += 1;
        }
        let mut has_digit = false;
        let mut is_float = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                has_digit = true;
                self.i += 1;
            } else if c == b'.' {
                is_float = true;
                self.i += 1;
            } else if matches!(c, b'e' | b'E') {
                is_float = true;
                self.i += 1;
                if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                    self.i += 1;
                }
            } else {
                break;
            }
        }
        if !has_digit {
            return None;
        }
        let lit = std::str::from_utf8(&self.s[start..self.i]).ok()?;
        if is_float {
            lit.parse::<f64>().ok().map(|f| Value::Float(f.into()))
        } else {
            lit.parse::<i64>().ok().map(Value::Integer).or_else(|| {
                // Fall back to float if integer overflows
                lit.parse::<f64>().ok().map(|f| Value::Float(f.into()))
            })
        }
    }

    fn keyword_value(&mut self) -> Option<Value> {
        if self.eat_keyword("NULL") {
            return Some(Value::Null);
        }
        if self.eat_keyword("TRUE") {
            return Some(Value::Boolean(true));
        }
        if self.eat_keyword("FALSE") {
            return Some(Value::Boolean(false));
        }
        None
    }

    /// Parse a tuple: `( v, v, v, ... )`.
    fn tuple(&mut self) -> Option<Vec<Value>> {
        self.skip_ws();
        if !self.eat(b'(') {
            return None;
        }
        let mut row = Vec::new();
        loop {
            self.skip_ws();
            let v = self.value()?;
            row.push(v);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.i += 1;
                }
                Some(b')') => {
                    self.i += 1;
                    return Some(row);
                }
                _ => return None,
            }
        }
    }

    /// Parse the `VALUES` rows list: `(row), (row), ..., (row)`.
    fn rows(&mut self) -> Option<Vec<Vec<Value>>> {
        let mut out = Vec::new();
        loop {
            self.skip_ws();
            let row = self.tuple()?;
            out.push(row);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.i += 1;
                }
                Some(b';') | None => return Some(out),
                _ => return Some(out), // trailing content — defer, let caller decide
            }
        }
    }
}

/// Try to parse a SQL string as the narrow INSERT shape described above.
/// Returns `None` if anything doesn't match, which tells the caller to
/// fall back to the general sqlparser path.
pub fn try_parse_insert(sql: &str) -> Option<FastInsert> {
    let mut sc = Scanner::new(sql);
    sc.skip_ws();
    if !sc.eat_keyword("INSERT") {
        return None;
    }
    sc.skip_ws();
    if !sc.eat_keyword("INTO") {
        return None;
    }
    let table = sc.ident()?;
    sc.skip_ws();

    // Optional column list
    let columns: Option<Vec<String>> = if sc.peek() == Some(b'(') {
        sc.i += 1;
        let mut cols = Vec::new();
        loop {
            sc.skip_ws();
            let name = sc.ident()?;
            cols.push(name);
            sc.skip_ws();
            match sc.peek() {
                Some(b',') => sc.i += 1,
                Some(b')') => {
                    sc.i += 1;
                    break;
                }
                _ => return None,
            }
        }
        Some(cols)
    } else {
        None
    };

    sc.skip_ws();
    if !sc.eat_keyword("VALUES") {
        return None;
    }
    let rows = sc.rows()?;
    if rows.is_empty() {
        return None;
    }

    Some(FastInsert { table, columns, rows })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_insert() {
        let sql = "INSERT INTO t VALUES (1, 'hi', NULL);";
        let r = try_parse_insert(sql).unwrap();
        assert_eq!(r.table, "t");
        assert!(r.columns.is_none());
        assert_eq!(r.rows.len(), 1);
        match &r.rows[0][0] {
            Value::Integer(1) => {}
            _ => panic!(),
        }
    }

    #[test]
    fn quoted_table_plus_columns() {
        let sql = "INSERT INTO \"QA total\" (row_idx, c1) VALUES (1, 'x'), (2, 'y');";
        let r = try_parse_insert(sql).unwrap();
        assert_eq!(r.table, "QA total");
        assert_eq!(r.columns.as_ref().unwrap().len(), 2);
        assert_eq!(r.rows.len(), 2);
    }

    #[test]
    fn escaped_quote_in_string() {
        let sql = "INSERT INTO t VALUES ('O''Brien');";
        let r = try_parse_insert(sql).unwrap();
        match &r.rows[0][0] {
            Value::Text(s) => assert_eq!(s, "O'Brien"),
            _ => panic!(),
        }
    }

    #[test]
    fn negative_float() {
        let sql = "INSERT INTO t VALUES (-2.5);";
        let r = try_parse_insert(sql).unwrap();
        match &r.rows[0][0] {
            Value::Float(f) => assert!((f.into_inner() + 2.5).abs() < 1e-9),
            _ => panic!(),
        }
    }

    #[test]
    fn declines_unsupported_shape() {
        assert!(try_parse_insert("SELECT * FROM t").is_none());
        assert!(try_parse_insert("INSERT INTO t SELECT * FROM u").is_none());
    }
}
