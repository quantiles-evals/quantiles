use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Deserializer};

use crate::llm::Sampler;

/// Configuration for a single benchmark.
///
/// Exactly one of the variants is deserialized based on the `type` field:
/// `builtin` (default when absent), `custom_code`, or `custom_nocode`.
#[derive(Debug, Clone)]
pub enum BenchmarkConfig {
    Builtin(BuiltinBenchmarkConfig),
    CustomCode(CustomCodeBenchmarkConfig),
    CustomNoCode(Box<CustomNoCodeBenchmarkConfig>),
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
            BenchmarkConfig::CustomNoCode(c) => {
                if !std::path::Path::new(&c.qa.prompt_template_file).is_file() {
                    bail!(
                        "custom_nocode benchmark config `prompt_template_file` must point to an existing file. File `{}` was not found",
                        c.qa.prompt_template_file
                    );
                }
                c.qa.validate()?;
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
            Some("custom_nocode") => {
                let config = CustomNoCodeBenchmarkConfig::deserialize(value).map_err(|e| {
                    serde::de::Error::custom(format!(
                        "failed to deserialize custom_nocode benchmark config: {e}"
                    ))
                })?;
                Ok(BenchmarkConfig::CustomNoCode(Box::new(config)))
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
                "invalid benchmark type `{other}`; expected `builtin`, `custom_code`, or `custom_nocode`",
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

mod custom_nocode;
pub use custom_nocode::*;

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

#[cfg(test)]
#[expect(clippy::needless_raw_string_hashes)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_builtin_without_type() {
        let toml = r#"
            [benchmarks.demo]
            samples = 10
        "#;
        let config: WorkspaceConfig = toml::from_str(toml).unwrap();
        let bench = config.benchmarks.get("demo").unwrap();
        assert!(matches!(bench, BenchmarkConfig::Builtin(_)));
        if let BenchmarkConfig::Builtin(b) = bench {
            assert_eq!(b.type_, "builtin");
            assert_eq!(b.samples, Some(10));
            assert!(b.model.is_none());
        }
    }

    #[test]
    fn deserialize_builtin_with_explicit_type() {
        let toml = r#"
            [benchmarks.demo]
            type = "builtin"
            samples = 5
            model = "openai:gpt-4"
        "#;
        let config: WorkspaceConfig = toml::from_str(toml).unwrap();
        let bench = config.benchmarks.get("demo").unwrap();
        assert!(matches!(bench, BenchmarkConfig::Builtin(_)));
    }

    #[test]
    fn deserialize_custom_code() {
        let toml = r#"
            [benchmarks.my-eval]
            type = "custom_code"
            command = ["python", "eval.py"]

            [benchmarks.my-eval.input]
            foo = "bar"
        "#;
        let config: WorkspaceConfig = toml::from_str(toml).unwrap();
        let bench = config.benchmarks.get("my-eval").unwrap();
        assert!(matches!(bench, BenchmarkConfig::CustomCode(_)));
        if let BenchmarkConfig::CustomCode(c) = bench {
            assert_eq!(c.command, vec!["python", "eval.py"]);
            assert!(c.input.is_some());
            assert_eq!(
                c.input.as_ref().unwrap().get("foo").unwrap().as_str(),
                Some("bar")
            );
        }
    }

    #[test]
    fn deserialize_custom_code_without_input() {
        let toml = r#"
            [benchmarks.my-eval]
            type = "custom_code"
            command = ["sh", "-c", "echo hello"]
        "#;
        let config: WorkspaceConfig = toml::from_str(toml).unwrap();
        let bench = config.benchmarks.get("my-eval").unwrap();
        assert!(matches!(bench, BenchmarkConfig::CustomCode(_)));
        if let BenchmarkConfig::CustomCode(c) = bench {
            assert!(c.input.is_none());
        }
    }

    #[test]
    fn builtin_rejects_command_field() {
        let toml = r#"
            [benchmarks.demo]
            type = "builtin"
            command = ["echo", "hello"]
        "#;
        let result: Result<WorkspaceConfig, _> = toml::from_str(toml);
        assert!(result.is_err(), "builtin should reject command field");
    }

    #[test]
    fn builtin_rejects_input_field() {
        let toml = r#"
            [benchmarks.demo]
            type = "builtin"

            [benchmarks.demo.input]
            foo = "bar"
        "#;
        let result: Result<WorkspaceConfig, _> = toml::from_str(toml);
        assert!(result.is_err(), "builtin should reject input field");
    }

    #[test]
    fn custom_code_rejects_samples_field() {
        let toml = r#"
            [benchmarks.my-eval]
            type = "custom_code"
            command = ["echo"]
            samples = 10
        "#;
        let result: Result<WorkspaceConfig, _> = toml::from_str(toml);
        assert!(result.is_err(), "custom_code should reject samples field");
    }

    #[test]
    fn validate_rejects_empty_command() {
        let bench = BenchmarkConfig::CustomCode(CustomCodeBenchmarkConfig {
            type_: "custom_code".to_owned(),
            command: vec![],
            input: None,
        });
        let err = bench.validate().unwrap_err();
        assert!(err.to_string().contains("non-empty `command`"));
    }

    #[test]
    fn validate_accepts_nonempty_command() {
        let bench = BenchmarkConfig::CustomCode(CustomCodeBenchmarkConfig {
            type_: "custom_code".to_owned(),
            command: vec!["python".to_owned(), "eval.py".to_owned()],
            input: None,
        });
        bench.validate().unwrap();
    }

    #[test]
    fn invalid_type_errors() {
        let toml = r#"
            [benchmarks.demo]
            type = "unknown"
        "#;
        let result: Result<WorkspaceConfig, _> = toml::from_str(toml);
        assert!(result.is_err());
    }

    /// `custom_code` benchmarks must have a `command` field; omitting it should fail at
    /// parse time because the struct requires it.
    #[test]
    fn custom_code_missing_command_errors() {
        let toml = r#"
            [benchmarks.my-eval]
            type = "custom_code"
        "#;
        let result: Result<WorkspaceConfig, _> = toml::from_str(toml);
        assert!(result.is_err(), "custom_code should require command field");
    }

    /// Nested TOML tables inside `input` should deserialize into nested JSON objects within
    /// the `HashMap<String, serde_json::Value>`.
    #[test]
    fn custom_code_nested_input_values() {
        let toml = r#"
            [benchmarks.my-eval]
            type = "custom_code"
            command = ["python", "eval.py"]

            [benchmarks.my-eval.input]
            dataset = "foo.jsonl"

            [benchmarks.my-eval.input.nested]
            a = 1
            b = true
        "#;
        let config: WorkspaceConfig = toml::from_str(toml).unwrap();
        let bench = config.benchmarks.get("my-eval").unwrap();
        if let BenchmarkConfig::CustomCode(c) = bench {
            let input = c.input.as_ref().unwrap();
            assert_eq!(input.get("dataset").unwrap().as_str(), Some("foo.jsonl"));
            let nested = input.get("nested").unwrap().as_object().unwrap();
            assert_eq!(nested.get("a").unwrap().as_i64(), Some(1));
            assert_eq!(nested.get("b").unwrap().as_bool(), Some(true));
        } else {
            panic!("expected custom_code config");
        }
    }

    /// An empty `[benchmarks.x.input]` TOML section should deserialize as an empty
    /// `HashMap`, not fail or become `None`.
    #[test]
    fn custom_code_empty_input_table() {
        let toml = r#"
            [benchmarks.my-eval]
            type = "custom_code"
            command = ["echo"]

            [benchmarks.my-eval.input]
        "#;
        let config: WorkspaceConfig = toml::from_str(toml).unwrap();
        let bench = config.benchmarks.get("my-eval").unwrap();
        if let BenchmarkConfig::CustomCode(c) = bench {
            assert!(c.input.is_some());
            assert!(c.input.as_ref().unwrap().is_empty());
        } else {
            panic!("expected custom_code config");
        }
    }
}
