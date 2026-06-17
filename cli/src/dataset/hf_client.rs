use anyhow::{Context, Result};
use reqwest::header;
use serde::Deserialize;
use serde_json::Value;

static HF_DATASETS_SERVER: &str = "https://datasets-server.huggingface.co";

/// Thin async HTTP client for the huggingface Dataset Viewer API.
pub struct HuggingFaceClient {
    http: reqwest::Client,
}

/// Splits response from HF API.
#[derive(Debug, Deserialize)]
struct SplitsResponse {
    splits: Vec<SplitItem>,
}

#[derive(Debug, Deserialize)]
struct SplitItem {
    #[serde(default)]
    config: String,
    #[serde(default)]
    split: String,
}

/// Rows response from HF API.
#[derive(Debug, Deserialize)]
struct RowsResponse {
    #[serde(default)]
    #[expect(dead_code)]
    features: Vec<Value>,
    #[serde(default)]
    rows: Vec<RowItem>,
}

#[derive(Debug, Deserialize)]
struct RowItem {
    #[serde(default)]
    row: Value,
}

/// Dataset size/info response.
#[derive(Debug, Deserialize)]
pub struct InfoResponse {
    #[serde(default)]
    pub num_examples: Option<usize>,
    #[serde(default)]
    pub num_examples_approximate: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct SizeResponse {
    #[serde(default)]
    size: SizeInner,
}

#[derive(Debug, Deserialize, Default)]
struct SizeInner {
    #[serde(default)]
    splits: Vec<SplitSize>,
}

#[derive(Debug, Deserialize)]
struct SplitSize {
    #[serde(default)]
    num_rows: Option<usize>,
}

impl HuggingFaceClient {
    /// Build a new client, injecting `HF_TOKEN` when present.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built.
    pub fn new() -> Result<Self> {
        let mut headers = header::HeaderMap::new();
        if let Ok(token) = std::env::var("HF_TOKEN") {
            headers.insert(
                header::AUTHORIZATION,
                header::HeaderValue::from_str(&format!("Bearer {token}"))
                    .context("invalid HF_TOKEN")?,
            );
        }

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .context("failed to build reqwest client")?;

        Ok(Self { http })
    }

    /// Query available splits for a dataset + config.
    ///
    /// # Errors
    ///
    /// Returns an error  if the splits for the given dataset at the
    /// given revision could not be fetched.
    pub async fn splits(
        &self,
        dataset_id: &str,
        config: &str,
        revision: Option<&str>,
    ) -> Result<Vec<String>> {
        let mut url = format!("{HF_DATASETS_SERVER}/splits?dataset={dataset_id}&config={config}");
        if let Some(rev) = revision {
            url = format!("{url}&revision={rev}");
        }

        let resp = self.http.get(&url).send().await?;
        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!(
                "dataset_unavailable: dataset `{dataset_id}` not found or not supported by the viewer API"
            );
        }
        let body: SplitsResponse = resp.error_for_status()?.json().await?;
        Ok(body.splits.into_iter().map(|s| s.split).collect())
    }

    /// Infer the default config for a dataset.
    ///
    /// # Errors
    ///
    /// Returns an error if configs couldn't be inferred from the given dataset
    /// and revision.
    pub async fn infer_config(&self, dataset_id: &str, revision: Option<&str>) -> Result<String> {
        let mut url = format!("{HF_DATASETS_SERVER}/splits?dataset={dataset_id}");
        if let Some(rev) = revision {
            url = format!("{url}&revision={rev}");
        }

        let resp = self.http.get(&url).send().await?;
        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("dataset_unavailable: dataset `{dataset_id}` not found");
        }
        let body: SplitsResponse = resp.error_for_status()?.json().await?;
        if body.splits.is_empty() {
            anyhow::bail!("dataset `{dataset_id}` has no splits");
        }
        Ok(body.splits[0].config.clone())
    }

    /// Fetch rows for a dataset slice.
    ///
    /// # Errors
    ///
    /// Returns an error if the rows in the range [offset, offset+limit)
    /// could not be fetched.
    pub async fn rows(
        &self,
        dataset_id: &str,
        config: &str,
        split: &str,
        offset: usize,
        limit: usize,
        revision: Option<&str>,
    ) -> Result<Vec<Value>> {
        let mut url = format!(
            "{HF_DATASETS_SERVER}/rows?dataset={dataset_id}&config={config}&split={split}&offset={offset}&length={limit}"
        );
        if let Some(rev) = revision {
            url = format!("{url}&revision={rev}");
        }

        let resp = self.http.get(&url).send().await?;
        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!(
                "dataset_unavailable: rows not found for `{dataset_id}/{config}/{split}`"
            );
        }
        let body: RowsResponse = resp.error_for_status()?.json().await?;
        Ok(body.rows.into_iter().map(|r| r.row).collect())
    }

    /// Fetch dataset info (total row count, etc.).
    ///
    /// # Errors
    ///
    /// Returns an error if dataset info could not be fetched.
    pub async fn info(
        &self,
        dataset_id: &str,
        config: &str,
        split: &str,
        revision: Option<&str>,
    ) -> Result<InfoResponse> {
        let mut url =
            format!("{HF_DATASETS_SERVER}/size?dataset={dataset_id}&config={config}&split={split}");
        if let Some(rev) = revision {
            url = format!("{url}&revision={rev}");
        }

        let resp = self.http.get(&url).send().await?;
        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("dataset_unavailable: size info not found for `{dataset_id}`");
        }
        let body: SizeResponse = resp.error_for_status()?.json().await?;
        let num_examples = body.size.splits.iter().find_map(|s| s.num_rows);
        Ok(InfoResponse {
            num_examples,
            num_examples_approximate: num_examples,
        })
    }
}
