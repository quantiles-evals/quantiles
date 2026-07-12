use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::builtins::common::hash_input;

#[derive(Clone, Debug, Serialize)]
pub(super) struct PromptChoice {
    label: String,
    text: String,
}

pub(super) struct PreparedRow {
    pub(super) choices: Vec<PromptChoice>,
    pub(super) golden: String,
}

pub(super) fn prepare_row(
    row_index: usize,
    row: &serde_json::Value,
    style: &crate::config::CustomNoCodeStyleConfig,
) -> Result<PreparedRow> {
    match style {
        crate::config::CustomNoCodeStyleConfig::ExactMatch { golden_column } => {
            let golden = extract_scalar(row, golden_column).with_context(|| {
                format!("row {row_index}: missing golden column `{golden_column}`")
            })?;
            Ok(PreparedRow {
                choices: Vec::new(),
                golden,
            })
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
                let seed = extract_scalar(row, &shuffle.seed_column).with_context(|| {
                    format!(
                        "row {row_index}: missing shuffle seed column `{}`",
                        shuffle.seed_column
                    )
                })?;
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

            Ok(PreparedRow {
                choices,
                golden: golden.context("correct choice disappeared while assigning labels")?,
            })
        }
    }
}

fn extract_choices(
    row_index: usize,
    row: &serde_json::Value,
    source: &crate::config::CustomNoCodeChoiceSource,
    labels: &[String],
) -> Result<Vec<String>> {
    match source {
        crate::config::CustomNoCodeChoiceSource::Columns(
            crate::config::CustomNoCodeChoiceColumns { columns },
        ) => columns
            .iter()
            .map(|column| {
                extract_scalar(row, column)
                    .with_context(|| format!("row {row_index}: missing choice column `{column}`"))
            })
            .collect(),
        crate::config::CustomNoCodeChoiceSource::Column(
            crate::config::CustomNoCodeChoiceColumn { column },
        ) => {
            let value = row
                .get(column)
                .with_context(|| format!("row {row_index}: missing choices column `{column}`"))?;
            let owned;
            let value = if let Some(encoded) = value.as_str() {
                owned = serde_json::from_str(encoded).unwrap_or_else(|_| value.clone());
                &owned
            } else {
                value
            };

            match value {
                serde_json::Value::Array(values) => values
                    .iter()
                    .map(value_to_scalar)
                    .collect::<Option<Vec<_>>>()
                    .with_context(|| format!("row {row_index}: choices column `{column}` contains non-scalar values")),
                serde_json::Value::Object(values) => labels
                    .iter()
                    .map(|label| {
                        values.get(label).and_then(value_to_scalar).with_context(|| {
                            format!("row {row_index}: choices object `{column}` has no scalar `{label}` value")
                        })
                    })
                    .collect(),
                _ => bail!("row {row_index}: choices column `{column}` must be an array or object"),
            }
        }
    }
}

fn resolve_correct_index(
    row_index: usize,
    row: &serde_json::Value,
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
            let raw = row.get(column).with_context(|| {
                format!("row {row_index}: missing golden index column `{column}`")
            })?;
            let index = raw
                .as_u64()
                .or_else(|| raw.as_str().and_then(|value| value.parse().ok()))
                .with_context(|| {
                    format!("row {row_index}: golden index column `{column}` is not an integer")
                })?;
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
            let golden = extract_scalar(row, column)
                .with_context(|| format!("row {row_index}: missing golden column `{column}`"))?;
            labels
                .iter()
                .position(|label| label.eq_ignore_ascii_case(golden.trim()))
                .with_context(|| {
                    format!("row {row_index}: golden label `{golden}` is not in `choice_labels`")
                })
        }
    }
}

fn extract_scalar(row: &serde_json::Value, key: &str) -> Option<String> {
    row.get(key).and_then(value_to_scalar)
}

fn value_to_scalar(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

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
        let row = json!({
            "options": {"A": "alpha", "B": "beta", "C": "gamma", "D": "delta"},
            "answer_idx": "C"
        });

        let prepared = prepare_row(0, &row, &config).unwrap();
        assert_eq!(prepared.golden, "C");
        assert_eq!(prepared.choices[2].text, "gamma");
    }

    #[test]
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
        let row = json!({"options": ["alpha", "beta", "gamma"], "answer": "B"});

        let prepared = prepare_row(0, &row, &config).unwrap();
        assert_eq!(prepared.golden, "B");
        assert_eq!(prepared.choices[1].text, "beta");
    }

    #[test]
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
        let row = json!({"opa": "alpha", "opb": "beta", "opc": "gamma", "opd": "delta", "cop": 1});

        let prepared = prepare_row(0, &row, &config).unwrap();
        assert_eq!(prepared.golden, "B");
        assert_eq!(prepared.choices[1].text, "beta");
    }

    #[test]
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
        let row = json!({
            "Correct Answer": "correct",
            "Incorrect Answer 1": "wrong one",
            "Incorrect Answer 2": "wrong two",
            "Incorrect Answer 3": "wrong three",
            "Record ID": "gpqa-row-1"
        });

        let first = prepare_row(0, &row, &config).unwrap();
        let second = prepare_row(0, &row, &config).unwrap();
        assert_eq!(
            serde_json::to_value(&first.choices).unwrap(),
            serde_json::to_value(&second.choices).unwrap()
        );
        let correct = first
            .choices
            .iter()
            .find(|choice| choice.label == first.golden)
            .unwrap();
        assert_eq!(correct.text, "correct");
    }
}
