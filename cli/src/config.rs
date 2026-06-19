use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::llm::Sampler;

/// Type of benchmark execution.
#[derive(Debug, Default, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkType {
    /// Native builtin eval implemented in the CLI.
    #[default]
    Builtin,
    /// External custom eval run as a child process.
    CustomCode,
}

/// Configuration for a single benchmark.
#[derive(Debug, Deserialize, Default)]
pub struct BenchmarkConfig {
    /// Execution type. Defaults to `builtin`.
    #[serde(default, rename = "type")]
    pub type_: BenchmarkType,
    /// Number of samples (rows) to evaluate. (builtin only)
    pub samples: Option<usize>,
    /// Which model sampler to use for this benchmark. (builtin only)
    pub model: Option<Sampler>,
    /// Maximum concurrent workers for this benchmark. (builtin only)
    pub max_workers: Option<usize>,
    /// Command and arguments to execute. (`custom_code` only)
    pub command: Option<Vec<String>>,
    /// Structured input object passed to the eval. (`custom_code` only)
    pub input: Option<serde_json::Value>,
}

impl BenchmarkConfig {
    /// Validate that the benchmark config fields are consistent with its `type`.
    ///
    /// # Errors
    ///
    /// Returns an error when a required field is missing or a disallowed field is present.
    pub fn validate(&self) -> Result<()> {
        match self.type_ {
            BenchmarkType::Builtin => {
                if self.command.is_some() {
                    bail!("builtin benchmark config cannot have a `command` field");
                }
                if self.input.is_some() {
                    bail!("builtin benchmark config cannot have an `input` field");
                }
            }
            BenchmarkType::CustomCode => {
                if self.command.as_ref().is_none_or(Vec::is_empty) {
                    bail!(
                        "custom_code benchmark config must have a non-empty `command` field"
                    );
                }
                if let Some(ref input) = self.input
                    && !input.is_object()
                {
                    bail!(
                        "custom_code benchmark config `input` must be a JSON object / TOML table"
                    );
                }
            }
        }
        Ok(())
    }
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
