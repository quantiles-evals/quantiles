use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::builtins::common::hash_input;

#[derive(Clone, Debug, Serialize)]
pub(super) struct PromptChoice {
    label: String,
    text: String,
}

/// A raw dataset row validated to be a JSON dictionary.
#[derive(Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub(super) struct DatasetRow(serde_json::Map<String, serde_json::Value>);

impl DatasetRow {
    /// Deserialize a required scalar column and include row and column context on failure.
    fn required_scalar(&self, row_index: usize, column: &str, kind: &str) -> Result<String> {
        let value = self
            .0
            .get(column)
            .with_context(|| format!("row {row_index}: missing {kind} column `{column}`"))?;
        let scalar: ScalarValue = serde_json::from_value(value.clone()).with_context(|| {
            format!("row {row_index}: {kind} column `{column}` must be a scalar value")
        })?;
        Ok(scalar.into_string())
    }

    /// Deserialize a required choices column, including support for JSON encoded as a string.
    fn required_choices(&self, row_index: usize, column: &str) -> Result<ChoiceCollection> {
        let value = self
            .0
            .get(column)
            .with_context(|| format!("row {row_index}: missing choices column `{column}`"))?;
        let decoded = value
            .as_str()
            .and_then(|encoded| serde_json::from_str(encoded).ok())
            .unwrap_or_else(|| value.clone());
        serde_json::from_value(decoded).with_context(|| {
            format!(
                "row {row_index}: choices column `{column}` must be an array or object of scalar values"
            )
        })
    }

    /// Deserialize a required answer index from either an unsigned integer or numeric string.
    fn required_index(&self, row_index: usize, column: &str) -> Result<u64> {
        let value = self
            .0
            .get(column)
            .with_context(|| format!("row {row_index}: missing golden index column `{column}`"))?;
        let index: IndexValue = serde_json::from_value(value.clone()).with_context(|| {
            format!("row {row_index}: golden index column `{column}` is not an integer")
        })?;
        match index {
            IndexValue::Number(index) => Ok(index),
            IndexValue::String(index) => index.parse().with_context(|| {
                format!("row {row_index}: golden index column `{column}` is not an integer")
            }),
        }
    }
}

/// A supported scalar dataset value that can be normalized to text for prompts and scoring.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ScalarValue {
    String(String),
    Number(serde_json::Number),
    Bool(bool),
}

impl ScalarValue {
    /// Convert a validated scalar dataset value to its prompt and scoring representation.
    fn into_string(self) -> String {
        match self {
            Self::String(value) => value,
            Self::Number(value) => value.to_string(),
            Self::Bool(value) => value.to_string(),
        }
    }
}

/// The supported array-backed and label-keyed representations of multiple-choice options.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ChoiceCollection {
    Array(Vec<ScalarValue>),
    Object(BTreeMap<String, ScalarValue>),
}

/// An answer index represented either as an unsigned JSON number or a numeric string.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum IndexValue {
    Number(u64),
    String(String),
}

/// A dataset row normalized into the mutually exclusive state required by its scoring style.
///
/// Ensures that evaluation logic can't confuse exact-match and multiple-choice row data.
///
/// NOTE: there is near-duplicated code in `builtins/custom_nocode/metrics.rs`, but it's used
/// for metrics, so it makes some sense to not build and share custom structs to hold
/// these data.
#[derive(Debug)]
pub(super) enum PreparedRow {
    ExactMatch {
        golden: String,
    },
    MultipleChoice {
        choices: Vec<PromptChoice>,
        golden_label: String,
        response_labels: Vec<String>,
    },
}

impl PreparedRow {
    /// Return the choices exposed to the prompt template, or an empty slice for exact match.
    pub(super) fn choices(&self) -> &[PromptChoice] {
        match self {
            Self::ExactMatch { .. } => &[],
            Self::MultipleChoice { choices, .. } => choices,
        }
    }

    /// Return the normalized golden response used for exact comparison.
    pub(super) fn golden(&self) -> &str {
        match self {
            Self::ExactMatch { golden } => golden,
            Self::MultipleChoice { golden_label, .. } => golden_label,
        }
    }

    /// Return valid response labels for multiple choice and `None` for exact match.
    pub(super) fn response_labels(&self) -> Option<&[String]> {
        match self {
            Self::ExactMatch { .. } => None,
            Self::MultipleChoice {
                response_labels, ..
            } => Some(response_labels),
        }
    }
}

/// Normalize one dataset row into the golden answer and labeled choices required for evaluation.
/// Exact-match rows have no choices; multiple-choice rows may be deterministically shuffled.
pub(super) fn prepare_row(
    row_index: usize,
    row: &DatasetRow,
    style: &crate::config::CustomNoCodeStyleConfig,
) -> Result<PreparedRow> {
    match style {
        crate::config::CustomNoCodeStyleConfig::ExactMatch { golden_column } => {
            let golden = row.required_scalar(row_index, golden_column, "golden")?;
            Ok(PreparedRow::ExactMatch { golden })
        }
        crate::config::CustomNoCodeStyleConfig::MultipleChoice {
            choices: choice_source,
            answer,
            choice_labels,
            shuffle,
        } => {
            let choice_values = extract_choices(row_index, row, choice_source, choice_labels)?;
            if choice_values.is_empty() || choice_values.len() > choice_labels.len() {
                bail!(
                    "row {row_index}: found {} choices but configured only {} labels",
                    choice_values.len(),
                    choice_labels.len()
                );
            }
            let labels = &choice_labels[..choice_values.len()];

            let correct_index = resolve_correct_index(
                row_index,
                row,
                choice_source,
                answer,
                labels,
                &choice_values,
            )?;
            let mut indexed_choices: Vec<(String, bool)> = choice_values
                .into_iter()
                .enumerate()
                .map(|(index, text)| (text, index == correct_index))
                .collect();

            if let Some(shuffle) = shuffle {
                let seed = row.required_scalar(row_index, &shuffle.seed_column, "shuffle seed")?;
                deterministic_shuffle(&mut indexed_choices, &seed);
            }

            let mut golden = None;
            let choices = indexed_choices
                .into_iter()
                .zip(labels)
                .map(|((text, is_correct), label)| {
                    if is_correct {
                        golden = Some(label.clone());
                    }
                    PromptChoice {
                        label: label.clone(),
                        text,
                    }
                })
                .collect();

            Ok(PreparedRow::MultipleChoice {
                choices,
                golden_label: golden
                    .context("correct choice disappeared while assigning labels")?,
                response_labels: choice_labels.clone(),
            })
        }
    }
}

/// Read multiple-choice values from either a single array/object column or several scalar columns.
fn extract_choices(
    row_index: usize,
    row: &DatasetRow,
    source: &crate::config::CustomNoCodeChoiceSource,
    labels: &[String],
) -> Result<Vec<String>> {
    match source {
        crate::config::CustomNoCodeChoiceSource::Columns(
            crate::config::CustomNoCodeChoiceColumns { columns },
        ) => columns
            .iter()
            .map(|column| row.required_scalar(row_index, column, "choice"))
            .collect(),
        crate::config::CustomNoCodeChoiceSource::Column(
            crate::config::CustomNoCodeChoiceColumn { column },
        ) => {
            match row.required_choices(row_index, column)? {
                ChoiceCollection::Array(values) => {
                    Ok(values.into_iter().map(ScalarValue::into_string).collect())
                }
                ChoiceCollection::Object(mut values) => labels
                    .iter()
                    .map(|label| {
                        values
                            .remove(label)
                            .map(ScalarValue::into_string)
                            .with_context(|| {
                                format!(
                                    "row {row_index}: choices object `{column}` has no scalar `{label}` value"
                                )
                            })
                    })
                    .collect(),
            }
        }
    }
}

/// Resolve the zero-based position of the correct choice using the configured answer source.
fn resolve_correct_index(
    row_index: usize,
    row: &DatasetRow,
    choice_source: &crate::config::CustomNoCodeChoiceSource,
    answer: &crate::config::CustomNoCodeAnswerSource,
    labels: &[String],
    choices: &[String],
) -> Result<usize> {
    match answer {
        crate::config::CustomNoCodeAnswerSource::IndexColumn(
            crate::config::CustomNoCodeIndexAnswer {
                index_column: column,
                index_base,
            },
        ) => {
            let index = row.required_index(row_index, column)?;
            let index = usize::try_from(index)
                .ok()
                .and_then(|index| index.checked_sub(*index_base))
                .with_context(|| {
                    format!("row {row_index}: golden index is below configured base")
                })?;
            if index >= choices.len() {
                bail!("row {row_index}: golden index {index} is outside the choice list");
            }
            Ok(index)
        }
        crate::config::CustomNoCodeAnswerSource::CorrectChoiceColumn(
            crate::config::CustomNoCodeCorrectChoiceAnswer {
                correct_choice_column: column,
            },
        ) => {
            let crate::config::CustomNoCodeChoiceSource::Columns(
                crate::config::CustomNoCodeChoiceColumns { columns },
            ) = choice_source
            else {
                bail!("correct-choice answer requires column-backed choices");
            };
            columns
                .iter()
                .position(|candidate| candidate == column)
                .context("validated correct choice is absent from choice columns")
        }
        crate::config::CustomNoCodeAnswerSource::LabelColumn(
            crate::config::CustomNoCodeLabelAnswer {
                label_column: column,
            },
        ) => {
            let golden = row.required_scalar(row_index, column, "golden")?;
            labels
                .iter()
                .position(|label| label.eq_ignore_ascii_case(golden.trim()))
                .with_context(|| {
                    format!("row {row_index}: golden label `{golden}` is not in `choice_labels`")
                })
        }
    }
}

/// Shuffle values reproducibly using a stable hash of the configured row seed.
fn deterministic_shuffle<T>(values: &mut [T], seed: &str) {
    for upper in (1..values.len()).rev() {
        let hash = hash_input(&format!("custom-nocode-choice-shuffle-v1:{seed}:{upper}"));
        let random = u64::from_str_radix(&hash, 16).unwrap_or_default();
        let index =
            usize::try_from(random % u64::try_from(upper + 1).unwrap_or(1)).unwrap_or_default();
        values.swap(upper, index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Deserialize a JSON object through the same typed boundary used by the dataset runner.
    fn dataset_row(value: serde_json::Value) -> DatasetRow {
        serde_json::from_value(value).unwrap()
    }

    /// Build a four-label multiple-choice configuration for row-preparation tests.
    fn multiple_choice_config(
        choices: crate::config::CustomNoCodeChoiceSource,
        answer: crate::config::CustomNoCodeAnswerSource,
    ) -> crate::config::CustomNoCodeStyleConfig {
        crate::config::CustomNoCodeStyleConfig::MultipleChoice {
            choices,
            answer,
            choice_labels: ["A", "B", "C", "D"].map(str::to_owned).to_vec(),
            shuffle: None,
        }
    }

    #[test]
    /// Verifies object-backed `MedQA` choices and label answers are normalized correctly.
    fn prepares_medqa_object_choices() {
        let config = multiple_choice_config(
            crate::config::CustomNoCodeChoiceSource::Column(
                crate::config::CustomNoCodeChoiceColumn {
                    column: "options".to_owned(),
                },
            ),
            crate::config::CustomNoCodeAnswerSource::LabelColumn(
                crate::config::CustomNoCodeLabelAnswer {
                    label_column: "answer_idx".to_owned(),
                },
            ),
        );
        let row = dataset_row(json!({
            "options": {"A": "alpha", "B": "beta", "C": "gamma", "D": "delta"},
            "answer_idx": "C"
        }));

        let prepared = prepare_row(0, &row, &config).unwrap();
        assert_eq!(prepared.golden(), "C");
        assert_eq!(prepared.choices()[2].text, "gamma");
    }

    #[test]
    /// Verifies object-backed choices remain supported when the dataset cache JSON-encodes them.
    fn prepares_json_encoded_object_choices() {
        let config = multiple_choice_config(
            crate::config::CustomNoCodeChoiceSource::Column(
                crate::config::CustomNoCodeChoiceColumn {
                    column: "options".to_owned(),
                },
            ),
            crate::config::CustomNoCodeAnswerSource::LabelColumn(
                crate::config::CustomNoCodeLabelAnswer {
                    label_column: "answer_idx".to_owned(),
                },
            ),
        );
        let row = dataset_row(json!({
            "options": r#"{"A":"alpha","B":"beta","C":"gamma","D":"delta"}"#,
            "answer_idx": "C"
        }));

        let prepared = prepare_row(0, &row, &config).unwrap();
        assert_eq!(prepared.golden(), "C");
        assert_eq!(prepared.choices()[2].text, "gamma");
    }

    #[test]
    /// Verifies array-backed MMLU-Pro choices are assigned their configured labels.
    fn prepares_mmlu_pro_array_choices() {
        let config = multiple_choice_config(
            crate::config::CustomNoCodeChoiceSource::Column(
                crate::config::CustomNoCodeChoiceColumn {
                    column: "options".to_owned(),
                },
            ),
            crate::config::CustomNoCodeAnswerSource::LabelColumn(
                crate::config::CustomNoCodeLabelAnswer {
                    label_column: "answer".to_owned(),
                },
            ),
        );
        let row = dataset_row(json!({"options": ["alpha", "beta", "gamma"], "answer": "B"}));

        let prepared = prepare_row(0, &row, &config).unwrap();
        assert_eq!(prepared.golden(), "B");
        assert_eq!(prepared.choices()[1].text, "beta");
    }

    #[test]
    /// Verifies `MedMCQA`'s separate choice columns and zero-based answer index are supported.
    fn prepares_medmcqa_indexed_choice_columns() {
        let config = multiple_choice_config(
            crate::config::CustomNoCodeChoiceSource::Columns(
                crate::config::CustomNoCodeChoiceColumns {
                    columns: ["opa", "opb", "opc", "opd"].map(str::to_owned).to_vec(),
                },
            ),
            crate::config::CustomNoCodeAnswerSource::IndexColumn(
                crate::config::CustomNoCodeIndexAnswer {
                    index_column: "cop".to_owned(),
                    index_base: 0,
                },
            ),
        );
        for answer_index in [json!(1), json!("1")] {
            let row = dataset_row(json!({
                "opa": "alpha",
                "opb": "beta",
                "opc": "gamma",
                "opd": "delta",
                "cop": answer_index
            }));

            let prepared = prepare_row(0, &row, &config).unwrap();
            assert_eq!(prepared.golden(), "B");
            assert_eq!(prepared.choices()[1].text, "beta");
        }
    }

    #[test]
    /// Verifies GPQA shuffling is deterministic and preserves the correct choice mapping.
    fn prepares_gpqa_with_deterministic_shuffle() {
        let config = crate::config::CustomNoCodeStyleConfig::MultipleChoice {
            choices: crate::config::CustomNoCodeChoiceSource::Columns(
                crate::config::CustomNoCodeChoiceColumns {
                    columns: [
                        "Correct Answer",
                        "Incorrect Answer 1",
                        "Incorrect Answer 2",
                        "Incorrect Answer 3",
                    ]
                    .map(str::to_owned)
                    .to_vec(),
                },
            ),
            answer: crate::config::CustomNoCodeAnswerSource::CorrectChoiceColumn(
                crate::config::CustomNoCodeCorrectChoiceAnswer {
                    correct_choice_column: "Correct Answer".to_owned(),
                },
            ),
            choice_labels: ["A", "B", "C", "D"].map(str::to_owned).to_vec(),
            shuffle: Some(crate::config::CustomNoCodeShuffleConfig {
                seed_column: "Record ID".to_owned(),
            }),
        };
        let row = dataset_row(json!({
            "Correct Answer": "correct",
            "Incorrect Answer 1": "wrong one",
            "Incorrect Answer 2": "wrong two",
            "Incorrect Answer 3": "wrong three",
            "Record ID": "gpqa-row-1"
        }));

        let first = prepare_row(0, &row, &config).unwrap();
        let second = prepare_row(0, &row, &config).unwrap();
        assert_eq!(
            serde_json::to_value(first.choices()).unwrap(),
            serde_json::to_value(second.choices()).unwrap()
        );
        let correct = first
            .choices()
            .iter()
            .find(|choice| choice.label == first.golden())
            .unwrap();
        assert_eq!(correct.text, "correct");
    }

    #[test]
    /// Verifies the typed row boundary rejects non-object dataset rows.
    fn dataset_row_rejects_non_object_values() {
        let result = serde_json::from_value::<DatasetRow>(json!(["not", "an", "object"]));
        assert!(result.is_err());
    }

    #[test]
    /// Verifies compound values are rejected when a configured scalar column is required.
    fn prepare_row_rejects_non_scalar_golden_value() {
        let config = crate::config::CustomNoCodeStyleConfig::ExactMatch {
            golden_column: "answer".to_owned(),
        };
        let row = dataset_row(json!({"answer": ["invalid"]}));

        let error = prepare_row(4, &row, &config).unwrap_err();
        assert!(error.to_string().contains("row 4"));
        assert!(error.to_string().contains("must be a scalar value"));
    }
}
