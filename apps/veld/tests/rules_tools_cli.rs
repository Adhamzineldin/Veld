//! `veld test --coverage`, `veld export rules` and `veld test --from-violation`, run through the
//! real binary (`spec/rules.md` sections 19.1, 19.2 and 17.3).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::fs;
use std::path::Path;

use common::*;
use predicates::prelude::*;
use serde_json::{Value, json};
use tempfile::tempdir;

/// The contract of the coverage tests of `veld-testing`: the package of a project is `demo`.
const COVERAGE: &str = include_str!("../../../crates/veld-testing/tests/fixtures/coverage.veld");
/// The contract of the `--from-violation` tests of `veld-testing`.
const STAYS: &str = include_str!("../../../crates/veld-testing/tests/fixtures/violation.veld");

const COVERAGE_REPORT: &str = concat!(
    "coverage: 5 of 12 rules held and violated (41%)\n",
    "  demo/NewStay.atMost30Days                             never violated\n",
    "  demo/Closure.reasonGiven                              never evaluated\n",
    "  demo/Bookings.payBooking allow ManageBooking          never denied\n",
    "  demo/Bookings.payBooking transition pending -> paid   never illegal\n",
    "  demo/Bookings.payBooking ensure paidTotal             never evaluated\n",
    "  demo/Bookings.payBooking ensure #2                    never evaluated\n",
    "  demo/Bookings.closeBooking require reasonNeeded       never evaluated\n",
);

/// A project named `demo` whose only contract is `contract`.
fn project(contract: &str) -> tempfile::TempDir {
    let tmp = tempdir().unwrap();
    veld(tmp.path())
        .args(["init", "--name", "demo"])
        .assert()
        .success();
    fs::remove_file(tmp.path().join("contracts/hello.veld")).unwrap();
    write(tmp.path(), "contracts/app.veld", contract);
    tmp
}

// ---- veld test --coverage -----------------------------------------------------------------

#[test]
fn coverage_prints_the_totals_and_every_rule_that_is_not_covered() {
    let tmp = project(COVERAGE);
    veld(tmp.path())
        .args(["test", "--coverage"])
        .assert()
        .success()
        .stdout(predicate::str::contains("9 passed"))
        .stdout(predicate::str::contains(COVERAGE_REPORT));
}

#[test]
fn the_report_follows_the_list_of_examples() {
    let tmp = project(COVERAGE);
    let out = veld(tmp.path())
        .args(["test", "--coverage"])
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.find("9 passed").unwrap() < text.find("coverage: ").unwrap(),
        "{text}"
    );
}

#[test]
fn without_the_flag_there_is_no_report() {
    let tmp = project(COVERAGE);
    veld(tmp.path())
        .arg("test")
        .assert()
        .success()
        .stdout(predicate::str::contains("coverage").not());
}

#[test]
fn fail_under_exits_one_below_the_limit_and_zero_at_it() {
    let tmp = project(COVERAGE);
    veld(tmp.path())
        .args(["test", "--coverage", "--fail-under", "41"])
        .assert()
        .success();
    veld(tmp.path())
        .args(["test", "--coverage", "--fail-under", "42"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(COVERAGE_REPORT))
        .stderr(predicate::str::contains(
            "coverage is 41%, below --fail-under 42",
        ));
    veld(tmp.path())
        .args(["test", "--coverage", "--fail-under", "0"])
        .assert()
        .success();
}

#[test]
fn fail_under_takes_a_number_from_zero_to_a_hundred_and_needs_coverage() {
    let tmp = project(COVERAGE);
    veld(tmp.path())
        .args(["test", "--coverage", "--fail-under", "101"])
        .assert()
        .code(2);
    veld(tmp.path())
        .args(["test", "--fail-under", "10"])
        .assert()
        .code(2);
}

#[test]
fn a_failing_example_still_fails_the_run_with_coverage() {
    let wrong = COVERAGE.replace("-> Forbidden\n", "-> NotFound\n");
    let tmp = project(&wrong);
    veld(tmp.path())
        .args(["test", "--coverage"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("coverage: "));
}

#[test]
fn a_workspace_without_rules_reports_no_rules_and_is_at_a_hundred() {
    let tmp = project("model A { n: int }\nservice S at \"/s\" { get \"/\" list() -> A }\n");
    veld(tmp.path())
        .args(["test", "--coverage", "--fail-under", "100"])
        .assert()
        .success()
        .stdout(predicate::str::contains("coverage: no rules\n"));
}

#[test]
fn rules_without_examples_are_at_zero_and_the_report_is_still_printed() {
    let text = COVERAGE
        .split("example \"a stay is accepted\"")
        .next()
        .unwrap();
    let tmp = project(text);
    veld(tmp.path())
        .args(["test", "--coverage", "--fail-under", "1"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "coverage: 0 of 12 rules held and violated (0%)",
        ));
}

// ---- veld export rules --------------------------------------------------------------------

#[test]
fn export_rules_prints_the_rule_book() {
    let tmp = project(COVERAGE);
    let out = veld(tmp.path())
        .args(["export", "rules"])
        .assert()
        .success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        text.starts_with("# Rule book\n\n## Package `demo`\n"),
        "{text}"
    );
    for part in [
        "### Declarations",
        "### State machines",
        "### Policies",
        "### Endpoints",
        "### Examples",
    ] {
        assert!(text.contains(part), "no {part} in\n{text}");
    }
    assert!(
        text.contains("- reasonNeeded: `body.reason != \"\"`, answers `ValidationError`"),
        "{text}"
    );
}

#[test]
fn export_rules_is_deterministic() {
    let tmp = project(COVERAGE);
    let run = || {
        veld(tmp.path())
            .args(["export", "rules"])
            .output()
            .unwrap()
            .stdout
    };
    assert_eq!(run(), run());
}

#[test]
fn export_rules_writes_a_file_the_user_owns() {
    let tmp = project(COVERAGE);
    let printed = veld(tmp.path())
        .args(["export", "rules"])
        .output()
        .unwrap()
        .stdout;
    veld(tmp.path())
        .args(["export", "rules", "-o", "RULES.md"])
        .assert()
        .success()
        .stdout(predicate::str::contains("created RULES.md"));
    assert_eq!(fs::read(tmp.path().join("RULES.md")).unwrap(), printed);
    veld(tmp.path())
        .args(["export", "rules", "-o", "RULES.md"])
        .assert()
        .success()
        .stdout(predicate::str::contains("unchanged RULES.md"));
    write(tmp.path(), "RULES.md", "mine\n");
    veld(tmp.path())
        .args(["export", "rules", "-o", "RULES.md"])
        .assert()
        .code(1);
    assert_eq!(
        fs::read_to_string(tmp.path().join("RULES.md")).unwrap(),
        "mine\n"
    );
    veld(tmp.path())
        .args(["export", "rules", "-o", "RULES.md", "--force"])
        .assert()
        .success();
    assert_eq!(fs::read(tmp.path().join("RULES.md")).unwrap(), printed);
}

#[test]
fn export_rules_of_a_workspace_without_rules_says_so() {
    let tmp = project("model A { n: int }\nservice S at \"/s\" { get \"/\" list() -> A }\n");
    veld(tmp.path())
        .args(["export", "rules"])
        .assert()
        .success()
        .stdout("# Rule book\n\nThis workspace has no rules.\n");
}

#[test]
fn export_rules_names_the_known_services_for_an_unknown_one() {
    let tmp = project(COVERAGE);
    veld(tmp.path())
        .args(["export", "rules", "--service", "Nope"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("known services"));
}

// ---- veld test --from-violation -----------------------------------------------------------

/// The signature of the one endpoint of `STAYS`, read from the real export.
fn stays_signature(dir: &Path) -> String {
    let out = veld(dir).args(["export", "ir"]).output().unwrap().stdout;
    let ir: Value = serde_json::from_slice(&out).unwrap();
    ir["services"][0]["endpoints"][0]["sig"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn capture_event(sig: &str, redacted: &Value, phone: bool) -> Value {
    let mut guest = json!({ "name": "Ada" });
    if phone {
        guest["phone"] = json!("+20100000000");
    }
    json!({
        "rule": "veld:demo/NewStay.endAfterStart@0000000000000000",
        "operation": sig,
        "position": "input",
        "outcome": "rejected",
        "capture": {
            "service": "Stays",
            "endpoint": "createStay",
            "operation": sig,
            "params": { "body": {
                "guests": 2,
                "start": "2026-06-12T00:00:00Z",
                "end": "2026-06-10T00:00:00Z",
                "guest": guest
            } },
            "facts": {},
            "principal": null,
            "now": "2026-06-01T00:00:00Z",
            "redacted": redacted
        }
    })
}

fn write_json(dir: &Path, name: &str, value: &Value) {
    write(dir, name, &serde_json::to_string_pretty(value).unwrap());
}

#[test]
fn from_violation_prints_the_example_block_and_nothing_else_on_stdout() {
    let tmp = project(STAYS);
    let sig = stays_signature(tmp.path());
    write_json(
        tmp.path(),
        "event.json",
        &capture_event(&sig, &json!([]), true),
    );
    let out = veld(tmp.path())
        .args(["test", "--from-violation", "event.json"])
        .assert()
        .success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        text.starts_with("example \"violation of NewStay.endAfterStart\" {\n"),
        "{text}"
    );
    assert!(text.trim_end().ends_with('}'), "{text}");
    assert!(text.contains("-> ValidationError"), "{text}");
    assert!(
        !text.contains("veld 2.0"),
        "a header is not part of the block:\n{text}"
    );
}

#[test]
fn the_block_pasted_into_the_contract_passes() {
    let tmp = project(STAYS);
    let sig = stays_signature(tmp.path());
    write_json(
        tmp.path(),
        "event.json",
        &capture_event(&sig, &json!([]), true),
    );
    let block = veld(tmp.path())
        .args(["test", "--from-violation", "event.json"])
        .output()
        .unwrap()
        .stdout;
    let block = String::from_utf8(block).unwrap();
    write(
        tmp.path(),
        "contracts/app.veld",
        &format!("{STAYS}\n{block}"),
    );
    veld(tmp.path())
        .arg("test")
        .assert()
        .success()
        .stdout(predicate::str::contains("1 passed"));
}

#[test]
fn a_redacted_value_is_filled_in_and_named_on_stderr() {
    let tmp = project(STAYS);
    let sig = stays_signature(tmp.path());
    let event = capture_event(&sig, &json!(["params.body.guest.phone"]), false);
    write_json(tmp.path(), "event.json", &event);
    veld(tmp.path())
        .args(["test", "--from-violation", "event.json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("phone: \"a\""))
        .stdout(predicate::str::contains("note:").not())
        .stderr(predicate::str::contains(
            "note: filled in params.body.guest.phone",
        ));
}

#[test]
fn an_event_without_a_capture_exits_one() {
    let tmp = project(STAYS);
    let sig = stays_signature(tmp.path());
    let mut event = capture_event(&sig, &json!([]), true);
    event.as_object_mut().unwrap().remove("capture");
    write_json(tmp.path(), "event.json", &event);
    veld(tmp.path())
        .args(["test", "--from-violation", "event.json"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains("the event has no capture"));
}

#[test]
fn an_operation_the_contract_does_not_have_exits_one() {
    let tmp = project(STAYS);
    let sig = stays_signature(tmp.path());
    let mut event = capture_event(&sig, &json!([]), true);
    event["capture"]["operation"] = json!("veld:demo/Stays.gone@0000000000000000");
    write_json(tmp.path(), "event.json", &event);
    veld(tmp.path())
        .args(["test", "--from-violation", "event.json"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains(
            "the contract has no endpoint with the signature veld:demo/Stays.gone@0000000000000000",
        ));
}

#[test]
fn a_capture_that_does_not_fit_the_contract_exits_one() {
    let tmp = project(STAYS);
    let sig = stays_signature(tmp.path());
    let mut event = capture_event(&sig, &json!([]), true);
    event["capture"]["params"]["body"]["guests"] = json!("two");
    write_json(tmp.path(), "event.json", &event);
    veld(tmp.path())
        .args(["test", "--from-violation", "event.json"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains(
            "the capture does not fit the contract",
        ));
}

#[test]
fn a_file_that_is_missing_or_not_json_exits_one() {
    let tmp = project(STAYS);
    veld(tmp.path())
        .args(["test", "--from-violation", "nothing.json"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("cannot read"));
    write(tmp.path(), "event.json", "not json");
    veld(tmp.path())
        .args(["test", "--from-violation", "event.json"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("is not JSON"));
}

#[test]
fn from_violation_cannot_be_combined_with_a_run() {
    let tmp = project(STAYS);
    veld(tmp.path())
        .args(["test", "--from-violation", "event.json", "--coverage"])
        .assert()
        .code(2);
}

// ---- the help text ------------------------------------------------------------------------

#[test]
fn the_help_of_test_and_export_names_the_new_flags() {
    let tmp = tempdir().unwrap();
    veld(tmp.path())
        .args(["test", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--coverage"))
        .stdout(predicate::str::contains("--fail-under"))
        .stdout(predicate::str::contains("--from-violation"));
    veld(tmp.path())
        .args(["export", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("rules"));
}
