//! Full-text query language, per-document matching, BM25 and snippets.
//!
//! Syntax (FTS5-like):
//! - `word` matches a term; adjacent terms are combined with implicit AND.
//! - `"exact phrase"` matches consecutive terms; `""` inside quotes is a quote.
//! - `prefix*` matches every term starting with `prefix` (also `"a phrase*"`).
//! - `AND`, `OR`, `NOT` (uppercase only, as in FTS5) and parentheses.
//!   Precedence: NOT > AND > OR. `a NOT b` means a AND NOT b. A leading `NOT x`
//!   matches every indexed text that lacks x.
//! - A bareword that the tokenizer splits (e.g. `e-mail` or `東京都`) is a phrase.

use super::tokenizer::{tokenize, Mode, Token};
use std::fmt;

/// One phrase slot: an exact term or a term prefix.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Element {
    pub term: String,
    pub prefix: bool,
}

/// Consecutive elements. A single-element phrase is a plain term.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Phrase {
    pub elements: Vec<Element>,
}

impl Phrase {
    /// Canonical key used for document-frequency statistics.
    pub fn key(&self) -> String {
        self.elements
            .iter()
            .map(|e| {
                if e.prefix {
                    format!("{}*", e.term)
                } else {
                    e.term.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Query {
    Phrase(Phrase),
    And(Vec<Query>),
    Or(Vec<Query>),
    Not(Box<Query>),
    /// A query with no searchable terms (e.g. only punctuation). Matches nothing.
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryError(pub String);

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid full-text query: {}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Lexeme {
    Open,
    Close,
    And,
    Or,
    Not,
    Phrase(Phrase),
}

fn phrase_from_text(text: &str, prefix: bool) -> Option<Phrase> {
    let tokens = tokenize(text, Mode::Query);
    if tokens.is_empty() {
        return None;
    }
    let last = tokens.len() - 1;
    let elements = tokens
        .into_iter()
        .enumerate()
        .map(|(i, t)| Element {
            prefix: t.prefix || (prefix && i == last),
            term: t.term,
        })
        .collect();
    Some(Phrase { elements })
}

fn lex(input: &str) -> Result<Vec<Lexeme>, QueryError> {
    let mut out = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '(' {
            chars.next();
            out.push(Lexeme::Open);
        } else if c == ')' {
            chars.next();
            out.push(Lexeme::Close);
        } else if c == '"' {
            chars.next();
            let mut text = String::new();
            let mut closed = false;
            while let Some(d) = chars.next() {
                if d == '"' {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        text.push('"');
                    } else {
                        closed = true;
                        break;
                    }
                } else {
                    text.push(d);
                }
            }
            if !closed {
                return Err(QueryError("unterminated quoted phrase".into()));
            }
            // Prefix marker after the quote ("lazy d"*) or on its last word ("lazy d*").
            let mut prefix = text.trim_end().ends_with('*');
            if chars.peek() == Some(&'*') {
                chars.next();
                prefix = true;
            }
            if let Some(phrase) = phrase_from_text(text.trim_end().trim_end_matches('*'), prefix) {
                out.push(Lexeme::Phrase(phrase));
            }
        } else {
            let mut word = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_whitespace() || d == '(' || d == ')' || d == '"' {
                    break;
                }
                word.push(d);
                chars.next();
            }
            match word.as_str() {
                "AND" => out.push(Lexeme::And),
                "OR" => out.push(Lexeme::Or),
                "NOT" => out.push(Lexeme::Not),
                _ => {
                    let prefix = word.ends_with('*');
                    if let Some(phrase) = phrase_from_text(word.trim_end_matches('*'), prefix) {
                        out.push(Lexeme::Phrase(phrase));
                    }
                }
            }
        }
    }
    Ok(out)
}

struct Parser {
    lexemes: Vec<Lexeme>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Lexeme> {
        self.lexemes.get(self.pos)
    }

    fn or_expr(&mut self) -> Result<Query, QueryError> {
        let mut parts = vec![self.and_expr()?];
        while self.peek() == Some(&Lexeme::Or) {
            self.pos += 1;
            parts.push(self.and_expr()?);
        }
        Ok(if parts.len() == 1 {
            parts.pop().unwrap()
        } else {
            Query::Or(parts)
        })
    }

    fn and_expr(&mut self) -> Result<Query, QueryError> {
        let mut parts = vec![self.not_expr()?];
        loop {
            match self.peek() {
                Some(Lexeme::And) => {
                    self.pos += 1;
                    parts.push(self.not_expr()?);
                }
                Some(Lexeme::Phrase(_)) | Some(Lexeme::Open) | Some(Lexeme::Not) => {
                    parts.push(self.not_expr()?);
                }
                _ => break,
            }
        }
        Ok(if parts.len() == 1 {
            parts.pop().unwrap()
        } else {
            Query::And(parts)
        })
    }

    fn not_expr(&mut self) -> Result<Query, QueryError> {
        let mut left = self.primary()?;
        while self.peek() == Some(&Lexeme::Not) {
            self.pos += 1;
            let right = self.primary()?;
            left = Query::And(vec![left, Query::Not(Box::new(right))]);
        }
        Ok(left)
    }

    fn primary(&mut self) -> Result<Query, QueryError> {
        match self.lexemes.get(self.pos).cloned() {
            Some(Lexeme::Phrase(phrase)) => {
                self.pos += 1;
                Ok(Query::Phrase(phrase))
            }
            Some(Lexeme::Open) => {
                self.pos += 1;
                let inner = self.or_expr()?;
                if self.peek() != Some(&Lexeme::Close) {
                    return Err(QueryError("missing closing parenthesis".into()));
                }
                self.pos += 1;
                Ok(inner)
            }
            Some(Lexeme::Not) => {
                self.pos += 1;
                Ok(Query::Not(Box::new(self.primary()?)))
            }
            Some(Lexeme::Close) => Err(QueryError("unexpected ')'".into())),
            Some(Lexeme::And) | Some(Lexeme::Or) => {
                Err(QueryError("operator is missing its left operand".into()))
            }
            None => Err(QueryError("operator is missing its right operand".into())),
        }
    }
}

impl Query {
    /// Parse a query string. Text without any searchable term yields `Empty`.
    pub fn parse(input: &str) -> Result<Query, QueryError> {
        let lexemes = lex(input)?;
        if lexemes.is_empty() {
            return Ok(Query::Empty);
        }
        let mut parser = Parser { lexemes, pos: 0 };
        let query = parser.or_expr()?;
        if parser.pos != parser.lexemes.len() {
            return Err(QueryError(match parser.peek() {
                Some(Lexeme::Close) => "unexpected ')'".into(),
                _ => "unexpected operator".into(),
            }));
        }
        Ok(query)
    }

    /// Phrases that contribute to ranking and highlighting (not under NOT).
    pub fn positive_phrases(&self) -> Vec<&Phrase> {
        let mut out = Vec::new();
        fn walk<'a>(q: &'a Query, out: &mut Vec<&'a Phrase>) {
            match q {
                Query::Phrase(p) => {
                    if !out.contains(&p) {
                        out.push(p)
                    }
                }
                Query::And(parts) | Query::Or(parts) => parts.iter().for_each(|q| walk(q, out)),
                Query::Not(_) | Query::Empty => {}
            }
        }
        walk(self, &mut out);
        out
    }

    /// Evaluate against document-mode tokens of one text.
    pub fn matches(&self, tokens: &[Token]) -> bool {
        match self {
            Query::Phrase(p) => !occurrences(tokens, p).is_empty(),
            Query::And(parts) => parts.iter().all(|q| q.matches(tokens)),
            Query::Or(parts) => parts.iter().any(|q| q.matches(tokens)),
            Query::Not(inner) => !inner.matches(tokens),
            Query::Empty => false,
        }
    }
}

fn element_matches(element: &Element, term: &str) -> bool {
    if element.prefix {
        term.starts_with(element.term.as_str())
    } else {
        term == element.term
    }
}

/// Token-index spans `(first, last)` of every occurrence of `phrase`.
pub fn occurrences(tokens: &[Token], phrase: &Phrase) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let Some((first, rest)) = phrase.elements.split_first() else {
        return found;
    };
    'start: for (i, token) in tokens.iter().enumerate() {
        if !element_matches(first, &token.term) {
            continue;
        }
        let mut last = i;
        for (k, element) in rest.iter().enumerate() {
            let wanted = token.position + k as u32 + 1;
            let next = tokens[last + 1..]
                .iter()
                .enumerate()
                .take_while(|(_, t)| t.position <= wanted)
                .find(|(_, t)| t.position == wanted && element_matches(element, &t.term));
            match next {
                Some((offset, _)) => last = last + 1 + offset,
                None => continue 'start,
            }
        }
        found.push((i, last));
    }
    found
}

/// BM25 parameters and corpus statistics for one (column, query) pair.
#[derive(Debug, Clone)]
pub struct CorpusStats {
    pub documents: f64,
    pub average_length: f64,
    /// Document frequency per `Phrase::key()`.
    pub document_frequency: std::collections::HashMap<String, f64>,
}

pub const DEFAULT_K1: f64 = 1.2;
pub const DEFAULT_B: f64 = 0.75;

/// Okapi BM25 with the non-negative idf `ln(1 + (N - df + 0.5) / (df + 0.5))`.
/// Returns None when the document does not match. Phrases count phrase
/// occurrences as term frequency; prefixes count every matching term.
pub fn bm25(
    tokens: &[Token],
    query: &Query,
    stats: Option<&CorpusStats>,
    k1: f64,
    b: f64,
) -> Option<f64> {
    if !query.matches(tokens) {
        return None;
    }
    let length = tokens.len() as f64;
    let (documents, average_length) = match stats {
        Some(s) if s.documents > 0.0 && s.average_length > 0.0 => (s.documents, s.average_length),
        _ => (1.0, length.max(1.0)),
    };
    let mut score = 0.0;
    for phrase in query.positive_phrases() {
        let tf = occurrences(tokens, phrase).len() as f64;
        if tf == 0.0 {
            continue;
        }
        let df = stats
            .and_then(|s| s.document_frequency.get(&phrase.key()).copied())
            .unwrap_or(1.0)
            .clamp(1.0, documents.max(1.0));
        let idf = (1.0 + (documents - df + 0.5) / (df + 0.5)).ln();
        score += idf * tf * (k1 + 1.0) / (tf + k1 * (1.0 - b + b * length / average_length));
    }
    Some(score)
}

/// Merged byte ranges of every positive phrase occurrence.
fn hit_ranges(tokens: &[Token], query: &Query) -> Vec<(usize, usize, usize)> {
    // (first token, last token, phrase index) sorted by first token
    let mut hits = Vec::new();
    for (index, phrase) in query.positive_phrases().into_iter().enumerate() {
        for (first, last) in occurrences(tokens, phrase) {
            hits.push((first, last, index));
        }
    }
    hits.sort_unstable();
    hits
}

fn merged_spans(
    tokens: &[Token],
    hits: &[(usize, usize, usize)],
    lo: usize,
    hi: usize,
) -> Vec<(usize, usize)> {
    // Byte spans for hits inside token window [lo, hi], merged when overlapping.
    let mut spans: Vec<(usize, usize)> = hits
        .iter()
        .filter(|(first, last, _)| *first >= lo && *last <= hi)
        .map(|(first, last, _)| (tokens[*first].start, tokens[*last].end))
        .collect();
    spans.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in spans {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

fn mark(
    text: &str,
    from: usize,
    to: usize,
    spans: &[(usize, usize)],
    open: &str,
    close: &str,
) -> String {
    let mut out = String::with_capacity(to - from + spans.len() * (open.len() + close.len()));
    let mut cursor = from;
    for &(start, end) in spans {
        out.push_str(&text[cursor..start]);
        out.push_str(open);
        out.push_str(&text[start..end]);
        out.push_str(close);
        cursor = end;
    }
    out.push_str(&text[cursor..to]);
    out
}

/// The whole text with every matching phrase wrapped in `open`/`close`.
pub fn highlight(text: &str, query: &Query, open: &str, close: &str) -> String {
    let tokens = tokenize(text, Mode::Document);
    if tokens.is_empty() {
        return text.to_string();
    }
    let hits = hit_ranges(&tokens, query);
    let spans = merged_spans(&tokens, &hits, 0, tokens.len() - 1);
    mark(text, 0, text.len(), &spans, open, close)
}

/// A window of at most `max_tokens` tokens around the best cluster of hits,
/// with hits marked and `ellipsis` where text was cut. The window covering the
/// most distinct query phrases wins, then the one with most hits, then the
/// earliest. Text without hits yields its leading tokens.
pub fn snippet(
    text: &str,
    query: &Query,
    open: &str,
    close: &str,
    ellipsis: &str,
    max_tokens: usize,
) -> String {
    let tokens = tokenize(text, Mode::Document);
    let max_tokens = max_tokens.max(1);
    if tokens.is_empty() {
        return text.to_string();
    }
    let hits = hit_ranges(&tokens, query);
    let n = tokens.len();
    let (lo, hi) = if n <= max_tokens {
        (0, n - 1)
    } else if hits.is_empty() {
        (0, max_tokens - 1)
    } else {
        let mut best: Option<((usize, usize), usize, usize)> = None; // ((phrases, hits), lo, last hit)
        for &(first, _, _) in &hits {
            let window_end = first + max_tokens - 1;
            let inside: Vec<_> = hits
                .iter()
                .filter(|(f, l, _)| *f >= first && *l <= window_end)
                .collect();
            let mut phrases: Vec<usize> = inside.iter().map(|h| h.2).collect();
            phrases.sort_unstable();
            phrases.dedup();
            let last_hit = inside.iter().map(|h| h.1).max().unwrap_or(first);
            let rank = (phrases.len(), inside.len());
            if best.as_ref().map_or(true, |(r, _, _)| rank > *r) {
                best = Some((rank, first, last_hit));
            }
        }
        let (_, first, last_hit) = best.unwrap();
        // Center the hit cluster in the window, without running past the end.
        let slack = max_tokens - (last_hit.min(first + max_tokens - 1) - first + 1);
        let lo = first.saturating_sub(slack / 2).min(n - max_tokens);
        (lo, lo + max_tokens - 1)
    };
    let from = if lo == 0 { 0 } else { tokens[lo].start };
    let to = if hi == n - 1 {
        text.len()
    } else {
        tokens[hi].end
    };
    let spans = merged_spans(&tokens, &hits, lo, hi);
    let mut out = String::new();
    if lo > 0 {
        out.push_str(ellipsis);
    }
    out.push_str(&mark(text, from, to, &spans, open, close));
    if hi < n - 1 {
        out.push_str(ellipsis);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> Vec<Token> {
        tokenize(text, Mode::Document)
    }

    fn m(query: &str, text: &str) -> bool {
        Query::parse(query).unwrap().matches(&doc(text))
    }

    #[test]
    fn parses_operators_with_fts5_precedence() {
        let q = Query::parse("a b OR c NOT d").unwrap();
        let term = |t: &str| {
            Query::Phrase(Phrase {
                elements: vec![Element {
                    term: t.into(),
                    prefix: false,
                }],
            })
        };
        assert_eq!(
            q,
            Query::Or(vec![
                Query::And(vec![term("a"), term("b")]),
                Query::And(vec![term("c"), Query::Not(Box::new(term("d")))]),
            ])
        );
        assert_eq!(Query::parse("  ").unwrap(), Query::Empty);
        assert_eq!(Query::parse("!!! ...").unwrap(), Query::Empty);
        for bad in ["(a", "a)", "a OR", "AND a", "\"open", "a NOT"] {
            assert!(Query::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn terms_phrases_prefixes_and_booleans() {
        let text = "The quick brown fox jumps over the lazy dog";
        assert!(m("QUICK fox", text));
        assert!(m("\"brown fox\"", text));
        assert!(!m("\"fox brown\"", text));
        assert!(m("jum*", text));
        assert!(m("\"lazy d*\"", text));
        assert!(!m("cat", text));
        assert!(m("cat OR dog", text));
        assert!(!m("dog NOT lazy", text));
        assert!(m("dog NOT cat", text));
        assert!(m("NOT cat", text));
        assert!(m("(cat OR fox) AND (dog OR bird)", text));
        assert!(!m("and", "nothing here"));
        // Lowercase operators are ordinary words, like FTS5.
        assert!(m("fox and dog", "fox and dog"));
        assert!(!Query::Empty.matches(&doc(text)));
    }

    #[test]
    fn hyphenated_words_and_cjk_substrings_are_phrases() {
        assert!(m("e-mail", "send an e-mail now"));
        assert!(!m("e-mail", "mail an e"));
        let ja = "東京タワーは東京都港区にあります";
        assert!(m("東京タワー", ja));
        assert!(m("港区", ja));
        assert!(m("港", ja));
        assert!(m("す", ja));
        assert!(m("あります", ja));
        assert!(!m("大阪", ja));
        assert!(m("タワー AND 港区", ja));
    }

    #[test]
    fn bm25_prefers_rarer_terms_and_higher_frequency() {
        let stats = CorpusStats {
            documents: 100.0,
            average_length: 5.0,
            document_frequency: [("rare".to_string(), 2.0), ("common".to_string(), 90.0)]
                .into_iter()
                .collect(),
        };
        let q_rare = Query::parse("rare").unwrap();
        let q_common = Query::parse("common").unwrap();
        let d = doc("rare common words here now");
        let rare = bm25(&d, &q_rare, Some(&stats), DEFAULT_K1, DEFAULT_B).unwrap();
        let common = bm25(&d, &q_common, Some(&stats), DEFAULT_K1, DEFAULT_B).unwrap();
        assert!(rare > common && common > 0.0);
        let twice = bm25(
            &doc("rare rare words here now"),
            &q_rare,
            Some(&stats),
            DEFAULT_K1,
            DEFAULT_B,
        )
        .unwrap();
        assert!(twice > rare);
        assert!(bm25(
            &doc("nothing"),
            &q_rare,
            Some(&stats),
            DEFAULT_K1,
            DEFAULT_B
        )
        .is_none());
    }

    #[test]
    fn highlight_and_snippet_mark_phrases_and_cut_with_ellipsis() {
        let q = Query::parse("\"brown fox\" OR dog").unwrap();
        assert_eq!(
            highlight("The quick brown fox saw a dog.", &q, "[", "]"),
            "The quick [brown fox] saw a [dog]."
        );
        let long = "one two three four five six seven eight nine ten eleven twelve target thirteen fourteen fifteen";
        let q = Query::parse("target").unwrap();
        let s = snippet(long, &q, "<b>", "</b>", "...", 5);
        assert_eq!(s, "...eleven twelve <b>target</b> thirteen fourteen...");
        assert_eq!(snippet("short text", &q, "[", "]", "...", 10), "short text");
        assert_eq!(
            snippet(long, &q, "[", "]", "...", 2),
            "...[target] thirteen..."
        );
        let none = Query::parse("absent").unwrap();
        assert_eq!(snippet(long, &none, "[", "]", "…", 3), "one two three…");
        let ja = Query::parse("港区").unwrap();
        assert_eq!(
            highlight("東京都港区です", &ja, "[", "]"),
            "東京都[港区]です"
        );
    }
}
