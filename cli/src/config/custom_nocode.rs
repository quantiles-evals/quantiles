use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::llm::Sampler;

/// Task-specific configuration for a no-code benchmark.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "style", rename_all = "snake_case")]
pub enum CustomNoCodeTaskConfig {
    /// Compare the trimmed model response with a golden dataset column.
    ExactMatch {
        prompt_template_file: String,
        golden_column: String,
    },
    /// Render labeled choices and score the selected label.
    MultipleChoice {
        prompt_template_file: String,
        choices: CustomNoCodeChoiceSource,
        answer: CustomNoCodeAnswerSource,
        choice_labels: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        shuffle: Option<CustomNoCodeShuffleConfig>,
    },
}

/// Source of the answer choices for a multiple-choice task.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum CustomNoCodeChoiceSource {
    Column(CustomNoCodeChoiceColumn),
    Columns(CustomNoCodeChoiceColumns),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct CustomNoCodeChoiceColumn {
    pub column: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct CustomNoCodeChoiceColumns {
    pub columns: Vec<String>,
}

/// Source of the correct answer for a multiple-choice task.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum CustomNoCodeAnswerSource {
    LabelColumn(CustomNoCodeLabelAnswer),
    IndexColumn(CustomNoCodeIndexAnswer),
    CorrectChoiceColumn(CustomNoCodeCorrectChoiceAnswer),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct CustomNoCodeLabelAnswer {
    pub label_column: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct CustomNoCodeIndexAnswer {
    pub index_column: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub index_base: usize,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct CustomNoCodeCorrectChoiceAnswer {
    pub correct_choice_column: String,
}

/// Deterministic choice-shuffling configuration.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct CustomNoCodeShuffleConfig {
    pub seed_column: String,
}

/// No-code custom benchmark configuration.
#[derive(Debug, Deserialize, Clone)]
pub struct CustomNoCodeBenchmarkConfig {
    #[serde(rename = "type")]
    pub type_: String,
    /// Hugging Face dataset coordinates.
    pub dataset: CustomNoCodeDatasetConfig,
    /// Model sampler to use.
    pub model: Option<Sampler>,
    /// Number of dataset rows to evaluate.
    pub limit: Option<usize>,
    /// Maximum concurrent workers.
    pub max_workers: Option<usize>,
    #[serde(flatten)]
    pub task: CustomNoCodeTaskConfig,
}

/// Hugging Face dataset coordinates for a no-code benchmark.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct CustomNoCodeDatasetConfig {
    /// Dataset identifier, for example `quantiles/simpleqa-verified`.
    pub name: String,
    /// Optional Hugging Face dataset configuration/subset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_name: Option<String>,
    /// Optional dataset split. The dataset manager chooses one when omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split: Option<String>,
    /// Optional dataset revision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
}

impl CustomNoCodeTaskConfig {
    #[must_use]
    pub fn prompt_template_file(&self) -> &str {
        match self {
            Self::ExactMatch {
                prompt_template_file,
                ..
            }
            | Self::MultipleChoice {
                prompt_template_file,
                ..
            } => prompt_template_file,
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        let Self::MultipleChoice {
            choices,
            answer,
            choice_labels,
            ..
        } = self
        else {
            return Ok(());
        };

        if choice_labels.is_empty() {
            bail!("multiple-choice `choice_labels` must not be empty");
        }
        let unique: std::collections::HashSet<&str> =
            choice_labels.iter().map(String::as_str).collect();
        if unique.len() != choice_labels.len() {
            bail!("multiple-choice `choice_labels` must contain unique values");
        }

        if let CustomNoCodeChoiceSource::Columns(CustomNoCodeChoiceColumns { columns }) = choices {
            if columns.is_empty() {
                bail!("multiple-choice `choices.columns` must not be empty");
            }
            if choice_labels.len() != columns.len() {
                bail!("`choice_labels` and `choices.columns` must have the same length");
            }
            if let CustomNoCodeAnswerSource::CorrectChoiceColumn(CustomNoCodeCorrectChoiceAnswer {
                correct_choice_column,
            }) = answer
                && !columns.contains(correct_choice_column)
            {
                bail!("`answer.correct_choice_column` must be present in `choices.columns`");
            }
        } else if matches!(answer, CustomNoCodeAnswerSource::CorrectChoiceColumn(_)) {
            bail!("`answer.correct_choice_column` requires `choices.columns`");
        }

        Ok(())
    }
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde skip_serializing_if predicates receive references"
)]
const fn is_zero(value: &usize) -> bool {
    *value == 0
}

#[cfg(test)]
mod tests {
    use super::super::{BenchmarkConfig, WorkspaceConfig};
    use super::*;

    #[test]
    fn deserialize_custom_nocode_exact_match() {
        let toml = r#"
            [benchmarks.nocode_custom]
            type = "custom_nocode"
            style = "exact_match"
            dataset = { name = "quantiles/simpleqa-verified" }
            model = "random"
            prompt_template_file = "prompts/qa.txt"
            golden_column = "answer"
            limit = 10
        "#;
        let config: WorkspaceConfig = toml::from_str(toml).unwrap();
        let bench = config.benchmarks.get("nocode_custom").unwrap();
        assert!(matches!(bench, BenchmarkConfig::CustomNoCode(_)));
        if let BenchmarkConfig::CustomNoCode(c) = bench {
            assert_eq!(c.dataset.name, "quantiles/simpleqa-verified");
            assert_eq!(c.model, Some(Sampler::Random));
            assert_eq!(c.limit, Some(10));
            let CustomNoCodeTaskConfig::ExactMatch {
                prompt_template_file,
                golden_column,
            } = &c.task
            else {
                panic!("expected exact-match task");
            };
            assert_eq!(prompt_template_file, "prompts/qa.txt");
            assert_eq!(golden_column, "answer");
        }
    }

    #[test]
    fn deserialize_custom_nocode_unsupported_style_errors() {
        let toml = r#"
            [benchmarks.nocode_custom]
            type = "custom_nocode"
            style = "judge"
            dataset = { name = "quantiles/simpleqa-verified" }
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
            style = "exact_match"
            dataset = { name = "quantiles/simpleqa-verified" }
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
            dataset: CustomNoCodeDatasetConfig {
                name: "quantiles/simpleqa-verified".to_owned(),
                config_name: None,
                split: None,
                revision: None,
            },
            model: Some(Sampler::Random),
            limit: None,
            max_workers: None,
            task: CustomNoCodeTaskConfig::ExactMatch {
                prompt_template_file: "does_not_exist.txt".to_owned(),
                golden_column: "answer".to_owned(),
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
            dataset: CustomNoCodeDatasetConfig {
                name: "quantiles/simpleqa-verified".to_owned(),
                config_name: None,
                split: None,
                revision: None,
            },
            model: Some(Sampler::Random),
            limit: None,
            max_workers: None,
            task: CustomNoCodeTaskConfig::ExactMatch {
                prompt_template_file: file.path().to_str().unwrap().to_owned(),
                golden_column: "answer".to_owned(),
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

        let Some(BenchmarkConfig::CustomNoCode(medmcqa)) = config.benchmarks.get("medmcqa") else {
            panic!("missing MedMCQA example");
        };
        assert!(matches!(
            medmcqa.task,
            CustomNoCodeTaskConfig::MultipleChoice { .. }
        ));
        assert_eq!(medmcqa.dataset.name, "quantiles/medmcqa");
        assert_eq!(medmcqa.dataset.split.as_deref(), Some("validation"));
    }

    #[test]
    fn multiple_choice_rejects_mixed_answer_sources() {
        let toml = r#"
            [benchmarks.invalid]
            type = "custom_nocode"
            style = "multiple_choice"
            dataset = { name = "fixture/qa" }
            prompt_template_file = "prompts/qa.txt"
            choices = { column = "options" }
            answer = { label_column = "answer", index_column = "answer_index" }
            choice_labels = ["A", "B"]
        "#;

        let result: Result<WorkspaceConfig, _> = toml::from_str(toml);
        assert!(result.is_err());
    }
}
