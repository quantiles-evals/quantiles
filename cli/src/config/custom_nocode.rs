use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::llm::Sampler;

/// Scoring style and style-specific configuration for a no-code benchmark.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CustomNoCodeStyleConfig {
    /// Compare the trimmed model response with a golden dataset column.
    ExactMatch { golden_column: String },
    /// Render labeled choices and score the selected label.
    MultipleChoice {
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

/// No-code custom benchmark configuration, including its benchmark-type discriminator.
#[derive(Debug, Clone)]
pub struct CustomNoCodeBenchmarkConfig {
    pub type_: String,
    /// Parameters shared with the runtime input consumed by the no-code builtin.
    pub params: CustomNoCodeParams,
}

/// Parameters used to configure and execute a no-code custom benchmark.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct CustomNoCodeParams {
    /// Hugging Face dataset coordinates.
    pub dataset: CustomNoCodeDatasetConfig,
    /// Model sampler to use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<Sampler>,
    /// Path to a Jinja template file for rendering prompts.
    pub prompt_template_file: String,
    /// Number of dataset rows to evaluate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    /// Maximum number of concurrent workers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_workers: Option<usize>,
    /// Optional aggregate metric families computed only for requested output surfaces.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metrics: Vec<CustomNoCodeMetricSelection>,
    /// Scoring style and its required dataset-column configuration.
    pub style: CustomNoCodeStyleConfig,
}

/// Optional aggregate metric family available for custom no-code multiple-choice runs.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CustomNoCodeMetricName {
    F1,
    Confusion,
}

/// Output surfaces on which an optional aggregate metric family is shown.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CustomNoCodeMetricShow {
    All,
    Json,
}

/// Detailed optional aggregate metric configuration.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CustomNoCodeMetricConfig {
    pub name: CustomNoCodeMetricName,
    pub show: CustomNoCodeMetricShow,
}

/// Shorthand metric names default to JSON-only output; inline tables can choose visibility.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(untagged)]
pub enum CustomNoCodeMetricSelection {
    Name(CustomNoCodeMetricName),
    Config(CustomNoCodeMetricConfig),
}

impl CustomNoCodeMetricSelection {
    #[must_use]
    pub const fn name(self) -> CustomNoCodeMetricName {
        match self {
            Self::Name(name) => name,
            Self::Config(config) => config.name,
        }
    }

    #[must_use]
    pub const fn show(self) -> CustomNoCodeMetricShow {
        match self {
            Self::Name(_) => CustomNoCodeMetricShow::Json,
            Self::Config(config) => config.show,
        }
    }

    #[must_use]
    pub const fn requested_for(self, json: bool) -> bool {
        matches!(self.show(), CustomNoCodeMetricShow::All) || json
    }
}

impl CustomNoCodeParams {
    pub(crate) fn validate_metrics(&self) -> Result<()> {
        let mut names = std::collections::HashSet::new();
        for selection in &self.metrics {
            if !names.insert(selection.name()) {
                bail!(
                    "custom_nocode `metrics` must not contain duplicate `{}` entries",
                    match selection.name() {
                        CustomNoCodeMetricName::F1 => "f1",
                        CustomNoCodeMetricName::Confusion => "confusion",
                    }
                );
            }
        }

        if !self.metrics.is_empty()
            && matches!(self.style, CustomNoCodeStyleConfig::ExactMatch { .. })
        {
            bail!("custom_nocode `metrics` are only supported for multiple-choice evaluations");
        }

        Ok(())
    }
}

impl<'de> Deserialize<'de> for CustomNoCodeBenchmarkConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut value = serde_json::Value::deserialize(deserializer)?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| serde::de::Error::custom("expected a custom_nocode config table"))?;
        let type_ = object
            .remove("type")
            .ok_or_else(|| serde::de::Error::missing_field("type"))?
            .as_str()
            .ok_or_else(|| serde::de::Error::custom("`type` must be a string"))?
            .to_owned();
        let params = CustomNoCodeParams::deserialize(value).map_err(serde::de::Error::custom)?;

        Ok(Self { type_, params })
    }
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

impl CustomNoCodeStyleConfig {
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
    reason = "serde skip_serializing_if predicates must receive references"
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
            style = { type = "exact_match", golden_column = "answer" }
            dataset = { name = "quantiles/simpleqa-verified" }
            model = "random"
            prompt_template_file = "prompts/qa.txt"
            limit = 10
        "#;
        let config: WorkspaceConfig = toml::from_str(toml).unwrap();
        let bench = config.benchmarks.get("nocode_custom").unwrap();
        assert!(matches!(bench, BenchmarkConfig::CustomNoCode(_)));
        if let BenchmarkConfig::CustomNoCode(c) = bench {
            assert_eq!(c.params.dataset.name, "quantiles/simpleqa-verified");
            assert_eq!(c.params.model, Some(Sampler::Random));
            assert_eq!(c.params.limit, Some(10));
            let CustomNoCodeStyleConfig::ExactMatch { golden_column } = &c.params.style else {
                panic!("expected exact-match task");
            };
            assert_eq!(c.params.prompt_template_file, "prompts/qa.txt");
            assert_eq!(golden_column, "answer");
        }
    }

    #[test]
    fn deserialize_custom_nocode_unsupported_style_errors() {
        let toml = r#"
            [benchmarks.nocode_custom]
            type = "custom_nocode"
            style = { type = "judge", golden_column = "answer" }
            dataset = { name = "quantiles/simpleqa-verified" }
            model = "random"
            prompt_template_file = "prompts/qa.txt"
        "#;
        let result: Result<WorkspaceConfig, _> = toml::from_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn custom_nocode_missing_required_field_errors() {
        let toml = r#"
            [benchmarks.nocode_custom]
            type = "custom_nocode"
            style = { type = "exact_match", golden_column = "answer" }
            dataset = { name = "quantiles/simpleqa-verified" }
            model = "random"
        "#;
        let result: Result<WorkspaceConfig, _> = toml::from_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn validate_rejects_missing_template_file() {
        let bench = BenchmarkConfig::CustomNoCode(Box::new(CustomNoCodeBenchmarkConfig {
            type_: "custom_nocode".to_owned(),
            params: CustomNoCodeParams {
                dataset: CustomNoCodeDatasetConfig {
                    name: "quantiles/simpleqa-verified".to_owned(),
                    config_name: None,
                    split: None,
                    revision: None,
                },
                model: Some(Sampler::Random),
                prompt_template_file: "does_not_exist.txt".to_owned(),
                limit: None,
                max_workers: None,
                metrics: Vec::new(),
                style: CustomNoCodeStyleConfig::ExactMatch {
                    golden_column: "answer".to_owned(),
                },
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
            params: CustomNoCodeParams {
                dataset: CustomNoCodeDatasetConfig {
                    name: "quantiles/simpleqa-verified".to_owned(),
                    config_name: None,
                    split: None,
                    revision: None,
                },
                model: Some(Sampler::Random),
                prompt_template_file: file.path().to_str().unwrap().to_owned(),
                limit: None,
                max_workers: None,
                metrics: Vec::new(),
                style: CustomNoCodeStyleConfig::ExactMatch {
                    golden_column: "answer".to_owned(),
                },
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

        for name in ["simpleqa-verified", "medqa", "medmcqa", "mmlu-pro", "gpqa"] {
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
            medmcqa.params.style,
            CustomNoCodeStyleConfig::MultipleChoice { .. }
        ));
        assert_eq!(medmcqa.params.dataset.name, "quantiles/medmcqa");
        assert_eq!(medmcqa.params.dataset.split.as_deref(), Some("validation"));
    }

    #[test]
    fn multiple_choice_rejects_mixed_answer_sources() {
        let toml = r#"
            [benchmarks.invalid]
            type = "custom_nocode"
            style = { type = "multiple_choice", choices = { column = "options" }, answer = { label_column = "answer", index_column = "answer_index" }, choice_labels = ["A", "B"] }
            dataset = { name = "fixture/qa" }
            prompt_template_file = "prompts/qa.txt"
        "#;

        let result: Result<WorkspaceConfig, _> = toml::from_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn exact_match_rejects_multiple_choice_fields() {
        let toml = r#"
            [benchmarks.invalid]
            type = "custom_nocode"
            style = { type = "exact_match", golden_column = "answer", choices = { column = "options" } }
            dataset = { name = "fixture/qa" }
            prompt_template_file = "prompts/qa.txt"
        "#;

        let result: Result<WorkspaceConfig, _> = toml::from_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn benchmark_rejects_style_fields_at_top_level() {
        let toml = r#"
            [benchmarks.invalid]
            type = "custom_nocode"
            style = { type = "exact_match", golden_column = "answer" }
            dataset = { name = "fixture/qa" }
            prompt_template_file = "prompts/qa.txt"
            golden_column = "answer"
        "#;

        let result: Result<WorkspaceConfig, _> = toml::from_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn parses_metric_shorthand_and_detailed_config() {
        let toml = r#"
            [benchmarks.metrics]
            type = "custom_nocode"
            style = { type = "multiple_choice", choices = { column = "options" }, answer = { label_column = "answer" }, choice_labels = ["A", "B"] }
            dataset = { name = "fixture/qa" }
            prompt_template_file = "prompts/qa.txt"
            metrics = ["f1", { name = "confusion", show = "all" }]
        "#;

        let config: WorkspaceConfig = toml::from_str(toml).unwrap();
        let Some(BenchmarkConfig::CustomNoCode(benchmark)) = config.benchmarks.get("metrics")
        else {
            panic!("missing custom_nocode benchmark");
        };
        assert_eq!(benchmark.params.metrics.len(), 2);
        assert_eq!(
            benchmark.params.metrics[0].show(),
            CustomNoCodeMetricShow::Json
        );
        assert_eq!(
            benchmark.params.metrics[1].show(),
            CustomNoCodeMetricShow::All
        );
    }

    #[test]
    fn validate_rejects_duplicate_metrics() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let benchmark = BenchmarkConfig::CustomNoCode(Box::new(CustomNoCodeBenchmarkConfig {
            type_: "custom_nocode".to_owned(),
            params: CustomNoCodeParams {
                dataset: CustomNoCodeDatasetConfig {
                    name: "fixture/qa".to_owned(),
                    config_name: None,
                    split: None,
                    revision: None,
                },
                model: None,
                prompt_template_file: file.path().to_string_lossy().into_owned(),
                limit: None,
                max_workers: None,
                metrics: vec![
                    CustomNoCodeMetricSelection::Name(CustomNoCodeMetricName::F1),
                    CustomNoCodeMetricSelection::Config(CustomNoCodeMetricConfig {
                        name: CustomNoCodeMetricName::F1,
                        show: CustomNoCodeMetricShow::All,
                    }),
                ],
                style: CustomNoCodeStyleConfig::MultipleChoice {
                    choices: CustomNoCodeChoiceSource::Column(CustomNoCodeChoiceColumn {
                        column: "options".to_owned(),
                    }),
                    answer: CustomNoCodeAnswerSource::LabelColumn(CustomNoCodeLabelAnswer {
                        label_column: "answer".to_owned(),
                    }),
                    choice_labels: vec!["A".to_owned(), "B".to_owned()],
                    shuffle: None,
                },
            },
        }));

        assert!(
            benchmark
                .validate()
                .unwrap_err()
                .to_string()
                .contains("duplicate `f1`")
        );
    }

    #[test]
    fn validate_rejects_metrics_for_exact_match() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let benchmark = BenchmarkConfig::CustomNoCode(Box::new(CustomNoCodeBenchmarkConfig {
            type_: "custom_nocode".to_owned(),
            params: CustomNoCodeParams {
                dataset: CustomNoCodeDatasetConfig {
                    name: "fixture/qa".to_owned(),
                    config_name: None,
                    split: None,
                    revision: None,
                },
                model: None,
                prompt_template_file: file.path().to_string_lossy().into_owned(),
                limit: None,
                max_workers: None,
                metrics: vec![CustomNoCodeMetricSelection::Name(
                    CustomNoCodeMetricName::F1,
                )],
                style: CustomNoCodeStyleConfig::ExactMatch {
                    golden_column: "answer".to_owned(),
                },
            },
        }));

        assert!(
            benchmark
                .validate()
                .unwrap_err()
                .to_string()
                .contains("only supported for multiple-choice")
        );
    }
}
