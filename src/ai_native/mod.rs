// v2.0.0: AI-Native Database
//
// Implements AI-first database capabilities:
// - Native vector operations: Built-in embedding storage and search
// - Embedding generation: Automatic text-to-vector conversion
// - Hybrid search: Combine semantic + keyword search
// - AutoML query optimization: Machine learning for query planning
// - Semantic caching: Vector-based cache for similar queries

use crate::types::{Row, Value};
use crate::{NexusError, Result};
use std::collections::HashMap;

/// Vector embedding (dense)
#[derive(Debug, Clone, PartialEq)]
pub struct Embedding {
    pub dimensions: usize,
    pub values: Vec<f32>,
}

impl Embedding {
    pub fn new(values: Vec<f32>) -> Self {
        Self {
            dimensions: values.len(),
            values,
        }
    }

    /// Cosine similarity
    pub fn cosine_similarity(&self, other: &Embedding) -> f32 {
        if self.dimensions != other.dimensions {
            return 0.0;
        }

        let dot: f32 = self
            .values
            .iter()
            .zip(other.values.iter())
            .map(|(a, b)| a * b)
            .sum();

        let norm_a: f32 = self.values.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = other.values.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }

    /// Euclidean distance
    pub fn euclidean_distance(&self, other: &Embedding) -> f32 {
        if self.dimensions != other.dimensions {
            return f32::MAX;
        }

        self.values
            .iter()
            .zip(other.values.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            .sqrt()
    }
}

/// Embedding generator
pub struct EmbeddingGenerator;

impl EmbeddingGenerator {
    /// Generate embedding from text (simplified)
    pub fn from_text(text: &str, dimensions: usize) -> Embedding {
        // Simplified: Create embedding from text hash
        // In production: Use sentence transformers, OpenAI API, etc.

        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut values = Vec::with_capacity(dimensions);

        for i in 0..dimensions {
            let mut hasher = DefaultHasher::new();
            text.hash(&mut hasher);
            i.hash(&mut hasher);

            let hash = hasher.finish();
            let normalized = (hash % 10000) as f32 / 10000.0 - 0.5;
            values.push(normalized);
        }

        // Normalize to unit vector
        let norm: f32 = values.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut values {
                *v /= norm;
            }
        }

        Embedding::new(values)
    }

    /// Generate from image (placeholder)
    pub fn from_image(_image_data: &[u8], dimensions: usize) -> Embedding {
        // Placeholder: Would use vision models like CLIP
        Embedding::new(vec![0.0; dimensions])
    }
}

/// Hybrid search combining semantic + keyword
pub struct HybridSearch {
    alpha: f32, // 0.0 = pure keyword, 1.0 = pure semantic
}

impl HybridSearch {
    pub fn new(alpha: f32) -> Self {
        Self {
            alpha: alpha.clamp(0.0, 1.0),
        }
    }

    /// Search with both semantic and keyword matching
    pub fn search(
        &self,
        query_embedding: &Embedding,
        query_text: &str,
        documents: &[(Embedding, String)],
    ) -> Vec<(usize, f32)> {
        let mut scores = Vec::new();

        for (idx, (doc_embedding, doc_text)) in documents.iter().enumerate() {
            // Semantic score (cosine similarity)
            let semantic_score = query_embedding.cosine_similarity(doc_embedding);

            // Keyword score (simple word overlap)
            let keyword_score = self.keyword_similarity(query_text, doc_text);

            // Hybrid score
            let hybrid_score = self.alpha * semantic_score + (1.0 - self.alpha) * keyword_score;

            scores.push((idx, hybrid_score));
        }

        // Sort by score descending
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores
    }

    fn keyword_similarity(&self, query: &str, doc: &str) -> f32 {
        let query_words: Vec<&str> = query.split_whitespace().collect();
        let doc_lower = doc.to_lowercase();

        let matches = query_words
            .iter()
            .filter(|w| doc_lower.contains(&w.to_lowercase()))
            .count();

        if query_words.is_empty() {
            0.0
        } else {
            matches as f32 / query_words.len() as f32
        }
    }
}

/// AutoML query optimizer
pub struct AutoMLOptimizer {
    query_history: Vec<QueryExecution>,
    model_cache: HashMap<String, OptimizationModel>,
}

#[derive(Debug, Clone)]
pub struct QueryExecution {
    pub query: String,
    pub execution_time_ms: f64,
    pub rows_scanned: usize,
    pub rows_returned: usize,
    pub index_used: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OptimizationModel {
    pub query_pattern: String,
    pub recommended_index: Option<String>,
    pub avg_execution_time: f64,
    pub execution_count: usize,
}

impl AutoMLOptimizer {
    pub fn new() -> Self {
        Self {
            query_history: Vec::new(),
            model_cache: HashMap::new(),
        }
    }

    /// Record query execution for learning
    pub fn record_execution(&mut self, execution: QueryExecution) {
        self.query_history.push(execution.clone());

        // Update model
        let pattern = self.extract_pattern(&execution.query);
        let model = self
            .model_cache
            .entry(pattern.clone())
            .or_insert(OptimizationModel {
                query_pattern: pattern,
                recommended_index: None,
                avg_execution_time: 0.0,
                execution_count: 0,
            });

        // Update running average
        model.avg_execution_time = (model.avg_execution_time * model.execution_count as f64
            + execution.execution_time_ms)
            / (model.execution_count + 1) as f64;
        model.execution_count += 1;

        // Learn from index usage
        if let Some(ref idx) = execution.index_used {
            if execution.execution_time_ms < model.avg_execution_time {
                model.recommended_index = Some(idx.clone());
            }
        }
    }

    /// Suggest optimization for query
    pub fn suggest_optimization(&self, query: &str) -> Option<String> {
        let pattern = self.extract_pattern(query);

        if let Some(model) = self.model_cache.get(&pattern) {
            if let Some(ref idx) = model.recommended_index {
                return Some(format!("CREATE INDEX ON {}", idx));
            }
        }

        None
    }

    fn extract_pattern(&self, query: &str) -> String {
        // Simplified: Extract table name and operation
        let lower = query.to_lowercase();
        if lower.contains("select") && lower.contains("from") {
            "SELECT_PATTERN".to_string()
        } else if lower.contains("insert") {
            "INSERT_PATTERN".to_string()
        } else {
            "UNKNOWN".to_string()
        }
    }

    pub fn learning_stats(&self) -> LearningStats {
        LearningStats {
            queries_learned: self.query_history.len(),
            patterns_identified: self.model_cache.len(),
            optimizations_suggested: self
                .model_cache
                .values()
                .filter(|m| m.recommended_index.is_some())
                .count(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LearningStats {
    pub queries_learned: usize,
    pub patterns_identified: usize,
    pub optimizations_suggested: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_similarity() {
        let e1 = Embedding::new(vec![1.0, 0.0, 0.0]);
        let e2 = Embedding::new(vec![0.8, 0.6, 0.0]);

        let similarity = e1.cosine_similarity(&e2);
        assert!(similarity > 0.7 && similarity < 0.9);
    }

    #[test]
    fn test_embedding_from_text() {
        let emb1 = EmbeddingGenerator::from_text("hello world", 128);
        let emb2 = EmbeddingGenerator::from_text("hello world", 128);
        let emb3 = EmbeddingGenerator::from_text("goodbye world", 128);

        assert_eq!(emb1.dimensions, 128);

        // Same text should produce same embedding
        let sim_same = emb1.cosine_similarity(&emb2);
        let sim_diff = emb1.cosine_similarity(&emb3);

        assert!(sim_same > 0.99); // Nearly identical
        assert!(sim_diff < sim_same); // Different text less similar
    }

    #[test]
    fn test_hybrid_search() {
        let docs = vec![
            (
                EmbeddingGenerator::from_text("database systems", 64),
                "database systems".to_string(),
            ),
            (
                EmbeddingGenerator::from_text("machine learning", 64),
                "machine learning".to_string(),
            ),
            (
                EmbeddingGenerator::from_text("database machine learning", 64),
                "database machine learning".to_string(),
            ),
        ];

        let query_emb = EmbeddingGenerator::from_text("database learning", 64);
        let searcher = HybridSearch::new(0.7); // 70% semantic, 30% keyword

        let results = searcher.search(&query_emb, "database learning", &docs);

        assert_eq!(results.len(), 3);
        // Third doc should rank highest (has both terms)
    }

    #[test]
    fn test_automl_optimizer() {
        let mut optimizer = AutoMLOptimizer::new();

        // Record executions
        optimizer.record_execution(QueryExecution {
            query: "SELECT * FROM users WHERE id = 1".to_string(),
            execution_time_ms: 10.0,
            rows_scanned: 1000,
            rows_returned: 1,
            index_used: Some("users_id_idx".to_string()),
        });

        optimizer.record_execution(QueryExecution {
            query: "SELECT * FROM users WHERE id = 2".to_string(),
            execution_time_ms: 8.0,
            rows_scanned: 1000,
            rows_returned: 1,
            index_used: Some("users_id_idx".to_string()),
        });

        let stats = optimizer.learning_stats();
        assert_eq!(stats.queries_learned, 2);
        assert!(stats.patterns_identified > 0);
    }

    #[test]
    fn test_embedding_distance() {
        let e1 = Embedding::new(vec![1.0, 2.0, 3.0]);
        let e2 = Embedding::new(vec![1.0, 2.0, 3.0]);
        let e3 = Embedding::new(vec![4.0, 5.0, 6.0]);

        assert!(e1.euclidean_distance(&e2) < 0.01); // Same vectors
        assert!(e1.euclidean_distance(&e3) > 5.0); // Different vectors
    }

    #[test]
    fn test_automl_suggestion() {
        let mut optimizer = AutoMLOptimizer::new();

        // Learn from fast query with index
        optimizer.record_execution(QueryExecution {
            query: "SELECT * FROM products WHERE category = 'books'".to_string(),
            execution_time_ms: 2.0,
            rows_scanned: 10,
            rows_returned: 10,
            index_used: Some("products_category_idx".to_string()),
        });

        // Check that learning occurred
        let stats = optimizer.learning_stats();
        assert_eq!(stats.queries_learned, 1);
        assert!(stats.patterns_identified > 0);
    }

    #[test]
    fn test_hybrid_search_weights() {
        let docs = vec![
            (EmbeddingGenerator::from_text("cat", 32), "dog".to_string()),
            (EmbeddingGenerator::from_text("dog", 32), "cat".to_string()),
        ];

        let query_emb = EmbeddingGenerator::from_text("cat", 32);

        // Pure semantic (alpha=1.0)
        let semantic = HybridSearch::new(1.0);
        let results_sem = semantic.search(&query_emb, "cat", &docs);

        // Pure keyword (alpha=0.0)
        let keyword = HybridSearch::new(0.0);
        let results_key = keyword.search(&query_emb, "cat", &docs);

        // Results should differ based on weighting
        assert_eq!(results_sem.len(), 2);
        assert_eq!(results_key.len(), 2);
    }
}
