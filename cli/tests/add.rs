use assert_cmd::Command;
use predicates::prelude::*;
use sha2::{Digest as _, Sha256};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[expect(
    clippy::allow_attributes,
    clippy::pedantic,
    reason = "ConnectRPC and Buffa generated code uses allow attributes"
)]
mod registry_proto {
    connectrpc::include_generated!();
}

const CONFIG: &str = r#"[benchmarks.existing]
type = "custom_code"
command = ["echo"]
"#;

#[test]
fn missing_benchmark_argument_error_is_json_when_requested() {
    assert_json_parse_error(
        &["add", "--json"],
        "the following required arguments were not provided",
    );
}

#[test]
fn unknown_option_error_is_json_when_requested() {
    assert_json_parse_error(
        &["add", "remote-test", "--json", "--unknown"],
        "unexpected argument '--unknown'",
    );
}

#[test]
fn duplicate_benchmark_error_is_json_when_requested() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("quantiles.toml"), CONFIG).unwrap();

    let expected = format!(
        "{}\n",
        serde_json::json!({
            "error": "benchmark `existing` is already present in the local configuration"
        })
    );
    Command::new(assert_cmd::cargo::cargo_bin!("qt"))
        .current_dir(temp.path())
        .args(["add", "existing", "--json"])
        .assert()
        .failure()
        .stdout(predicate::eq(expected))
        .stderr(predicate::str::is_empty());

    assert_eq!(
        std::fs::read_to_string(temp.path().join("quantiles.toml")).unwrap(),
        CONFIG
    );
}

#[test]
fn duplicate_benchmark_error_is_human_readable_by_default() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("quantiles.toml"), CONFIG).unwrap();

    Command::new(assert_cmd::cargo::cargo_bin!("qt"))
        .current_dir(temp.path())
        .args(["add", "existing"])
        .assert()
        .failure()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::eq(
            "Error: benchmark `existing` is already present in the local configuration\n",
        ));
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_remote_benchmark_returns_json_and_does_not_create_config() {
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
    let temp = tempfile::tempdir().unwrap();

    let expected = format!(
        "{}\n",
        serde_json::json!({
            "error": "benchmark `missing` was not found in the remote registry"
        })
    );
    Command::new(assert_cmd::cargo::cargo_bin!("qt"))
        .current_dir(temp.path())
        .env("QUANTILES_REMOTE_URL", server.uri())
        .args(["add", "missing", "--json"])
        .assert()
        .failure()
        .stdout(predicate::eq(expected))
        .stderr(predicate::str::is_empty());

    assert!(!temp.path().join("quantiles.toml").exists());
    assert!(!temp.path().join(".quantiles").exists());
    assert!(!temp.path().join("missing-prompt").exists());
}

fn assert_json_parse_error(args: &[&str], expected_message: &str) {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("qt"))
        .args(args)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout.as_object().unwrap().len(), 1);
    assert!(stdout["error"].as_str().unwrap().contains(expected_message));
}

#[tokio::test(flavor = "multi_thread")]
async fn successful_add_creates_config_and_emits_json_only() {
    let server = MockServer::start().await;
    mock_successful_registry(&server).await;
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().canonicalize().unwrap().join("quantiles.toml");

    let output = Command::new(assert_cmd::cargo::cargo_bin!("qt"))
        .current_dir(temp.path())
        .env("QUANTILES_REMOTE_URL", server.uri())
        .args(["add", "remote-test", "--json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        stdout,
        serde_json::json!({
            "benchmark_name": "remote-test",
            "version": "v1",
            "config_path": config_path.display().to_string(),
        })
    );
    assert_added_files(temp.path(), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn successful_add_appends_config_and_emits_human_output_only() {
    let server = MockServer::start().await;
    mock_successful_registry(&server).await;
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().canonicalize().unwrap().join("quantiles.toml");
    let original = "# preserve this comment\n[benchmarks.existing]\ntype = \"custom_code\"\ncommand = [\"echo\"]\n";
    std::fs::write(&config_path, original).unwrap();

    Command::new(assert_cmd::cargo::cargo_bin!("qt"))
        .current_dir(temp.path())
        .env("QUANTILES_REMOTE_URL", server.uri())
        .args(["add", "remote-test"])
        .assert()
        .success()
        .stdout(predicate::eq(format!(
            "Added benchmark `remote-test` (version v1) to {}\n",
            config_path.display()
        )))
        .stderr(predicate::str::is_empty());

    assert_added_files(temp.path(), Some(original));
}

async fn mock_successful_registry(server: &MockServer) {
    use buffa::Message as _;
    use registry_proto::quantiles::benchmark::v1::{
        BenchmarkResource, ResolveBenchmarkResponse, ResourceKind,
    };

    let definition = br#"[benchmarks.remote-test]
type = "custom_nocode"
dataset = { name = "quantiles/example" }
prompt_template_file = "prompts/qa.txt"
style = { type = "exact_match", golden_column = "answer" }
"#;
    let prompt = b"{{ row.question }}\nAnswer:";
    let manifest_sha256 = "a".repeat(64);
    let resource =
        |id: &str, logical_path: &str, kind: ResourceKind, route: &str, contents: &[u8]| {
            BenchmarkResource {
                resource_id: id.to_owned(),
                logical_path: logical_path.to_owned(),
                kind: kind.into(),
                download_url: format!("{}{route}", server.uri()),
                sha256: format!("{:x}", Sha256::digest(contents)),
                size_bytes: u64::try_from(contents.len()).unwrap(),
                content_type: "application/octet-stream".to_owned(),
                ..Default::default()
            }
        };
    let response = ResolveBenchmarkResponse {
        benchmark_name: "remote-test".to_owned(),
        version: "v1".to_owned(),
        manifest_sha256: manifest_sha256.clone(),
        resources: vec![
            resource(
                "definition",
                "bundle/quantiles.toml",
                ResourceKind::Definition,
                "/definition",
                definition,
            ),
            resource(
                "prompt",
                "bundle/prompts/qa.txt",
                ResourceKind::PromptTemplate,
                "/prompt",
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
        .expect(1)
        .mount(server)
        .await;
    for (route, contents) in [
        ("/definition", definition.as_slice()),
        ("/prompt", prompt.as_slice()),
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(contents))
            .expect(1)
            .mount(server)
            .await;
    }
}

fn assert_added_files(root: &std::path::Path, prefix: Option<&str>) {
    let config_contents = std::fs::read_to_string(root.join("quantiles.toml")).unwrap();
    if let Some(prefix) = prefix {
        assert!(config_contents.starts_with(prefix));
    }
    let config: qt::config::WorkspaceConfig = toml::from_str(&config_contents).unwrap();
    assert!(config.benchmarks.contains_key("remote-test"));
    if prefix.is_some() {
        assert!(config.benchmarks.contains_key("existing"));
    }
    let qt::config::BenchmarkConfig::CustomNoCode(remote) =
        config.benchmarks.get("remote-test").unwrap()
    else {
        panic!("expected custom_nocode benchmark");
    };
    assert_eq!(
        remote.params.prompt_template_file,
        "remote-test-prompt/prompt.txt"
    );

    let prompt_path = root.join("remote-test-prompt/prompt.txt");
    assert_eq!(
        std::fs::read_to_string(prompt_path).unwrap(),
        "{{ row.question }}\nAnswer:"
    );
}
