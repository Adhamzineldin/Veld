//! `veld test` and the `E0953` finding of `veld check`, run through the real binary.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::fs;
use std::net::TcpListener;
use std::path::Path;

use common::*;
use predicates::prelude::*;
use tempfile::tempdir;

fn init(dir: &Path) {
    veld(dir)
        .args(["init", "--name", "demo"])
        .assert()
        .success();
}

fn bookings() -> String {
    fs::read_to_string(repo_root().join("tests/rules/endpoints/bookings.veld")).unwrap()
}

/// A project with the bookings contract and its 20 examples.
fn project_with(contract: &str) -> tempfile::TempDir {
    let tmp = tempdir().unwrap();
    init(tmp.path());
    write(tmp.path(), "contracts/bookings.veld", contract);
    tmp
}

const UNFILLABLE: &str = r#"model Code {
  value: string @pattern("^[A-Z]+$")
}

service Codes at "/codes" {
  post "/" make(body: Code) -> Code
}

example "a code with nothing written" {
  make(body: {}) -> ok
}
"#;

#[test]
fn passing_examples_are_listed_and_exit_zero() {
    let tmp = project_with(&bookings());
    veld(tmp.path())
        .arg("test")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "a full room is rejected with its capacity",
        ))
        .stdout(predicate::str::contains("ok  "))
        .stdout(predicate::str::contains("20 passed"))
        .stdout(predicate::str::contains("FAIL").not());
}

#[test]
fn a_failing_example_shows_expected_and_actual_and_exits_one() {
    let wrong = bookings().replace("RoomFull { capacity: 2 }", "RoomFull { capacity: 9 }");
    let tmp = project_with(&wrong);
    veld(tmp.path())
        .arg("test")
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "FAIL a full room is rejected with its capacity",
        ))
        .stdout(predicate::str::contains(
            "expected: RoomFull {\"capacity\":9}",
        ))
        .stdout(predicate::str::contains(
            "actual:   RoomFull 409 {\"capacity\":2}",
        ))
        .stdout(predicate::str::contains(
            "at $.capacity: expected 9, actual 2",
        ))
        .stderr(predicate::str::contains("1 failed"));
}

#[test]
fn quiet_still_prints_failures() {
    let wrong = bookings().replace("RoomFull { capacity: 2 }", "RoomFull { capacity: 9 }");
    let tmp = project_with(&wrong);
    veld(tmp.path())
        .args(["--quiet", "test"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL a full room"))
        .stdout(predicate::str::contains("ok  ").not());
}

#[test]
fn filter_runs_only_the_matching_examples() {
    let tmp = project_with(&bookings());
    veld(tmp.path())
        .args(["test", "--filter", "full room"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 passed"))
        .stdout(predicate::str::contains("a missing room").not());
    veld(tmp.path())
        .args(["test", "--filter", "nothing like this"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no examples"));
}

#[test]
fn a_project_without_examples_says_so_and_exits_zero() {
    let tmp = tempdir().unwrap();
    init(tmp.path());
    veld(tmp.path())
        .arg("test")
        .assert()
        .success()
        .stdout(predicate::str::contains("no examples"));
}

#[test]
fn dry_run_needs_live() {
    let tmp = project_with(&bookings());
    veld(tmp.path())
        .args(["test", "--dry-run"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--live"));
}

#[test]
fn live_against_nothing_names_the_url_and_exits_one() {
    let tmp = project_with(&bookings());
    let addr = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    veld(tmp.path())
        .args(["test", "--live", &format!("http://{addr}")])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(addr.to_string()));
}

#[test]
fn an_example_that_cannot_be_filled_in_fails_the_run_and_the_check() {
    let tmp = project_with(UNFILLABLE);
    veld(tmp.path())
        .arg("test")
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL a code with nothing written"))
        .stdout(predicate::str::contains("E0953"));
    veld(tmp.path())
        .arg("check")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("error[E0953]"))
        .stderr(predicate::str::contains("bookings.veld:9:1"))
        .stderr(predicate::str::contains("1 example error"));
}

#[test]
fn check_is_clean_when_every_example_can_be_filled_in() {
    let tmp = project_with(&bookings());
    veld(tmp.path())
        .arg("check")
        .assert()
        .success()
        .stderr(predicate::str::contains("E0953").not());
}
