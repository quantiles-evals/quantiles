use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::data::{DatasetRow, PreparedRow};
use super::metrics::SampleResult;
use crate::builtins::common::{hash_input, run_timed_step};

/// Style-specific step output for each row, stored as JSON in the step
/// record.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum RowOutput {
    Classification {
        input: String,
        response: String,
        parsed_response: Option<String>,
        golden: String,
        is_correct: bool,
    },
    Similarity {
        input: String,
        response: String,
        golden: String,
        similarity_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        embedding_model: Option<String>,
        similarity_score: f64,
    },
}

/// Canonical serialized scoring configuration used in durable step hashes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ScoringIdentity(String);

impl ScoringIdentity {
    /// Build an identity from a no-code scoring style.
    pub(super) fn from_style(style: &crate::config::CustomNoCodeStyleConfig) -> Result<Self> {
        let style_as_string = serde_json::to_string(style)?;
        Ok(Self(style_as_string))
    }
}

impl std::fmt::Display for ScoringIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub(super) struct EvaluateRowArgs<'a> {
    pub(super) i: usize,
    pub(super) row: &'a DatasetRow,
    pub(super) prepared: PreparedRow,
    pub(super) template_str: &'a str,
    pub(super) env: &'a jinja::Environment<'a>,
    pub(super) model_name: &'a str,
    /// Scoring identity included in this row's durable step hash.
    pub(super) scoring_identity: &'a ScoringIdentity,
    /// Resolved scorer for a similarity-style row.
    pub(super) similarity: Option<&'a super::runtime::ResolvedSimilarityMetric>,
    pub(super) llm: &'a std::sync::Arc<dyn crate::llm::LLMSampler>,
    pub(super) db: &'a sea_orm::DatabaseConnection,
    pub(super) metrics_store: &'a crate::metrics_store::MetricsStore,
    pub(super) run_id: i64,
}

/// Render, sample, score, and record one normalized dataset row.
/// Returns the style-specific sample result used for aggregate metrics.
#[expect(clippy::too_many_lines)]
pub(super) async fn evaluate_row(
    benchmark_name: &str,
    args: EvaluateRowArgs<'_>,
) -> Result<SampleResult> {
    let prepared = args.prepared;

    let rendered = args
        .env
        .render_str(
            args.template_str,
            jinja::context!(row => args.row, choices => prepared.choices()),
        )
        .with_context(|| format!("row {}: failed to render prompt template", args.i))?;

    let golden = prepared.golden().to_owned();
    let input_hash = hash_input(&format!(
        "{rendered}\ngolden={}\nmodel={}\nworkflow={benchmark_name}\nscoring={}",
        golden, args.model_name, args.scoring_identity
    ));
    let step_key = format!("row-{}", args.i);

    let (output, step_id) = run_timed_step(
        args.db,
        args.metrics_store,
        args.run_id,
        &step_key,
        &input_hash,
        async {
            let model_response = args
                .llm
                .sample(&rendered)
                .await
                .with_context(|| format!("failed to sample LLM for row {}", args.i))?;

            match &prepared {
                PreparedRow::Similarity { .. } => {
                    let similarity = args
                        .similarity
                        .context("similarity style did not initialize a similarity metric")?;
                    let similarity_score = similarity
                        .metric
                        .compute(&model_response, &golden)
                        .await
                        .with_context(|| {
                            format!("failed to compute similarity for row {}", args.i)
                        })?;
                    Ok(RowOutput::Similarity {
                        input: rendered.clone(),
                        response: model_response,
                        golden: golden.clone(),
                        similarity_name: similarity.name.to_owned(),
                        embedding_model: similarity.embedding_model.map(str::to_owned),
                        similarity_score,
                    })
                }
                PreparedRow::ExactMatch { .. } | PreparedRow::MultipleChoice { .. } => {
                    let parsed_response = match prepared.response_labels() {
                        Some(choice_labels) => extract_choice_label(&model_response, choice_labels),
                        None => Some(model_response.trim().to_owned()),
                    };
                    let is_correct = parsed_response.as_deref() == Some(golden.trim());

                    Ok(RowOutput::Classification {
                        input: rendered.clone(),
                        response: model_response,
                        parsed_response,
                        golden: golden.clone(),
                        is_correct,
                    })
                }
            }
        },
    )
    .await?;

    match output {
        RowOutput::Classification {
            parsed_response,
            is_correct,
            ..
        } => {
            if let Some(step_id) = step_id {
                args.metrics_store
                    .emit(
                        args.run_id,
                        Some(step_id),
                        "is_correct",
                        if is_correct { 1.0 } else { 0.0 },
                        None,
                    )
                    .await;
                args.metrics_store
                    .emit(
                        args.run_id,
                        Some(step_id),
                        "response_parsed",
                        if parsed_response.is_some() { 1.0 } else { 0.0 },
                        None,
                    )
                    .await;
            }
            Ok(SampleResult::Classification(is_correct))
        }
        RowOutput::Similarity {
            similarity_score, ..
        } => {
            if let Some(step_id) = step_id {
                args.metrics_store
                    .emit(
                        args.run_id,
                        Some(step_id),
                        "similarity_score",
                        similarity_score,
                        None,
                    )
                    .await;
            }
            Ok(SampleResult::Similarity(similarity_score))
        }
    }
}

/// Extract a configured choice label from a direct response or its final few tokens.
fn extract_choice_label(response: &str, labels: &[String]) -> Option<String> {
    let trimmed = response.trim();
    if let Some(label) = labels
        .iter()
        .find(|label| label.eq_ignore_ascii_case(trimmed))
    {
        return Some(label.clone());
    }

    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    for token in tokens.iter().rev().take(8) {
        let cleaned = token.trim_matches(|character: char| !character.is_ascii_alphanumeric());
        if let Some(label) = labels
            .iter()
            .find(|label| label.eq_ignore_ascii_case(cleaned))
        {
            return Some(label.clone());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;

    struct FixedSampler;

    #[async_trait]
    impl crate::llm::LLMSampler for FixedSampler {
        async fn sample(&self, _prompt: &str) -> Result<String> {
            Ok("hello".to_owned())
        }
    }

    #[test]
    /// Verifies templates can access arbitrary fields through the `row` variable.
    fn render_template_with_row_variable() {
        let template = "Answer this: {{ row.question }}";
        let env = jinja::Environment::new();
        let rendered = env
            .render_str(
                template,
                jinja::context!(row => json!({"question": "what is 2+2"})),
            )
            .unwrap();
        assert_eq!(rendered, "Answer this: what is 2+2");
    }

    #[test]
    /// Verifies prompt rendering preserves intentional line boundaries.
    fn render_template_preserves_newlines() {
        let template = "Question:\n{{ row.question }}\nAnswer:";
        let env = jinja::Environment::new();
        let rendered = env
            .render_str(
                template,
                jinja::context!(row => json!({"question": "hello"})),
            )
            .unwrap();
        assert_eq!(rendered, "Question:\nhello\nAnswer:");
    }

    #[test]
    /// Verifies direct, formatted, and invalid multiple-choice responses are parsed correctly.
    fn extracts_multiple_choice_labels() {
        let labels = vec!["A".to_owned(), "B".to_owned(), "C".to_owned()];
        assert_eq!(extract_choice_label("B", &labels).as_deref(), Some("B"));
        assert_eq!(
            extract_choice_label("The answer is (c).", &labels).as_deref(),
            Some("C")
        );
        assert_eq!(extract_choice_label("unknown", &labels), None);
    }

    #[tokio::test]
    async fn evaluates_and_records_levenshtein_similarity() {
        let tmpdir = tempfile::tempdir().unwrap();
        crate::db::init_workspace(tmpdir.path()).await.unwrap();
        let db = crate::db::open_workspace(tmpdir.path()).await.unwrap();
        let metrics_store =
            crate::metrics_store::MetricsStore::new(crate::db::metrics_dir(tmpdir.path())).unwrap();
        let run_id = crate::db::create_run(&db, "similarity-test", None)
            .await
            .unwrap();
        let style = crate::config::CustomNoCodeStyleConfig::Similarity {
            golden_column: "answer".to_owned(),
            metric: crate::config::CustomNoCodeSimilarityMetric::Levenshtein(
                crate::config::CustomNoCodeLevenshteinMetric::Levenshtein,
            ),
        };
        let row: DatasetRow = serde_json::from_value(json!({
            "question": "say hello",
            "answer": "hello"
        }))
        .unwrap();
        let prepared = super::super::data::prepare_row(0, &row, &style).unwrap();
        let similarity = super::super::runtime::resolve_similarity_metric(&style)
            .unwrap()
            .unwrap();
        let env = jinja::Environment::new();
        let llm: std::sync::Arc<dyn crate::llm::LLMSampler> = std::sync::Arc::new(FixedSampler);
        let scoring_identity = ScoringIdentity::from_style(&style).unwrap();

        let result = evaluate_row(
            "similarity-test",
            EvaluateRowArgs {
                i: 0,
                row: &row,
                prepared,
                template_str: "{{ row.question }}",
                env: &env,
                model_name: "fixed",
                scoring_identity: &scoring_identity,
                similarity: Some(&similarity),
                llm: &llm,
                db: &db,
                metrics_store: &metrics_store,
                run_id,
            },
        )
        .await
        .unwrap();

        assert!(
            matches!(result, SampleResult::Similarity(score) if (score - 1.0).abs() < f64::EPSILON)
        );
        metrics_store.flush(run_id).await.unwrap();
        let metrics = metrics_store.list_for_run(run_id).await.unwrap();
        assert!(metrics.iter().any(|metric| {
            metric.metric_name == "similarity_score"
                && (metric.metric_value - 1.0).abs() < f64::EPSILON
        }));
        assert!(
            !metrics
                .iter()
                .any(|metric| metric.metric_name == "is_correct")
        );

        let steps = crate::db::list_steps_for_run(&db, run_id).await.unwrap();
        let output: serde_json::Value =
            serde_json::from_str(steps[0].output.as_deref().unwrap()).unwrap();
        assert_eq!(output["similarity_name"], "levenshtein");
        assert_eq!(output["similarity_score"], 1.0);
        assert!(output.get("embedding_model").is_none());
    }
}
