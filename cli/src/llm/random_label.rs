use anyhow::Result;
use async_trait::async_trait;
use rand::seq::SliceRandom;
use rand::thread_rng;

use super::LLMSampler;

/// Fake sampler that returns one valid label from a list of candidates.
///
/// Commonly used with QA-style benchmarks. For example, `PubMedQA` uses
/// labels "yes", "no" and "maybe"
pub struct RandomLabelSampler {
    labels: Vec<String>,
}

impl RandomLabelSampler {
    #[must_use]
    pub(crate) fn new(labels: &[&str]) -> Self {
        Self {
            labels: labels.iter().map(|&s| s.to_string()).collect(),
        }
    }
}

#[async_trait]
impl LLMSampler for RandomLabelSampler {
    async fn sample(&self, _prompt: &str) -> Result<String> {
        Ok(self
            .labels
            .choose(&mut thread_rng())
            .cloned()
            .unwrap_or_else(|| "maybe".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn random_label_sampler_returns_valid_pubmedqa_label() {
        let sampler = RandomLabelSampler::new(&["yes", "no", "maybe"]);

        for _ in 0..100 {
            let label = sampler.sample("prompt").await.unwrap();
            assert!(
                matches!(label.as_str(), "yes" | "no" | "maybe"),
                "unexpected label: {label}"
            );
        }
    }
}
