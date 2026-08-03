use buffa::Message as _;
use sha2::{Digest as _, Sha256};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::proto::v1::{BenchmarkResource, ResolveBenchmarkResponse, ResourceKind};
use super::resolve_and_download;

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
