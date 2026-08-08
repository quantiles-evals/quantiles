use std::sync::Arc;

use anyhow::{Context, Result, bail};

use crate::builtins::common::resolve_sampler;
use crate::dataset::DatasetManager;
use crate::llm::random::RandomSampler;
use crate::llm::random_label::RandomLabelSampler;

/// A similarity scorer resolved once and shared by every row in a run.
#[derive(Clone)]
pub(super) struct SimilarityMetric {
    pub(super) name: &'static str,
    pub(super) embedding_model: Option<&'static str>,
    pub(super) metric: Arc<dyn crate::similarity::SimilarityMetric>,
}

impl SimilarityMetric {
    /// Resolve the configured similarity metric when the selected style needs one.
    pub(super) fn new(style: &crate::config::CustomNoCodeStyleConfig) -> Result<Option<Self>> {
        let crate::config::CustomNoCodeStyleConfig::Similarity { metric, .. } = style else {
            return Ok(None);
        };

        let resolved = match metric {
            crate::config::CustomNoCodeSimilarityMetric::Levenshtein(_) => Self {
                name: "levenshtein",
                embedding_model: None,
                metric: crate::similarity::build_similarity_metric(
                    crate::similarity::SimilarityMetricName::Levenshtein,
                )?,
            },
            crate::config::CustomNoCodeSimilarityMetric::Cosine(config) => {
                let embedding_model = match config.embedding_model {
                    crate::config::CustomNoCodeEmbeddingModel::Fastembed => "fastembed",
                };
                Self {
                    name: "cosine",
                    embedding_model: Some(embedding_model),
                    metric: crate::similarity::build_similarity_metric(
                        crate::similarity::SimilarityMetricName::Cosine,
                    )?,
                }
            }
        };

        Ok(Some(resolved))
    }
}

/// A validated prompt template and the Jinja environment
/// used to render it.
pub(super) struct LoadedTemplate {
    /// The prompt template source.
    pub(super) template: String,
    /// The environment used to render the template.
    pub(super) environment: jinja::Environment<'static>,
}

/// Resolve the configured sampler, using configured choice labels for random multiple-choice runs.
pub(super) fn resolve_sampler_for_style(
    model: Option<&crate::llm::Sampler>,
    style: &crate::config::CustomNoCodeStyleConfig,
) -> Result<Arc<dyn crate::llm::LLMSampler>> {
    if model.is_none_or(|sampler| matches!(sampler, crate::llm::Sampler::Random))
        && let crate::config::CustomNoCodeStyleConfig::MultipleChoice { choice_labels, .. } = style
    {
        return Ok(Arc::new(RandomLabelSampler::new(choice_labels)));
    }

    resolve_sampler(model, || Arc::new(RandomSampler::new(80)))
}

/// Deserialize and validate the runtime input required by a custom no-code benchmark.
pub(super) fn parse_input(input: Option<&str>) -> Result<crate::config::CustomNoCodeParams> {
    let config: crate::config::CustomNoCodeParams = input
        .map(serde_json::from_str)
        .transpose()
        .context("invalid builtin input JSON")?
        .context("custom_nocode benchmark requires input configuration")?;

    if config.limit == Some(0) {
        bail!("limit must be > 0");
    }

    Ok(config)
}

/// Read a prompt template and validate its Jinja syntax without rendering it.
pub(super) fn load_template(path: &str) -> Result<LoadedTemplate> {
    let template_str = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read prompt template file `{path}`"))?;
    let env = validate_template(&template_str, &format!("prompt template file `{path}`"))?;
    Ok(LoadedTemplate {
        template: template_str,
        environment: env,
    })
}

/// Validate an in-memory prompt template and return its rendering environment.
pub(super) fn load_template_string(template_str: String) -> Result<LoadedTemplate> {
    let env = validate_template(&template_str, "remote prompt template")?;
    Ok(LoadedTemplate {
        template: template_str,
        environment: env,
    })
}

fn validate_template(template_str: &str, source: &str) -> Result<jinja::Environment<'static>> {
    let mut env = jinja::Environment::new();
    env.add_template_owned(source.to_owned(), template_str.to_owned())
        .with_context(|| format!("invalid jinja syntax in {source}"))?;
    Ok(env)
}

/// Initialize the configured dataset and clamp the requested limit to its available row count.
pub(super) async fn resolve_dataset_limit(
    dataset: &str,
    dataset_config: Option<&str>,
    split: Option<&str>,
    revision: Option<&str>,
    limit: Option<usize>,
) -> Result<(DatasetManager, crate::dataset::DatasetInfo, usize)> {
    let manager = DatasetManager::new()?;
    let info = manager
        .init(dataset, dataset_config, split, revision)
        .await?;

    let total = info
        .total_rows
        .context("could not determine dataset size; pass an explicit limit")?;
    let limit = limit.unwrap_or(total).min(total);

    Ok((manager, info, limit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_validation_allows_nested_row_fields() {
        let template = r#"{{ row.context.contexts | join("\n") }}"#;
        let env = validate_template(template, "test template").unwrap();
        let rendered = env
            .render_str(
                template,
                jinja::context!(row => serde_json::json!({
                    "context": {"contexts": ["first", "second"]}
                })),
            )
            .unwrap();

        assert_eq!(rendered, "first\nsecond");
    }

    #[test]
    fn template_validation_rejects_invalid_syntax() {
        let error = validate_template("{{ row.question", "test template").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("invalid jinja syntax in test template")
        );
    }

    /// Build a standard four-label style for sampler-selection tests.
    fn multiple_choice_style() -> crate::config::CustomNoCodeStyleConfig {
        crate::config::CustomNoCodeStyleConfig::MultipleChoice {
            choices: crate::config::CustomNoCodeChoiceSource::Column(
                crate::config::CustomNoCodeChoiceColumn {
                    column: "options".to_owned(),
                },
            ),
            answer: crate::config::CustomNoCodeAnswerSource::LabelColumn(
                crate::config::CustomNoCodeLabelAnswer {
                    label_column: "answer".to_owned(),
                },
            ),
            choice_labels: ["A", "B", "C", "D"].map(str::to_owned).to_vec(),
            shuffle: None,
        }
    }

    #[tokio::test]
    /// Verifies implicit and explicit random models sample only configured choice labels.
    async fn multiple_choice_random_sampler_uses_configured_labels() {
        let style = multiple_choice_style();
        let configured_random = crate::llm::Sampler::Random;

        for model in [None, Some(&configured_random)] {
            let sampler = resolve_sampler_for_style(model, &style).unwrap();
            for _ in 0..100 {
                let response = sampler.sample("ignored prompt").await.unwrap();
                assert!(matches!(response.as_str(), "A" | "B" | "C" | "D"));
            }
        }
    }

    #[tokio::test]
    /// Verifies exact-match runs retain the generic random-text sampler.
    async fn exact_match_random_sampler_remains_alphanumeric() {
        let style = crate::config::CustomNoCodeStyleConfig::ExactMatch {
            golden_column: "answer".to_owned(),
        };
        let sampler =
            resolve_sampler_for_style(Some(&crate::llm::Sampler::Random), &style).unwrap();

        for _ in 0..100 {
            let response = sampler.sample("ignored prompt").await.unwrap();
            assert!(!response.is_empty());
            assert!(response.len() <= 80);
            assert!(
                response
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
            );
        }
    }
}
