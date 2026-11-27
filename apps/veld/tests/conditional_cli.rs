//! `@version` and `@idempotent` through the real binary (`spec/rules.md` sections 11.7 to 11.9):
//! `veld test` runs the pending vector contracts offline, `veld check` accepts them, `veld fmt`
//! leaves them alone once formatted, and `veld generate` refuses a contract that uses either.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::fs;

use common::*;
use predicates::prelude::*;
use tempfile::tempdir;

fn vectors(name: &str) -> String {
    fs::read_to_string(repo_root().join("tests/rules/endpoints").join(name)).unwrap()
}

fn project_with(contract: &str) -> tempfile::TempDir {
    let tmp = tempdir().unwrap();
    veld(tmp.path())
        .args(["init", "--name", "demo"])
        .assert()
        .success();
    write(tmp.path(), "contracts/rules.veld", contract);
    tmp
}

#[test]
fn veld_test_runs_the_version_vectors_offline() {
    let tmp = project_with(&vectors("version.veld.pending"));
    veld(tmp.path())
        .arg("test")
        .assert()
        .success()
        .stdout(predicate::str::contains("19 passed"))
        .stdout(predicate::str::contains(
            "a tag that is not the current version is refused",
        ))
        .stdout(predicate::str::contains("FAIL").not());
}

#[test]
fn veld_test_runs_the_idempotency_vectors_offline() {
    let tmp = project_with(&vectors("idempotency.veld.pending"));
    veld(tmp.path())
        .arg("test")
        .assert()
        .success()
        .stdout(predicate::str::contains("21 passed"))
        .stdout(predicate::str::contains("a stored error is sent again"))
        .stdout(predicate::str::contains("FAIL").not());
}

#[test]
fn a_wrong_expectation_fails_with_both_outcomes() {
    let wrong =
        vectors("version.veld.pending").replace("expected: 3, actual: 4", "expected: 3, actual: 5");
    let tmp = project_with(&wrong);
    veld(tmp.path())
        .arg("test")
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "FAIL a tag that is not the current version is refused",
        ))
        .stdout(predicate::str::contains("PreconditionFailed 412"));
}

#[test]
fn check_accepts_the_contracts_and_fmt_is_stable() {
    for name in ["version.veld.pending", "idempotency.veld.pending"] {
        let tmp = project_with(&vectors(name));
        veld(tmp.path()).arg("check").assert().success();
        veld(tmp.path()).arg("fmt").assert().success();
        veld(tmp.path()).args(["fmt", "--check"]).assert().success();
        let text = fs::read_to_string(tmp.path().join("contracts/rules.veld")).unwrap();
        assert!(
            text.contains("ifMatch") || text.contains("key \""),
            "{name}"
        );
        veld(tmp.path()).arg("test").assert().success();
    }
}

#[test]
fn a_generator_that_does_not_implement_the_features_refuses_with_e0801() {
    let tmp = project_with(&vectors("version.veld.pending"));
    veld(tmp.path())
        .arg("generate")
        .assert()
        .failure()
        .stderr(predicate::str::contains("E0801"))
        .stderr(predicate::str::contains("rules_version"));
    let tmp = project_with(&vectors("idempotency.veld.pending"));
    veld(tmp.path())
        .arg("generate")
        .assert()
        .failure()
        .stderr(predicate::str::contains("E0801"))
        .stderr(predicate::str::contains("idempotency"));
}
