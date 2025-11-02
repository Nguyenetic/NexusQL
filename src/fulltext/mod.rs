// Week 17: Full-Text Search
//
// Implements PostgreSQL-compatible full-text search:
// - tsvector: Normalized document representation with lexemes
// - tsquery: Search query with boolean operators
// - Operators: @@ (match), ||, &&, !!, <-> (phrase)
// - Functions: to_tsvector, to_tsquery, plainto_tsquery, ts_rank
// - Stemming: Basic word normalization
// - Stop words: Common word filtering

use crate::types::Value;
use crate::{NexusError, Result};
use std::collections::{HashMap, HashSet};

/// Lexeme with position and weight
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Lexeme {
    pub word: String,
    pub positions: Vec<u32>,
    pub weight: char, // A, B, C, D (A = highest)
}

/// Document vector (normalized, stemmed)
#[derive(Debug, Clone, PartialEq)]
pub struct TSVector {
    pub lexemes: Vec<Lexeme>,
}

impl TSVector {
    /// Create empty tsvector
    pub fn new() -> Self {
        Self {
            lexemes: Vec::new(),
        }
    }

    /// Add lexeme
    pub fn add_lexeme(&mut self, word: String, position: u32, weight: char) {
        if let Some(lex) = self.lexemes.iter_mut().find(|l| l.word == word) {
            if !lex.positions.contains(&position) {
                lex.positions.push(position);
            }
        } else {
            self.lexemes.push(Lexeme {
                word,
                positions: vec![position],
                weight,
            });
        }
    }

    /// Concatenate two tsvectors (|| operator)
    pub fn concat(&self, other: &TSVector) -> TSVector {
        let mut result = self.clone();

        for lex in &other.lexemes {
            result.add_lexeme(lex.word.clone(), lex.positions[0], lex.weight);
        }

        result
    }

    /// Match against tsquery (@@ operator)
    pub fn matches(&self, query: &TSQuery) -> bool {
        query.matches(&self.lexemes)
    }

    /// Get total lexeme count
    pub fn length(&self) -> usize {
        self.lexemes.len()
    }
}

/// Text search query
#[derive(Debug, Clone, PartialEq)]
pub enum TSQuery {
    /// Single term
    Term(String),
    /// AND (term1 & term2)
    And(Box<TSQuery>, Box<TSQuery>),
    /// OR (term1 | term2)
    Or(Box<TSQuery>, Box<TSQuery>),
    /// NOT (!term)
    Not(Box<TSQuery>),
    /// Phrase (term1 <-> term2)
    Phrase(Box<TSQuery>, Box<TSQuery>),
}

impl TSQuery {
    /// Match against lexemes
    pub fn matches(&self, lexemes: &[Lexeme]) -> bool {
        match self {
            TSQuery::Term(term) => lexemes.iter().any(|lex| lex.word == *term),
            TSQuery::And(left, right) => left.matches(lexemes) && right.matches(lexemes),
            TSQuery::Or(left, right) => left.matches(lexemes) || right.matches(lexemes),
            TSQuery::Not(inner) => !inner.matches(lexemes),
            TSQuery::Phrase(left, right) => {
                // Check if terms appear consecutively
                if let (TSQuery::Term(t1), TSQuery::Term(t2)) = (left.as_ref(), right.as_ref()) {
                    let lex1 = lexemes.iter().find(|l| l.word == *t1);
                    let lex2 = lexemes.iter().find(|l| l.word == *t2);

                    if let (Some(l1), Some(l2)) = (lex1, lex2) {
                        return l1
                            .positions
                            .iter()
                            .any(|&p1| l2.positions.contains(&(p1 + 1)));
                    }
                }
                false
            }
        }
    }

    /// Combine with AND (&&)
    pub fn and(self, other: TSQuery) -> TSQuery {
        TSQuery::And(Box::new(self), Box::new(other))
    }

    /// Combine with OR (||)
    pub fn or(self, other: TSQuery) -> TSQuery {
        TSQuery::Or(Box::new(self), Box::new(other))
    }

    /// Negate (!!)
    pub fn not(self) -> TSQuery {
        TSQuery::Not(Box::new(self))
    }

    /// Phrase search (<->)
    pub fn followed_by(self, other: TSQuery) -> TSQuery {
        TSQuery::Phrase(Box::new(self), Box::new(other))
    }
}

/// Full-text search functions
pub struct FullTextSearch;

impl FullTextSearch {
    /// Common English stop words
    const STOP_WORDS: &'static [&'static str] = &[
        "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "has", "he", "in", "is",
        "it", "its", "of", "on", "that", "the", "to", "was", "will", "with",
    ];

    /// Convert text to tsvector
    pub fn to_tsvector(text: &str) -> TSVector {
        let mut vec = TSVector::new();

        // Tokenize and normalize
        let words: Vec<&str> = text.split_whitespace().collect();

        for (pos, word) in words.iter().enumerate() {
            let normalized = Self::normalize_word(word);

            // Skip stop words
            if Self::is_stop_word(&normalized) {
                continue;
            }

            // Stem the word
            let stemmed = Self::stem(&normalized);

            vec.add_lexeme(stemmed, (pos + 1) as u32, 'D');
        }

        vec
    }

    /// Convert text to tsquery
    pub fn to_tsquery(text: &str) -> Result<TSQuery> {
        let normalized = Self::normalize_word(text);
        let stemmed = Self::stem(&normalized);
        Ok(TSQuery::Term(stemmed))
    }

    /// Convert plain text to tsquery (combines terms with AND)
    pub fn plainto_tsquery(text: &str) -> Result<TSQuery> {
        let words: Vec<&str> = text
            .split_whitespace()
            .filter(|w| !Self::is_stop_word(&Self::normalize_word(w)))
            .collect();

        if words.is_empty() {
            return Ok(TSQuery::Term(String::new()));
        }

        let mut query = TSQuery::Term(Self::stem(&Self::normalize_word(words[0])));

        for word in &words[1..] {
            let stemmed = Self::stem(&Self::normalize_word(word));
            query = query.and(TSQuery::Term(stemmed));
        }

        Ok(query)
    }

    /// Convert phrase to tsquery (combines with FOLLOWED BY)
    pub fn phraseto_tsquery(text: &str) -> Result<TSQuery> {
        let words: Vec<&str> = text
            .split_whitespace()
            .filter(|w| !Self::is_stop_word(&Self::normalize_word(w)))
            .collect();

        if words.is_empty() {
            return Ok(TSQuery::Term(String::new()));
        }

        let mut query = TSQuery::Term(Self::stem(&Self::normalize_word(words[0])));

        for word in &words[1..] {
            let stemmed = Self::stem(&Self::normalize_word(word));
            query = query.followed_by(TSQuery::Term(stemmed));
        }

        Ok(query)
    }

    /// Calculate relevance rank (ts_rank)
    pub fn ts_rank(vector: &TSVector, query: &TSQuery) -> f64 {
        if !vector.matches(query) {
            return 0.0;
        }

        // Count matching lexemes with weights
        let mut score = 0.0;

        for lexeme in &vector.lexemes {
            if Self::lexeme_matches_query(lexeme, query) {
                // Weight contribution: A=1.0, B=0.4, C=0.2, D=0.1
                let weight_score = match lexeme.weight {
                    'A' => 1.0,
                    'B' => 0.4,
                    'C' => 0.2,
                    _ => 0.1,
                };

                // Frequency contribution
                score += weight_score * lexeme.positions.len() as f64;
            }
        }

        // Normalize by document length
        let doc_length = vector
            .lexemes
            .iter()
            .map(|l| l.positions.len())
            .sum::<usize>() as f64;

        if doc_length > 0.0 {
            score / doc_length.sqrt()
        } else {
            0.0
        }
    }

    /// Check if lexeme matches query
    fn lexeme_matches_query(lexeme: &Lexeme, query: &TSQuery) -> bool {
        match query {
            TSQuery::Term(term) => lexeme.word == *term,
            TSQuery::And(left, right) => {
                Self::lexeme_matches_query(lexeme, left)
                    && Self::lexeme_matches_query(lexeme, right)
            }
            TSQuery::Or(left, right) => {
                Self::lexeme_matches_query(lexeme, left)
                    || Self::lexeme_matches_query(lexeme, right)
            }
            TSQuery::Not(inner) => !Self::lexeme_matches_query(lexeme, inner),
            TSQuery::Phrase(left, right) => {
                Self::lexeme_matches_query(lexeme, left)
                    || Self::lexeme_matches_query(lexeme, right)
            }
        }
    }

    /// Normalize word (lowercase, remove punctuation)
    fn normalize_word(word: &str) -> String {
        word.to_lowercase()
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_string()
    }

    /// Check if word is a stop word
    fn is_stop_word(word: &str) -> bool {
        Self::STOP_WORDS.contains(&word)
    }

    /// Simple English stemming (Porter-like)
    fn stem(word: &str) -> String {
        let word = word.to_lowercase();

        // Very basic stemming rules (order matters - longer suffixes first!)
        if word.ends_with("ies") && word.len() > 4 {
            return format!("{}y", &word[..word.len() - 3]);
        }
        if word.ends_with("ing") && word.len() > 4 {
            return word[..word.len() - 3].to_string();
        }
        if word.ends_with("ed") && word.len() > 3 {
            return word[..word.len() - 2].to_string();
        }
        if word.ends_with("es") && word.len() > 3 {
            return word[..word.len() - 2].to_string();
        }
        if word.ends_with("s") && word.len() > 2 {
            return word[..word.len() - 1].to_string();
        }

        word
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_tsvector_basic() {
        let vec = FullTextSearch::to_tsvector("The quick brown fox jumps over the lazy dog");
        // Stop words removed: "the", "over"
        assert!(vec.lexemes.iter().any(|l| l.word == "quick"));
        assert!(vec.lexemes.iter().any(|l| l.word == "fox"));
        assert!(!vec.lexemes.iter().any(|l| l.word == "the"));
    }

    #[test]
    fn test_to_tsvector_stemming() {
        let vec = FullTextSearch::to_tsvector("cats dogs");
        // "cats" and "dogs" should stem to "cat" and "dog"
        assert!(vec.lexemes.iter().any(|l| l.word == "cat"));
        assert!(vec.lexemes.iter().any(|l| l.word == "dog"));
    }

    #[test]
    fn test_tsquery_match_simple() {
        let vec = FullTextSearch::to_tsvector("fat cats ate fat rats");
        let query = FullTextSearch::plainto_tsquery("cat rat").unwrap();

        assert!(vec.matches(&query));
    }

    #[test]
    fn test_tsquery_and_or() {
        let vec = FullTextSearch::to_tsvector("the quick brown fox");

        let q1 = TSQuery::Term("quick".to_string());
        let q2 = TSQuery::Term("fox".to_string());
        let q3 = TSQuery::Term("cat".to_string());

        // quick AND fox = true
        assert!(vec.matches(&q1.clone().and(q2.clone())));

        // quick AND cat = false
        assert!(!vec.matches(&q1.clone().and(q3.clone())));

        // quick OR cat = true
        assert!(vec.matches(&q1.clone().or(q3.clone())));
    }

    #[test]
    fn test_tsquery_not() {
        let vec = FullTextSearch::to_tsvector("the quick brown fox");

        let q_fox = TSQuery::Term("fox".to_string());
        let q_cat = TSQuery::Term("cat".to_string());

        // NOT cat = true (cat not in document)
        assert!(vec.matches(&q_cat.clone().not()));

        // NOT fox = false (fox IS in document)
        assert!(!vec.matches(&q_fox.clone().not()));
    }

    #[test]
    fn test_tsquery_phrase() {
        let vec = FullTextSearch::to_tsvector("fat cats ate fat rats");

        // "fat" followed by "cat"
        let query = FullTextSearch::phraseto_tsquery("fat cat").unwrap();
        assert!(vec.matches(&query));

        // "cat" followed by "fat" - should not match
        let query2 = FullTextSearch::phraseto_tsquery("cat fat").unwrap();
        assert!(!vec.matches(&query2));
    }

    #[test]
    fn test_ts_rank() {
        let vec1 = FullTextSearch::to_tsvector("cat cat cat");
        let vec2 = FullTextSearch::to_tsvector("cat dog bird");
        let query = FullTextSearch::to_tsquery("cat").unwrap();

        let rank1 = FullTextSearch::ts_rank(&vec1, &query);
        let rank2 = FullTextSearch::ts_rank(&vec2, &query);

        // vec1 has more "cat" occurrences, should rank higher
        assert!(rank1 > rank2);
    }

    #[test]
    fn test_stop_words() {
        let vec = FullTextSearch::to_tsvector("the quick brown fox is in the forest");

        // Stop words should be filtered out
        assert!(!vec.lexemes.iter().any(|l| l.word == "the"));
        assert!(!vec.lexemes.iter().any(|l| l.word == "is"));
        assert!(!vec.lexemes.iter().any(|l| l.word == "in"));

        // Content words should remain
        assert!(vec.lexemes.iter().any(|l| l.word == "quick"));
        assert!(vec.lexemes.iter().any(|l| l.word == "forest"));
    }

    #[test]
    fn test_tsvector_concat() {
        let vec1 = FullTextSearch::to_tsvector("quick brown");
        let vec2 = FullTextSearch::to_tsvector("lazy dog");

        let combined = vec1.concat(&vec2);

        assert!(combined.lexemes.iter().any(|l| l.word == "quick"));
        assert!(combined.lexemes.iter().any(|l| l.word == "dog"));
    }
}
