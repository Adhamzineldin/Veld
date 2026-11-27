//! Rate limits in the reference evaluator (`spec/platform.md` section 5): the goldens of
//! `tests/wire/limits`, the rule of the memory limiter and the key of a limit.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value as Json;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, LimitSubject, LimitWindow, PackageKind};
use veld_rules::limits::{Decision, MemoryLimiter, key, replay};
use veld_rules::tenant::{caller_tenant, text};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn contract() -> Ir {
    let text = std::fs::read_to_string(root().join("tests/wire/limits/contract.veld"))
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

fn file() -> Json {
    let text = std::fs::read_to_string(root().join("tests/wire/limits/cases.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn decision(allowed: bool, retry: u32) -> Decision {
    Decision {
        allowed,
        retry_after_seconds: retry,
    }
}

#[test]
fn there_are_ten_window_cases_and_seven_keys() {
    let f = file();
    assert_eq!(f["windows"].as_array().unwrap().len(), 10);
    assert_eq!(f["keys"].as_array().unwrap().len(), 7);
}

#[test]
fn every_window_case_gives_the_golden_decisions() {
    for case in file()["windows"].as_array().unwrap() {
        let calls: Vec<(&str, i64)> = case["calls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| (c["key"].as_str().unwrap(), c["t"].as_i64().unwrap()))
            .collect();
        let got = replay(
            &calls,
            u32::try_from(case["limit"].as_u64().unwrap()).unwrap(),
            u32::try_from(case["windowSeconds"].as_u64().unwrap()).unwrap(),
        );
        let want: Vec<Decision> = case["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| {
                decision(
                    d["allowed"].as_bool().unwrap(),
                    u32::try_from(d["retryAfterSeconds"].as_u64().unwrap()).unwrap(),
                )
            })
            .collect();
        assert_eq!(got, want, "{}", case["name"]);
    }
}

#[test]
fn every_key_case_gives_the_golden_key() {
    let ir = contract();
    for case in file()["keys"].as_array().unwrap() {
        let ep = ir
            .services
            .iter()
            .flat_map(|s| &s.endpoints)
            .find(|e| e.name == case["endpoint"].as_str().unwrap())
            .unwrap();
        let limit = ep.limit.unwrap();
        let identity = match limit.by {
            LimitSubject::User => case["caller"].as_str().unwrap().to_owned(),
            LimitSubject::Ip => case["address"].as_str().unwrap().to_owned(),
            LimitSubject::Tenant => {
                let user = ir.services[0].principal.as_ref().unwrap().decl;
                let tenant = caller_tenant(&ir, user, &case["principal"]).unwrap();
                text(ir.decl(user).tenant_field().unwrap(), &tenant).unwrap()
            }
        };
        assert_eq!(
            key(&ep.sig, limit.by, &identity),
            case["key"].as_str().unwrap(),
            "{}",
            case["endpoint"]
        );
    }
}

#[test]
fn the_contract_of_the_keys_lowers_every_limit() {
    let ir = contract();
    let find = |n: &str| {
        ir.services
            .iter()
            .flat_map(|s| &s.endpoints)
            .find(|e| e.name == n)
            .unwrap()
            .limit
            .unwrap()
    };
    assert_eq!(
        (find("byUser").count, find("byUser").per),
        (10, LimitWindow::Minute)
    );
    assert_eq!(find("byTenant").by, LimitSubject::Tenant);
    assert_eq!(find("archive").per.seconds(), 86_400);
    // The service default reaches `listing`; the endpoint's own replaces it on `hot`.
    assert_eq!(find("listing").count, 20);
    assert_eq!(find("hot").count, 2);
}

#[test]
fn a_window_is_open_up_to_but_not_including_its_end() {
    let mut l = MemoryLimiter::new();
    assert_eq!(l.take("k", 1, 60, 0), decision(true, 0));
    assert_eq!(l.take("k", 1, 60, 59_999), decision(false, 1));
    assert_eq!(l.take("k", 1, 60, 60_000), decision(true, 0));
}

#[test]
fn a_refused_call_changes_nothing() {
    let mut l = MemoryLimiter::new();
    l.take("k", 1, 10, 0);
    for t in [1, 2, 3, 4_000, 9_999] {
        assert!(!l.take("k", 1, 10, t).allowed);
    }
    assert!(l.take("k", 1, 10, 10_000).allowed);
}

#[test]
fn the_decisions_do_not_depend_on_other_keys() {
    let mut mixed = MemoryLimiter::new();
    let mut alone = MemoryLimiter::new();
    for t in [0, 5, 10, 20_000, 70_000] {
        mixed.take("noise", 1, 60, t + 1);
        assert_eq!(mixed.take("k", 2, 60, t), alone.take("k", 2, 60, t));
    }
}

#[test]
fn the_key_has_three_parts_joined_with_a_bar() {
    assert_eq!(key("op", LimitSubject::Ip, "1.2.3.4"), "op|ip|1.2.3.4");
    assert_eq!(key("op", LimitSubject::User, ""), "op|user|");
}

#[test]
fn a_tenant_text_is_the_text_of_the_value() {
    let ir = contract();
    let user = ir.services[0].principal.as_ref().unwrap().decl;
    let field = ir.decl(user).tenant_field().unwrap();
    assert_eq!(
        text(
            field,
            &serde_json::json!("0B5C9C3E-AAAA-4D0E-9A55-1F0F6C2F7A01")
        )
        .as_deref(),
        Some("0b5c9c3e-aaaa-4d0e-9a55-1f0f6c2f7a01")
    );
    assert_eq!(text(field, &serde_json::json!(5)), None);
}
