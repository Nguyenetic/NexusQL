//! Inverted index for `CREATE INDEX ... USING FULLTEXT (column)`.
//!
//! Postings are keyed by internal document ids that never change while a row
//! lives, so deleting rows only remaps the small row<->document tables instead
//! of rewriting every posting list. Document ids are assigned in increasing
//! order, which keeps every posting list sorted by appending.
//!
//! The index is derived data: it is never written to disk. Its definition is
//! persisted in the table schema and it is rebuilt from rows on open, exactly
//! like GiST. Rollback restores it with the table snapshot.

use super::query::{CorpusStats, Element, Phrase, Query};
use super::tokenizer::{tokenize, Mode};
use crate::types::Value;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};

const DEAD: u32 = u32::MAX;

#[derive(Debug, Clone)]
struct Posting {
    doc: u32,
    positions: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct FullTextIndex {
    pub name: String,
    pub column: String,
    terms: BTreeMap<String, Vec<Posting>>,
    /// Document id -> row offset (`DEAD` once removed).
    doc_row: Vec<u32>,
    doc_len: Vec<u32>,
    /// Row offset -> document id (`DEAD` for NULL / non-text values).
    row_doc: Vec<u32>,
    live_docs: u64,
    dead_docs: u64,
    total_len: u64,
}

/// Only TEXT values are indexed; every other value never matches.
pub fn indexed_text(value: Option<&Value>) -> Option<&str> {
    match value {
        Some(Value::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

impl FullTextIndex {
    pub fn new(name: String, column: String) -> Self {
        Self {
            name,
            column,
            terms: BTreeMap::new(),
            doc_row: Vec::new(),
            doc_len: Vec::new(),
            row_doc: Vec::new(),
            live_docs: 0,
            dead_docs: 0,
            total_len: 0,
        }
    }

    /// Rebuild from all rows' values of the indexed column, in row order.
    pub fn build<'a>(&mut self, values: impl Iterator<Item = Option<&'a Value>>) {
        *self = Self::new(
            std::mem::take(&mut self.name),
            std::mem::take(&mut self.column),
        );
        for value in values {
            self.push_row(value);
        }
    }

    /// Number of rows tracked (equals the table's row count).
    pub fn row_count(&self) -> usize {
        self.row_doc.len()
    }

    /// Number of indexed TEXT values.
    pub fn document_count(&self) -> u64 {
        self.live_docs
    }

    pub fn term_count(&self) -> usize {
        self.terms.len()
    }

    /// Append the next row (row offset == current row count).
    pub fn push_row(&mut self, value: Option<&Value>) {
        let row = self.row_doc.len() as u32;
        let doc = match indexed_text(value) {
            Some(text) => self.add_doc(row, text),
            None => DEAD,
        };
        self.row_doc.push(doc);
    }

    fn add_doc(&mut self, row: u32, text: &str) -> u32 {
        let doc = self.doc_row.len() as u32;
        let tokens = tokenize(text, Mode::Document);
        let mut by_term: HashMap<&str, Vec<u32>> = HashMap::new();
        for token in &tokens {
            by_term
                .entry(token.term.as_str())
                .or_default()
                .push(token.position);
        }
        for (term, positions) in by_term {
            let list = match self.terms.get_mut(term) {
                Some(list) => list,
                None => self.terms.entry(term.to_string()).or_default(),
            };
            list.push(Posting { doc, positions });
        }
        self.doc_row.push(row);
        self.doc_len.push(tokens.len() as u32);
        self.live_docs += 1;
        self.total_len += tokens.len() as u64;
        doc
    }

    fn remove_doc(&mut self, doc: u32, text: &str) {
        if self.doc_row.get(doc as usize).copied().unwrap_or(DEAD) == DEAD {
            return;
        }
        let mut terms: Vec<String> = tokenize(text, Mode::Document)
            .into_iter()
            .map(|t| t.term)
            .collect();
        terms.sort_unstable();
        terms.dedup();
        for term in terms {
            if let Some(list) = self.terms.get_mut(&term) {
                if let Ok(i) = list.binary_search_by_key(&doc, |p| p.doc) {
                    list.remove(i);
                }
                if list.is_empty() {
                    self.terms.remove(&term);
                }
            }
        }
        self.doc_row[doc as usize] = DEAD;
        self.live_docs -= 1;
        self.dead_docs += 1;
        self.total_len -= self.doc_len[doc as usize] as u64;
    }

    /// Re-index one row whose value changed.
    pub fn replace_row(&mut self, row: usize, old: Option<&Value>, new: Option<&Value>) {
        let (old_text, new_text) = (indexed_text(old), indexed_text(new));
        if old_text == new_text {
            return;
        }
        let doc = self.row_doc[row];
        if let (true, Some(text)) = (doc != DEAD, old_text) {
            self.remove_doc(doc, text);
        }
        self.row_doc[row] = match new_text {
            Some(text) => self.add_doc(row as u32, text),
            None => DEAD,
        };
        self.maybe_compact();
    }

    /// Remove a row's postings. Call `remap_row_ids` after all removals.
    pub fn remove_row(&mut self, row: usize, old: Option<&Value>) {
        let doc = self.row_doc[row];
        if let (true, Some(text)) = (doc != DEAD, indexed_text(old)) {
            self.remove_doc(doc, text);
        }
        self.row_doc[row] = DEAD;
    }

    /// `offsets[old_row]` is the surviving row's new offset or `usize::MAX`.
    pub fn remap_row_ids(&mut self, offsets: &[usize]) {
        let mut row_doc = Vec::with_capacity(self.row_doc.len());
        for (old_row, &new_row) in offsets.iter().enumerate() {
            if new_row == usize::MAX {
                continue;
            }
            debug_assert_eq!(new_row, row_doc.len());
            let doc = self.row_doc[old_row];
            if doc != DEAD {
                self.doc_row[doc as usize] = new_row as u32;
            }
            row_doc.push(doc);
        }
        self.row_doc = row_doc;
        self.maybe_compact();
    }

    /// Renumber documents once removed slots dominate. Order is preserved, so
    /// posting lists stay sorted.
    fn maybe_compact(&mut self) {
        if self.dead_docs < 1024 || self.dead_docs < self.live_docs {
            return;
        }
        let mut remap = vec![DEAD; self.doc_row.len()];
        let mut doc_row = Vec::with_capacity(self.live_docs as usize);
        let mut doc_len = Vec::with_capacity(self.live_docs as usize);
        for (doc, &row) in self.doc_row.iter().enumerate() {
            if row != DEAD {
                remap[doc] = doc_row.len() as u32;
                doc_row.push(row);
                doc_len.push(self.doc_len[doc]);
            }
        }
        for list in self.terms.values_mut() {
            list.retain_mut(|p| {
                p.doc = remap[p.doc as usize];
                p.doc != DEAD
            });
        }
        self.terms.retain(|_, list| !list.is_empty());
        for doc in &mut self.row_doc {
            if *doc != DEAD {
                *doc = remap[*doc as usize];
            }
        }
        self.doc_row = doc_row;
        self.doc_len = doc_len;
        self.dead_docs = 0;
    }

    fn element_postings(&self, element: &Element) -> Cow<'_, [Posting]> {
        if !element.prefix {
            return match self.terms.get(&element.term) {
                Some(list) => Cow::Borrowed(list.as_slice()),
                None => Cow::Owned(Vec::new()),
            };
        }
        let mut lists = self
            .terms
            .range(element.term.clone()..)
            .take_while(|(term, _)| term.starts_with(element.term.as_str()))
            .map(|(_, list)| list);
        let Some(first) = lists.next() else {
            return Cow::Owned(Vec::new());
        };
        let rest: Vec<_> = lists.collect();
        if rest.is_empty() {
            return Cow::Borrowed(first.as_slice());
        }
        let mut merged: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for list in std::iter::once(first).chain(rest) {
            for posting in list {
                merged
                    .entry(posting.doc)
                    .or_default()
                    .extend_from_slice(&posting.positions);
            }
        }
        Cow::Owned(
            merged
                .into_iter()
                .map(|(doc, mut positions)| {
                    positions.sort_unstable();
                    positions.dedup();
                    Posting { doc, positions }
                })
                .collect(),
        )
    }

    /// `(document, phrase frequency)` for every live document containing `phrase`.
    pub fn phrase_documents(&self, phrase: &Phrase) -> Vec<(u32, u32)> {
        let lists: Vec<Cow<'_, [Posting]>> = phrase
            .elements
            .iter()
            .map(|e| self.element_postings(e))
            .collect();
        let Some((first, rest)) = lists.split_first() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        'docs: for posting in first.iter() {
            if self.doc_row[posting.doc as usize] == DEAD {
                continue;
            }
            if rest.is_empty() {
                out.push((posting.doc, posting.positions.len() as u32));
                continue;
            }
            let mut others = Vec::with_capacity(rest.len());
            for list in rest {
                match list.binary_search_by_key(&posting.doc, |p| p.doc) {
                    Ok(i) => others.push(&list[i].positions),
                    Err(_) => continue 'docs,
                }
            }
            let frequency = posting
                .positions
                .iter()
                .filter(|&&start| {
                    others.iter().enumerate().all(|(k, positions)| {
                        positions.binary_search(&(start + k as u32 + 1)).is_ok()
                    })
                })
                .count() as u32;
            if frequency > 0 {
                out.push((posting.doc, frequency));
            }
        }
        out
    }

    fn universe(&self) -> Vec<u32> {
        (0..self.doc_row.len() as u32)
            .filter(|&doc| self.doc_row[doc as usize] != DEAD)
            .collect()
    }

    fn evaluate(&self, query: &Query) -> Vec<u32> {
        match query {
            Query::Empty => Vec::new(),
            Query::Phrase(phrase) => self
                .phrase_documents(phrase)
                .into_iter()
                .map(|(doc, _)| doc)
                .collect(),
            Query::Not(inner) => difference(&self.universe(), &self.evaluate(inner)),
            Query::Or(parts) => {
                let mut docs: Vec<u32> = parts.iter().flat_map(|q| self.evaluate(q)).collect();
                docs.sort_unstable();
                docs.dedup();
                docs
            }
            Query::And(parts) => {
                let (negative, positive): (Vec<_>, Vec<_>) =
                    parts.iter().partition(|q| matches!(q, Query::Not(_)));
                let mut docs = match positive.split_first() {
                    Some((first, rest)) => {
                        let mut docs = self.evaluate(first);
                        for q in rest {
                            if docs.is_empty() {
                                break;
                            }
                            docs = intersection(&docs, &self.evaluate(q));
                        }
                        docs
                    }
                    None => self.universe(),
                };
                for q in negative {
                    if let Query::Not(inner) = q {
                        if docs.is_empty() {
                            break;
                        }
                        docs = difference(&docs, &self.evaluate(inner));
                    }
                }
                docs
            }
        }
    }

    /// Row offsets (ascending) whose indexed text matches `query`. Exactly the
    /// rows for which per-row evaluation of the same query is true.
    pub fn search(&self, query: &Query) -> Vec<usize> {
        let mut rows: Vec<usize> = self
            .evaluate(query)
            .into_iter()
            .map(|doc| self.doc_row[doc as usize] as usize)
            .collect();
        rows.sort_unstable();
        rows
    }

    /// Corpus statistics for ranking `query` against this column.
    pub fn stats(&self, query: &Query) -> CorpusStats {
        let documents = self.live_docs as f64;
        CorpusStats {
            documents,
            average_length: if self.live_docs == 0 {
                0.0
            } else {
                self.total_len as f64 / documents
            },
            document_frequency: query
                .positive_phrases()
                .into_iter()
                .map(|p| (p.key(), self.phrase_documents(p).len() as f64))
                .collect(),
        }
    }
}

fn intersection(a: &[u32], b: &[u32]) -> Vec<u32> {
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                out.push(a[i]);
                i += 1;
                j += 1;
            }
        }
    }
    out
}

fn difference(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut j = 0;
    a.iter()
        .copied()
        .filter(|x| {
            while j < b.len() && b[j] < *x {
                j += 1;
            }
            !(j < b.len() && b[j] == *x)
        })
        .collect()
}

/// Statistics computed by scanning a column that has no full-text index.
pub fn scan_stats<'a>(
    values: impl Iterator<Item = Option<&'a Value>>,
    query: &Query,
) -> CorpusStats {
    let phrases = query.positive_phrases();
    let mut frequency = vec![0.0; phrases.len()];
    let (mut documents, mut total) = (0.0, 0.0);
    for value in values {
        let Some(text) = indexed_text(value) else {
            continue;
        };
        let tokens = tokenize(text, Mode::Document);
        documents += 1.0;
        total += tokens.len() as f64;
        for (i, phrase) in phrases.iter().enumerate() {
            if !super::query::occurrences(&tokens, phrase).is_empty() {
                frequency[i] += 1.0;
            }
        }
    }
    CorpusStats {
        documents,
        average_length: if documents > 0.0 {
            total / documents
        } else {
            0.0
        },
        document_frequency: phrases.iter().map(|p| p.key()).zip(frequency).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> Value {
        Value::Text(s.to_string())
    }

    fn rows_matching(values: &[Value], query: &str) -> Vec<usize> {
        let q = Query::parse(query).unwrap();
        values
            .iter()
            .enumerate()
            .filter(|(_, v)| {
                indexed_text(Some(v)).is_some_and(|t| q.matches(&tokenize(t, Mode::Document)))
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn assert_consistent(index: &FullTextIndex, values: &[Value]) {
        assert_eq!(index.row_count(), values.len());
        for query in [
            "rust",
            "rus*",
            "\"fast database\"",
            "rust OR python",
            "database NOT slow",
            "NOT rust",
            "東京",
            "京",
            "(fast OR slow) AND database",
            "zzz",
        ] {
            let q = Query::parse(query).unwrap();
            assert_eq!(
                index.search(&q),
                rows_matching(values, query),
                "query {query}"
            );
        }
    }

    #[test]
    fn index_matches_per_row_evaluation_through_mutations() {
        let mut values = vec![
            text("Rust is a fast database language"),
            Value::Null,
            text("Python makes a slow database"),
            text("東京の高速データベース"),
            Value::Integer(7),
            text("rusty nails and rust"),
        ];
        let mut index = FullTextIndex::new("idx".into(), "body".into());
        index.build(values.iter().map(Some));
        assert_consistent(&index, &values);
        assert_eq!(index.document_count(), 4);

        index.push_row(Some(&text("fast database fast database")));
        values.push(text("fast database fast database"));
        assert_consistent(&index, &values);

        let new = text("now about python only");
        index.replace_row(0, Some(&values[0]), Some(&new));
        values[0] = new;
        index.replace_row(1, Some(&Value::Null), Some(&text("rust appears")));
        values[1] = text("rust appears");
        index.replace_row(3, Some(&values[3].clone()), Some(&Value::Null));
        values[3] = Value::Null;
        assert_consistent(&index, &values);

        // Delete rows 1 and 2, then remap like Table::retain_rows.
        let mut offsets = Vec::new();
        let mut next = 0;
        for row in 0..values.len() {
            if row == 1 || row == 2 {
                index.remove_row(row, Some(&values[row]));
                offsets.push(usize::MAX);
            } else {
                offsets.push(next);
                next += 1;
            }
        }
        index.remap_row_ids(&offsets);
        values.remove(2);
        values.remove(1);
        assert_consistent(&index, &values);
    }

    #[test]
    fn phrase_frequency_and_stats() {
        let values = [text("a b a b"), text("b a"), text("a c b")];
        let mut index = FullTextIndex::new("i".into(), "c".into());
        index.build(values.iter().map(Some));
        let q = Query::parse("\"a b\"").unwrap();
        let Query::Phrase(p) = &q else { panic!() };
        assert_eq!(index.phrase_documents(p), vec![(0, 2)]);
        let stats = index.stats(&q);
        assert_eq!(stats.documents, 3.0);
        assert_eq!(stats.average_length, 3.0);
        assert_eq!(stats.document_frequency["a b"], 1.0);
        let scanned = scan_stats(values.iter().map(Some), &q);
        assert_eq!(scanned.document_frequency, stats.document_frequency);
    }

    #[test]
    fn compaction_preserves_results() {
        let mut values: Vec<Value> = (0..3000)
            .map(|i| text(&format!("doc{} common", i % 7)))
            .collect();
        let mut index = FullTextIndex::new("i".into(), "c".into());
        index.build(values.iter().map(Some));
        for row in 0..2500 {
            let new = text(&format!("changed{} common", row % 5));
            index.replace_row(row, Some(&values[row]), Some(&new));
            values[row] = new;
        }
        assert!(index.dead_docs < index.live_docs, "compaction ran");
        for query in ["common", "doc3", "changed2", "chan*", "NOT common"] {
            let q = Query::parse(query).unwrap();
            assert_eq!(index.search(&q), rows_matching(&values, query), "{query}");
        }
    }
}
