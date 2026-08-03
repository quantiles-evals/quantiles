//! Client support for resolving remotely hosted benchmark definitions.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use connectrpc::ErrorCode;
use connectrpc::client::{ClientConfig, HttpClient};
use futures::StreamExt as _;
use reqwest::Url;
use rustls_platform_verifier::ConfigVerifierExt as _;
use sha2::{Digest as _, Sha256};

use crate::config::{BenchmarkConfig, CustomNoCodeBenchmarkConfig, WorkspaceConfig};

#[expect(
    clippy::allow_attributes,
    clippy::pedantic,
    reason = "ConnectRPC and Buffa generated code uses allow attributes"
)]
mod proto {
    connectrpc::include_generated!();
}

use proto::quantiles::benchmark::v1::{
    BenchmarkRegistryServiceClient, BenchmarkResource, ResolveBenchmarkRequest,
    ResolveBenchmarkResponse, ResourceKind,
};

pub(crate) const DEFAULT_REMOTE_URL: &str = "https://api.quantiles.io";
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_RESOURCE_COUNT: usize = 32;
const MAX_RESOURCE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_BUNDLE_BYTES: u64 = 50 * 1024 * 1024;

/// A downloaded benchmark ready to execute without materializing its resources on disk.
pub struct RemoteBenchmark {
    pub config: CustomNoCodeBenchmarkConfig,
    pub prompt_template: String,
    pub version: String,
    pub manifest_sha256: String,
}

/// Select the remote service URL and reject ambiguous CLI/environment configuration.
///
/// # Errors
///
/// Returns an error when both sources are set or the environment value is not valid UTF-8.
pub fn select_remote_url(cli_url: Option<&str>, env_url: Option<OsString>) -> Result<String> {
    if cli_url.is_some() && env_url.is_some() {
        bail!("cannot use both `--remote-url` and `QUANTILES_REMOTE_URL`");
    }

    if let Some(url) = cli_url {
        return Ok(url.to_owned());
    }

    env_url.map_or_else(
        || Ok(DEFAULT_REMOTE_URL.to_owned()),
        |url| {
            url.into_string()
                .map_err(|_| anyhow::anyhow!("`QUANTILES_REMOTE_URL` must be valid UTF-8"))
        },
    )
}

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
    let remote = build_remote_benchmark(benchmark_name, response, downloaded)?;
    Ok(Some(remote))
}

fn validate_remote_url(remote_url: &str) -> Result<Url> {
    let mut url = Url::parse(remote_url)
        .with_context(|| format!("invalid remote benchmark service URL `{remote_url}`"))?;
    if !matches!(url.scheme(), "http" | "https") {
        bail!("remote benchmark service URL must use http or https");
    }
    if url.host_str().is_none() {
        bail!("remote benchmark service URL must include a host");
    }
    if url.query().is_some() || url.fragment().is_some() {
        bail!("remote benchmark service URL must not include a query or fragment");
    }
    while url.path().ends_with('/') && url.path() != "/" {
        let trimmed = url.path().trim_end_matches('/').to_owned();
        url.set_path(&trimmed);
    }
    Ok(url)
}

async fn resolve_manifest(
    benchmark_name: &str,
    endpoint: &Url,
) -> Result<Option<ResolveBenchmarkResponse>> {
    let uri = endpoint
        .as_str()
        .parse()
        .with_context(|| format!("invalid remote benchmark service URL `{endpoint}`"))?;
    let transport = if endpoint.scheme() == "https" {
        let tls = connectrpc::rustls::ClientConfig::with_platform_verifier()
            .context("failed to configure TLS for remote benchmark service")?;
        HttpClient::with_tls(Arc::new(tls))
    } else {
        HttpClient::plaintext()
    };
    let config = ClientConfig::new(uri)
        .with_default_timeout(Duration::from_secs(15))
        .with_default_max_message_size(MAX_MANIFEST_BYTES);
    let client = BenchmarkRegistryServiceClient::new(transport, config);
    let request = ResolveBenchmarkRequest {
        benchmark_name: benchmark_name.to_owned(),
        version: String::new(),
        ..Default::default()
    };

    match client.resolve_benchmark(request).await {
        Ok(response) => Ok(Some(response.into_owned())),
        Err(error) if error.code == ErrorCode::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| {
            format!("failed to resolve benchmark `{benchmark_name}` from `{endpoint}`")
        }),
    }
}

fn validate_response_identity(
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

fn validate_resources(
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

fn validate_logical_path(path: &str) -> Result<PathBuf> {
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

async fn download_resources(resources: &[BenchmarkResource]) -> Result<HashMap<PathBuf, Vec<u8>>> {
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

fn build_remote_benchmark(
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

#[cfg(test)]
mod tests {
    use super::*;
    use buffa::Message as _;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn remote_url_sources_are_mutually_exclusive() {
        let error = select_remote_url(
            Some("http://127.0.0.1:8787"),
            Some(OsString::from("https://api.quantiles.io")),
        )
        .unwrap_err();
        assert!(error.to_string().contains("cannot use both"));
    }

    #[test]
    fn remote_url_defaults_to_production() {
        assert_eq!(select_remote_url(None, None).unwrap(), DEFAULT_REMOTE_URL);
    }

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

    #[tokio::test]
    async fn resolves_and_downloads_a_custom_nocode_benchmark_in_memory() {
        let server = MockServer::start().await;
        let definition = br#"
            [benchmarks.remote-test]
            type = "custom_nocode"
            dataset = { name = "quantiles/example" }
            prompt_template_file = "prompts/qa.txt"
            style = { type = "exact_match", golden_column = "answer" }
        "#;
        let prompt = b"{{ row.question }}";
        let definition_url = format!("{}/resources/definition", server.uri());
        let prompt_url = format!("{}/resources/prompt", server.uri());
        let response = ResolveBenchmarkResponse {
            benchmark_name: "remote-test".to_owned(),
            version: "v1".to_owned(),
            manifest_sha256: "a".repeat(64),
            resources: vec![
                resource(
                    "definition",
                    "bundle/quantiles.toml",
                    ResourceKind::Definition,
                    &definition_url,
                    definition,
                ),
                resource(
                    "prompt",
                    "bundle/prompts/qa.txt",
                    ResourceKind::PromptTemplate,
                    &prompt_url,
                    prompt,
                ),
            ],
            ..Default::default()
        };

        Mock::given(method("POST"))
            .and(path(
                "/quantiles.benchmark.v1.BenchmarkRegistryService/ResolveBenchmark",
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/proto")
                    .set_body_bytes(response.encode_to_vec()),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/resources/definition"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(definition))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/resources/prompt"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(prompt))
            .mount(&server)
            .await;

        let benchmark = resolve_and_download("remote-test", &server.uri())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(benchmark.version, "v1");
        assert_eq!(benchmark.prompt_template, "{{ row.question }}");
        assert_eq!(benchmark.config.params.dataset.name, "quantiles/example");
    }

    #[tokio::test]
    async fn connect_not_found_is_reported_as_benchmark_absence() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(
                "/quantiles.benchmark.v1.BenchmarkRegistryService/ResolveBenchmark",
            ))
            .respond_with(
                ResponseTemplate::new(404)
                    .insert_header("content-type", "application/json")
                    .set_body_raw(
                        r#"{"code":"not_found","message":"benchmark does not exist"}"#,
                        "application/json",
                    ),
            )
            .mount(&server)
            .await;

        assert!(
            resolve_and_download("missing", &server.uri())
                .await
                .unwrap()
                .is_none()
        );
    }

    fn resource(
        id: &str,
        logical_path: &str,
        kind: ResourceKind,
        download_url: &str,
        contents: &[u8],
    ) -> BenchmarkResource {
        BenchmarkResource {
            resource_id: id.to_owned(),
            logical_path: logical_path.to_owned(),
            kind: kind.into(),
            download_url: download_url.to_owned(),
            sha256: format!("{:x}", Sha256::digest(contents)),
            size_bytes: u64::try_from(contents.len()).unwrap(),
            content_type: "application/octet-stream".to_owned(),
            ..Default::default()
        }
    }
}
