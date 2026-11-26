//! The audit event in the reference evaluator (`spec/platform.md` section 4): the byte goldens of
//! `tests/wire/audit`, and what they cannot state.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_json::{Value as Json, json};
use veld_codegen::canonical::canonical;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_rules::audit::{Ended, event};

use common::audit_runner::run;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn contract() -> Ir {
    let text = std::fs::read_to_string(root().join("tests/wire/audit/contract.veld"))
        .unwrap()
        .replace("\r\n", "\n");
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "bookings".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "t.veld".into(),
                text,
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    let errors: Vec<String> = out
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| format!("{}: {}", d.code, d.message))
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    out.ir.unwrap()
}

fn cases() -> Vec<Json> {
    let text = std::fs::read_to_string(root().join("tests/wire/audit/cases.json")).unwrap();
    let file: Json = serde_json::from_str(&text).unwrap();
    file["cases"].as_array().unwrap().clone()
}

fn clock(case: &Json) -> i64 {
    veld_rules::helpers::instant::parse_datetime(case["clock"].as_str().unwrap()).unwrap()
}

#[test]
fn there_are_seventeen_cases() {
    assert_eq!(cases().len(), 17);
}

#[test]
fn every_case_gives_the_golden_event_status_and_error() {
    let ir = contract();
    for case in cases() {
        let name = case["name"].as_str().unwrap();
        let ran = run(&ir, &case, clock(&case));
        assert_eq!(json!(ran.status), case["status"], "{name}");
        assert_eq!(json!(ran.error), case["error"], "{name}");
        let want = case["event"].as_str();
        let got = ran.event.as_ref().map(canonical);
        assert_eq!(got.as_deref(), want, "{name}");
    }
}

#[test]
fn every_way_a_request_can_end_has_a_case() {
    let statuses: BTreeSet<u64> = cases()
        .iter()
        .map(|c| c["status"].as_u64().unwrap())
        .collect();
    for status in [200, 201, 204, 401, 403, 404, 409, 412, 422, 429, 500] {
        assert!(
            statuses.contains(&status),
            "{status} is missing: {statuses:?}"
        );
    }
}

#[test]
fn no_event_holds_a_value_of_a_pii_or_secret_field() {
    for case in cases() {
        let Some(text) = case["event"].as_str() else {
            continue;
        };
        for marker in [
            "4111111111111111",
            "s3cr3t",
            "example.test",
            "gift",
            "vip",
            "Guest 1",
            "Guest 2",
        ] {
            assert!(!text.contains(marker), "{}: {marker}", case["name"]);
        }
    }
}

#[test]
fn an_event_has_the_same_members_in_every_case() {
    for case in cases() {
        let Some(text) = case["event"].as_str() else {
            continue;
        };
        let e: Json = serde_json::from_str(text).unwrap();
        let obj = e.as_object().unwrap();
        for always in [
            "operation",
            "time",
            "outcome",
            "parameters",
            "facts",
            "hidden",
            "redacted",
        ] {
            assert!(obj.contains_key(always), "{}: {always}", case["name"]);
        }
        assert!(obj["parameters"].is_object() && obj["facts"].is_object());
        assert!(obj["hidden"].is_array() && obj["redacted"].is_array());
        let keys: Vec<&String> = obj.keys().collect();
        assert!(keys.iter().all(|k| {
            [
                "operation",
                "time",
                "principal",
                "outcome",
                "parameters",
                "facts",
                "result",
                "hidden",
                "redacted",
            ]
            .contains(&k.as_str())
        }));
    }
}

#[test]
fn only_an_audited_endpoint_has_an_event() {
    let ir = contract();
    let names: Vec<(&str, &str, bool)> = ir
        .services
        .iter()
        .flat_map(|s| {
            s.endpoints
                .iter()
                .map(|e| (s.name.as_str(), e.name.as_str(), e.audited))
        })
        .collect();
    assert!(names.contains(&("Bookings", "payBooking", true)));
    assert!(names.contains(&("Bookings", "touch", false)));
    // The service-level annotation reaches the endpoint of `Public`.
    assert!(names.contains(&("Public", "publicBooking", true)));
}

#[test]
fn a_constructed_ending_needs_no_pipeline() {
    let ir = contract();
    let s = ir.services.iter().find(|s| s.name == "Bookings").unwrap();
    let ep = s.endpoints.iter().find(|e| e.name == "payBooking").unwrap();
    let none = BTreeMap::new();
    let ended = Ended {
        time_ms: 0,
        principal: None,
        status: 401,
        error: Some("Unauthorized"),
        parameters: None,
        facts: &none,
        result: None,
        hidden: &[],
    };
    let got = event(&ir, ep, s.principal.as_ref().map(|p| p.decl), &ended).unwrap();
    assert_eq!(
        got["outcome"],
        json!({ "status": 401, "error": "Unauthorized" })
    );
    assert_eq!(got["time"], "1970-01-01T00:00:00Z");
    assert!(got.get("principal").is_none() && got.get("result").is_none());
    assert_eq!(got["parameters"], json!({}));
}

#[test]
fn a_value_of_the_wrong_shape_builds_no_event() {
    let ir = contract();
    let s = ir.services.iter().find(|s| s.name == "Bookings").unwrap();
    let ep = s.endpoints.iter().find(|e| e.name == "payBooking").unwrap();
    let params = BTreeMap::from([("body".to_owned(), json!([1, 2]))]);
    let none = BTreeMap::new();
    let ended = Ended {
        time_ms: 0,
        principal: None,
        status: 200,
        error: None,
        parameters: Some(&params),
        facts: &none,
        result: None,
        hidden: &[],
    };
    assert!(event(&ir, ep, None, &ended).is_err());
}
