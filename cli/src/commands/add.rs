use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;

pub async fn add(benchmark_name: &str, cli_remote_url: Option<&str>, json: bool) -> Result<()> {
    let cwd = std::env::current_dir().context("failed to determine current directory")?;
    let config_path = config_path(&cwd)?;
    let existing = read_config(config_path.as_deref())?;

    if existing
        .as_ref()
        .is_some_and(|config| config.benchmarks.contains_key(benchmark_name))
    {
        bail!("benchmark `{benchmark_name}` is already present in the local configuration");
    }

    let remote_url = qt::benchmark_registry::select_remote_url(
        cli_remote_url,
        std::env::var_os("QUANTILES_REMOTE_URL"),
    )?;
    let remote = qt::benchmark_registry::resolve_and_download(benchmark_name, None, &remote_url)
        .await?
        .with_context(|| {
            format!("benchmark `{benchmark_name}` was not found in the remote registry")
        })?;

    let config_path = config_path.unwrap_or_else(|| cwd.join("quantiles.toml"));
    let version = remote.version.clone();
    persist_remote_benchmark(benchmark_name, remote, &config_path)?;

    if json {
        println!(
            "{}",
            serde_json::to_string(&AddOutput {
                benchmark_name,
                version: &version,
                config_path: config_path.display().to_string(),
            })?
        );
    } else {
        println!(
            "Added benchmark `{benchmark_name}` (version {version}) to {}",
            config_path.display()
        );
    }
    Ok(())
}

#[derive(Serialize)]
struct AddOutput<'a> {
    benchmark_name: &'a str,
    version: &'a str,
    config_path: String,
}

fn config_path(cwd: &Path) -> Result<Option<PathBuf>> {
    let plain = cwd.join("quantiles.toml");
    let dot = cwd.join(".quantiles.toml");
    match (plain.exists(), dot.exists()) {
        (true, true) => bail!(
            "both `quantiles.toml` and `.quantiles.toml` found in {}. remove one to avoid ambiguity",
            cwd.display()
        ),
        (true, false) => Ok(Some(plain)),
        (false, true) => Ok(Some(dot)),
        (false, false) => Ok(None),
    }
}

fn read_config(path: Option<&Path>) -> Result<Option<qt::config::WorkspaceConfig>> {
    let Some(path) = path else {
        return Ok(None);
    };
    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let config =
        toml::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(config))
}

fn persist_remote_benchmark(
    benchmark_name: &str,
    mut remote: qt::benchmark_registry::RemoteBenchmark,
    config_path: &Path,
) -> Result<()> {
    let prompt_relative = prompt_relative_path(benchmark_name)?;
    let config_dir = config_path
        .parent()
        .context("configuration path has no parent directory")?;
    let prompt_path = config_dir.join(&prompt_relative);
    persist_prompt(&prompt_path, remote.prompt_template.as_bytes())?;
    remote.config.params.prompt_template_file = path_for_toml(&prompt_relative);

    let section = render_benchmark_section(benchmark_name, &remote.config)?;
    append_section(config_path, &section)
}

fn prompt_relative_path(benchmark_name: &str) -> Result<PathBuf> {
    let unsafe_character = benchmark_name
        .chars()
        .any(|character| character.is_control() || r#"/\:*?"<>|"#.contains(character));
    if benchmark_name.is_empty() || matches!(benchmark_name, "." | "..") || unsafe_character {
        bail!("benchmark name `{benchmark_name}` cannot be used for a local prompt directory");
    }
    Ok(PathBuf::from(format!("{benchmark_name}-prompt")).join("prompt.txt"))
}

fn persist_prompt(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .context("prompt path has no parent directory")?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => file
            .write_all(contents)
            .with_context(|| format!("failed to write {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing = fs::read(path)
                .with_context(|| format!("failed to read existing {}", path.display()))?;
            if existing == contents {
                Ok(())
            } else {
                bail!(
                    "existing registry prompt {} has unexpected contents",
                    path.display()
                )
            }
        }
        Err(error) => Err(error).with_context(|| format!("failed to create {}", path.display())),
    }
}

fn render_benchmark_section(
    benchmark_name: &str,
    config: &qt::config::CustomNoCodeBenchmarkConfig,
) -> Result<String> {
    let mut benchmark = toml::Value::try_from(&config.params)
        .context("failed to serialize remote benchmark configuration")?
        .as_table()
        .cloned()
        .context("remote benchmark configuration did not serialize to a TOML table")?;
    benchmark.insert(
        "type".to_owned(),
        toml::Value::String("custom_nocode".to_owned()),
    );

    let mut benchmarks = toml::map::Map::new();
    benchmarks.insert(benchmark_name.to_owned(), toml::Value::Table(benchmark));
    let mut root = toml::map::Map::new();
    root.insert("benchmarks".to_owned(), toml::Value::Table(benchmarks));
    toml::to_string_pretty(&toml::Value::Table(root))
        .context("failed to render remote benchmark configuration as TOML")
}

fn append_section(path: &Path, section: &str) -> Result<()> {
    let existing = if path.exists() {
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?
    } else {
        String::new()
    };
    let separator = if existing.is_empty() || existing.ends_with("\n\n") {
        ""
    } else if existing.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    write!(file, "{separator}{section}")
        .with_context(|| format!("failed to append benchmark to {}", path.display()))
}

fn path_for_toml(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use qt::config::{CustomNoCodeDatasetConfig, CustomNoCodeParams, CustomNoCodeStyleConfig};

    fn remote() -> qt::benchmark_registry::RemoteBenchmark {
        qt::benchmark_registry::RemoteBenchmark {
            config: qt::config::CustomNoCodeBenchmarkConfig {
                type_: "custom_nocode".to_owned(),
                params: CustomNoCodeParams {
                    dataset: CustomNoCodeDatasetConfig {
                        name: "quantiles/example".to_owned(),
                        config_name: None,
                        split: None,
                        revision: None,
                    },
                    model: None,
                    prompt_template_file: "prompts/qa.txt".to_owned(),
                    limit: None,
                    max_workers: None,
                    metrics: vec![],
                    style: CustomNoCodeStyleConfig::ExactMatch {
                        golden_column: "answer".to_owned(),
                    },
                },
            },
            prompt_template: "{{ row.question }}".to_owned(),
            version: "v1".to_owned(),
            manifest_sha256: "a".repeat(64),
        }
    }

    #[test]
    fn creates_runnable_configuration_and_prompt() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("quantiles.toml");

        persist_remote_benchmark("remote-test", remote(), &config_path).unwrap();

        let contents = fs::read_to_string(&config_path).unwrap();
        let parsed: qt::config::WorkspaceConfig = toml::from_str(&contents).unwrap();
        assert!(parsed.benchmarks.contains_key("remote-test"));
        assert_eq!(
            fs::read_to_string(temp.path().join("remote-test-prompt/prompt.txt")).unwrap(),
            "{{ row.question }}"
        );
        let qt::config::BenchmarkConfig::CustomNoCode(config) =
            parsed.benchmarks.get("remote-test").unwrap()
        else {
            panic!("expected custom_nocode benchmark");
        };
        assert_eq!(
            config.params.prompt_template_file,
            "remote-test-prompt/prompt.txt"
        );
    }

    #[test]
    fn appends_after_existing_configuration_without_rewriting_it() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("quantiles.toml");
        let original = "# keep this comment\n[benchmarks.local]\ntype = \"custom_code\"\ncommand = [\"echo\"]\n";
        fs::write(&config_path, original).unwrap();

        persist_remote_benchmark("remote-test", remote(), &config_path).unwrap();

        let contents = fs::read_to_string(config_path).unwrap();
        assert!(contents.starts_with(original));
        assert!(contents.contains("[benchmarks.remote-test]"));
    }

    #[test]
    fn rejects_benchmark_names_that_cannot_form_a_safe_prompt_directory() {
        for name in ["../outside", "nested/name", r"nested\name"] {
            let temp = tempfile::tempdir().unwrap();
            let config_path = temp.path().join("quantiles.toml");
            let error = persist_remote_benchmark(name, remote(), &config_path).unwrap_err();
            assert!(error.to_string().contains("local prompt directory"));
            assert!(!config_path.exists());
        }
    }
}
