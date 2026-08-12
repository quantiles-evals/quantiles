use assert_cmd::Command;
use predicates::prelude::*;

#[tokio::test]
async fn completed_run_error_is_json_when_requested() {
    let tmpdir = tempfile::tempdir().unwrap();
    let root = tmpdir.path();
    qt::db::init_workspace(root).await.unwrap();
    let db = qt::db::open_workspace(root).await.unwrap();
    let metrics_store = qt::metrics_store::MetricsStore::new(qt::db::metrics_dir(root)).unwrap();
    let run_id = qt::db::create_run(&db, "completed-test", None)
        .await
        .unwrap();
    qt::db::complete_run(&db, &metrics_store, run_id)
        .await
        .unwrap();

    let message = format!(
        "run {run_id} is already completed; create a new run or resume a running/failed one"
    );
    let expected = format!("{}\n", serde_json::json!({ "error": message }));

    Command::new(assert_cmd::cargo::cargo_bin!("qt"))
        .current_dir(root)
        .args(["resume", &run_id.to_string(), "--json"])
        .assert()
        .failure()
        .stdout(predicate::eq(expected))
        .stderr(predicate::str::is_empty());
}
