use serde_json::json;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{ChannelId, DeclId, Ir, PackageKind, ServiceId};

use super::*;

const SRC: &str = "event Paid { id: string\n amount: int = 1\n rule pos: amount > 0 }\n\
    event Plain { id: string }\n\
    channel Pays { carries Paid, Plain\n key id }\n\
    service S { publishes Pays }\n";

fn ir() -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "p".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "p.veld".into(),
                text: SRC.into(),
            }],
            base_url: None,
        }],
        consumes: Default::default(),
    });
    out.ir.unwrap()
}

fn event(ir: &Ir, name: &str) -> DeclId {
    ir.decls.iter().find(|d| d.name == name).unwrap().id
}

fn send(ir: &Ir, name: &str, value: serde_json::Value) -> Result<Vec<u8>, PublishError> {
    encode(
        ir,
        ChannelId(0),
        event(ir, name),
        &value,
        &Inputs {
            id: "i1",
            source: &source_of(ir, ServiceId(0)),
            time_ms: 1_000,
        },
    )
}

#[test]
fn the_type_name_drops_the_hash() {
    assert_eq!(type_name("veld:p/Paid@0123"), "veld:p/Paid");
    assert_eq!(type_name("veld:p/Paid"), "veld:p/Paid");
}

#[test]
fn the_source_is_the_package_and_the_service() {
    assert_eq!(source_of(&ir(), ServiceId(0)), "veld:p/S");
}

#[test]
fn a_message_has_the_attributes_of_the_spec_in_canonical_order() {
    let ir = ir();
    let bytes = send(&ir, "Paid", json!({ "id": "k" })).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.starts_with(r#"{"data":{"amount":1,"id":"k"},"datacontenttype":"application/json","id":"i1","source":"veld:p/S","specversion":"1.0","subject":"k","time":"1970-01-01T00:00:01Z","type":"veld:p/Paid@"#), "{text}");
    assert!(text.contains(r#""veldrules":""#), "{text}");
    let plain = send(&ir, "Plain", json!({ "id": "k" })).unwrap();
    assert!(!String::from_utf8(plain).unwrap().contains("veldrules"));
}

#[test]
fn nothing_is_sent_for_an_invalid_event() {
    let ir = ir();
    assert!(matches!(
        send(&ir, "Paid", json!({ "id": "k", "amount": 0 })),
        Err(PublishError::Invalid(v)) if v.len() == 1
    ));
    assert!(matches!(
        send(&ir, "Paid", json!({ "amount": 3 })),
        Err(PublishError::Shape(_))
    ));
    assert!(matches!(
        send(&ir, "Paid", json!({ "id": "" })),
        Err(PublishError::EmptyKey(_))
    ));
}

fn receive(ir: &Ir, text: &str) -> Result<Received, ConsumeError> {
    decode(ir, &[event(ir, "Paid")], text.as_bytes())
}

#[test]
fn a_consumer_gets_what_a_publisher_sent() {
    let ir = ir();
    let bytes = send(&ir, "Paid", json!({ "id": "k", "amount": 5 })).unwrap();
    let got = decode(&ir, &[event(&ir, "Paid")], &bytes).unwrap();
    assert_eq!(got.id, "i1");
    assert_eq!(got.subject.as_deref(), Some("k"));
    assert_eq!(got.data, json!({ "id": "k", "amount": 5 }));
    assert!(got.rules_checked && !got.type_differs && !got.rules_differ());
}

#[test]
fn the_failures_of_section_8_are_typed() {
    let ir = ir();
    for bad in ["", "[]", "{}", r#"{"specversion":"1.0"}"#] {
        assert!(
            matches!(receive(&ir, bad), Err(ConsumeError::Malformed(_))),
            "{bad}"
        );
    }
    let ty = ir.decl(event(&ir, "Paid")).sig.clone();
    let msg = |extra: &str, data: &str| {
        format!(
            r#"{{"specversion":"1.0","id":"a","source":"s","type":"{ty}"{extra},"data":{data}}}"#
        )
    };
    assert!(matches!(
        receive(&ir, &msg(r#","datacontenttype":"text/plain""#, "{}")),
        Err(ConsumeError::Malformed(_))
    ));
    assert!(matches!(
        receive(&ir, &msg("", r#"{"amount":2}"#)),
        Err(ConsumeError::Shape(_))
    ));
    let veld_ir::DeclKind::Event(paid) = &ir.decl(event(&ir, "Paid")).kind else {
        panic!("Paid is not an event");
    };
    let digest = paid.rules_digest.clone().unwrap();
    let own = format!(r#","veldrules":"{digest}""#);
    let broken = r#"{"id":"k","amount":0}"#;
    assert!(matches!(
        receive(&ir, &msg(&own, broken)),
        Err(ConsumeError::Rule(_))
    ));
    assert!(receive(&ir, &msg(r#","veldrules":"0000000000000000""#, broken)).is_ok());
    let other = msg("", "{}").replace("Paid@", "Nope@");
    let err = receive(&ir, &other).unwrap_err();
    assert!(matches!(err, ConsumeError::UnknownType(_)) && err.rejection().is_none());
}

// ---- phase E2: the dead-letter copy, the settlement and the inbox -----------------------------

#[test]
fn a_copy_of_a_valid_message_is_a_valid_message() {
    let ir = ir();
    let original = send(&ir, "Paid", json!({ "id": "k" })).unwrap();
    let issue = Issue {
        path: "$.id".into(),
        code: "type".into(),
        message: "no".into(),
    };
    let copy = dead_letter_copy(&original, Reason::Shape, "g", 0, &[issue], &[]);
    let handled = [event(&ir, "Paid")];
    let got = decode(&ir, &handled, &copy).unwrap();
    assert_eq!(got.id, "i1");
    // The members the copy added are envelope members the consumer does not know: ignored.
    let m: serde_json::Value = serde_json::from_slice(&copy).unwrap();
    assert_eq!(m["veldreason"], "shape");
    assert_eq!(m["veldgroup"], "g");
    assert_eq!(m["veldattempts"], 0);
    assert!(m["veldissues"].is_string() && m.get("veldviolations").is_none());
}

#[test]
fn a_copy_without_issues_and_violations_has_neither_member() {
    let copy = dead_letter_copy(br#"{"a":1}"#, Reason::Handler, "g", 4, &[], &[]);
    assert_eq!(
        String::from_utf8(copy).unwrap(),
        r#"{"a":1,"veldattempts":4,"veldgroup":"g","veldreason":"handler"}"#
    );
}

#[test]
fn what_is_not_an_object_is_copied_as_it_is() {
    for original in [&b"[]"[..], b"7", b"nope", b"\xff", b""] {
        let copy = dead_letter_copy(original, Reason::Malformed, "g", 0, &[], &[]);
        assert_eq!(copy, original);
    }
}

#[test]
fn the_reason_of_a_rejection_is_the_word_of_the_spec() {
    assert_eq!(Reason::Malformed.name(), "malformed");
    assert_eq!(Reason::Shape.name(), "shape");
    assert_eq!(Reason::Rule.name(), "rule");
    assert_eq!(Reason::Handler.name(), "handler");
    assert_eq!(
        Reason::of_rejection(&ConsumeError::Malformed(String::new())),
        Some(Reason::Malformed)
    );
    assert_eq!(
        Reason::of_rejection(&ConsumeError::UnknownType(String::new())),
        None
    );
}

#[test]
fn the_wait_doubles_up_to_the_maximum_and_never_overflows() {
    let t = Timing {
        delay_ms: 1000,
        max_delay_ms: 60_000,
    };
    let waits: Vec<u64> = (1..=8)
        .map(|k| wait_ms(veld_ir::Backoff::Exponential, k, t))
        .collect();
    assert_eq!(
        waits,
        [1000, 2000, 4000, 8000, 16_000, 32_000, 60_000, 60_000]
    );
    assert_eq!(wait_ms(veld_ir::Backoff::Exponential, u32::MAX, t), 60_000);
    assert_eq!(wait_ms(veld_ir::Backoff::Fixed, 5, t), 1000);
    let huge = Timing {
        delay_ms: u64::MAX,
        max_delay_ms: u64::MAX,
    };
    assert_eq!(wait_ms(veld_ir::Backoff::Exponential, 70, huge), u64::MAX);
}

#[test]
fn a_failed_handler_under_retry_runs_max_plus_one_times() {
    let policy = Policy {
        retry: Some(veld_ir::Retry {
            max: 2,
            backoff: veld_ir::Backoff::Fixed,
        }),
        dead_letter: true,
    };
    let s = handled(policy, Timing::default(), 1, &[HandlerResult::Failed]);
    assert_eq!(s.runs, 3);
    assert_eq!(s.waits_ms, [1000, 1000]);
    assert_eq!(s.copy, Some(Reason::Handler));
    assert_eq!(s.copy_attempts, 3);
    assert_eq!(s.answer, Answer::Done);
    assert_eq!(s.answer_if_the_copy_fails(), Answer::Again);
    assert_eq!(
        s.failures
            .iter()
            .map(|f| (f.attempt, f.last))
            .collect::<Vec<_>>(),
        [(1, false), (2, false), (3, true)]
    );
}

#[test]
fn without_retry_a_failed_handler_leaves_the_message_to_the_transport() {
    let s = handled(
        Policy::default(),
        Timing::default(),
        2,
        &[HandlerResult::Failed],
    );
    assert_eq!((s.runs, s.answer, s.copy), (1, Answer::Again, None));
    assert_eq!(s.failures.len(), 1);
    assert!(!s.failures[0].last && s.failures[0].attempt == 2);
}

#[test]
fn a_rejection_is_acknowledged_and_copied_only_with_a_dead_letter() {
    let plain = rejected(Policy::default(), Reason::Rule);
    assert_eq!(
        (plain.runs, plain.copy, plain.answer),
        (0, None, Answer::Done)
    );
    let dead = rejected(
        Policy {
            retry: None,
            dead_letter: true,
        },
        Reason::Malformed,
    );
    assert_eq!(dead.copy, Some(Reason::Malformed));
    assert_eq!(dead.copy_attempts, 0);
    assert_eq!(ignored().answer, Answer::Done);
}

#[test]
fn the_unchecked_message_is_the_message_of_a_publisher_that_did_not_check() {
    let ir = ir();
    let inputs = Inputs {
        id: "i1",
        source: &source_of(&ir, ServiceId(0)),
        time_ms: 1_000,
    };
    // A value the publisher refuses: the invariant `amount > 0` is violated.
    let bad = json!({ "id": "k", "amount": 0 });
    assert!(matches!(
        encode(&ir, ChannelId(0), event(&ir, "Paid"), &bad, &inputs),
        Err(PublishError::Invalid(_))
    ));
    let bytes = encode_unchecked(&ir, ChannelId(0), event(&ir, "Paid"), &bad, &inputs).unwrap();
    let m: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(m["data"], bad);
    assert_eq!(m["subject"], "k");
    assert!(
        m.get("veldrules").is_some(),
        "the consumer must check the rules"
    );
    // The consumer rejects it as the reference says: `rule`.
    let got = decode(&ir, &[event(&ir, "Paid")], &bytes);
    assert!(matches!(got, Err(ConsumeError::Rule(_))), "{got:?}");
    // A value of the wrong shape, and a missing key, still make a message.
    let worse = json!({ "amount": "x" });
    let bytes = encode_unchecked(&ir, ChannelId(0), event(&ir, "Paid"), &worse, &inputs).unwrap();
    let m: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(m.get("subject").is_none());
    assert!(matches!(
        decode(&ir, &[event(&ir, "Paid")], &bytes),
        Err(ConsumeError::Shape(_))
    ));
}
