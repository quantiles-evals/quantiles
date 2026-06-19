use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Deserializer};

use crate::llm::Sampler;

/// Configuration for a single benchmark.
///
/// Exactly one of the two variants is deserialized based on the `type` field:
/// `builtin` (default when absent) or `custom_code`.
#[derive(Debug, Clone)]
pub enum BenchmarkConfig {
    Builtin(BuiltinBenchmarkConfig),
    CustomCode(CustomCodeBenchmarkConfig),
}

impl BenchmarkConfig {
    /// Validate post-deserialization constraints.
    ///
    /// # Errors
    ///
    /// Returns an error when a field has an invalid value.
    pub fn validate(&self) -> Result<()> {
        match self {
            BenchmarkConfig::Builtin(_) => Ok(()),
            BenchmarkConfig::CustomCode(c) => {
                if c.command.is_empty() {
                    bail!("custom_code benchmark config must have a non-empty `command` field");
                }
                Ok(())
            }
        }
    }
}

impl<'de> Deserialize<'de> for BenchmarkConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value =
            serde_json::Value::deserialize(deserializer).map_err(serde::de::Error::custom)?;

        let type_str = value.get("type").and_then(|v| v.as_str());

        match type_str {
            Some("custom_code") => {
                let config = CustomCodeBenchmarkConfig::deserialize(value).map_err(|e| {
                    serde::de::Error::custom(format!(
                        "failed to deserialize custom_code benchmark config: {e}"
                    ))
                })?;
                Ok(BenchmarkConfig::CustomCode(config))
            }
            Some("builtin") | None => {
                let config = BuiltinBenchmarkConfig::deserialize(value).map_err(|e| {
                    serde::de::Error::custom(format!(
                        "failed to deserialize builtin benchmark config: {e}"
                    ))
                })?;
                Ok(BenchmarkConfig::Builtin(config))
            }
            Some(other) => Err(serde::de::Error::custom(format!(
                "invalid benchmark type `{other}`; expected `builtin` or `custom_code`",
            ))),
        }
    }
}

/// Built-in benchmark configuration.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct BuiltinBenchmarkConfig {
    #[serde(default = "default_type_builtin", rename = "type")]
    pub type_: String,
    /// Number of samples (rows) to evaluate.
    pub samples: Option<usize>,
    /// Which model sampler to use for this benchmark.
    pub model: Option<Sampler>,
    /// Maximum concurrent workers for this benchmark.
    pub max_workers: Option<usize>,
}

fn default_type_builtin() -> String {
    "builtin".to_owned()
}

/// Custom-code benchmark configuration.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct CustomCodeBenchmarkConfig {
    #[serde(rename = "type")]
    pub type_: String,
    /// Command and arguments to execute.
    pub command: Vec<String>,
    /// Structured input object passed to the eval.
    pub input: Option<HashMap<String, serde_json::Value>>,
}

/// Top-level workspace configuration read from `quantiles.toml` or
/// `.quantiles.toml` in the current working directory.
#[derive(Debug, Deserialize, Default)]
pub struct WorkspaceConfig {
    /// Per-benchmark overrides keyed by the workflow name.
    #[serde(default)]
    pub benchmarks: HashMap<String, BenchmarkConfig>,
}

/// Look for `quantiles.toml` or `.quantiles.toml` in the current working
/// directory and parse it.
///
/// If neither file exists, returns an empty default config.
/// If a file exists but cannot be read or parsed, returns an error so the
/// caller can fail hard.
///
/// # Errors
///
/// Returns an error when the current directory cannot be determined, or when
/// a present config file cannot be read or parsed as TOML.
pub fn load() -> Result<WorkspaceConfig> {
    let cwd = std::env::current_dir().context("failed to determine current directory")?;

    let plain = cwd.join("quantiles.toml");
    let dot_prefix = cwd.join(".quantiles.toml");
    let plain_exists = plain.exists();
    let dot_prefix_exists = dot_prefix.exists();

    if plain_exists && dot_prefix_exists {
        bail!(
            "both `quantiles.toml` and `.quantiles.toml` found in {}. \
             remove one to avoid ambiguity",
            cwd.display()
        );
    }

    let path = if plain_exists {
        plain
    } else if dot_prefix_exists {
        dot_prefix
    } else {
        return Ok(WorkspaceConfig::default());
    };

    let filename = path.file_name().unwrap_or_default().to_string_lossy();
    let contents =
        std::fs::read_to_string(&path).with_context(|| format!("failed to read {filename}"))?;
    let config: WorkspaceConfig =
        toml::from_str(&contents).with_context(|| format!("failed to parse {filename}"))?;
    Ok(config)
}
