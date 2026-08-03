use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use reqwest::Url;

use super::proto::v1::{BenchmarkResource, ResolveBenchmarkResponse, ResourceKind};

/// Maximum number of resources allowed in a remote benchmark manifest.
const MAX_RESOURCE_COUNT: usize = 32;
/// Maximum declared and downloaded size of one benchmark resource.
pub(super) const MAX_RESOURCE_BYTES: u64 = 10 * 1024 * 1024;
/// Maximum aggregate declared size of all resources in a benchmark manifest.
const MAX_BUNDLE_BYTES: u64 = 50 * 1024 * 1024;

/// Validate that a response identifies the requested immutable benchmark manifest.
pub(super) fn validate_response_identity(
    benchmark_name: &str,
    response: &ResolveBenchmarkResponse,
) -> Result<()> {
    if response.benchmark_name != benchmark_name {
        bail!(
            "remote benchmark response name `{}` does not match requested name `{benchmark_name}`",
            response.benchmark_name
        );
    }
    if response.version.is_empty() {
        bail!("remote benchmark response is missing an immutable version");
    }
    validate_sha256("manifest", &response.manifest_sha256)?;
    Ok(())
}

/// Validate a manifest's resource identities, locations, kinds, digests, and sizes.
pub(super) fn validate_resources(
    resources: &[BenchmarkResource],
    allow_plaintext_downloads: bool,
) -> Result<Vec<BenchmarkResource>> {
    if resources.is_empty() {
        bail!("remote benchmark manifest contains no resources");
    }
    if resources.len() > MAX_RESOURCE_COUNT {
        bail!(
            "remote benchmark manifest contains {} resources; maximum is {MAX_RESOURCE_COUNT}",
            resources.len()
        );
    }

    let mut resource_ids = HashSet::new();
    let mut logical_paths = HashSet::new();
    let mut total_bytes = 0_u64;
    let mut definition_count = 0;

    for resource in resources {
        if resource.resource_id.is_empty() || !resource_ids.insert(resource.resource_id.clone()) {
            bail!("remote benchmark resource IDs must be non-empty and unique");
        }
        let logical_path = validate_logical_path(&resource.logical_path)?;
        if !logical_paths.insert(logical_path) {
            bail!("remote benchmark resource logical paths must be unique");
        }
        let kind = resource
            .kind
            .as_known()
            .with_context(|| format!("resource `{}` has an unknown kind", resource.logical_path))?;
        if kind == ResourceKind::Unspecified {
            bail!(
                "resource `{}` has an unspecified kind",
                resource.logical_path
            );
        }
        if kind == ResourceKind::Definition {
            definition_count += 1;
        }
        validate_download_url(&resource.download_url, allow_plaintext_downloads)?;
        validate_sha256(
            &format!("resource `{}`", resource.logical_path),
            &resource.sha256,
        )?;
        if resource.size_bytes > MAX_RESOURCE_BYTES {
            bail!(
                "resource `{}` declares {} bytes; maximum is {MAX_RESOURCE_BYTES}",
                resource.logical_path,
                resource.size_bytes
            );
        }
        total_bytes = total_bytes
            .checked_add(resource.size_bytes)
            .context("remote benchmark resource sizes overflowed")?;
        if total_bytes > MAX_BUNDLE_BYTES {
            bail!("remote benchmark declares more than {MAX_BUNDLE_BYTES} bytes");
        }
    }

    if definition_count != 1 {
        bail!("remote benchmark manifest must contain exactly one definition resource");
    }
    Ok(resources.to_vec())
}

/// Validate and normalize a resource path relative to its benchmark bundle.
pub(super) fn validate_logical_path(path: &str) -> Result<PathBuf> {
    if path.is_empty() || path.contains('\\') {
        bail!("resource logical path `{path}` is invalid");
    }
    let path = Path::new(path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!(
            "resource logical path `{}` must be a safe relative path",
            path.display()
        );
    }
    Ok(path.to_path_buf())
}

/// Validate a resource URL and enforce the benchmark service's transport security.
fn validate_download_url(download_url: &str, allow_plaintext: bool) -> Result<()> {
    let url = Url::parse(download_url)
        .with_context(|| format!("invalid benchmark resource URL `{download_url}`"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        bail!("benchmark resource URL must be an absolute http or https URL");
    }
    if url.scheme() == "http" && !allow_plaintext {
        bail!("an HTTPS benchmark service must return HTTPS resource URLs");
    }
    Ok(())
}

fn validate_sha256(label: &str, digest: &str) -> Result<()> {
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("{label} SHA-256 must be 64 lowercase hexadecimal characters");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_paths_reject_traversal_and_absolute_paths() {
        for path in ["../prompt.txt", "/prompt.txt", "prompts/../prompt.txt", ""] {
            assert!(validate_logical_path(path).is_err(), "accepted `{path}`");
        }
        assert_eq!(
            validate_logical_path("prompts/qa.txt").unwrap(),
            PathBuf::from("prompts/qa.txt")
        );
    }
}
