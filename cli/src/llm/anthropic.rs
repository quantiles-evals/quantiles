use anyhow::{Context, Result};
use async_trait::async_trait;

use super::LLMSampler;

// TODO: Make Anthropic configuration fully explicit.
//
// Currently we rely on `genai`'s default `Client` and namespaced model inference
// (`anthropic::claude-...`) to route to the Anthropic adapter. This works for
// standard models with `ANTHROPIC_API_KEY` in the environment, but does NOT support:
//   - Custom base URLs (Anthropic-compatible proxies)
//   - Per-benchmark API keys
//   - Alternative API key environment variable names
//   - Explicit adapter binding without inference
//
// When we need those features, refactor this to use `genai::ClientBuilder` with:
//   - `.with_adapter_kind(AdapterKind::Anthropic)`
//   - `.with_auth_resolver()` or `.with_auth_resolver_fn()` for explicit auth
//   - `ProviderConfig` with `Endpoint` + `AuthData` for custom URLs/keys
//   - Consider extending `Sampler::Anthropic` variant with fields:
//       base_url: Option<String>,
//       api_key_env: Option<String>,
//       api_key: Option<String>,
//   - Update `sampler.rs` deserialization to accept table fields via TOML:
//       model = { type = "anthropic:claude-...", base_url = "...", api_key_env = "..." }

/// Sampler that forwards prompts to `Anthropic` (or compatible) endpoints via the
/// `genai` crate. The API key is read from the `ANTHROPIC_API_KEY` environment
/// variable by the underlying `genai::Client`.
pub struct AnthropicSampler {
    model_id: String,
    client: genai::Client,
}

impl AnthropicSampler {
    /// Create a new sampler for the given model identifier (e.g. `claude-haiku-4-5-20251001`).
    #[must_use]
    pub fn new(model_id: &str) -> Self {
        Self {
            model_id: model_id.to_string(),
            client: genai::Client::default(),
        }
    }
}

#[async_trait]
impl LLMSampler for AnthropicSampler {
    async fn sample(&self, prompt: &str) -> Result<String> {
        let chat_req = genai::chat::ChatRequest::from_user(prompt);

        let namespaced = format!("anthropic::{}", self.model_id);
        let response = self
            .client
            .exec_chat(&namespaced, chat_req, None)
            .await
            .context("Anthropic API request failed")?;

        response
            .into_first_text()
            .context("Anthropic response contained no text")
    }
}
