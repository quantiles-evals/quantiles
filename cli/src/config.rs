use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::llm::Sampler;

/// Configuration for a single benchmark.
#[derive(Debug, Deserialize, Default)]
pub struct BenchmarkConfig {
    /// Number of samples (rows) to evaluate.
    pub samples: Option<usize>,
    /// Which model sampler to use for this benchmark.
    pub model: Option<Sampler>,
    /// Maximum concurrent workers for this benchmark.
    pub max_workers: Option<usize>,
}

/// Top-level workspace configuration read from `quantiles.toml` or
/// `.quantiles.toml` in the current working directory.
#[derive(Debug, Deserialize, Default)]
pub struct WorkspaceConfig {
    /// Per-benchmark overrides keyed by the builtin workflow name.
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
