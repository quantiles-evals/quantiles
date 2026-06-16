use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use serde::Serialize;

use super::LLMSampler;

/// Sampler that forwards prompts via the Cloudflare Workers AI REST API,
/// routing requests through an AI Gateway using the `cf-aig-gateway-id` header
/// for observability.
pub struct CloudflareAIGateway {
    model_id: String,
    account_id: String,
    gateway_id: String,
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl CloudflareAIGateway {
    /// Create a new sampler using the production Cloudflare API endpoint.
    #[must_use]
    pub fn new(model_id: &str, account_id: &str, gateway_id: &str, api_key: &str) -> Self {
        Self {
            model_id: model_id.to_string(),
            account_id: account_id.to_string(),
            gateway_id: gateway_id.to_string(),
            api_key: api_key.to_string(),
            base_url: "https://api.cloudflare.com".to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Override the base URL (useful for testing against a mock server).
    #[must_use]
    #[cfg(test)]
    pub(crate) fn with_base_url(mut self, base_url: &str) -> Self {
        self.base_url = base_url.to_string();
        self
    }
}

#[derive(Serialize)]
struct WorkersAIRequest {
    model: String,
    input: WorkersAIInput,
    max_tokens: u32,
}

#[derive(Serialize)]
struct WorkersAIInput {
    messages: Vec<WorkersAIMessage>,
}

#[derive(Serialize)]
struct WorkersAIMessage {
    role: String,
    content: String,
}

/// Extract the generated text from a Cloudflare Workers AI response.
///
/// Different models return the result in different shapes; this function
/// tries the most common locations.
fn extract_response(raw: &serde_json::Value) -> Option<String> {
    // Standard text-generation shape: {"result":{"response":"..."}}
    if let Some(result) = raw.get("result")
        && let Some(response) = result.get("response")
        && let Some(text) = response.as_str()
    {
        return Some(text.to_string());
    }

    // OpenAI-compatible chat shape:
    // {"result":{"choices":[{"message":{"content":"..."}}]}}
    if let Some(result) = raw.get("result")
        && let Some(choices) = result.get("choices")
        && let Some(choices_arr) = choices.as_array()
        && let Some(first) = choices_arr.first()
        && let Some(message) = first.get("message")
        && let Some(content) = message.get("content")
        && let Some(text) = content.as_str()
    {
        return Some(text.to_string());
    }

    // Fallback: raw result string
    if let Some(result) = raw.get("result")
        && let Some(text) = result.as_str()
    {
        return Some(text.to_string());
    }

    None
}

#[async_trait]
impl LLMSampler for CloudflareAIGateway {
    async fn sample(&self, prompt: &str) -> Result<String> {
        let url = format!(
            "{}/client/v4/accounts/{}/ai/run",
            self.base_url, self.account_id
        );

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("cf-aig-gateway-id", &self.gateway_id)
            .header("Content-Type", "application/json")
            .json(&WorkersAIRequest {
                model: self.model_id.clone(),
                input: WorkersAIInput {
                    messages: vec![WorkersAIMessage {
                        role: "user".to_string(),
                        content: prompt.to_string(),
                    }],
                },
                max_tokens: 4096,
            })
            .send()
            .await
            .context("failed to send request to Cloudflare Workers AI")?;

        let status = resp.status();
        let body_text = resp
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable>".to_string());

        if !status.is_success() {
            bail!("Cloudflare Workers AI returned {status}: {body_text}");
        }

        let raw: serde_json::Value = serde_json::from_str(&body_text)
            .with_context(|| format!("invalid JSON from Cloudflare Workers AI: {body_text}"))?;

        if let Some(result) = raw.get("result")
            && let Some(choices) = result.get("choices")
            && let Some(choices_arr) = choices.as_array()
            && let Some(first) = choices_arr.first()
            && let Some(finish_reason) = first.get("finish_reason")
            && let Some(reason) = finish_reason.as_str()
            && reason == "length"
        {
            bail!(
                "Cloudflare Workers AI response was truncated (finish_reason=length). \
                 Try increasing max_tokens or shortening the prompt."
            );
        }

        extract_response(&raw).with_context(|| {
            format!(
                "could not find response text in Cloudflare Workers AI output. \
                 Full response: {raw}",
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn sample_returns_result_response() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/client/v4/accounts/acc123/ai/run"))
            .and(body_json(serde_json::json!({
                "model": "@cf/test",
                "input": {
                    "messages": [{"role": "user", "content": "test prompt"}]
                },
                "max_tokens": 4096
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": { "response": "hello from cf" }
            })))
            .mount(&server)
            .await;

        let sampler = CloudflareAIGateway::new("@cf/test", "acc123", "gate456", "fake_key")
            .with_base_url(&server.uri());

        let result = sampler.sample("test prompt").await.unwrap();
        assert_eq!(result, "hello from cf");
    }

    #[tokio::test]
    async fn sample_errors_on_non_2xx() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/client/v4/accounts/acc123/ai/run"))
            .respond_with(ResponseTemplate::new(503).set_body_string("overloaded"))
            .mount(&server)
            .await;

        let sampler = CloudflareAIGateway::new("@cf/test", "acc123", "gate456", "fake_key")
            .with_base_url(&server.uri());

        let result = sampler.sample("test prompt").await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("503"));
        assert!(msg.contains("overloaded"));
    }

    #[tokio::test]
    async fn sample_sends_gateway_header() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/client/v4/accounts/acc123/ai/run"))
            .and(wiremock::matchers::header("cf-aig-gateway-id", "gate456"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": { "response": "ok" }
            })))
            .mount(&server)
            .await;

        let sampler = CloudflareAIGateway::new("@cf/test", "acc123", "gate456", "fake_key")
            .with_base_url(&server.uri());

        sampler.sample("hello").await.unwrap();
    }
}
