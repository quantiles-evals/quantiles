use anyhow::Result;
use async_trait::async_trait;
use strsim::levenshtein;

use super::SimilarityMetric;

/// Levenshtein-based similarity that returns a normalized score in [0, 1].
pub struct LevenshteinSimilarity;

#[async_trait]
impl SimilarityMetric for LevenshteinSimilarity {
    async fn compute(&self, predicted: &str, golden: &str) -> Result<f64> {
        let distance = levenshtein(predicted, golden);
        let max_len = predicted.chars().count().max(golden.chars().count());
        if max_len == 0 {
            return Ok(1.0);
        }
        #[expect(clippy::cast_precision_loss)]
        let score = 1.0 - (distance as f64 / max_len as f64);
        Ok(score.clamp(0.0, 1.0))
    }
}
