//! MaxSim Operator - ColBERT Late Interaction
//! PROPRIETARY - Innovation #2

use ndarray::Array2;

/// Compute MaxSim score between query and document
/// 
/// For each query token, find max similarity with any document token
pub fn maxsim_score(query_embeddings: &Array2<f32>, doc_embeddings: &Array2<f32>) -> f32 {
    let mut total_score = 0.0;
    
    // For each query token
    for q_token in query_embeddings.rows() {
        let mut max_sim = f32::MIN;
        
        // Find maximum similarity with any doc token
        for d_token in doc_embeddings.rows() {
            let sim = cosine_similarity(&q_token, &d_token);
            max_sim = max_sim.max(sim);
        }
        
        total_score += max_sim;
    }
    
    total_score
}

/// Cosine similarity between two vectors
fn cosine_similarity(a: &ndarray::ArrayView1<f32>, b: &ndarray::ArrayView1<f32>) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    
    // Already normalized in embeddings, so dot product = cosine sim
    dot
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::arr2;
    
    #[test]
    fn test_maxsim() {
        let query = arr2(&[[1.0, 0.0], [0.0, 1.0]]);
        let doc = arr2(&[[1.0, 0.0], [0.5, 0.5]]);
        
        let score = maxsim_score(&query, &doc);
        assert!(score > 0.0);
    }
}
