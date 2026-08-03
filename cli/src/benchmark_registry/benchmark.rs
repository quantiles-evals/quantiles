use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::manifest::validate_logical_path;
use super::proto::v1::{ResolveBenchmarkResponse, ResourceKind};
use crate::config::{BenchmarkConfig, CustomNoCodeBenchmarkConfig, WorkspaceConfig};

/// A downloaded benchmark ready to execute without materializing its resources on disk.
pub struct RemoteBenchmark {
    pub config: CustomNoCodeBenchmarkConfig,
    pub prompt_template: String,
    pub version: String,
    pub manifest_sha256: String,
}

pub(super) fn build_remote_benchmark(
    benchmark_name: &str,
    response: ResolveBenchmarkResponse,
    mut downloaded: HashMap<PathBuf, Vec<u8>>,
) -> Result<RemoteBenchmark> {
    let definition = response
        .resources
        .iter()
        .find(|resource| resource.kind.as_known() == Some(ResourceKind::Definition))
        .context("remote benchmark definition resource is missing")?;
    let definition_path = validate_logical_path(&definition.logical_path)?;
    let definition_bytes = downloaded
        .remove(&definition_path)
        .context("downloaded benchmark definition is missing")?;
    let definition_toml = std::str::from_utf8(&definition_bytes)
        .context("remote benchmark definition is not valid UTF-8")?;
    let mut workspace: WorkspaceConfig = toml::from_str(definition_toml)
        .context("failed to parse remote benchmark definition as Quantiles TOML")?;
    let benchmark = workspace
        .benchmarks
        .remove(benchmark_name)
        .with_context(|| {
            format!("remote definition does not contain benchmark `{benchmark_name}`")
        })?;
    let BenchmarkConfig::CustomNoCode(config) = benchmark else {
        bail!("remote benchmark `{benchmark_name}` must have type `custom_nocode`");
    };
    config.params.style.validate()?;
    config.params.validate_metrics()?;

    let prompt_path = definition_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(validate_logical_path(&config.params.prompt_template_file)?);
    let prompt_resource = response.resources.iter().find(|resource| {
        resource.kind.as_known() == Some(ResourceKind::PromptTemplate)
            && validate_logical_path(&resource.logical_path).ok().as_ref() == Some(&prompt_path)
    });
    if prompt_resource.is_none() {
        bail!(
            "remote benchmark prompt template `{}` is not declared as a prompt-template resource",
            prompt_path.display()
        );
    }
    let prompt_bytes = downloaded.remove(&prompt_path).with_context(|| {
        format!(
            "downloaded prompt template `{}` is missing",
            prompt_path.display()
        )
    })?;
    let prompt_template = String::from_utf8(prompt_bytes).with_context(|| {
        format!(
            "prompt template `{}` is not valid UTF-8",
            prompt_path.display()
        )
    })?;

    Ok(RemoteBenchmark {
        config: *config,
        prompt_template,
        version: response.version,
        manifest_sha256: response.manifest_sha256,
    })
}
