use anyhow::Result;
use async_trait::async_trait;

/// Abstract trait for sampling text from a generative model.
#[async_trait]
pub trait LLMSampler: Send + Sync {
    /// Generate a response for the given prompt.
    async fn sample(&self, prompt: &str) -> Result<String>;
}

pub mod anthropic;
pub mod cloudflare;
pub mod gemini;
pub mod openai;
pub mod random;
pub mod random_label;
pub mod sampler;

pub use sampler::Sampler;
