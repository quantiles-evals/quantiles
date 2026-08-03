use std::ffi::OsString;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use connectrpc::ErrorCode;
use connectrpc::client::{ClientConfig, HttpClient};
use reqwest::Url;
use rustls_platform_verifier::ConfigVerifierExt as _;

use super::proto::v1::{
    BenchmarkRegistryServiceClient, ResolveBenchmarkRequest, ResolveBenchmarkResponse,
};

const DEFAULT_REMOTE_URL: &str = "https://api.quantiles.io";
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;

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

/// Parse and validate a remote benchmark service URL.
pub(super) fn validate_remote_url(remote_url: &str) -> Result<Url> {
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

/// Resolve benchmark metadata from the remote ConnectRPC service.
pub(super) async fn resolve_manifest(
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

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

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

        let endpoint = validate_remote_url(&server.uri()).unwrap();
        assert!(
            resolve_manifest("missing", &endpoint)
                .await
                .unwrap()
                .is_none()
        );
    }
}
