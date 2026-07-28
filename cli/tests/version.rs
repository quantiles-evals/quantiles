use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn version_flag_prints_compiled_version_without_a_prefix() {
    let version = option_env!("QT_CLI_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"));
    let expected = predicate::eq(format!("{version}\n"));

    Command::new(assert_cmd::cargo::cargo_bin!("qt"))
        .arg("--version")
        .assert()
        .success()
        .stdout(expected);
}
