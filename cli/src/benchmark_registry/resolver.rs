use anyhow::Result;

use super::RemoteBenchmark;
use super::client::{resolve_manifest, validate_remote_url};
use super::download::download_resources;
use super::manifest::{validate_resources, validate_response_identity};

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

#[cfg(test)]
mod tests {
    use buffa::Message as _;
    use sha2::{Digest as _, Sha256};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::proto::v1::{BenchmarkResource, ResolveBenchmarkResponse, ResourceKind};
    use super::*;

    #[tokio::test]
    async fn resolves_and_downloads_a_custom_nocode_benchmark_in_memory() {
        let server = MockServer::start().await;
        let definition = br#"
            [benchmarks.remote-test]
            type = "custom_nocode"
            dataset = { name = "quantiles/example" }
            prompt_template_file = "prompts/qa.txt"
            style = { type = "similarity", golden_column = "answer", metric = "levenshtein" }
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
        assert!(matches!(
            benchmark.config.params.style,
            crate::config::CustomNoCodeStyleConfig::Similarity {
                metric: crate::config::CustomNoCodeSimilarityMetric::Levenshtein(_),
                ..
            }
        ));
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
