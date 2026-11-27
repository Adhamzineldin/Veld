//! Derived fields through the real binary (`spec/rules.md` section 17.2): `veld test` runs the
//! pending vector contract offline, `veld check` accepts it, `veld fmt` leaves it alone once
//! formatted and keeps every expression, and `veld generate` refuses a contract that has a
//! derived field until a generator implements them.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::fs;

use common::*;
use predicates::prelude::*;
use tempfile::tempdir;

fn vectors(group: &str, name: &str) -> String {
    fs::read_to_string(repo_root().join("tests/rules").join(group).join(name)).unwrap()
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
fn veld_test_runs_the_derived_vectors_offline() {
    let tmp = project_with(&vectors("endpoints", "derived.veld.pending"));
    veld(tmp.path())
        .arg("test")
        .assert()
        .success()
        .stdout(predicate::str::contains("9 passed"))
        .stdout(predicate::str::contains(
            "the result is given its derived field",
        ))
        .stdout(predicate::str::contains("FAIL").not());
}

#[test]
fn a_wrong_derived_value_in_a_pattern_fails_with_both_outcomes() {
    let wrong = vectors("endpoints", "derived.veld.pending").replace("short: 20 }", "short: 21 }");
    let tmp = project_with(&wrong);
    veld(tmp.path())
        .arg("test")
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "FAIL a payable amount above the available one fails the require with computed numbers",
        ))
        .stdout(predicate::str::contains("NotEnough 402"));
}

#[test]
fn check_accepts_the_contracts_and_fmt_is_stable() {
    for (group, name) in [
        ("endpoints", "derived.veld.pending"),
        ("core", "derived.veld.pending"),
    ] {
        let tmp = project_with(&vectors(group, name));
        veld(tmp.path()).arg("check").assert().success();
        veld(tmp.path()).arg("fmt").assert().success();
        veld(tmp.path()).args(["fmt", "--check"]).assert().success();
        let text = fs::read_to_string(tmp.path().join("contracts/rules.veld")).unwrap();
        assert!(
            text.contains("subtotal: int = sum(items, i => i.total)")
                || text.contains("sum: int = double + b"),
            "{name}"
        );
    }
}

#[test]
fn an_example_that_writes_a_derived_field_is_refused_by_check() {
    let src = format!(
        "{}\nexample \"writes it\" {{\n  given wallet = {{ balance: 1, held: 0, available: 1 }}\n  pay(id: wallet.id, body: {{ items: [] }}) -> ok\n}}\n",
        vectors("endpoints", "derived.veld.pending")
    );
    let tmp = project_with(&src);
    veld(tmp.path())
        .arg("check")
        .assert()
        .failure()
        .stderr(predicate::str::contains("E0952"))
        .stderr(predicate::str::contains("derived field"));
}

#[test]
fn a_cycle_is_refused_by_check() {
    let tmp = project_with("model M { a: int = b + 1\n b: int = a + 1 }\n");
    veld(tmp.path())
        .arg("check")
        .assert()
        .failure()
        .stderr(predicate::str::contains("E0960"));
}

#[test]
fn a_generator_that_does_not_implement_derived_fields_refuses_with_e0801() {
    let tmp = project_with(&vectors("endpoints", "derived.veld.pending"));
    veld(tmp.path())
        .arg("generate")
        .assert()
        .failure()
        .stderr(predicate::str::contains("E0801"))
        .stderr(predicate::str::contains("rules_derived"));
}
