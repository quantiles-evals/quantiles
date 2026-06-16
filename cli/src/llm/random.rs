use anyhow::Result;
use async_trait::async_trait;
use rand::distributions::Alphanumeric;
use rand::{Rng, thread_rng};

use super::LLMSampler;

/// A dummy sampler that returns random alphanumeric characters.
pub struct RandomSampler {
    pub max_length: usize,
}

impl RandomSampler {
    /// Create a new random sampler with the given maximum output length.
    #[must_use]
    pub fn new(max_length: usize) -> Self {
        Self { max_length }
    }
}

#[async_trait]
impl LLMSampler for RandomSampler {
    async fn sample(&self, _prompt: &str) -> Result<String> {
        let mut rng = thread_rng();
        let len = rng.gen_range(1..=self.max_length);
        let s: String = (0..len).map(|_| rng.sample(Alphanumeric) as char).collect();
        Ok(s)
    }
}
