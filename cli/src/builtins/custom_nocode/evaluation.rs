use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::data::{DatasetRow, PreparedRow};
use super::metrics::SampleResult;
use crate::builtins::common::{hash_input, run_timed_step};

/// Per-row step output stored as JSON in the step record.
#[derive(Debug, Serialize, Deserialize)]
struct RowOutput {
    input: String,
    response: String,
    parsed_response: Option<String>,
    golden: String,
    is_correct: bool,
}

pub(super) struct EvaluateRowArgs<'a> {
    pub(super) i: usize,
    pub(super) row: &'a DatasetRow,
    pub(super) prepared: PreparedRow,
    pub(super) template_str: &'a str,
    pub(super) env: &'a jinja::Environment<'a>,
    pub(super) model_name: &'a str,
    pub(super) llm: &'a std::sync::Arc<dyn crate::llm::LLMSampler>,
    pub(super) db: &'a sea_orm::DatabaseConnection,
    pub(super) metrics_store: &'a crate::metrics_store::MetricsStore,
    pub(super) run_id: i64,
}

/// Render, sample, score, and record one normalized dataset row.
/// Returns the style-specific sample result used for aggregate metrics.
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
        "{rendered}\ngolden={}\nmodel={}\nworkflow={benchmark_name}",
        golden, args.model_name
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

            let parsed_response = match prepared.response_labels() {
                Some(choice_labels) => extract_choice_label(&model_response, choice_labels),
                None => Some(model_response.trim().to_owned()),
            };
            let is_correct = parsed_response.as_deref() == Some(golden.trim());

            Ok(RowOutput {
                input: rendered.clone(),
                response: model_response,
                parsed_response,
                golden: golden.clone(),
                is_correct,
            })
        },
    )
    .await?;

    if let Some(step_id) = step_id {
        args.metrics_store
            .emit(
                args.run_id,
                Some(step_id),
                "is_correct",
                if output.is_correct { 1.0 } else { 0.0 },
                None,
            )
            .await;
        args.metrics_store
            .emit(
                args.run_id,
                Some(step_id),
                "response_parsed",
                if output.parsed_response.is_some() {
                    1.0
                } else {
                    0.0
                },
                None,
            )
            .await;
    }

    Ok(SampleResult::new(output.is_correct))
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
    use serde_json::json;

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
}
