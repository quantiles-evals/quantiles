use std::fmt;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, Serializer};

use super::{LLMSampler, anthropic, cloudflare, gemini, openai, random, random_label};

/// Supported sampler backends, selectable per-benchmark in the config file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sampler {
    /// Generic random-alphanumeric dummy sampler.
    Random,
    /// Random yes/no/maybe dummy sampler (used by `PubMedQA`).
    RandomLabel,
    /// Cloudflare AI Gateway (Workers AI).
    CloudflareAIGateway {
        /// Workers AI model identifier (e.g. `@cf/moonshotai/kimi-k2.6`).
        model_id: String,
        /// Cloudflare account id.
        account_id: String,
        /// AI Gateway name.
        gateway_id: String,
    },
    /// `OpenAI` API
    ///
    /// TODO: support custom endpoints
    OpenAI {
        /// `OpenAI` model identifier (e.g. `gpt-5.6`).
        model_id: String,
    },
    /// Anthropic API
    Anthropic {
        /// Anthropic model identifier (e.g. `claude-haiku-4-5-20251001`).
        model_id: String,
    },
    /// Google Gemini API via Google's AI Studio (<https://ai.dev>)
    Gemini {
        /// Gemini model identifier (e.g. `gemini-1.5-flash`).
        model_id: String,
    },
}

impl Sampler {
    /// Resolve a [`Sampler`] enum into a concrete [`LLMSampler`] implementation.
    ///
    /// # Errors
    ///
    /// Returns an error if the required environment variables are missing to convert `self`
    /// to an actual `LLMSampler` (e.g. `self` is a `Sampler::CloudflareAIGateway` variant
    /// and the `CLOUDFLARE_API_KEY` env var is missing).
    pub fn resolve(&self) -> Result<Arc<dyn LLMSampler>> {
        match self {
            Sampler::Random => Ok(Arc::new(random::RandomSampler::new(80))),
            Sampler::RandomLabel => Ok(Arc::new(random_label::RandomLabelSampler::new(&[
                "yes", "no", "maybe",
            ]))),
            Sampler::CloudflareAIGateway {
                model_id,
                account_id,
                gateway_id,
            } => {
                let api_key = std::env::var("CLOUDFLARE_API_KEY")
                    .with_context(|| "CLOUDFLARE_API_KEY must be set")?;
                Ok(Arc::new(cloudflare::CloudflareAIGateway::new(
                    model_id, account_id, gateway_id, &api_key,
                )))
            }
            Sampler::OpenAI { model_id } => Ok(Arc::new(openai::OpenAISampler::new(model_id))),
            Sampler::Anthropic { model_id } => {
                Ok(Arc::new(anthropic::AnthropicSampler::new(model_id)))
            }
            Sampler::Gemini { model_id } => Ok(Arc::new(gemini::GeminiSampler::new(model_id))),
        }
    }
}

impl fmt::Display for Sampler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Sampler::Random => write!(f, "random"),
            Sampler::RandomLabel => write!(f, "random_label"),
            Sampler::CloudflareAIGateway { model_id, .. } => {
                write!(f, "cloudflare_ai_gateway:{model_id}")
            }
            Sampler::OpenAI { model_id } => {
                write!(f, "openai:{model_id}")
            }
            Sampler::Anthropic { model_id } => {
                write!(f, "anthropic:{model_id}")
            }
            Sampler::Gemini { model_id } => {
                write!(f, "gemini:{model_id}")
            }
        }
    }
}

impl Serialize for Sampler {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Sampler::Random => serializer.serialize_str("random"),
            Sampler::RandomLabel => serializer.serialize_str("random_label"),
            Sampler::CloudflareAIGateway {
                model_id,
                account_id,
                gateway_id,
            } => {
                let mut map = serializer.serialize_map(Some(3))?;
                map.serialize_entry("type", &format!("cloudflare_ai_gateway:{model_id}"))?;
                map.serialize_entry("account_id", account_id)?;
                map.serialize_entry("gateway_id", gateway_id)?;
                map.end()
            }
            Sampler::OpenAI { model_id } => serializer.serialize_str(&format!("openai:{model_id}")),
            Sampler::Anthropic { model_id } => {
                serializer.serialize_str(&format!("anthropic:{model_id}"))
            }
            Sampler::Gemini { model_id } => serializer.serialize_str(&format!("gemini:{model_id}")),
        }
    }
}

impl<'de> Deserialize<'de> for Sampler {
    #[expect(clippy::too_many_lines)]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SamplerVisitor;

        impl<'de> Visitor<'de> for SamplerVisitor {
            type Value = Sampler;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(
                    "a string like \"random\" or \"provider:model_id\", \
                     or a map with \"type\" field",
                )
            }

            fn visit_str<E>(self, value: &str) -> Result<Sampler, E>
            where
                E: de::Error,
            {
                match value {
                    "random" => Ok(Sampler::Random),
                    "random_label" => Ok(Sampler::RandomLabel),
                    other => {
                        let Some((provider, model_id)) = other.split_once(':') else {
                            return Err(de::Error::invalid_value(
                                de::Unexpected::Str(other),
                                &"expected format \"provider:model_id\"",
                            ));
                        };
                        if model_id.is_empty() {
                            return Err(de::Error::custom("model_id cannot be empty"));
                        }
                        match provider {
                            "cloudflare_ai_gateway" => {
                                let account_id =
                                    std::env::var("CLOUDFLARE_ACCOUNT_ID").map_err(|e| {
                                        de::Error::custom(format!(
                                            "CLOUDFLARE_ACCOUNT_ID must be set: {e}"
                                        ))
                                    })?;
                                let gateway_id =
                                    std::env::var("CLOUDFLARE_GATEWAY_ID").map_err(|e| {
                                        de::Error::custom(format!(
                                            "CLOUDFLARE_GATEWAY_ID must be set: {e}"
                                        ))
                                    })?;
                                Ok(Sampler::CloudflareAIGateway {
                                    model_id: model_id.to_string(),
                                    account_id,
                                    gateway_id,
                                })
                            }
                            "openai" => Ok(Sampler::OpenAI {
                                model_id: model_id.to_string(),
                            }),
                            "anthropic" => Ok(Sampler::Anthropic {
                                model_id: model_id.to_string(),
                            }),
                            "gemini" => Ok(Sampler::Gemini {
                                model_id: model_id.to_string(),
                            }),
                            other => Err(de::Error::unknown_variant(
                                other,
                                &[
                                    "random",
                                    "random_label",
                                    "cloudflare_ai_gateway",
                                    "openai",
                                    "anthropic",
                                    "gemini",
                                ],
                            )),
                        }
                    }
                }
            }

            fn visit_map<M>(self, map: M) -> Result<Sampler, M::Error>
            where
                M: MapAccess<'de>,
            {
                #[derive(Deserialize)]
                struct SamplerTable {
                    #[serde(rename = "type")]
                    model_type: String,
                    account_id: Option<String>,
                    gateway_id: Option<String>,
                }

                let table = SamplerTable::deserialize(de::value::MapAccessDeserializer::new(map))?;

                match table.model_type.as_str() {
                    "random" => Ok(Sampler::Random),
                    "random_label" => Ok(Sampler::RandomLabel),
                    other => {
                        let Some((provider, model_id)) = other.split_once(':') else {
                            return Err(de::Error::invalid_value(
                                de::Unexpected::Str(other),
                                &"expected format \"provider:model_id\"",
                            ));
                        };
                        if model_id.is_empty() {
                            return Err(de::Error::custom("model_id cannot be empty"));
                        }
                        match provider {
                            "cloudflare_ai_gateway" => {
                                let account_id = table
                                    .account_id
                                    .or_else(|| std::env::var("CLOUDFLARE_ACCOUNT_ID").ok())
                                    .ok_or_else(|| {
                                        de::Error::custom(
                                            "CLOUDFLARE_ACCOUNT_ID must be set or provided in config",
                                        )
                                    })?;
                                let gateway_id = table
                                    .gateway_id
                                    .or_else(|| std::env::var("CLOUDFLARE_GATEWAY_ID").ok())
                                    .ok_or_else(|| {
                                        de::Error::custom(
                                            "CLOUDFLARE_GATEWAY_ID must be set or provided in config",
                                        )
                                    })?;
                                Ok(Sampler::CloudflareAIGateway {
                                    model_id: model_id.to_string(),
                                    account_id,
                                    gateway_id,
                                })
                            }
                            "openai" => Ok(Sampler::OpenAI {
                                model_id: model_id.to_string(),
                            }),
                            "anthropic" => Ok(Sampler::Anthropic {
                                model_id: model_id.to_string(),
                            }),
                            "gemini" => Ok(Sampler::Gemini {
                                model_id: model_id.to_string(),
                            }),
                            other => Err(de::Error::unknown_variant(
                                other,
                                &[
                                    "random",
                                    "random_label",
                                    "cloudflare_ai_gateway",
                                    "openai",
                                    "anthropic",
                                    "gemini",
                                ],
                            )),
                        }
                    }
                }
            }
        }

        deserializer.deserialize_any(SamplerVisitor)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// Serialize tests that mutate the process environment to avoid race
    /// conditions when running tests concurrently.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn deserialize_random_string() {
        let s: Sampler = serde_json::from_str("\"random\"").unwrap();
        assert_eq!(s, Sampler::Random);
    }

    #[test]
    fn deserialize_random_label_string() {
        let s: Sampler = serde_json::from_str("\"random_label\"").unwrap();
        assert_eq!(s, Sampler::RandomLabel);
    }

    #[test]
    fn deserialize_cloudflare_string_fails_without_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::remove_var("CLOUDFLARE_ACCOUNT_ID");
            std::env::remove_var("CLOUDFLARE_GATEWAY_ID");
        }
        let result: Result<Sampler, _> = serde_json::from_str("\"cloudflare_ai_gateway:@cf/test\"");
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_cloudflare_table_with_explicit_fields() {
        let s: Sampler = serde_json::from_str(
            r#"{"type":"cloudflare_ai_gateway:@cf/test","account_id":"acc123","gateway_id":"gate456"}"#,
        )
        .unwrap();
        assert_eq!(
            s,
            Sampler::CloudflareAIGateway {
                model_id: "@cf/test".to_string(),
                account_id: "acc123".to_string(),
                gateway_id: "gate456".to_string(),
            }
        );
    }

    #[test]
    fn deserialize_cloudflare_table_falls_back_to_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::set_var("CLOUDFLARE_ACCOUNT_ID", "env_acc");
            std::env::set_var("CLOUDFLARE_GATEWAY_ID", "env_gate");
        }
        let s: Sampler =
            serde_json::from_str(r#"{"type":"cloudflare_ai_gateway:@cf/test"}"#).unwrap();
        assert_eq!(
            s,
            Sampler::CloudflareAIGateway {
                model_id: "@cf/test".to_string(),
                account_id: "env_acc".to_string(),
                gateway_id: "env_gate".to_string(),
            }
        );
        unsafe {
            std::env::remove_var("CLOUDFLARE_ACCOUNT_ID");
            std::env::remove_var("CLOUDFLARE_GATEWAY_ID");
        }
    }

    #[test]
    fn deserialize_cloudflare_table_fails_when_env_missing() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::remove_var("CLOUDFLARE_ACCOUNT_ID");
            std::env::remove_var("CLOUDFLARE_GATEWAY_ID");
        }
        let result: Result<Sampler, _> =
            serde_json::from_str(r#"{"type":"cloudflare_ai_gateway:@cf/test"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_unknown_provider_errors() {
        let result: Result<Sampler, _> = serde_json::from_str("\"unknown:model\"");
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_missing_colon_errors() {
        let result: Result<Sampler, _> = serde_json::from_str("\"cloudflare_ai_gateway\"");
        assert!(result.is_err());
    }

    #[test]
    fn serialize_random() {
        let json = serde_json::to_string(&Sampler::Random).unwrap();
        assert_eq!(json, "\"random\"");
    }

    #[test]
    fn serialize_random_label() {
        let json = serde_json::to_string(&Sampler::RandomLabel).unwrap();
        assert_eq!(json, "\"random_label\"");
    }

    #[test]
    fn serialize_cloudflare() {
        let s = Sampler::CloudflareAIGateway {
            model_id: "@cf/test".to_string(),
            account_id: "acc".to_string(),
            gateway_id: "gate".to_string(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let expected =
            r#"{"type":"cloudflare_ai_gateway:@cf/test","account_id":"acc","gateway_id":"gate"}"#;
        assert_eq!(json, expected);
    }

    #[test]
    fn display_random() {
        assert_eq!(Sampler::Random.to_string(), "random");
    }

    #[test]
    fn display_cloudflare() {
        let s = Sampler::CloudflareAIGateway {
            model_id: "@cf/test".to_string(),
            account_id: "acc".to_string(),
            gateway_id: "gate".to_string(),
        };
        assert_eq!(s.to_string(), "cloudflare_ai_gateway:@cf/test");
    }

    #[test]
    fn deserialize_openai_string() {
        let s: Sampler = serde_json::from_str("\"openai:gpt-5.6\"").unwrap();
        assert_eq!(
            s,
            Sampler::OpenAI {
                model_id: "gpt-5.6".to_string(),
            }
        );
    }

    #[test]
    fn deserialize_openai_table() {
        let s: Sampler = serde_json::from_str(r#"{"type":"openai:gpt-5.6"}"#).unwrap();
        assert_eq!(
            s,
            Sampler::OpenAI {
                model_id: "gpt-5.6".to_string(),
            }
        );
    }

    #[test]
    fn serialize_openai() {
        let s = Sampler::OpenAI {
            model_id: "gpt-5.6".to_string(),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"openai:gpt-5.6\"");
    }

    #[test]
    fn display_openai() {
        let s = Sampler::OpenAI {
            model_id: "gpt-5.6".to_string(),
        };
        assert_eq!(s.to_string(), "openai:gpt-5.6");
    }

    #[test]
    fn deserialize_anthropic_string() {
        let s: Sampler = serde_json::from_str("\"anthropic:claude-3-5-haiku\"").unwrap();
        assert_eq!(
            s,
            Sampler::Anthropic {
                model_id: "claude-3-5-haiku".to_string(),
            }
        );
    }

    #[test]
    fn deserialize_anthropic_table() {
        let s: Sampler = serde_json::from_str(r#"{"type":"anthropic:claude-3-5-haiku"}"#).unwrap();
        assert_eq!(
            s,
            Sampler::Anthropic {
                model_id: "claude-3-5-haiku".to_string(),
            }
        );
    }

    #[test]
    fn serialize_anthropic() {
        let s = Sampler::Anthropic {
            model_id: "claude-3-5-haiku".to_string(),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"anthropic:claude-3-5-haiku\"");
    }

    #[test]
    fn display_anthropic() {
        let s = Sampler::Anthropic {
            model_id: "claude-3-5-haiku".to_string(),
        };
        assert_eq!(s.to_string(), "anthropic:claude-3-5-haiku");
    }

    #[test]
    fn deserialize_gemini_string() {
        let s: Sampler = serde_json::from_str("\"gemini:gemini-1.5-flash\"").unwrap();
        assert_eq!(
            s,
            Sampler::Gemini {
                model_id: "gemini-1.5-flash".to_string(),
            }
        );
    }

    #[test]
    fn deserialize_gemini_table() {
        let s: Sampler = serde_json::from_str(r#"{"type":"gemini:gemini-1.5-flash"}"#).unwrap();
        assert_eq!(
            s,
            Sampler::Gemini {
                model_id: "gemini-1.5-flash".to_string(),
            }
        );
    }

    #[test]
    fn serialize_gemini() {
        let s = Sampler::Gemini {
            model_id: "gemini-1.5-flash".to_string(),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"gemini:gemini-1.5-flash\"");
    }

    #[test]
    fn display_gemini() {
        let s = Sampler::Gemini {
            model_id: "gemini-1.5-flash".to_string(),
        };
        assert_eq!(s.to_string(), "gemini:gemini-1.5-flash");
    }
}
