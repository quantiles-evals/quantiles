use anyhow::Result;
use serde::{Deserialize, Deserializer};

use crate::llm::Sampler;

/// Style of a no-code custom benchmark.
///
/// Future variants may include:
/// - `Judge` – evaluate responses against a rubric using a judge model.
/// - `Similarity` – score responses with a similarity metric.
/// - `Agent` – run multi-step agent loops.
#[derive(Debug, Clone)]
pub enum CustomNoCodeStyle {
    /// A qa-style benchmark. These benchmarks generally have datasets with
    /// a prompt and a golden answer, which the model under test must exactly
    /// match (modulo minor whitespace and other cleanup) for it to "pass"
    /// the sample
    Qa,
}

impl<'de> Deserialize<'de> for CustomNoCodeStyle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "qa" => Ok(CustomNoCodeStyle::Qa),
            other => Err(serde::de::Error::custom(format!(
                "unsupported custom_nocode style `{other}`; expected `qa`",
            ))),
        }
    }
}

/// QA-specific configuration for a no-code custom benchmark.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct CustomNoCodeQaConfig {
    /// Path to a Jinja template file for rendering prompts.
    pub prompt_template_file: String,
    /// Dataset column containing the prompt text.
    pub prompt_column: String,
    /// Dataset column containing the golden answer.
    pub golden_column: String,
    /// Number of dataset rows to evaluate.
    pub limit: Option<usize>,
    /// Maximum concurrent workers.
    pub max_workers: Option<usize>,
}

/// No-code custom benchmark configuration.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct CustomNoCodeBenchmarkConfig {
    #[serde(rename = "type")]
    pub type_: String,
    pub style: CustomNoCodeStyle,
    /// Dataset identifier in `HuggingFace` (e.g. `quantiles/simpleqa-verified`).
    pub dataset: String,
    /// Model sampler to use.
    pub model: Option<Sampler>,
    #[serde(flatten)]
    pub qa: CustomNoCodeQaConfig,
}

#[cfg(test)]
mod tests {
    use super::super::{BenchmarkConfig, WorkspaceConfig};
    use super::*;

    #[test]
    fn deserialize_custom_nocode_qa() {
        let toml = r#"
            [benchmarks.nocode_custom]
            type = "custom_nocode"
            style = "qa"
            dataset = "quantiles/simpleqa-verified"
            model = "random"
            prompt_template_file = "prompts/qa.txt"
            prompt_column = "problem"
            golden_column = "answer"
            limit = 10
        "#;
        let config: WorkspaceConfig = toml::from_str(toml).unwrap();
        let bench = config.benchmarks.get("nocode_custom").unwrap();
        assert!(matches!(bench, BenchmarkConfig::CustomNoCode(_)));
        if let BenchmarkConfig::CustomNoCode(c) = bench {
            assert!(matches!(c.style, CustomNoCodeStyle::Qa));
            assert_eq!(c.dataset, "quantiles/simpleqa-verified");
            assert_eq!(c.model, Some(Sampler::Random));
            assert_eq!(c.qa.prompt_template_file, "prompts/qa.txt");
            assert_eq!(c.qa.prompt_column, "problem");
            assert_eq!(c.qa.golden_column, "answer");
            assert_eq!(c.qa.limit, Some(10));
        }
    }

    #[test]
    fn deserialize_custom_nocode_unsupported_style_errors() {
        let toml = r#"
            [benchmarks.nocode_custom]
            type = "custom_nocode"
            style = "judge"
            dataset = "quantiles/simpleqa-verified"
            model = "random"
            prompt_template_file = "prompts/qa.txt"
            prompt_column = "problem"
            golden_column = "answer"
        "#;
        let result: Result<WorkspaceConfig, _> = toml::from_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn custom_nocode_missing_required_field_errors() {
        let toml = r#"
            [benchmarks.nocode_custom]
            type = "custom_nocode"
            style = "qa"
            dataset = "quantiles/simpleqa-verified"
            model = "random"
            prompt_column = "problem"
            golden_column = "answer"
        "#;
        let result: Result<WorkspaceConfig, _> = toml::from_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn validate_rejects_missing_template_file() {
        let bench = BenchmarkConfig::CustomNoCode(CustomNoCodeBenchmarkConfig {
            type_: "custom_nocode".to_owned(),
            style: CustomNoCodeStyle::Qa,
            dataset: "quantiles/simpleqa-verified".to_owned(),
            model: Some(Sampler::Random),
            qa: CustomNoCodeQaConfig {
                prompt_template_file: "does_not_exist.txt".to_owned(),
                prompt_column: "problem".to_owned(),
                golden_column: "answer".to_owned(),
                limit: None,
                max_workers: None,
            },
        });
        let err = bench.validate().unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn validate_accepts_existing_template_file() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let bench = BenchmarkConfig::CustomNoCode(CustomNoCodeBenchmarkConfig {
            type_: "custom_nocode".to_owned(),
            style: CustomNoCodeStyle::Qa,
            dataset: "quantiles/simpleqa-verified".to_owned(),
            model: Some(Sampler::Random),
            qa: CustomNoCodeQaConfig {
                prompt_template_file: file.path().to_str().unwrap().to_owned(),
                prompt_column: "problem".to_owned(),
                golden_column: "answer".to_owned(),
                limit: None,
                max_workers: None,
            },
        });
        bench.validate().unwrap();
    }
}
