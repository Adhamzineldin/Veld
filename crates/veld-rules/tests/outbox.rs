//! The outbox and the relay of `spec/events.md` section 11.1 against the goldens of
//! `tests/wire/events/outbox.json` and the pending contract `tests/events/outbox.veld.pending`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value as Json, json};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{ChannelId, DeclId, Ir, PackageKind, ServiceId};
use veld_rules::outbox::{
    MemoryOutbox, OutboxError, Save, Step, WrittenFault, check_written, expected_events, save_of,
};
use veld_rules::{Verdict, run_example};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn compile_as(package: &str, path: &str) -> Ir {
    let text = std::fs::read_to_string(root().join(path))
        .unwrap()
        .replace("\r\n", "\n");
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: package.into(),
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
    assert!(errors.is_empty(), "{path}: {errors:?}");
    out.ir.unwrap()
}

fn goldens() -> Json {
    let path = root().join("tests/wire/events/outbox.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn event_id(ir: &Ir, name: &str) -> DeclId {
    ir.decls
        .iter()
        .find(|d| d.is_event() && d.name == name)
        .unwrap_or_else(|| panic!("no event {name}"))
        .id
}

fn channel_id(ir: &Ir, name: &str) -> ChannelId {
    let at = ir.channels.iter().position(|c| c.name == name).unwrap();
    ChannelId(u32::try_from(at).unwrap())
}

fn service_id(ir: &Ir, name: &str) -> ServiceId {
    ir.services.iter().find(|s| s.name == name).unwrap().id
}

fn time_ms(text: &str) -> i64 {
    veld_rules::helpers::instant::parse_datetime(text).unwrap()
}

fn text<'a>(v: &'a Json, key: &str) -> &'a str {
    v[key].as_str().unwrap()
}

// --- the entry of an emitted event -------------------------------------------------------------

#[test]
fn the_outbox_entry_of_every_golden_has_the_topic_the_key_and_the_bytes() {
    let cases = goldens()["entries"].as_array().unwrap().clone();
    assert!(cases.len() >= 13, "{}", cases.len());
    for case in &cases {
        let ir = compile_as("bookings", text(case, "contract"));
        let saved = save_of(
            &ir,
            service_id(&ir, text(case, "service")),
            channel_id(&ir, text(case, "channel")),
            event_id(&ir, text(case, "event")),
            &case["value"],
            text(case, "id"),
            time_ms(text(case, "time")),
        )
        .unwrap();
        assert_eq!(saved.topic, text(case, "topic"), "{}", case["name"]);
        assert_eq!(
            saved.key.as_deref(),
            case["key"].as_str(),
            "{}",
            case["name"]
        );
        assert_eq!(
            String::from_utf8(saved.bytes).unwrap(),
            text(case, "canonical"),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn the_bytes_of_an_entry_are_the_bytes_of_the_publisher() {
    let ir = compile_as("bookings", "tests/events/bookings.veld");
    let service = service_id(&ir, "Bookings");
    let channel = channel_id(&ir, "BookingEvents");
    let event = event_id(&ir, "BookingPaid");
    let value = json!({"bookingId": "5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21", "amount": "120.00",
                       "paidAt": "2026-06-10T11:59:58Z"});
    let saved = save_of(&ir, service, channel, event, &value, "i-1", 0).unwrap();
    let source = veld_rules::envelope::source_of(&ir, service);
    let sent = veld_rules::envelope::encode(
        &ir,
        channel,
        event,
        &value,
        &veld_rules::envelope::Inputs {
            id: "i-1",
            source: &source,
            time_ms: 0,
        },
    )
    .unwrap();
    assert_eq!(saved.bytes, sent);
}

#[test]
fn nothing_is_saved_for_a_value_the_publisher_would_refuse() {
    let ir = compile_as("bookings", "tests/events/bookings.veld");
    let (service, channel, event) = (
        service_id(&ir, "Bookings"),
        channel_id(&ir, "BookingEvents"),
        event_id(&ir, "BookingPaid"),
    );
    let broken = json!({"bookingId": "5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21", "amount": "0.00",
                        "paidAt": "2026-06-10T11:59:58Z"});
    assert!(matches!(
        save_of(&ir, service, channel, event, &broken, "i", 0),
        Err(OutboxError::Publish(
            veld_rules::envelope::PublishError::Invalid(_)
        ))
    ));
    let no_amount = json!({"bookingId": "5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21"});
    assert!(matches!(
        save_of(&ir, service, channel, event, &no_amount, "i", 0),
        Err(OutboxError::Publish(
            veld_rules::envelope::PublishError::Shape(_)
        ))
    ));
    let fine = json!({"bookingId": "5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21", "amount": "1.00",
                      "paidAt": "2026-06-10T11:59:58Z"});
    assert!(matches!(
        save_of(&ir, service, channel, event, &fine, "", 0),
        Err(OutboxError::Publish(
            veld_rules::envelope::PublishError::EmptyId
        ))
    ));
}

#[test]
fn a_service_that_does_not_publish_the_channel_saves_nothing() {
    let ir = compile_as("bookings", "tests/events/bookings.veld");
    let value = json!({"bookingId": "5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21", "amount": "1.00",
                       "paidAt": "2026-06-10T11:59:58Z"});
    let got = save_of(
        &ir,
        service_id(&ir, "Notifications"),
        channel_id(&ir, "BookingEvents"),
        event_id(&ir, "BookingPaid"),
        &value,
        "i",
        0,
    );
    assert_eq!(got, Err(OutboxError::NotPublished("BookingEvents".into())));
}

// --- the memory outbox and the relay -------------------------------------------------------------

fn step(v: &Json) -> Step {
    match v {
        Json::String(s) if s == "accepted" => Step::Accepted,
        Json::String(s) if s == "crashAfterPublish" => Step::CrashAfterPublish,
        Json::Object(o) if o.contains_key("publishFails") => {
            Step::PublishFails(o["publishFails"].as_str().unwrap().into())
        }
        Json::Object(o) if o.contains_key("doneFails") => {
            Step::DoneFails(o["doneFails"].as_str().unwrap().into())
        }
        other @ (Json::Null
        | Json::Bool(_)
        | Json::Number(_)
        | Json::String(_)
        | Json::Array(_)
        | Json::Object(_)) => panic!("unknown step {other}"),
    }
}

fn names(v: &Json) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn every_relay_golden_publishes_marks_done_and_stops_where_it_says() {
    let cases = goldens()["relay"].as_array().unwrap().clone();
    assert_eq!(cases.len(), 9);
    for case in &cases {
        let mut outbox = MemoryOutbox::new();
        for i in 1..=case["saves"].as_u64().unwrap() {
            let id = outbox.save(Save {
                topic: "t".into(),
                key: None,
                bytes: i.to_string().into_bytes(),
            });
            assert_eq!(id, i.to_string());
        }
        for run in case["runs"].as_array().unwrap() {
            let script: Vec<Step> = run["script"].as_array().unwrap().iter().map(step).collect();
            let limit = usize::try_from(run["limit"].as_u64().unwrap()).unwrap();
            let got = outbox.run_relay(limit, &script);
            let want = &run["expect"];
            assert_eq!(got.published, names(&want["published"]), "{}", case["name"]);
            assert_eq!(got.done, names(&want["done"]), "{}", case["name"]);
            assert_eq!(
                got.sent as u64,
                want["sent"].as_u64().unwrap(),
                "{}",
                case["name"]
            );
            let failure = got
                .failure
                .map(|(entry, error)| json!({"entry": entry, "error": error}));
            assert_eq!(
                failure.unwrap_or(Json::Null),
                want["failure"],
                "{}",
                case["name"]
            );
            let pending: Vec<String> = outbox
                .pending(usize::MAX)
                .into_iter()
                .map(|e| e.id)
                .collect();
            assert_eq!(pending, names(&run["pendingAfter"]), "{}", case["name"]);
        }
    }
}

#[test]
fn the_memory_outbox_lists_oldest_first_up_to_the_limit_and_forgets_what_is_done() {
    let mut outbox = MemoryOutbox::new();
    assert!(outbox.is_empty());
    let save = |n: u8| Save {
        topic: "shop".into(),
        key: Some(format!("k{n}")),
        bytes: vec![n],
    };
    let ids: Vec<String> = (1..=3).map(|n| outbox.save(save(n))).collect();
    assert_eq!(ids, ["1", "2", "3"]);
    assert_eq!(outbox.pending(2).len(), 2);
    assert_eq!(outbox.pending(2)[1].key.as_deref(), Some("k2"));
    outbox.done("2");
    outbox.done("2");
    outbox.done("nope");
    let left: Vec<String> = outbox.pending(10).into_iter().map(|e| e.id).collect();
    assert_eq!(left, ["1", "3"]);
    assert_eq!(outbox.len(), 2);
    // An id is never reused.
    assert_eq!(outbox.save(save(4)), "4");
}

// --- what the server checks after a handler ------------------------------------------------------

fn pending_contract() -> Ir {
    compile_as("t", "tests/events/outbox.veld.pending")
}

fn transition_of<'a>(
    ir: &'a Ir,
    endpoint: &str,
) -> &'a veld_ir::endpoint_rules::EndpointTransition {
    ir.service(service_id(ir, "Bookings"))
        .endpoints
        .iter()
        .find(|e| e.name == endpoint)
        .unwrap()
        .rules
        .transition
        .as_ref()
        .unwrap()
}

#[test]
fn the_events_of_a_move_depend_on_the_edge_taken() {
    let ir = pending_contract();
    let named = |ids: Vec<DeclId>| -> Vec<String> {
        ids.into_iter().map(|d| ir.decl(d).name.clone()).collect()
    };
    let pay = transition_of(&ir, "pay");
    assert_eq!(named(expected_events(&ir, pay, "pending")), ["BookingPaid"]);
    assert!(expected_events(&ir, pay, "paid").is_empty());
    let cancel = transition_of(&ir, "cancel");
    assert_eq!(
        named(expected_events(&ir, cancel, "pending")),
        ["BookingCancelled"]
    );
    assert_eq!(named(expected_events(&ir, cancel, "paid")), ["RefundDue"]);
    assert!(expected_events(&ir, cancel, "refunded").is_empty());
    assert!(expected_events(&ir, transition_of(&ir, "refund"), "paid").is_empty());
}

#[test]
fn the_ir_of_the_pending_contract_names_the_event_and_the_channel_of_each_move() {
    let ir = pending_contract();
    let cancel = transition_of(&ir, "cancel");
    let got: Vec<(String, String)> = cancel
        .emits
        .iter()
        .map(|e| {
            (
                ir.decl(e.event).name.clone(),
                ir.channel(e.channel).name.clone(),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            ("BookingCancelled".to_owned(), "BookingEvents".to_owned()),
            ("RefundDue".to_owned(), "RefundEvents".to_owned())
        ]
    );
    assert!(transition_of(&ir, "refund").emits.is_empty());
}

#[test]
fn every_written_golden_gets_the_verdict_it_names() {
    let ir = pending_contract();
    let ids = |v: &Json| -> Vec<DeclId> { names(v).iter().map(|n| event_id(&ir, n)).collect() };
    let cases = goldens()["written"].as_array().unwrap().clone();
    assert_eq!(cases.len(), 6);
    for case in &cases {
        let got = match check_written(&ids(&case["expected"]), &ids(&case["written"])) {
            Ok(()) => "ok".to_owned(),
            Err(WrittenFault::Missing(e)) => format!("missing {}", ir.decl(e).name),
            Err(WrittenFault::Twice(e)) => format!("twice {}", ir.decl(e).name),
            Err(WrittenFault::Unexpected(e)) => format!("unexpected {}", ir.decl(e).name),
        };
        assert_eq!(got, text(case, "result"), "{}", case["name"]);
    }
}

// --- the pending contract ----------------------------------------------------------------------

#[test]
fn the_pending_contract_runs_offline_and_the_emitted_lines_are_live_only() {
    let ir = pending_contract();
    assert_eq!(ir.examples.len(), 10);
    let mut live_only = 0;
    for example in &ir.examples {
        let report = run_example(&ir, example).unwrap();
        match &report.verdict {
            Verdict::Passed => assert!(example.emitted.is_none(), "{report}"),
            Verdict::Skipped(why) => {
                assert!(example.emitted.is_some(), "{report}");
                assert!(why.starts_with("live only: "), "{why}");
                live_only += 1;
            }
            Verdict::Failed(_) => panic!("{report}"),
        }
    }
    assert_eq!(live_only, 5);
}

#[test]
fn an_emitted_line_does_not_hide_a_wrong_outcome() {
    let mut ir = pending_contract();
    let at = ir
        .examples
        .iter()
        .position(|e| e.name == "paying a pending booking writes BookingPaid")
        .unwrap();
    // The example now says the call is refused; the reference answers `ok` and the example fails.
    let refused = ir
        .examples
        .iter()
        .find(|e| e.name == "a move that is refused writes nothing")
        .unwrap()
        .outcome
        .clone();
    ir.examples[at].outcome = refused;
    let report = run_example(&ir, &ir.examples[at]).unwrap();
    assert!(matches!(report.verdict, Verdict::Failed(_)), "{report}");
}
