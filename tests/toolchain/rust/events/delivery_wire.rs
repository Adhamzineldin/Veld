//! The delivery goldens of events phase E2 through the generated consumer: every case of
//! `tests/wire/events/dead-letter.json` through `dead_letter_copy`, and every case of
//! `tests/wire/events/retry.json` through a consumer built the way the generated subscriber builds
//! it, over a transport that records what is published, with a `sleep` that records the wait it was
//! asked for, and again with a transport that cannot publish the copy. Reads `dead-letter.json`,
//! `retry.json` and `delivery.json` (`{ "sig", "digest" }` of `OrderPlaced`, written by the test)
//! from the current directory and prints `delivery wire ok`.
#[path = "../common.rs"]
mod common;

use std::sync::{Arc, Mutex};

use common::*;
use events_server::bookings::t::{ORDER_PLACED_SCHEMA, OrderPlaced};
use events_server::veld::block_on;
use events_server::veld::Issue;
use events_server::veld::events::{
    Backoff, ConsumerOptions, DeadLetterReason, Delivery, EventBinding, EventDef, Outcome, RetryPolicy,
    SubscriptionDef, TransportMessage, Violation, dead_letter_copy, subscribe_events_with,
};
use serde_json::{Value, json};

fn file(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(name).unwrap()).unwrap()
}

fn bytes_of(case: &Value, text: &str, hex: &str) -> Vec<u8> {
    if let Some(h) = case.get(hex).and_then(Value::as_str) {
        return (0..h.len() / 2).map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap()).collect();
    }
    case[text].as_str().unwrap().as_bytes().to_vec()
}

fn reason_of(name: &str) -> DeadLetterReason {
    match name {
        "malformed" => DeadLetterReason::Malformed,
        "shape" => DeadLetterReason::Shape,
        "rule" => DeadLetterReason::Rule,
        "handler" => DeadLetterReason::Handler,
        other => panic!("unknown reason {other}"),
    }
}

fn dead_letters() -> usize {
    let cases = file("dead-letter.json");
    let cases = cases.as_array().unwrap();
    for case in cases {
        let issues: Vec<Issue> = case["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| Issue {
                path: i["path"].as_str().unwrap().to_string(),
                code: i["code"].as_str().unwrap().to_string(),
                message: i["message"].as_str().unwrap().to_string(),
            })
            .collect();
        let violations: Vec<Violation> = case["violations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| Violation {
                rule: v["rule"].as_str().unwrap().to_string(),
                path: v["path"].as_str().unwrap().to_string(),
                fields: v["fields"].as_array().unwrap().iter().map(|f| f.as_str().unwrap().to_string()).collect(),
                message: v["message"].as_str().unwrap().to_string(),
                error: v.get("error").and_then(Value::as_str).map(str::to_string),
            })
            .collect();
        let got = dead_letter_copy(
            &bytes_of(case, "original", "originalHex"),
            reason_of(case["reason"].as_str().unwrap()),
            case["group"].as_str().unwrap(),
            u32::try_from(case["attempts"].as_u64().unwrap()).unwrap(),
            &issues,
            &violations,
        );
        let want = bytes_of(case, "copy", "copyHex");
        assert_eq!(
            String::from_utf8_lossy(&got),
            String::from_utf8_lossy(&want),
            "{}",
            case["name"]
        );
        assert_eq!(got, want, "{}", case["name"]);
    }
    cases.len()
}

/// The envelope of an `OrderPlaced` with `quantity`, or the message that the case `rejected` asks for.
fn message(def: &Value, case: &Value) -> Vec<u8> {
    let sig = def["sig"].as_str().unwrap();
    let digest = def["digest"].clone();
    let envelope = |ty: &str, data: Value| {
        let mut m = json!({"specversion":"1.0","id":"m-1","source":"veld:bookings/Orders","type":ty,
            "datacontenttype":"application/json","time":"2026-06-10T12:00:00Z","subject":"7d1d6c8e-84c8-4b5b-9d87-0a1a2b3c4d5e","data":data});
        if !digest.is_null() {
            m["veldrules"] = digest.clone();
        }
        serde_json::to_vec(&m).unwrap()
    };
    let order = "7d1d6c8e-84c8-4b5b-9d87-0a1a2b3c4d5e";
    match (case.get("rejected").and_then(Value::as_str), case.get("ignored")) {
        // A JSON object that is not an envelope (no `id`), so that the copy is an object too.
        (Some("malformed"), _) => br#"{"specversion":"1.0","source":"s","type":"t","data":{}}"#.to_vec(),
        (Some("shape"), _) => envelope(sig, json!({ "orderId": 5 })),
        (Some("rule"), _) => envelope(sig, json!({ "orderId": order, "quantity": 0 })),
        (_, Some(v)) if !v.is_null() => envelope("veld:bookings/NotOneOfOurs@0000000000000000", json!({})),
        _ => envelope(sig, json!({ "orderId": order, "quantity": 2 })),
    }
}

#[derive(Default)]
struct Seen {
    runs: usize,
    failures: Vec<(u32, bool)>,
    waits: Vec<u64>,
}

/// One delivery of `bytes` to a consumer for the case; what it answered and what it saw.
fn deliver(def: &Value, case: &Value, bytes: &[u8], fail_copy: bool) -> (Outcome, Seen, Arc<Recorder>) {
    let sig: &'static str = Box::leak(def["sig"].as_str().unwrap().to_string().into_boxed_str());
    let digest: Option<&'static str> =
        def["digest"].as_str().map(|d| &*Box::leak(d.to_string().into_boxed_str()));
    let event: &'static EventDef = Box::leak(Box::new(EventDef { name: "OrderPlaced", sig, schema: &ORDER_PLACED_SCHEMA, digest }));
    let results: Vec<bool> = case["results"]
        .as_array()
        .map(|r| r.iter().map(|x| x == "failed").collect())
        .unwrap_or_default();
    let seen = Arc::new(Mutex::new(Seen::default()));
    let runs = Arc::clone(&seen);
    let binding = EventBinding::typed(event, move |_: OrderPlaced, _meta| {
        let mut s = runs.lock().unwrap();
        s.runs += 1;
        let fails = results.get(s.runs - 1).or_else(|| results.last()).copied().unwrap_or(false);
        std::future::ready(if fails { Err(events_server::veld::Error::other("the handler failed")) } else { Ok(()) })
    });
    let retry = case["retry"].as_object().map(|r| RetryPolicy {
        max: u32::try_from(r["max"].as_u64().unwrap()).unwrap(),
        backoff: if r["backoff"] == "fixed" { Backoff::Fixed } else { Backoff::Exponential },
    });
    let delivery = Delivery { retry, dead_letter: case["deadLetter"] == true, ordered: false };
    let (failed, slept) = (Arc::clone(&seen), Arc::clone(&seen));
    let options = ConsumerOptions::new()
        .retry_delay_ms(case["delayMs"].as_u64().unwrap_or(1000))
        .retry_max_delay_ms(case["maxDelayMs"].as_u64().unwrap_or(60_000))
        .sleep(move |ms| {
            slept.lock().unwrap().waits.push(ms);
            Box::pin(std::future::ready(()))
        })
        .on_handler_error(move |f| failed.lock().unwrap().failures.push((f.attempt, f.is_final)));
    let recorder = Recorder::new();
    if fail_copy {
        recorder.fail(Some("shop.orders.v1.dlq"));
    }
    let sub = SubscriptionDef { topic: "shop.orders.v1", group: "fulfilment", on: vec![binding] };
    block_on(subscribe_events_with(recorder.clone(), vec![(sub, delivery)], options)).unwrap();
    let answer = block_on((recorder.deliver())(TransportMessage {
        key: Some("k".to_string()),
        bytes: bytes.to_vec(),
        attempt: u32::try_from(case["transportAttempt"].as_u64().unwrap_or(1)).unwrap(),
    }));
    let seen = std::mem::take(&mut *seen.lock().unwrap());
    (answer, seen, recorder)
}

fn answer_of(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Done => "done",
        Outcome::Again => "again",
    }
}

fn retries() -> usize {
    let def = file("delivery.json");
    let cases = file("retry.json");
    let cases = cases.as_array().unwrap();
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let expect = &case["expect"];
        let bytes = message(&def, case);
        let (answer, seen, recorder) = deliver(&def, case, &bytes, false);
        assert_eq!(seen.runs as u64, expect["runs"].as_u64().unwrap(), "{name}: runs");
        let failures: Vec<Value> = seen.failures.iter().map(|(a, l)| json!({"attempt": a, "last": l})).collect();
        assert_eq!(Value::from(failures), expect["failures"], "{name}: failures");
        assert_eq!(Value::from(seen.waits), expect["waitsMs"], "{name}: waits");
        assert_eq!(answer_of(answer), expect["answer"], "{name}: answer");
        let copies: Vec<Sent> = recorder.messages().into_iter().filter(|m| m.0 == "shop.orders.v1.dlq").collect();
        match expect["copy"].as_str() {
            None => assert!(copies.is_empty(), "{name}: no copy"),
            Some(reason) => {
                assert_eq!(copies.len(), 1, "{name}: one copy");
                let copy: Value = serde_json::from_slice(&copies[0].2).unwrap();
                assert_eq!(copy["veldreason"], reason, "{name}: reason");
                assert_eq!(copy["veldattempts"], expect["copyAttempts"], "{name}: attempts");
                assert_eq!(copy["veldgroup"], "fulfilment", "{name}: group");
                assert_eq!(copies[0].1.as_deref(), Some("k"), "{name}: the key of the original");
            }
        }
        let (again, _, _) = deliver(&def, case, &bytes, true);
        assert_eq!(answer_of(again), expect["answerIfCopyFails"], "{name}: answerIfCopyFails");
    }
    cases.len()
}

fn main() {
    let copies = dead_letters();
    let cases = retries();
    println!("delivery wire ok: {copies} copies, {cases} retry cases");
}
