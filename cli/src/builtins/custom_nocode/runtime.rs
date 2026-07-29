use std::sync::Arc;

use anyhow::{Context, Result, bail};

use super::data::PromptChoice;
use crate::builtins::common::resolve_sampler;
use crate::dataset::DatasetManager;
use crate::llm::random::RandomSampler;
use crate::llm::random_label::RandomLabelSampler;

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

    if config.samples == Some(0) {
        bail!("samples must be > 0");
    }

    Ok(config)
}

/// Read a prompt template and validate its Jinja syntax against the available variables.
pub(super) fn load_template(path: &str) -> Result<(String, jinja::Environment<'_>)> {
    let template_str = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read prompt template file `{path}`"))?;
    let env = jinja::Environment::new();
    env.render_str(
        &template_str,
        jinja::context!(row => serde_json::json!({}), choices => Vec::<PromptChoice>::new()),
    )
    .with_context(|| format!("invalid jinja syntax in prompt template file `{path}`"))?;
    Ok((template_str, env))
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
        .context("could not determine dataset size; pass an explicit samples value")?;
    let limit = limit.unwrap_or(total).min(total);

    Ok((manager, info, limit))
}

#[cfg(test)]
mod tests {
    use super::*;

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
