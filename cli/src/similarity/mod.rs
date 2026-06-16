use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Abstract trait for comparing two strings and returning a similarity score.
#[async_trait]
pub trait SimilarityMetric: Send + Sync {
    /// Compute similarity between `predicted` and `golden`.
    /// Higher values mean more similar.
    async fn compute(&self, predicted: &str, golden: &str) -> Result<f64>;
}

/// Supported similarity metric names for builtin configuration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SimilarityMetricName {
    /// Cosine similarity using text embeddings.
    #[default]
    Cosine,
    /// Levenshtein (edit-distance) similarity.
    Levenshtein,
}

impl std::fmt::Display for SimilarityMetricName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cosine => write!(f, "cosine"),
            Self::Levenshtein => write!(f, "levenshtein"),
        }
    }
}

pub mod levenshtein;
pub mod vector;
