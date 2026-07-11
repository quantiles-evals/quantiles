pub mod cache;
pub mod hf_client;

use crate::dataset::cache::DatasetCache;
use crate::dataset::hf_client::HuggingFaceClient;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::Value;

/// Metadata returned after initializing a dataset source.
#[derive(Debug, Serialize)]
pub struct DatasetInfo {
    pub total_rows: Option<usize>,
    pub available_splits: Vec<String>,
    pub selected_split: String,
    pub config: String,
}

/// Central manager that coordinates fetching from huggingface and local caching.
pub struct DatasetManager {
    client: HuggingFaceClient,
    pub(crate) cache: DatasetCache,
}

impl DatasetManager {
    /// Create a new manager with the default cache location.
    ///
    /// # Errors
    ///
    /// Returns an error if the system's cache directory cannot be determined.
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::home_dir()
            .context("failed to determine home directory")?
            .join(".cache")
            .join("quantiles")
            .join("datasets");
        let client = HuggingFaceClient::new()?;
        Ok(Self {
            client,
            cache: DatasetCache::new(cache_dir),
        })
    }

    #[cfg(test)]
    /// Create a manager backed by a specific cache directory (useful for tests
    /// that pre-populate local fixture files).
    ///
    /// # Errors
    ///
    /// Returns an error if the `HuggingFace` client cannot be created.
    pub fn new_with_cache_dir(cache_dir: std::path::PathBuf) -> Result<Self> {
        let client = HuggingFaceClient::new()?;
        Ok(Self {
            client,
            cache: DatasetCache::new(cache_dir),
        })
    }

    /// Initialize a huggingface dataset: validate it exists, pick a split,
    /// and return metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if the dataset is unreachable, has no splits, or
    /// the requested split/config does not exist.
    pub async fn init(
        &self,
        dataset_id: &str,
        config: Option<&str>,
        split: Option<&str>,
        revision: Option<&str>,
    ) -> Result<DatasetInfo> {
        let config = if let Some(c) = config {
            c.to_string()
        } else {
            self.client.infer_config(dataset_id, revision).await?
        };

        let splits = self.client.splits(dataset_id, &config, revision).await?;
        if splits.is_empty() {
            anyhow::bail!("dataset `{dataset_id}` has no splits");
        }

        let selected_split = if let Some(s) = split {
            if splits.contains(&s.to_string()) {
                s.to_string()
            } else {
                anyhow::bail!("split `{s}` not found; available: {}", splits.join(", "))
            }
        } else {
            Self::pick_split(&splits)?
        };

        let info = self
            .client
            .info(dataset_id, &config, &selected_split, revision)
            .await?;

        Ok(DatasetInfo {
            total_rows: info.num_examples.or(info.num_examples_approximate),
            available_splits: splits,
            selected_split,
            config,
        })
    }

    /// Fetch a batch of rows, using the local cache when available.
    ///
    /// # Errors
    ///
    /// Returns an error on network failure, parsing failure, or cache I/O failure.
    pub async fn batch(
        &self,
        dataset_id: &str,
        config: &str,
        split: &str,
        offset: usize,
        limit: usize,
        revision: Option<&str>,
    ) -> Result<Vec<Value>> {
        let cache_key = cache::cache_key(dataset_id, config, split, revision);
        let cache_path = self.cache.batch_path(&cache_key, offset, limit);

        // Serve from cache if the file already exists.
        if tokio::fs::try_exists(&cache_path).await.unwrap_or(false) {
            return self.cache.read_batch(&cache_path).await;
        }

        let rows = self
            .client
            .rows(dataset_id, config, split, offset, limit, revision)
            .await?;

        self.cache.write_batch(&cache_path, &rows).await?;
        Ok(rows)
    }

    fn pick_split(splits: &[String]) -> Result<String> {
        let preferred = ["test", "validation", "eval", "train"];
        for candidate in preferred {
            if let Some(s) = splits.iter().find(|sp| sp == &candidate) {
                return Ok(s.clone());
            }
        }
        splits.first().cloned().context("dataset has no splits")
    }
}

/// Resolve a configured Hugging Face dataset source to the dataset ID expected
/// by the existing Hugging Face download client.
pub fn resolve_hf_dataset_source(source: &str) -> Result<&str> {
    if let Some(dataset_id) = source
        .strip_prefix("hf://")
        .or_else(|| source.strip_prefix("huggingface://"))
    {
        if dataset_id.is_empty() {
            bail!("dataset source `{source}` is missing a Hugging Face dataset id");
        }
        Ok(dataset_id)
    } else if source.contains("://") {
        bail!("unsupported dataset source `{source}`; expected `hf://...` or `huggingface://...`");
    } else {
        bail!("dataset source `{source}` is missing required `hf://` or `huggingface://` prefix");
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_hf_dataset_source;

    #[test]
    fn resolve_hf_dataset_source_strips_hf_prefix() {
        assert_eq!(
            resolve_hf_dataset_source("hf://quantiles/PubMedQA").unwrap(),
            "quantiles/PubMedQA"
        );
    }

    #[test]
    fn resolve_hf_dataset_source_strips_huggingface_prefix() {
        assert_eq!(
            resolve_hf_dataset_source("huggingface://quantiles/PubMedQA").unwrap(),
            "quantiles/PubMedQA"
        );
    }

    #[test]
    fn resolve_hf_dataset_source_rejects_other_prefixes() {
        let err = resolve_hf_dataset_source("s3://bucket/dataset").unwrap_err();
        assert!(
            err.to_string()
                .contains("unsupported dataset source `s3://bucket/dataset`")
        );
    }

    #[test]
    fn resolve_hf_dataset_source_requires_prefix() {
        let err = resolve_hf_dataset_source("quantiles/PubMedQA").unwrap_err();
        assert!(
            err.to_string()
                .contains("missing required `hf://` or `huggingface://` prefix")
        );
    }
}
