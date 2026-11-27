//! The capture of `onRuleViolation` in the generated Java server (`spec/rules.md` section 17.3,
//! `spec/targets/java.md` section 18.2).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::live::require_javac;
use common::p4::{compiles, generate, run_driver};
use common::*;
use serde_json::{Value, json};
use veld_codegen::Role;

const CONTRACT: &str = "tests/toolchain/contract/platform_p4/capture.veld";
const REDACTION_CONTRACT: &str = "tests/wire/redaction/contract.veld";
const REDACTION_CASES: &str = "tests/wire/redaction/cases.json";

fn option() -> Value {
    json!({ "capture-violations": true })
}

/// Every captured event, through `veld_testing::from_violation`: the printed example compiles in
/// the contract, an input violation reproduces and passes, an output one says so, and every
/// redacted path is named as filled in.
fn round_trip(work: &std::path::Path, ir: &veld_ir::Ir) {
    let text = std::fs::read_to_string(repo_root().join(CONTRACT)).unwrap();
    let written: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(work.join("captures.json")).unwrap())
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
        for path in event["capture"]["redacted"].as_array().unwrap() {
            let note = format!("filled in {}", path.as_str().unwrap());
            assert!(
                found.notes.contains(&note),
                "{name}: {note} in {:?}",
                found.notes
            );
        }
        let with_example = compile_source(
            format!("{text}\n{}\n", found.block),
            "bookings",
            "the contract with the captured example",
        );
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

/// The production handler: an input violation with the principal and `@pii` values at two depths, a
/// `require` with facts, an optional fact found and not found, secrets in the body, the fact and
/// the principal, an output violation, an endpoint without auth, the clock, the option turned off
/// and a hook that throws; then every captured event through `from_violation`.
#[test]
fn the_capture_behaves_over_the_production_handler_and_round_trips() {
    if !require_javac("the_capture_behaves_over_the_production_handler_and_round_trips") {
        return;
    }
    let ir = compile_path(CONTRACT, "bookings");
    let server = generate(&ir, Role::Server, "gen.b", &option());
    let work = run_driver(&[&server], "CaptureHttp", &[], &[]);
    round_trip(work.path(), &ir);
}

/// The 11 cases of `tests/wire/redaction/cases.json` through the redaction of a server that
/// captures and audits nothing: the redaction is shared with the audit log, so its code is there
/// for a capture too. The contract has no service: one with an invariant is appended (a capture
/// needs an event of `onRuleViolation`).
#[test]
fn every_case_of_the_redaction_goldens_holds_for_a_server_that_only_captures() {
    if !require_javac("every_case_of_the_redaction_goldens_holds_for_a_server_that_only_captures") {
        return;
    }
    let mut text = std::fs::read_to_string(repo_root().join(REDACTION_CONTRACT)).unwrap();
    text.push_str(
        "\nmodel Probe {\n  n: int\n  rule nonNegative: n >= 0\n}\n\n@auth(none)\nservice Guests at \"/guests\" {\n  publishes GuestEvents\n\n  get \"/{id}\" getGuest(id: uuid) -> Guest\n  post \"/probe\" probe(body: Probe) -> Probe\n}\n",
    );
    let ir = compile_source(text, "guests", REDACTION_CONTRACT);
    let server = generate(&ir, Role::Server, "gen.b", &option());
    let hooks = text_of(&server, "HooksX");
    assert!(
        !hooks.contains("AuditHook"),
        "nothing is audited, so no hook asks for the audit log"
    );
    run_driver(
        &[&server],
        "platform_p1/AuditRedaction",
        &[(REDACTION_CASES, "cases.json")],
        &[],
    );
}

/// The text of the hooks of the only service of `files` (a file whose name ends with `Hooks.java`).
fn text_of(files: &veld_codegen::FileSet, _what: &str) -> String {
    let path = files
        .paths()
        .find(|p| p.ends_with("Hooks.java") && !p.contains("/runtime/"))
        .unwrap()
        .to_string();
    text(files, &path).to_string()
}

#[test]
fn snapshots_of_a_server_with_captures() {
    let ir = compile_path(CONTRACT, "bookings");
    let server = generate(&ir, Role::Server, "gen.b", &option());
    for (name, path) in [
        ("BookingsHooks", "gen/b/bookings/server/BookingsHooks.java"),
        (
            "BookingsHandler",
            "gen/b/bookings/server/BookingsHandler.java",
        ),
        ("Guest", "gen/b/bookings/capture/Guest.java"),
        ("Platform", "gen/b/runtime/Platform.java"),
        ("RuleWalk", "gen/b/runtime/RuleWalk.java"),
    ] {
        insta::assert_snapshot!(format!("capture_{name}"), text(&server, path));
    }
}

/// Off by default: the option adds the code, and without it the server has none of it, however many
/// rules it has: the files are what they were, byte for byte, with the option absent or `false`.
#[test]
fn a_server_without_the_option_has_none_of_the_code() {
    let ir = compile_path(CONTRACT, "bookings");
    let absent = generate(&ir, Role::Server, "gen.b", &json!({}));
    let off = generate(
        &ir,
        Role::Server,
        "gen.b",
        &json!({ "capture-violations": false }),
    );
    for path in absent.paths() {
        assert_eq!(text(&absent, path), text(&off, path), "{path} differs");
        for word in [
            "captureViolations",
            "Platform.Captured",
            "captureOf",
            "toJson()",
            "Platform.captured",
        ] {
            assert!(
                !text(&absent, path).contains(word) || path.ends_with("AuditEvent.java"),
                "`{word}` is in {path}"
            );
        }
    }
    let on = generate(&ir, Role::Server, "gen.b", &option());
    let walk = text(&on, "gen/b/runtime/RuleWalk.java");
    for word in [
        "Json.Obj capture",
        "public Json.Obj toJson()",
        "Platform.captureOf",
    ] {
        assert!(walk.contains(word), "`{word}` is missing");
    }
    let platform = text(&on, "gen/b/runtime/Platform.java");
    for word in [
        "static Json.Obj captureOf(",
        "public static Captured captured(",
    ] {
        assert!(platform.contains(word), "`{word}` is missing");
    }
    // The audit log is not part of it: nothing is audited, and the hooks have no audit parameter.
    assert!(!text_of(&on, "hooks").contains("AuditHook"));
    // A client never has any of it.
    let client = generate(&ir, Role::Client, "gen.c", &option());
    for (path, bytes) in client.iter() {
        let body = String::from_utf8_lossy(bytes);
        assert!(
            !body.contains("captureViolations") && !body.contains("Redact"),
            "{path}"
        );
    }
}

/// The option on a contract without rules or tenancy has nothing to capture: no event exists.
#[test]
fn the_option_on_a_contract_without_an_event_changes_nothing() {
    let ir = compile_path("tests/toolchain/contract/slash/items.veld", "items");
    let plain = generate(&ir, Role::Server, "gen.b", &json!({}));
    let with = generate(&ir, Role::Server, "gen.b", &option());
    assert_eq!(plain.paths().count(), with.paths().count());
    for path in plain.paths() {
        assert_eq!(
            text(&plain, path),
            text(&with, path),
            "{path} changed with the option"
        );
    }
}

/// A capture next to a version guard, an `@idempotent` endpoint, derived fields, a deadline, a
/// service level, an audited endpoint, timed moves and events: the files of each combination
/// compile with `-Xlint:all -Werror`. Nothing is run.
#[test]
fn the_capture_compiles_next_to_the_other_features() {
    if !require_javac("the_capture_compiles_next_to_the_other_features") {
        return;
    }
    let base = std::fs::read_to_string(repo_root().join(CONTRACT)).unwrap();
    let extra = [
        "",
        "\nmodel Shelf {\n  id: uuid\n  name: string\n  @version\n  revision: long\n}\n\nmodel Rename { name: string }\n\n@auth(bearer)\nservice Shelves at \"/shelves\" {\n  @idempotent\n  put \"/{id}\" rename(id: uuid, body: Rename) -> Shelf {\n    fact shelf: Shelf = id\n  }\n}\n",
        "\nmodel Memo {\n  note: string\n  shout: string = note\n}\n\nservice Memos at \"/memos\" {\n  post \"/\" memo(body: Memo) -> Memo\n}\n",
        "\n@auth(bearer)\nservice Slo at \"/slo\" {\n  @slo(p99: ms(300))\n  @timeout(ms(500))\n  @audited\n  get \"/{id}\" slo(id: uuid) -> Booking\n}\n",
    ];
    for tail in extra {
        let ir = compile_source(format!("{base}{tail}"), "bookings", CONTRACT);
        let server = generate(&ir, Role::Server, "gen.b", &option());
        compiles(&[&server]);
    }
    // Next to the events runtime and an audited, tenant-scoped, limited contract.
    for (path, package) in [
        (
            "tests/toolchain/contract/platform_p4/no_values_events.veld",
            "events",
        ),
        ("tests/toolchain/contract/platform_p1/all_four.veld", "four"),
        ("tests/toolchain/contract/platform_p2/all_four.veld", "four"),
    ] {
        let ir = compile_path(path, package);
        let server = generate(&ir, Role::Server, "gen.b", &option());
        compiles(&[&server]);
    }
}
