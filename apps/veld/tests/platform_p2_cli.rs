//! Platform phase P2 through the real binary (`spec/platform.md` sections 10 to 13): `veld test`
//! runs the pending vector contracts offline, `veld check` accepts them and lists the flags,
//! `veld fmt` leaves them alone once formatted, `veld generate` refuses what no generator
//! implements yet, and `veld export slo` writes OpenSLO.
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

const SLO: &str = "model Booking { id: uuid }

service Bookings at \"/bookings\" {
  @slo(p99: ms(300), errors: 0.1)
  @timeout(seconds(2))
  get \"/{id}\" getBooking(id: uuid) -> Booking | NotFound
}
";

#[test]
fn veld_test_runs_the_timed_vectors_offline() {
    let tmp = project_with(&vectors("timed.veld.pending"));
    veld(tmp.path())
        .arg("test")
        .assert()
        .success()
        .stdout(predicate::str::contains("8 passed"))
        .stdout(predicate::str::contains(
            "a booking that has waited one millisecond less is not due",
        ))
        .stdout(predicate::str::contains("FAIL").not());
}

#[test]
fn veld_test_runs_the_flag_vectors_offline() {
    let tmp = project_with(&vectors("flags.veld.pending"));
    veld(tmp.path())
        .arg("test")
        .assert()
        .success()
        .stdout(predicate::str::contains("14 passed"))
        .stdout(predicate::str::contains("FAIL").not());
}

#[test]
fn check_lists_the_flags_and_graph_json_has_them() {
    let tmp = project_with(&vectors("flags.veld.pending"));
    veld(tmp.path())
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "flags: archiving (1 statement), betaDesk (1 statement), lockSuspended (1 statement), openOnly (1 statement)",
        ));
    let out = veld(tmp.path())
        .args(["check", "--graph=json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let graph: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(graph["flags"]["openOnly"]["requires"], 1);
    assert_eq!(graph["flags"]["archiving"]["flows"], 1);
    // A workspace without flags has no such key.
    let plain = project_with(&vectors("timed.veld.pending"));
    let out = veld(plain.path())
        .args(["check", "--graph=json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let graph: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert!(graph.get("flags").is_none());
}

#[test]
fn check_names_the_timed_endpoints_and_graph_json_lists_them() {
    let tmp = project_with(&vectors("timed.veld.pending"));
    veld(tmp.path())
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "timed: Bookings.archiveBooking (pending -> expired after 172800000 ms), Bookings.expireBooking (pending -> cancelled after 86400000 ms)",
        ));
    let out = veld(tmp.path())
        .args(["check", "--graph=json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let graph: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(
        graph["timed"],
        serde_json::json!([
            { "service": "Bookings", "endpoint": "archiveBooking", "from": "pending", "to": "expired", "after_ms": 172_800_000 },
            { "service": "Bookings", "endpoint": "expireBooking", "from": "pending", "to": "cancelled", "after_ms": 86_400_000 }
        ])
    );
    // A workspace with no timed edge has neither the key nor the line.
    let plain = project_with(&vectors("flags.veld.pending"));
    veld(plain.path())
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains("timed:").not());
    let out = veld(plain.path())
        .args(["check", "--graph=json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let graph: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert!(graph.get("timed").is_none());
}

#[test]
fn check_accepts_the_contracts_and_fmt_is_stable() {
    for name in ["timed.veld.pending", "flags.veld.pending"] {
        let tmp = project_with(&vectors(name));
        veld(tmp.path()).arg("check").assert().success();
        veld(tmp.path()).arg("fmt").assert().success();
        veld(tmp.path()).args(["fmt", "--check"]).assert().success();
        let text = fs::read_to_string(tmp.path().join("contracts/rules.veld")).unwrap();
        assert!(
            text.contains("after hours(24)") || text.contains("when flag(\"openOnly\")"),
            "{name}"
        );
        veld(tmp.path()).arg("test").assert().success();
    }
}

#[test]
fn a_generator_that_does_not_implement_the_features_refuses_with_e0801() {
    for (contract, feature) in [
        (vectors("timed.veld.pending"), "rules_timed"),
        (vectors("flags.veld.pending"), "rules_flags"),
        (
            SLO.replace("@slo(p99: ms(300), errors: 0.1)\n  ", ""),
            "timeouts",
        ),
        (SLO.replace("@timeout(seconds(2))\n  ", ""), "slo"),
    ] {
        let tmp = project_with(&contract);
        veld(tmp.path())
            .arg("generate")
            .assert()
            .failure()
            .stderr(predicate::str::contains("E0801"))
            .stderr(predicate::str::contains(feature));
    }
}

#[test]
fn export_slo_writes_openslo_to_stdout_and_to_a_file() {
    let tmp = project_with(SLO);
    let out = veld(tmp.path())
        .args(["export", "slo"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.starts_with("apiVersion: \"openslo/v1\"\nkind: \"SLO\"\n"),
        "{text}"
    );
    assert_eq!(text.matches("kind: \"SLO\"").count(), 2);
    assert!(
        text.contains("target: 0.99\n") && text.contains("target: 0.999\n"),
        "{text}"
    );
    veld(tmp.path())
        .args(["export", "slo", "-o", "slo.yaml"])
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(tmp.path().join("slo.yaml")).unwrap(),
        text
    );
}

#[test]
fn export_slo_without_an_objective_is_e0803() {
    let tmp = tempdir().unwrap();
    veld(tmp.path())
        .args(["init", "--name", "demo"])
        .assert()
        .success();
    veld(tmp.path())
        .args(["export", "slo"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("E0803"));
}

#[test]
fn export_openapi_has_the_deadline_and_the_service_level() {
    let tmp = project_with(SLO);
    let out = veld(tmp.path())
        .args(["export", "openapi"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let doc: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let op = &doc["paths"]["/bookings/{id}"]["get"];
    assert_eq!(op["x-veld-timeout-ms"], 2000);
    assert_eq!(op["x-veld-slo"]["p99_ms"], 300);
    assert!(op["responses"].get("504").is_some());
}
