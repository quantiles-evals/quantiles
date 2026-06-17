use anyhow::{Context, Result};
use async_trait::async_trait;

use super::LLMSampler;

// TODO: Make Gemini configuration fully explicit.
//
// Currently we rely on `genai`'s default `Client` and namespaced model inference
// (`gemini::gemini-...`) to route to the Gemini adapter. This works for the
// public Gemini API with `GEMINI_API_KEY` in the environment, but does NOT support:
//   - Google Cloud Vertex AI (which uses ADC / service-account auth)
//   - Per-benchmark API keys
//   - Custom base URLs
//
// For Google Cloud Vertex AI with Application Default Credentials or service
// accounts, use the `vertex:` provider prefix (e.g. `vertex:gemini-1.5-flash`),
// which routes through genai's Vertex adapter and uses the `gcp_auth` crate
// for credential resolution.
//
// When we need first-class config support for these auth modes, refactor this to
// use `genai::ClientBuilder` with:
//   - `.with_adapter_kind(AdapterKind::Gemini)` or `.with_adapter_kind(AdapterKind::Vertex)`
//   - `.with_auth_resolver()` or `.with_auth_resolver_fn()` for explicit auth
//   - `ProviderConfig` with `Endpoint` + `AuthData` for custom URLs/keys
//   - Consider extending `Sampler::Gemini` variant with fields:
//       base_url: Option<String>,
//       api_key_env: Option<String>,
//       api_key: Option<String>,
//       use_vertex: bool,  // to force Vertex adapter instead of Gemini adapter
//   - Update `sampler.rs` deserialization to accept table fields via TOML:
//       model = { type = "gemini:gemini-...", api_key_env = "..." }

/// Sampler that forwards prompts to the Gemini API via the `genai` crate.
///
/// By default this uses the public Gemini API. The API key is read from the
/// `GEMINI_API_KEY` environment variable by the underlying `genai::Client`.
pub struct GeminiSampler {
    model_id: String,
    client: genai::Client,
}

impl GeminiSampler {
    /// Create a new sampler for the given model identifier
    /// (e.g. `gemini-1.5-flash`).
    #[must_use]
    pub fn new(model_id: &str) -> Self {
        Self {
            model_id: model_id.to_string(),
            client: genai::Client::default(),
        }
    }
}

#[async_trait]
impl LLMSampler for GeminiSampler {
    async fn sample(&self, prompt: &str) -> Result<String> {
        let chat_req = genai::chat::ChatRequest::from_user(prompt);

        let namespaced = format!("gemini::{}", self.model_id);
        let response = self
            .client
            .exec_chat(&namespaced, chat_req, None)
            .await
            .context("Gemini API request failed")?;

        response
            .into_first_text()
            .context("Gemini response contained no text")
    }
}
