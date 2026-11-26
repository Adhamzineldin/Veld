//! The dead-letter and retry goldens of `tests/wire/events` against the reference implementation
//! (`spec/events.md` sections 10.4 and 10.5), and the reference inbox (section 10.3).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use serde_json::Value as Json;
use veld_ir::{Backoff, Retry};
use veld_rules::Violation;
use veld_rules::envelope::{
    Answer, HandlerResult, Issue, MemoryInbox, Policy, Reason, Settlement, Timing,
    dead_letter_copy, handled, ignored, rejected,
};

fn cases(file: &str) -> Vec<Json> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/wire/events")
        .join(file);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn reason(name: &str) -> Reason {
    match name {
        "malformed" => Reason::Malformed,
        "shape" => Reason::Shape,
        "rule" => Reason::Rule,
        "handler" => Reason::Handler,
        other => panic!("unknown reason {other}"),
    }
}

fn text(case: &Json, key: &str) -> String {
    case[key].as_str().unwrap().to_owned()
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

fn issues(case: &Json) -> Vec<Issue> {
    case["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| Issue {
            path: text(i, "path"),
            code: text(i, "code"),
            message: text(i, "message"),
        })
        .collect()
}

fn violations(case: &Json) -> Vec<Violation> {
    case["violations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| Violation {
            rule_name: String::new(),
            rule_id: text(v, "rule"),
            path: text(v, "path"),
            fields: v["fields"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| f.as_str().unwrap().to_owned())
                .collect(),
            message: text(v, "message"),
            error: v.get("error").map(|e| match e.as_str().unwrap() {
                "overflow" => veld_rules::EvalError::Overflow,
                "division_by_zero" => veld_rules::EvalError::DivisionByZero,
                "non_finite" => veld_rules::EvalError::NonFinite,
                other => panic!("unknown error {other}"),
            }),
        })
        .collect()
}

#[test]
fn the_copy_of_every_golden_is_what_the_reference_writes() {
    let all = cases("dead-letter.json");
    assert!(all.len() >= 20, "{}", all.len());
    for case in all {
        let original = match case.get("originalHex") {
            Some(h) => hex(h.as_str().unwrap()),
            None => text(&case, "original").into_bytes(),
        };
        let want = match case.get("copyHex") {
            Some(h) => hex(h.as_str().unwrap()),
            None => text(&case, "copy").into_bytes(),
        };
        let got = dead_letter_copy(
            &original,
            reason(&text(&case, "reason")),
            &text(&case, "group"),
            u32::try_from(case["attempts"].as_u64().unwrap()).unwrap(),
            &issues(&case),
            &violations(&case),
        );
        assert_eq!(
            String::from_utf8_lossy(&got),
            String::from_utf8_lossy(&want),
            "{}",
            case["name"]
        );
        assert_eq!(got, want, "{}", case["name"]);
    }
}

fn policy_of(case: &Json) -> Policy {
    let retry = case["retry"].as_object().map(|r| Retry {
        max: u32::try_from(r["max"].as_u64().unwrap()).unwrap(),
        backoff: match r["backoff"].as_str().unwrap() {
            "fixed" => Backoff::Fixed,
            _ => Backoff::Exponential,
        },
    });
    Policy {
        retry,
        dead_letter: case["deadLetter"].as_bool().unwrap(),
    }
}

fn answer(a: Answer) -> &'static str {
    a.name()
}

fn check(case: &Json, got: &Settlement) {
    let want = &case["expect"];
    let name = &case["name"];
    assert_eq!(json_u64(got.runs), want["runs"], "{name}: runs");
    let failures: Vec<Json> = got
        .failures
        .iter()
        .map(|f| serde_json::json!({ "attempt": f.attempt, "last": f.last }))
        .collect();
    assert_eq!(Json::Array(failures), want["failures"], "{name}: failures");
    let waits: Vec<Json> = got.waits_ms.iter().map(|w| Json::from(*w)).collect();
    assert_eq!(Json::Array(waits), want["waitsMs"], "{name}: waits");
    let copy = got.copy.map_or(Json::Null, |r| Json::from(r.name()));
    assert_eq!(copy, want["copy"], "{name}: copy");
    assert_eq!(
        json_u64(got.copy_attempts),
        want["copyAttempts"],
        "{name}: copy attempts"
    );
    assert_eq!(
        Json::from(answer(got.answer)),
        want["answer"],
        "{name}: answer"
    );
    assert_eq!(
        Json::from(answer(got.answer_if_the_copy_fails())),
        want["answerIfCopyFails"],
        "{name}: answer when the copy fails"
    );
}

fn json_u64(n: u32) -> Json {
    Json::from(u64::from(n))
}

#[test]
fn the_settlement_of_every_golden_is_what_the_reference_decides() {
    let all = cases("retry.json");
    assert!(all.len() >= 25, "{}", all.len());
    for case in all {
        let policy = policy_of(&case);
        let got = if let Some(r) = case.get("rejected") {
            rejected(policy, reason(r.as_str().unwrap()))
        } else if case.get("ignored").is_some() {
            ignored()
        } else {
            let results: Vec<HandlerResult> = case["results"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| match r.as_str().unwrap() {
                    "returned" => HandlerResult::Returned,
                    _ => HandlerResult::Failed,
                })
                .collect();
            let timing = Timing {
                delay_ms: case["delayMs"].as_u64().unwrap(),
                max_delay_ms: case["maxDelayMs"].as_u64().unwrap(),
            };
            let attempt = u32::try_from(case["transportAttempt"].as_u64().unwrap()).unwrap();
            handled(policy, timing, attempt, &results)
        };
        check(&case, &got);
    }
}

#[test]
fn the_memory_inbox_runs_a_pair_once_and_forgets_a_failure() {
    let mut inbox = MemoryInbox::new();
    let mut runs = 0;
    let first = inbox.handle("veld:bookings/Bookings", "a1", || {
        runs += 1;
        Ok::<(), String>(())
    });
    assert_eq!(first, Ok(true));
    let again = inbox.handle("veld:bookings/Bookings", "a1", || {
        runs += 1;
        Ok::<(), String>(())
    });
    assert_eq!(again, Ok(false));
    assert_eq!(runs, 1);
    // Another id, and another source with the same id, are other pairs.
    assert_eq!(
        inbox.handle("veld:bookings/Bookings", "a2", || Ok::<(), String>(())),
        Ok(true)
    );
    assert_eq!(
        inbox.handle("veld:other/Other", "a1", || Ok::<(), String>(())),
        Ok(true)
    );
    assert_eq!(inbox.len(), 3);
    // A failure is the handler's, and the pair is not recorded: the next delivery runs again.
    let failed = inbox.handle("s", "x", || Err::<(), String>("boom".into()));
    assert_eq!(failed, Err("boom".to_owned()));
    assert!(!inbox.contains("s", "x"));
    assert_eq!(inbox.handle("s", "x", || Ok::<(), String>(())), Ok(true));
    assert!(inbox.contains("s", "x"));
}
