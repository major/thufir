//! CLI integration tests for the Thufir binary.

use assert_cmd::Command;
use predicates::prelude::*;

/// `--help` exits 0 without requiring any secrets or environment variables.
#[test]
fn cli_help_exits_zero() {
    Command::cargo_bin("thufir")
        .unwrap()
        .arg("--help")
        .env_clear()
        .assert()
        .success()
        .stdout(predicate::str::contains("thufir"));
}

/// `--version` exits 0 without requiring any secrets.
#[test]
fn cli_version_exits_zero() {
    Command::cargo_bin("thufir")
        .unwrap()
        .arg("--version")
        .env_clear()
        .assert()
        .success();
}
