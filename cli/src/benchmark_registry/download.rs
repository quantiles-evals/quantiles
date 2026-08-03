use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use futures::StreamExt as _;
use sha2::{Digest as _, Sha256};

use super::manifest::{MAX_RESOURCE_BYTES, validate_logical_path};
use super::proto::v1::BenchmarkResource;

/// Download validated resources into memory and verify their sizes and digests.
pub(super) async fn download_resources(
    resources: &[BenchmarkResource],
) -> Result<HashMap<PathBuf, Vec<u8>>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to create benchmark resource HTTP client")?;
    let mut downloaded = HashMap::with_capacity(resources.len());

    for resource in resources {
        let response = client
            .get(&resource.download_url)
            .send()
            .await
            .with_context(|| format!("failed to download resource `{}`", resource.logical_path))?
            .error_for_status()
            .with_context(|| format!("failed to download resource `{}`", resource.logical_path))?;
        let mut bytes = Vec::with_capacity(usize::try_from(resource.size_bytes).unwrap_or(0));
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.with_context(|| {
                format!(
                    "failed while downloading resource `{}`",
                    resource.logical_path
                )
            })?;
            let next_len = bytes
                .len()
                .checked_add(chunk.len())
                .context("downloaded benchmark resource size overflowed")?;
            if next_len > usize::try_from(MAX_RESOURCE_BYTES).unwrap_or(usize::MAX) {
                bail!(
                    "resource `{}` exceeded the download size limit",
                    resource.logical_path
                );
            }
            bytes.extend_from_slice(&chunk);
        }

        if u64::try_from(bytes.len()).ok() != Some(resource.size_bytes) {
            bail!(
                "resource `{}` size mismatch: expected {}, downloaded {}",
                resource.logical_path,
                resource.size_bytes,
                bytes.len()
            );
        }
        let actual_sha256 = format!("{:x}", Sha256::digest(&bytes));
        if actual_sha256 != resource.sha256 {
            bail!(
                "resource `{}` failed SHA-256 verification",
                resource.logical_path
            );
        }
        downloaded.insert(validate_logical_path(&resource.logical_path)?, bytes);
    }

    Ok(downloaded)
}
