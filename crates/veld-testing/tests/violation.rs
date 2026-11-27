//! `veld test --from-violation` (`spec/rules.md` section 17.3): from a captured violation to an
//! `example` block. The contract is `fixtures/violation.veld`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use serde_json::{Value, json};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_testing::{ViolationError, from_violation, run_offline};

const FIXTURE: &str = include_str!("fixtures/violation.veld");

fn compile_text(text: &str) -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "t.veld".into(),
                text: text.to_owned(),
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    let Some(ir) = out.ir.clone() else {
        panic!("does not compile:\n{}", out.render());
    };
    ir
}

fn sig(ir: &Ir) -> String {
    ir.services[0].endpoints[0].sig.clone()
}

fn rule_id(ir: &Ir) -> String {
    let decl = ir.decls.iter().find(|d| d.name == "NewStay").unwrap();
    decl.rules()[0].id.clone()
}

/// A capture of a stay that ends before it starts, with `redacted` left out.
fn event(ir: &Ir, body: &Value, redacted: &Value) -> Value {
    let sig = sig(ir);
    json!({
        "rule": rule_id(ir),
        "operation": sig,
        "position": "input",
        "outcome": "rejected",
        "violation": { "rule": rule_id(ir), "path": "$.body", "fields": ["start", "end"],
                       "message": "The stay must end after it starts" },
        "elapsedMicros": 41,
        "capture": {
            "service": "Stays",
            "endpoint": "createStay",
            "operation": sig,
            "params": { "body": body },
            "facts": {},
            "principal": null,
            "now": "2026-06-01T00:00:00Z",
            "redacted": redacted
        }
    })
}

fn full_body() -> Value {
    json!({
        "guests": 2,
        "start": "2026-06-12T00:00:00Z",
        "end": "2026-06-10T00:00:00Z",
        "guest": { "name": "Ada", "phone": "+20100000000" }
    })
}

/// The block, appended to the contract, compiles and passes; returns the example's name.
fn round_trip(block: &str) -> String {
    let ir = compile_text(&format!("{FIXTURE}\n{block}\n"));
    let runs = run_offline(&ir, None);
    assert_eq!(runs.len(), 1, "{block}");
    let report = runs[0]
        .result
        .as_ref()
        .unwrap_or_else(|e| panic!("{e}\n{block}"));
    assert!(report.passed(), "{report}\n{block}");
    runs[0].name.clone()
}

#[test]
fn a_capture_becomes_an_example_the_compiler_accepts_and_the_evaluator_agrees_with() {
    let ir = compile_text(FIXTURE);
    let found = from_violation(&ir, &event(&ir, &full_body(), &json!([]))).unwrap();
    assert!(found.notes.is_empty(), "{:?}", found.notes);
    assert!(
        found
            .block
            .starts_with("example \"violation of NewStay.endAfterStart\" {\n"),
        "{}",
        found.block
    );
    assert!(
        found.block.contains("at \"2026-06-01T00:00:00Z\""),
        "{}",
        found.block
    );
    assert!(
        found.block.contains("phone: \"+20100000000\""),
        "{}",
        found.block
    );
    assert!(
        found.block.contains("-> ValidationError"),
        "{}",
        found.block
    );
    assert_eq!(
        round_trip(&found.block),
        "violation of NewStay.endAfterStart"
    );
}

#[test]
fn a_redacted_value_is_filled_in_and_named() {
    let ir = compile_text(FIXTURE);
    let mut body = full_body();
    body["guest"].as_object_mut().unwrap().remove("phone");
    let event = event(&ir, &body, &json!(["params.body.guest.phone"]));
    let found = from_violation(&ir, &event).unwrap();
    assert_eq!(found.notes, ["filled in params.body.guest.phone"]);
    assert!(found.block.contains("phone: \"a\""), "{}", found.block);
    round_trip(&found.block);
}

#[test]
fn an_example_that_ends_elsewhere_says_it_does_not_reproduce() {
    let ir = compile_text(FIXTURE);
    // The stay is valid: the evaluator passes the request, the event's rule is not violated.
    let mut body = full_body();
    body["end"] = json!("2026-06-14T00:00:00Z");
    let mut event = event(&ir, &body, &json!(["params.body.guest.phone"]));
    event["capture"]["params"]["body"]["guest"]
        .as_object_mut()
        .unwrap()
        .remove("phone");
    let found = from_violation(&ir, &event).unwrap();
    let last = found.notes.last().unwrap();
    assert!(
        last.starts_with("the example does not reproduce the violation of veld:"),
        "{last}"
    );
    assert!(last.contains("it ends in ok"), "{last}");
    assert!(
        last.ends_with("a value was filled in, so the original values may matter"),
        "{last}"
    );
}

#[test]
fn a_violation_of_an_output_rule_says_a_result_is_needed() {
    let ir = compile_text(FIXTURE);
    let mut event = event(&ir, &full_body(), &json!([]));
    event["position"] = json!("output");
    let found = from_violation(&ir, &event).unwrap();
    assert!(
        found
            .notes
            .iter()
            .any(|n| n.starts_with("the violation is of an output rule")),
        "{:?}",
        found.notes
    );
}

#[test]
fn an_event_without_a_capture_is_reported() {
    let ir = compile_text(FIXTURE);
    for event in [
        json!({ "rule": "x", "operation": sig(&ir) }),
        json!({ "rule": "x", "capture": null }),
    ] {
        assert_eq!(from_violation(&ir, &event), Err(ViolationError::NoCapture));
    }
    let text = ViolationError::NoCapture.to_string();
    assert_eq!(
        text,
        "the event has no capture; turn the capture option of the server on"
    );
}

#[test]
fn an_operation_the_contract_does_not_have_is_reported() {
    let ir = compile_text(FIXTURE);
    let mut event = event(&ir, &full_body(), &json!([]));
    event["capture"]["operation"] = json!("veld:t/Stays.gone@0000000000000000");
    let err = from_violation(&ir, &event).unwrap_err();
    assert_eq!(
        err,
        ViolationError::UnknownOperation("veld:t/Stays.gone@0000000000000000".to_owned())
    );
    assert_eq!(
        err.to_string(),
        "the contract has no endpoint with the signature veld:t/Stays.gone@0000000000000000"
    );
}

type Change = Box<dyn Fn(&mut Value)>;

#[test]
fn a_capture_that_does_not_fit_the_contract_is_reported() {
    let ir = compile_text(FIXTURE);
    let cases: Vec<(&str, Change)> = vec![
        (
            "an unknown parameter",
            Box::new(|e| e["capture"]["params"]["other"] = json!(1)),
        ),
        (
            "a missing parameter",
            Box::new(|e| e["capture"]["params"] = json!({})),
        ),
        (
            "a clock that is not a time",
            Box::new(|e| e["capture"]["now"] = json!("yesterday")),
        ),
        (
            "a missing clock",
            Box::new(|e| {
                e["capture"].as_object_mut().unwrap().remove("now");
            }),
        ),
        (
            "a value of the wrong type",
            Box::new(|e| e["capture"]["params"]["body"]["guests"] = json!("two")),
        ),
        (
            "an unknown fact",
            Box::new(|e| e["capture"]["facts"] = json!({ "room": null })),
        ),
        (
            "a principal nobody declared",
            Box::new(|e| e["capture"]["principal"] = json!({ "id": "x" })),
        ),
        (
            "a root that is not params, facts or principal",
            Box::new(|e| e["capture"]["redacted"] = json!(["header.x"])),
        ),
        (
            "a path that is not a path",
            Box::new(|e| e["capture"]["redacted"] = json!(["params.body..x"])),
        ),
    ];
    for (what, change) in cases {
        let mut event = event(&ir, &full_body(), &json!([]));
        change(&mut event);
        match from_violation(&ir, &event) {
            Err(ViolationError::DoesNotFit(why)) => assert!(!why.is_empty(), "{what}"),
            other => panic!("{what}: {other:?}"),
        }
    }
}

#[test]
fn a_value_that_cannot_be_filled_in_is_reported() {
    let text = FIXTURE.replace("phone: string", "phone: string @pattern(\"^[0-9]+$\")");
    let ir = compile_text(&text);
    let mut body = full_body();
    body["guest"].as_object_mut().unwrap().remove("phone");
    let event = event(&ir, &body, &json!(["params.body.guest.phone"]));
    match from_violation(&ir, &event) {
        Err(ViolationError::DoesNotFit(why)) => assert!(why.contains("@pattern"), "{why}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_required_value_the_capture_lacks_and_redacted_does_not_name_does_not_fit() {
    let ir = compile_text(FIXTURE);
    let mut body = full_body();
    body["guest"].as_object_mut().unwrap().remove("name");
    for redacted in [json!([]), json!(["params.body.guest.phone"])] {
        let event = event(&ir, &body, &redacted);
        match from_violation(&ir, &event) {
            Err(ViolationError::DoesNotFit(why)) => {
                assert!(
                    why.contains("`params.body.guest.name` is required"),
                    "{why}"
                );
            }
            other => panic!("{other:?}"),
        }
    }
    // Named, the same value is filled in.
    let event = event(&ir, &body, &json!(["params.body.guest.name"]));
    let found = from_violation(&ir, &event).unwrap();
    assert_eq!(found.notes, ["filled in params.body.guest.name"]);
}

const ROOMS: &str = r#"model Room {
  id: uuid
  capacity: int @min(1)
  rule fits: capacity <= 10
}

@auth(none)
service Rooms at "/rooms" {
  post "/{id}/book" book(id: uuid) -> Room {
    fact room: Room = id
    require room.capacity >= 2
  }
}
"#;

fn rooms_event(ir: &Ir, room: &Value, redacted: &Value) -> Value {
    let sig = ir.services[0].endpoints[0].sig.clone();
    json!({
        "operation": sig,
        "capture": {
            "operation": sig,
            "params": { "id": "00000000-0000-4000-8000-0000000000aa" },
            "facts": { "room": room },
            "now": "2026-06-01T00:00:00Z",
            "redacted": redacted
        }
    })
}

#[test]
fn a_fact_is_held_to_the_same_rule() {
    let ir = compile_text(ROOMS);
    let whole = json!({ "id": "00000000-0000-4000-8000-0000000000bb", "capacity": 1 });
    let found = from_violation(&ir, &rooms_event(&ir, &whole, &json!([]))).unwrap();
    assert!(found.notes.is_empty(), "{:?}", found.notes);
    assert!(
        found
            .block
            .contains("given room = { id: \"00000000-0000-4000-8000-0000000000bb\", capacity: 1 }"),
        "{}",
        found.block
    );

    let without = json!({ "id": "00000000-0000-4000-8000-0000000000bb" });
    match from_violation(&ir, &rooms_event(&ir, &without, &json!([]))) {
        Err(ViolationError::DoesNotFit(why)) => {
            assert!(why.contains("`facts.room.capacity` is required"), "{why}");
        }
        other => panic!("{other:?}"),
    }
    let named = rooms_event(&ir, &without, &json!(["facts.room.capacity"]));
    let found = from_violation(&ir, &named).unwrap();
    assert_eq!(found.notes[0], "filled in facts.room.capacity");
}
