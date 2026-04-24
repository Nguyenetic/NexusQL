//! BERT with ONNX - Simplified for now
//! PROPRIETARY

#[cfg(feature = "vector-search")]
use tokenizers::Tokenizer;
use ndarray::Array2;
use std::path::Path;
use anyhow::Result;

pub struct BertEncoder {
    #[cfg(feature = "vector-search")]
    tokenizer: Option<Tokenizer>,
    dimension: usize,
}

impl BertEncoder {
    pub fn new(dimension: usize) -> Self {
        Self {
            #[cfg(feature = "vector-search")]
            tokenizer: None,
            dimension,
        }
    }

    pub async fn load_model(
        &mut self,
        _model_path: impl AsRef<Path>,
        #[cfg(feature = "vector-search")] tokenizer_path: impl AsRef<Path>,
        #[cfg(not(feature = "vector-search"))] _tokenizer_path: impl AsRef<Path>,
    ) -> Result<()> {
        #[cfg(feature = "vector-search")]
        {
            let tokenizer = Tokenizer::from_file(tokenizer_path)
                .map_err(|e| anyhow::anyhow!("Tokenizer error: {}", e))?;
            self.tokenizer = Some(tokenizer);
        }
        Ok(())
    }

    pub fn encode(&self, text: &str) -> Result<Array2<f32>> {
        let tokens: Vec<&str> = text.split_whitespace().collect();
        let num_tokens = tokens.len().max(1);

        let mut embeddings = Array2::<f32>::zeros((num_tokens, self.dimension));

        for (i, token) in tokens.iter().enumerate() {
            let hash = token.bytes().map(|b| b as usize).sum::<usize>();
            for j in 0..self.dimension {
                embeddings[[i, j]] = ((hash + j) as f32).sin();
            }

            let mut row = embeddings.row_mut(i);
            let norm: f32 = row.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                row.iter_mut().for_each(|x| *x /= norm);
            }
        }

        Ok(embeddings)
    }
}
