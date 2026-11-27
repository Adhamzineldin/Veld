//! The capture of `onRuleViolation` in the generated TypeScript server (`spec/rules.md` section
//! 17.3, `spec/targets/typescript.md` section 5l).
//!
//! Without `node` or `node_modules` the toolchain tests are skipped unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::toolchain::*;
use common::*;
use serde_json::{Value, json};

const CONTRACT: &str = "tests/toolchain/contract/platform_p4/capture.veld";
const REDACTION_CONTRACT: &str = "tests/wire/redaction/contract.veld";
const REDACTION_CASES: &str = "tests/wire/redaction/cases.json";

fn option() -> Value {
    json!({ "capture-violations": true })
}

/// The production handler: an input violation with the principal and `@pii` values at two depths, a
/// `require` with facts, an optional fact found and not found, secrets in the body, the fact and
/// the principal, an output violation, an endpoint without auth, the clock, the option turned off
/// and a fact that cannot be walked.
fn http_run(edition: &str, prefix: &str) -> Option<(tempfile::TempDir, veld_ir::Ir)> {
    let tsc = tsc_path()?;
    let ir = compile_path(CONTRACT);
    let guard = scratch(prefix);
    build_services(
        guard.path(),
        &tsc,
        &ir,
        &["Bookings"],
        &["capture_http.ts"],
        edition,
        &option(),
    );
    run(
        std::process::Command::new("node")
            .arg("out/capture_http.js")
            .current_dir(guard.path()),
        "node capture_http.js",
    );
    Some((guard, ir))
}

/// Every captured event, through `veld_testing::from_violation`: the printed example compiles in
/// the contract, an input violation reproduces and passes, an output one says so, and every
/// redacted path is named as filled in.
fn round_trip(guard: &tempfile::TempDir, ir: &veld_ir::Ir) {
    let text = std::fs::read_to_string(repo_root().join(CONTRACT)).unwrap();
    let written: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(guard.path().join("captures.json")).unwrap())
            .unwrap();
    assert!(
        written.len() >= 6,
        "{} captures were written",
        written.len()
    );
    for entry in &written {
        let name = entry["name"].as_str().unwrap();
        let event = &entry["event"];
        let found = veld_testing::from_violation(ir, event)
            .unwrap_or_else(|e| panic!("{name}: from_violation: {e}"));
        let redacted = event["capture"]["redacted"].as_array().unwrap();
        for path in redacted {
            let note = format!("filled in {}", path.as_str().unwrap());
            assert!(
                found.notes.contains(&note),
                "{name}: {note} in {:?}",
                found.notes
            );
        }
        let with_example = compile_text(format!("{text}\n{}\n", found.block));
        let runs = veld_testing::run_offline(&with_example, None);
        assert_eq!(runs.len(), 1, "{name}: {}", found.block);
        match entry["kind"].as_str().unwrap() {
            "input" => {
                let report = runs[0]
                    .result
                    .as_ref()
                    .unwrap_or_else(|e| panic!("{name}: {e}\n{}", found.block));
                assert!(report.passed(), "{name}: {report}\n{}", found.block);
                assert!(
                    !found.notes.iter().any(|n| n.contains("does not reproduce")),
                    "{name}: {:?}",
                    found.notes
                );
            }
            _ => assert!(
                found
                    .notes
                    .iter()
                    .any(|n| n.contains("the violation is of an output rule")),
                "{name}: {:?}",
                found.notes
            ),
        }
    }
}

#[test]
fn the_capture_behaves_over_the_production_handler_and_round_trips_es2020() {
    if let Some((guard, ir)) = http_run("es2020", "capture-2020-") {
        round_trip(&guard, &ir);
    }
}

#[test]
fn the_capture_behaves_over_the_production_handler_and_round_trips_es2022() {
    if let Some((guard, ir)) = http_run("es2022", "capture-2022-") {
        round_trip(&guard, &ir);
    }
}

/// The 11 cases of `tests/wire/redaction/cases.json` through the redaction of a server that
/// captures and audits nothing: the redaction is shared with the audit log, so its region is on
/// for a capture too. The contract has no service: one with an invariant is appended (a capture
/// needs an event of `onRuleViolation`).
#[test]
fn every_case_of_the_redaction_goldens_holds_for_a_server_that_only_captures() {
    let Some(tsc) = tsc_path() else { return };
    let mut text = std::fs::read_to_string(repo_root().join(REDACTION_CONTRACT)).unwrap();
    text.push_str(
        "\nmodel Probe {\n  n: int\n  rule nonNegative: n >= 0\n}\n\n@auth(none)\nservice Guests at \"/guests\" {\n  publishes GuestEvents\n\n  get \"/{id}\" getGuest(id: uuid) -> Guest\n  post \"/probe\" probe(body: Probe) -> Probe\n}\n",
    );
    let ir = compile_text(text);
    for (edition, _) in TARGETS {
        let guard = scratch("capture-redaction-");
        let work = guard.path();
        build_services(
            work,
            &tsc,
            &ir,
            &["Guests"],
            &["audit_redaction.ts"],
            edition,
            &option(),
        );
        let runtime = std::fs::read_to_string(work.join("server/runtime/veld.ts")).unwrap();
        assert!(runtime.contains("export function redactNode("));
        assert!(
            !runtime.contains("function withAudit("),
            "nothing is audited"
        );
        std::fs::copy(repo_root().join(REDACTION_CASES), work.join("cases.json")).unwrap();
        run(
            std::process::Command::new("node")
                .arg("out/audit_redaction.js")
                .current_dir(work),
            "node audit_redaction.js",
        );
    }
}

#[test]
fn snapshots_of_a_server_with_captures() {
    let ir = compile_path(CONTRACT);
    let server = gen_server_with(&ir, "Bookings", &[], &option());
    insta::assert_snapshot!("capture_server_module", text(&server, "server/bookings.ts"));
    insta::assert_snapshot!("capture_server_models", text(&server, "models/bookings.ts"));
}

/// Off by default: the option adds the code, and without it the server has none of it, however
/// many rules it has: the runtime, the models and the module are what they were.
#[test]
fn a_server_without_the_option_has_none_of_the_code() {
    let ir = compile_path(CONTRACT);
    for (label, options) in [
        ("default", json!({})),
        ("false", json!({ "capture-violations": false })),
    ] {
        let server = gen_server_with(&ir, "Bookings", &[], &options);
        for path in server.paths() {
            let body = String::from_utf8_lossy(server.get(path).unwrap_or_default()).into_owned();
            for word in [
                "captureViolations",
                "ViolationCapture",
                "CaptureSpec",
                "captureState",
                "captureOf",
                "capture:",
                "pii: true",
            ] {
                assert!(!body.contains(word), "{label}: `{word}` is in {path}");
            }
        }
    }
    let on = gen_server_with(&ir, "Bookings", &[], &option());
    let runtime = text(&on, "runtime/veld.ts");
    for word in [
        "export interface ViolationCapture",
        "function withCapture(",
        "function captureParameters(",
        "function captureFact(",
        "function captureOf(",
        "export function redactNode(",
        "captureViolations?: boolean;",
    ] {
        assert!(runtime.contains(word), "`{word}` is missing");
    }
    // The audit log is not part of it.
    assert!(!runtime.contains("function withAudit("));
    // A client never has any of it.
    let client = gen_client(&ir);
    for path in client.paths() {
        let body = String::from_utf8_lossy(client.get(path).unwrap_or_default()).into_owned();
        assert!(
            !body.contains("captureViolations") && !body.contains("redactNode"),
            "{path}"
        );
    }
}

/// The option on a contract without rules or tenancy has nothing to capture: no event exists.
#[test]
fn the_option_on_a_contract_without_an_event_changes_nothing() {
    let ir = standard_ir();
    let plain = gen_server_with(&ir, "Orders", &["Iam"], &json!({}));
    let with = gen_server_with(&ir, "Orders", &["Iam"], &option());
    for path in plain.paths() {
        assert_eq!(
            text(&plain, path),
            text(&with, path),
            "{path} changed with the option"
        );
    }
}

/// A capture next to a version guard, an `@idempotent` endpoint, derived fields, a deadline, a
/// service level, an audited endpoint and tenancy: the runtime of each combination compiles under
/// `tsc --strict`. Nothing is run.
#[test]
fn the_capture_compiles_next_to_the_other_features() {
    let Some(tsc) = tsc_path() else { return };
    let base = std::fs::read_to_string(repo_root().join(CONTRACT)).unwrap();
    let extra = [
        "",
        "\nmodel Shelf {\n  id: uuid\n  name: string\n  @version\n  revision: long\n}\n\nmodel Rename { name: string }\n\n@auth(bearer)\nservice Shelves at \"/shelves\" {\n  @idempotent\n  put \"/{id}\" rename(id: uuid, body: Rename) -> Shelf {\n    fact shelf: Shelf = id\n  }\n}\n",
        "\nmodel Memo {\n  note: string\n  shout: string = note\n}\n",
        "\n@auth(bearer)\nservice Slo at \"/slo\" {\n  @slo(p99: ms(300))\n  @timeout(ms(500))\n  @audited\n  get \"/{id}\" slo(id: uuid) -> Booking\n}\n",
    ];
    for tail in extra {
        let ir = compile_text(format!("{base}{tail}"));
        let services: Vec<&str> = ir.services.iter().map(|s| s.name.as_str()).collect();
        for (edition, _) in TARGETS {
            let guard = scratch("capture-combo-");
            build_services(guard.path(), &tsc, &ir, &services, &[], edition, &option());
        }
    }
}
