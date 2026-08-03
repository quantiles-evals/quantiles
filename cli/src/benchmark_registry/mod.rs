//! Client support for resolving remotely hosted benchmark definitions.

use anyhow::Result;

pub use self::benchmark::RemoteBenchmark;
pub use self::client::select_remote_url;
use self::client::{resolve_manifest, validate_remote_url};
use self::download::download_resources;
use self::manifest::{validate_resources, validate_response_identity};

mod benchmark;
mod client;
mod download;
mod manifest;
mod proto;

/// Resolve a benchmark and download all of its resources into memory.
///
/// `Ok(None)` means the registry returned Connect's `not_found` status. Other transport and
/// service failures are returned to the caller rather than treated as absence.
///
/// # Errors
///
/// Returns an error for invalid endpoints, RPC failures, malformed manifests, failed downloads,
/// digest mismatches, invalid UTF-8, or invalid no-code benchmark definitions.
pub async fn resolve_and_download(
    benchmark_name: &str,
    remote_url: &str,
) -> Result<Option<RemoteBenchmark>> {
    let endpoint = validate_remote_url(remote_url)?;
    let Some(response) = resolve_manifest(benchmark_name, &endpoint).await? else {
        return Ok(None);
    };

    validate_response_identity(benchmark_name, &response)?;
    let resources = validate_resources(&response.resources, endpoint.scheme() == "http")?;
    let downloaded = download_resources(&resources).await?;
    let remote = RemoteBenchmark::new(benchmark_name, response, downloaded)?;
    Ok(Some(remote))
}
