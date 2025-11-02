//! ColBERT Token Embeddings
//! PROPRIETARY

use ndarray::Array2;

pub struct EmbeddingGenerator {
    dimension: usize,
}

impl EmbeddingGenerator {
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }
    
    pub fn encode(&self, text: &str) -> Array2<f32> {
        let tokens: Vec<&str> = text.split_whitespace().collect();
        let num_tokens = tokens.len().max(1);
        
        let mut embeddings = Array2::<f32>::zeros((num_tokens, self.dimension));
        
        for (i, token) in tokens.iter().enumerate() {
            let hash = self.hash_token(token);
            for j in 0..self.dimension {
                embeddings[[i, j]] = ((hash + j) as f32).sin();
            }
            
            let mut row_slice = embeddings.row_mut(i);
            let norm: f32 = row_slice.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                row_slice.iter_mut().for_each(|x| *x /= norm);
            }
        }
        
        embeddings
    }
    
    fn hash_token(&self, token: &str) -> usize {
        token.bytes().map(|b| b as usize).sum()
    }
}
