use std::sync::Mutex;

use anyhow::Result;
use async_trait::async_trait;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

use super::SimilarityMetric;

/// Embedding-based cosine similarity using fastembed.
pub struct CosineSimilarity {
    embedder: Mutex<TextEmbedding>,
}

impl CosineSimilarity {
    /// Build a new cosine similarity metric backed by the default fastembed model.
    ///
    /// Uses a **single** embedder session with all available CPU cores. This is
    /// faster than a pool of sessions because ONNX Runtime already parallelises
    /// inference internally; multiple sessions just create thread-pool
    /// oversubscription.
    ///
    /// # Errors
    ///
    /// Returns an error if the fastembed model cannot be initialized.
    pub fn try_new() -> Result<Self> {
        let embedder = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2).with_show_download_progress(false),
        )?;
        Ok(Self {
            embedder: Mutex::new(embedder),
        })
    }

    fn cosine(a: &[f32], b: &[f32]) -> f64 {
        let mut dot = 0.0_f64;
        let mut norm_a = 0.0_f64;
        let mut norm_b = 0.0_f64;
        for (va, vb) in a.iter().zip(b.iter()) {
            let a_val = f64::from(*va);
            let b_val = f64::from(*vb);
            dot += a_val * b_val;
            norm_a += a_val * a_val;
            norm_b += b_val * b_val;
        }
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        let raw = dot / (norm_a.sqrt() * norm_b.sqrt());
        raw.clamp(-1.0, 1.0)
    }
}

#[async_trait]
impl SimilarityMetric for CosineSimilarity {
    async fn compute(&self, predicted: &str, golden: &str) -> Result<f64> {
        let mut guard = self
            .embedder
            .lock()
            .map_err(|e| anyhow::anyhow!("fastembed mutex poisoned: {e}"))?;
        let embeddings = guard.embed(vec![predicted, golden], None)?;
        drop(guard);
        if embeddings.len() < 2 {
            anyhow::bail!("expected at least 2 embeddings from fastembed");
        }
        let score = Self::cosine(&embeddings[0], &embeddings[1]);
        // Shift from [-1, 1] to [0, 1] for easier interpretation
        Ok(f64::midpoint(score, 1.0))
    }
}
