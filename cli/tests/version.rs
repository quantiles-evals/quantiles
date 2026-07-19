use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn version_flag_prints_cargo_package_version_without_a_prefix() {
    let expected = predicate::eq(format!("{}\n", env!("CARGO_PKG_VERSION")));

    Command::new(assert_cmd::cargo::cargo_bin!("qt"))
        .arg("--version")
        .assert()
        .success()
        .stdout(expected);
}
