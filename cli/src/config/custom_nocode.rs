use std::collections::HashSet;

use anyhow::{Result, bail};
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
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct CustomNoCodeQaConfig {
    /// Path to a Jinja template file for rendering prompts.
    pub prompt_template_file: String,
    /// Dataset column containing the golden answer label or text.
    pub golden_column: Option<String>,
    /// Dataset column containing a zero- or one-based golden answer index.
    pub golden_index_column: Option<String>,
    /// Base used by `golden_index_column`. Defaults to zero.
    #[serde(default)]
    pub golden_index_base: usize,
    /// Dataset column whose value identifies the correct choice before shuffling.
    pub correct_choice_column: Option<String>,
    /// Dataset column containing choices as an array or object.
    pub choices_column: Option<String>,
    /// Dataset columns containing choices in their original order.
    pub choice_columns: Option<Vec<String>>,
    /// Labels assigned to choices, for example `["A", "B", "C", "D"]`.
    pub choice_labels: Option<Vec<String>>,
    /// Deterministically shuffle choices before rendering the prompt.
    #[serde(default)]
    pub shuffle_choices: bool,
    /// Dataset column used as the stable seed for choice shuffling.
    pub shuffle_seed_column: Option<String>,
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
    /// Optional Hugging Face dataset configuration/subset.
    pub dataset_config: Option<String>,
    /// Optional dataset split. The dataset manager chooses one when omitted.
    pub split: Option<String>,
    /// Optional dataset revision.
    pub revision: Option<String>,
    /// Model sampler to use.
    pub model: Option<Sampler>,
    #[serde(flatten)]
    pub qa: CustomNoCodeQaConfig,
}

impl CustomNoCodeQaConfig {
    /// Validate mutually exclusive answer and choice source fields.
    pub(crate) fn validate(&self) -> Result<()> {
        let answer_sources = [
            self.golden_column.is_some(),
            self.golden_index_column.is_some(),
            self.correct_choice_column.is_some(),
        ]
        .into_iter()
        .filter(|configured| *configured)
        .count();
        if answer_sources != 1 {
            bail!(
                "custom_nocode QA config requires exactly one of `golden_column`, `golden_index_column`, or `correct_choice_column`"
            );
        }

        let choice_sources = [self.choices_column.is_some(), self.choice_columns.is_some()]
            .into_iter()
            .filter(|configured| *configured)
            .count();
        if choice_sources > 1 {
            bail!(
                "custom_nocode QA config accepts only one of `choices_column` or `choice_columns`"
            );
        }

        let is_multiple_choice = choice_sources == 1;
        if (self.golden_index_column.is_some() || self.correct_choice_column.is_some())
            && !is_multiple_choice
        {
            bail!(
                "`golden_index_column` and `correct_choice_column` require a multiple-choice source"
            );
        }
        if is_multiple_choice && self.choice_labels.as_ref().is_none_or(Vec::is_empty) {
            bail!("multiple-choice custom_nocode QA config requires non-empty `choice_labels`");
        }
        if !is_multiple_choice && self.choice_labels.is_some() {
            bail!("`choice_labels` requires `choices_column` or `choice_columns`");
        }
        if self.shuffle_choices && !is_multiple_choice {
            bail!("`shuffle_choices` requires `choices_column` or `choice_columns`");
        }
        if self.shuffle_choices && self.shuffle_seed_column.is_none() {
            bail!("`shuffle_choices = true` requires `shuffle_seed_column`");
        }
        if !self.shuffle_choices && self.shuffle_seed_column.is_some() {
            bail!("`shuffle_seed_column` requires `shuffle_choices = true`");
        }
        if self.golden_index_column.is_none() && self.golden_index_base != 0 {
            bail!("`golden_index_base` requires `golden_index_column`");
        }

        if let Some(columns) = &self.choice_columns {
            if columns.is_empty() {
                bail!("`choice_columns` must not be empty");
            }
            if let Some(labels) = &self.choice_labels
                && labels.len() != columns.len()
            {
                bail!("`choice_labels` and `choice_columns` must have the same length");
            }
            if let Some(correct) = &self.correct_choice_column
                && !columns.contains(correct)
            {
                bail!("`correct_choice_column` must be present in `choice_columns`");
            }
        }

        if let Some(labels) = &self.choice_labels {
            let unique: HashSet<&str> = labels.iter().map(String::as_str).collect();
            if unique.len() != labels.len() {
                bail!("`choice_labels` must contain unique values");
            }
        }

        Ok(())
    }
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
            assert_eq!(c.qa.golden_column.as_deref(), Some("answer"));
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
            golden_column = "answer"
        "#;
        let result: Result<WorkspaceConfig, _> = toml::from_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn validate_rejects_missing_template_file() {
        let bench = BenchmarkConfig::CustomNoCode(Box::new(CustomNoCodeBenchmarkConfig {
            type_: "custom_nocode".to_owned(),
            style: CustomNoCodeStyle::Qa,
            dataset: "quantiles/simpleqa-verified".to_owned(),
            dataset_config: None,
            split: None,
            revision: None,
            model: Some(Sampler::Random),
            qa: CustomNoCodeQaConfig {
                prompt_template_file: "does_not_exist.txt".to_owned(),
                golden_column: Some("answer".to_owned()),
                limit: None,
                max_workers: None,
                ..CustomNoCodeQaConfig::default()
            },
        }));
        let err = bench.validate().unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn validate_accepts_existing_template_file() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let bench = BenchmarkConfig::CustomNoCode(Box::new(CustomNoCodeBenchmarkConfig {
            type_: "custom_nocode".to_owned(),
            style: CustomNoCodeStyle::Qa,
            dataset: "quantiles/simpleqa-verified".to_owned(),
            dataset_config: None,
            split: None,
            revision: None,
            model: Some(Sampler::Random),
            qa: CustomNoCodeQaConfig {
                prompt_template_file: file.path().to_str().unwrap().to_owned(),
                golden_column: Some("answer".to_owned()),
                limit: None,
                max_workers: None,
                ..CustomNoCodeQaConfig::default()
            },
        }));
        bench.validate().unwrap();
    }

    #[test]
    fn parses_all_custom_nocode_examples() {
        let config: WorkspaceConfig = toml::from_str(include_str!(
            "../../../custom-nocode-examples/quantiles.toml"
        ))
        .unwrap();

        for name in ["nocode_custom", "medqa", "medmcqa", "mmlu-pro", "gpqa"] {
            assert!(
                matches!(
                    config.benchmarks.get(name),
                    Some(BenchmarkConfig::CustomNoCode(_))
                ),
                "missing custom_nocode example `{name}`"
            );
        }
    }
}
