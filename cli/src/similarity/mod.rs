use std::sync::Arc;

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

/// Construct one of the similarity metrics built into the CLI.
///
/// # Errors
///
/// Returns an error when the selected metric cannot be initialized, such as when
/// the local FastEmbed-backed cosine model is unavailable.
pub fn build_similarity_metric(name: SimilarityMetricName) -> Result<Arc<dyn SimilarityMetric>> {
    match name {
        SimilarityMetricName::Cosine => Ok(Arc::new(vector::CosineSimilarity::try_new()?)),
        SimilarityMetricName::Levenshtein => Ok(Arc::new(levenshtein::LevenshteinSimilarity)),
    }
}

pub mod levenshtein;
pub mod vector;
