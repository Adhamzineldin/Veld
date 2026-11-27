//! Sagas (`spec/platform.md` sections 17, 18 and 20): what a `saga post` endpoint lowers to, the
//! implicit idempotency and errors, `E1201`, `E1202`, `E1203`, `W1201`, and the saga lines of an
//! example.

use std::collections::BTreeMap;

use crate::testing::{codes, ir as build, one};
use crate::{CompileInput, CompileOutput, PackageInput, SourceFile, compile};
use veld_ir::{Feature, Ir, PackageKind, SagaValue};

const BASE: &str = r#"
model Hold { id: uuid }
model Pay { id: uuid }
model Booking { id: uuid }
model Req { roomId: uuid, note?: string }
error RoomFull 409 { capacity: int }
error PaymentDeclined 402 { reason: string }

service Rooms at "/rooms" {
  @idempotent
  post "/hold" hold(body: Req) -> Hold | RoomFull
  @idempotent
  post "/release/{id}" release(id: uuid) -> 204
  post "/loose" loose(body: Req) -> Hold
  get "/watch" watch() -> stream Hold
}
service Payments at "/pay" {
  @idempotent
  post "/charge" charge(body: Req) -> Pay | PaymentDeclined
  @idempotent
  post "/refund/{id}" refund(id: uuid) -> 204
}
service Bookings at "/bookings" {
  @idempotent
  post "/confirm/{id}" confirm(id: uuid) -> Booking
}
"#;

const CHECKOUT: &str = r#"
service Checkout at "/checkout" {
  saga post "/" checkout(body: Req) -> Booking | RoomFull | PaymentDeclined {
    step hold    = Rooms.hold(body: { roomId: body.roomId })  compensate Rooms.release(id: hold.id)
    step charge  = Payments.charge(body: body)                compensate Payments.refund(id: charge.id)
    step confirm = Bookings.confirm(id: hold.id)
    timeout minutes(5)
  }
}
"#;

fn with(service: &str) -> String {
    format!("{BASE}{service}")
}

fn saga_codes(service: &str) -> Vec<&'static str> {
    codes(&with(service))
}

fn saga_of(ir: &Ir) -> &veld_ir::Saga {
    let ep = ir
        .services
        .iter()
        .flat_map(|s| &s.endpoints)
        .find(|e| e.saga.is_some());
    match ep.and_then(|e| e.saga.as_ref()) {
        Some(s) => s,
        None => panic!("no saga in the IR"),
    }
}

fn checkout_endpoint(ir: &Ir) -> &veld_ir::Endpoint {
    match ir
        .services
        .iter()
        .flat_map(|s| &s.endpoints)
        .find(|e| e.name == "checkout")
    {
        Some(e) => e,
        None => panic!("no checkout"),
    }
}

// ---- what a saga lowers to --------------------------------------------------------------------

#[test]
fn a_saga_lowers_to_steps_calls_and_a_timeout() {
    let ir = build(&with(CHECKOUT));
    let saga = saga_of(&ir);
    let names: Vec<&str> = saga.steps.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["hold", "charge", "confirm"]);
    assert_eq!(saga.timeout_ms, 300_000);
    let hold = &saga.steps[0];
    assert_eq!(hold.call.endpoint, "hold");
    assert_eq!(
        hold.compensate.as_ref().map(|c| c.endpoint.as_str()),
        Some("release")
    );
    assert!(saga.steps[2].compensate.is_none());
    let service = &ir.services[hold.call.service.index()];
    assert_eq!(service.name, "Rooms");
}

#[test]
fn an_object_argument_keeps_wire_names_and_leaves_out_what_is_not_given() {
    let ir = build(&with(CHECKOUT));
    let saga = saga_of(&ir);
    let arg = &saga.steps[0].call.args[0];
    assert_eq!(arg.name, "body");
    let SagaValue::Object { object } = &arg.value else {
        panic!("not an object: {arg:?}")
    };
    assert_eq!(object.len(), 1);
    assert_eq!(object[0].name, "roomId");
    assert_eq!(object[0].wire, "roomId");
}

#[test]
fn a_step_is_read_as_a_fact_and_a_parameter_as_a_param() {
    let ir = build(&with(CHECKOUT));
    let saga = saga_of(&ir);
    let json = serde_json::to_string(&saga.steps[0].compensate).unwrap_or_default();
    assert!(json.contains("{\"fact\":\"hold\"}"), "{json}");
    let first = serde_json::to_string(&saga.steps[0].call).unwrap_or_default();
    assert!(first.contains("{\"param\":\"body\"}"), "{first}");
}

#[test]
fn the_saga_is_idempotent_and_required_and_declares_its_errors() {
    let ir = build(&with(CHECKOUT));
    let ep = checkout_endpoint(&ir);
    let idem = ep.idempotency.as_ref();
    assert_eq!(idem.map(|i| i.header.as_str()), Some("Idempotency-Key"));
    assert_eq!(idem.map(|i| i.required), Some(true));
    let errors: Vec<&str> = ep
        .errors
        .iter()
        .map(|e| ir.decls[e.decl.index()].name.as_str())
        .collect();
    for want in [
        "SagaFailed",
        "Conflict",
        "ValidationError",
        "RoomFull",
        "PaymentDeclined",
        "Internal",
    ] {
        assert!(errors.contains(&want), "{want} in {errors:?}");
    }
    let failed = ep
        .errors
        .iter()
        .find(|e| ir.decls[e.decl.index()].name == "SagaFailed");
    assert_eq!(failed.map(|e| e.status), Some(502));
}

#[test]
fn only_the_errors_the_saga_writes_pass_through() {
    let ir = build(&with(CHECKOUT));
    let saga = saga_of(&ir);
    let mut passes: Vec<&str> = saga
        .passes
        .iter()
        .map(|d| ir.decls[d.index()].name.as_str())
        .collect();
    passes.sort_unstable();
    assert_eq!(passes, ["PaymentDeclined", "RoomFull"]);
}

#[test]
fn a_saga_uses_the_sagas_feature_and_a_plain_service_does_not() {
    let ir = build(&with(CHECKOUT));
    let checkout = ir.services.iter().position(|s| s.name == "Checkout");
    let id = veld_ir::ServiceId(u32::try_from(checkout.unwrap_or(0)).unwrap_or(0));
    assert!(ir.features_used(&[id]).contains(&Feature::Sagas));
    let rooms = ir
        .services
        .iter()
        .position(|s| s.name == "Rooms")
        .unwrap_or(0);
    let rooms = veld_ir::ServiceId(u32::try_from(rooms).unwrap_or(0));
    assert!(!ir.features_used(&[rooms]).contains(&Feature::Sagas));
}

#[test]
fn a_contract_without_a_saga_keeps_its_ir() {
    let text = serde_json::to_string(&build(BASE)).unwrap_or_default();
    for key in ["\"saga\"", "SagaFailed", "\"passes\"", "\"timeout_ms\""] {
        assert!(
            !text.contains(key),
            "{key} in the IR of a contract without a saga"
        );
    }
}

#[test]
fn a_saga_changes_no_signature_of_what_it_calls() {
    let plain = build(BASE);
    let sagas = build(&with(CHECKOUT));
    for name in ["hold", "release", "charge", "refund", "confirm"] {
        let sig = |ir: &Ir| {
            ir.services
                .iter()
                .flat_map(|s| &s.endpoints)
                .find(|e| e.name == name)
                .map(|e| e.sig.clone())
        };
        assert_eq!(sig(&plain), sig(&sagas), "{name}");
    }
}

#[test]
fn a_saga_that_names_the_idempotency_header_keeps_it() {
    let src = CHECKOUT.replace(
        "  saga post",
        "  @idempotent(header: \"X-Saga\")\n  saga post",
    );
    let ir = build(&with(&src));
    let i = checkout_endpoint(&ir).idempotency.as_ref();
    assert_eq!(i.map(|i| i.header.as_str()), Some("X-Saga"));
    assert_eq!(i.map(|i| i.required), Some(true));
}

// ---- E1201 ------------------------------------------------------------------------------------

fn saga_with(steps: &str) -> String {
    format!(
        "service Checkout at \"/checkout\" {{\n  saga post \"/\" checkout(body: Req) -> Hold | RoomFull {{\n{steps}\n    timeout minutes(5)\n  }}\n}}\n"
    )
}

#[test]
fn an_unknown_service_or_endpoint_is_e1201() {
    let service = saga_with("    step hold = Roms.hold(body: { roomId: body.roomId })");
    assert_eq!(saga_codes(&service), ["E1201"]);
    let endpoint = saga_with("    step hold = Rooms.holds(body: { roomId: body.roomId })");
    assert_eq!(saga_codes(&endpoint), ["E1201"]);
}

#[test]
fn a_stream_cannot_be_a_step() {
    let src = saga_with("    step hold = Rooms.watch()");
    assert_eq!(saga_codes(&src), ["E1201"]);
}

#[test]
fn arguments_must_fit_the_endpoint() {
    for (what, line) in [
        (
            "an unknown parameter",
            "    step hold = Rooms.hold(nope: body)",
        ),
        ("a missing parameter", "    step hold = Rooms.hold()"),
        (
            "a parameter twice",
            "    step hold = Rooms.hold(body: body, body: body)",
        ),
        (
            "a value of another type",
            "    step hold = Rooms.hold(body: body.roomId)",
        ),
        (
            "a field that is not there",
            "    step hold = Rooms.hold(body: { roomId: body.roomId, nope: 1 })",
        ),
        (
            "a missing required field",
            "    step hold = Rooms.hold(body: { note: \"x\" })",
        ),
        (
            "an object where a scalar goes",
            "    step hold = Rooms.release(id: { a: 1 })",
        ),
    ] {
        let found = saga_codes(&saga_with(line));
        assert!(
            !found.is_empty() && found.iter().all(|c| *c == "E1201"),
            "{what}: {found:?}"
        );
    }
}

#[test]
fn a_service_that_is_not_consumed_is_e1201_and_a_consumed_one_is_not() {
    let rooms = "model Hold { id: uuid }\nmodel Req { roomId: uuid }\nservice Rooms at \"/rooms\" {\n  @idempotent\n  post \"/hold\" hold(body: Req) -> Hold\n}\n";
    let checkout = "model Mine { roomId: uuid }\nservice Checkout at \"/checkout\" {\n  saga post \"/\" go(body: Mine) -> 204 {\n    step hold = Rooms.hold(body: { roomId: body.roomId })\n    timeout seconds(30)\n  }\n}\n";
    let out = two_packages(rooms, checkout, &[]);
    assert!(out.ir.is_none());
    let found: Vec<&str> = out.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(found, ["E1201"], "{}", out.render());
    assert!(out.render().contains("not consumed"), "{}", out.render());
    let consumed = two_packages(rooms, checkout, &["rooms"]);
    assert!(consumed.ir.is_some(), "{}", consumed.render());
    let ir = consumed.ir.unwrap_or_else(|| panic!("no IR"));
    let saga = saga_of(&ir);
    assert_eq!(
        ir.services[saga.steps[0].call.service.index()].name,
        "Rooms"
    );
}

fn two_packages(rooms: &str, checkout: &str, consumed: &[&str]) -> CompileOutput {
    let pkg = |name: &str, text: &str| PackageInput {
        name: name.to_string(),
        kind: PackageKind::Service,
        files: vec![SourceFile {
            path: "app.veld".to_string(),
            text: text.to_string(),
        }],
        base_url: None,
    };
    compile(&CompileInput {
        packages: vec![pkg("rooms", rooms), pkg("checkout", checkout)],
        consumes: BTreeMap::from([(
            "checkout".to_string(),
            consumed.iter().map(ToString::to_string).collect(),
        )]),
    })
}

// ---- E1202 ------------------------------------------------------------------------------------

#[test]
fn a_step_or_a_compensation_that_is_not_idempotent_is_e1202() {
    let step = saga_with("    step hold = Rooms.loose(body: body)");
    assert_eq!(saga_codes(&step), ["E1202"]);
    let undo = saga_with(
        "    step hold = Rooms.hold(body: body) compensate Rooms.loose(body: body)\n    step again = Rooms.hold(body: body)",
    );
    assert_eq!(saga_codes(&undo), ["E1202"]);
}

#[test]
fn another_saga_is_idempotent_and_can_be_a_step() {
    let src = format!(
        "{CHECKOUT}service Outer at \"/outer\" {{\n  saga post \"/\" outer(body: Req) -> Booking | RoomFull | PaymentDeclined {{\n    step inner = Checkout.checkout(body: body)\n    timeout hours(1)\n  }}\n}}\n"
    );
    assert!(one(&with(&src)).ir.is_some());
}

// ---- E1203 ------------------------------------------------------------------------------------

#[test]
fn a_saga_without_steps_or_without_a_timeout_is_e1203() {
    let none = "service C at \"/c\" {\n  saga post \"/\" go(body: Req) -> 204 {\n    timeout seconds(1)\n  }\n}\n";
    assert_eq!(saga_codes(none), ["E1203"]);
    let no_timeout = "service C at \"/c\" {\n  saga post \"/\" go(body: Req) -> 204 {\n    step a = Rooms.release(id: body.roomId)\n  }\n}\n";
    assert_eq!(saga_codes(no_timeout), ["E1203"]);
}

#[test]
fn the_timeout_is_a_positive_constant_duration() {
    for (what, timeout) in [
        ("zero", "seconds(0)"),
        ("negative", "-seconds(1)"),
        ("not a duration", "5"),
        ("not constant", "seconds(len(body.note))"),
    ] {
        let src = format!(
            "service C at \"/c\" {{\n  saga post \"/\" go(body: Req) -> 204 {{\n    step a = Rooms.release(id: body.roomId)\n    timeout {timeout}\n  }}\n}}\n"
        );
        let found = saga_codes(&src);
        assert!(
            found.contains(&"E1203") || found.contains(&"E0902") || found.contains(&"E0903"),
            "{what}: {found:?}"
        );
    }
    let sum = "service C at \"/c\" {\n  saga post \"/\" go(body: Req) -> 204 {\n    step a = Rooms.release(id: body.roomId)\n    timeout minutes(1) + seconds(30)\n  }\n}\n";
    let ir = build(&with(sum));
    assert_eq!(saga_of(&ir).timeout_ms, 90_000);
}

#[test]
fn step_names_must_be_free() {
    let twice =
        saga_with("    step hold = Rooms.hold(body: body)\n    step hold = Rooms.hold(body: body)");
    assert!(saga_codes(&twice).contains(&"E1203"));
    let param = saga_with("    step body = Rooms.hold(body: body)");
    assert!(saga_codes(&param).contains(&"E1203"));
}

#[test]
fn reading_a_later_step_or_the_clock_is_e1203() {
    let later = saga_with(
        "    step hold = Rooms.release(id: confirm.id)\n    step confirm = Bookings.confirm(id: body.roomId)",
    );
    assert_eq!(saga_codes(&later), ["E1203", "W1201"]);
    let clock =
        saga_with("    step hold = Rooms.hold(body: { roomId: body.roomId, note: toText(now) })");
    assert!(saga_codes(&clock).contains(&"E1203") || saga_codes(&clock).contains(&"E0906"));
    // A step can read itself in its own compensation.
    let own = saga_with(
        "    step hold = Rooms.hold(body: body) compensate Rooms.release(id: hold.id)\n    step next = Rooms.hold(body: body)",
    );
    assert!(one(&with(&own)).ir.is_some());
}

#[test]
fn the_last_step_must_answer_the_type_of_the_saga() {
    let wrong = "service C at \"/c\" {\n  saga post \"/\" go(body: Req) -> Booking {\n    step a = Rooms.hold(body: body)\n    timeout seconds(1)\n  }\n}\n";
    assert_eq!(saga_codes(wrong), ["E1203"]);
    let right = "service C at \"/c\" {\n  saga post \"/\" go(body: Req) -> Hold {\n    step a = Rooms.hold(body: body)\n    timeout seconds(1)\n  }\n}\n";
    assert!(one(&with(right)).ir.is_some());
}

#[test]
fn a_statement_of_an_endpoint_block_is_e1203() {
    let src = "service C at \"/c\" {\n  saga post \"/\" go(body: Req) -> Hold {\n    require true\n    step a = Rooms.hold(body: body)\n    timeout seconds(1)\n  }\n}\n";
    assert_eq!(saga_codes(src), ["E1203"]);
}

// ---- W1201 ------------------------------------------------------------------------------------

#[test]
fn a_step_before_the_last_without_compensate_is_w1201() {
    let src = saga_with(
        "    step hold = Rooms.hold(body: body)\n    step again = Rooms.hold(body: body)",
    );
    assert_eq!(saga_codes(&src), ["W1201"]);
    let out = one(&with(&src));
    assert!(out.ir.is_some(), "a warning does not stop the compile");
}

// ---- examples ---------------------------------------------------------------------------------

const EXAMPLE_TAIL: &str = r#"
example "a declined payment releases the room" {
  given step hold = { id: "7a1c1a7e-0000-4000-8000-000000000001" }
  given step charge = PaymentDeclined { reason: "funds" }
  given undo hold = ok
  key "k-1"
  checkout(body: { roomId: "7a1c1a7e-0000-4000-8000-000000000002" }) -> PaymentDeclined
  expect compensated hold
}
"#;

#[test]
fn the_saga_lines_of_an_example_are_lowered() {
    let ir = build(&format!("{}{EXAMPLE_TAIL}", with(CHECKOUT)));
    let ex = ir.examples.iter().find(|e| e.endpoint == "checkout");
    let saga = ex.and_then(|e| e.saga.as_ref());
    let Some(saga) = saga else {
        panic!("no saga lines in {ex:?}")
    };
    let steps: Vec<&str> = saga.steps.iter().map(|s| s.step.as_str()).collect();
    assert_eq!(steps, ["hold", "charge"]);
    assert_eq!(saga.undos.len(), 1);
    assert_eq!(
        saga.expect,
        Some(veld_ir::SagaExpect::Compensated(vec!["hold".to_string()]))
    );
    assert_eq!(
        ex.and_then(|e| e.key.as_ref()).map(|k| k.value.as_str()),
        Some("k-1")
    );
}

#[test]
fn the_saga_lines_are_checked() {
    for (what, bad) in [
        (
            "an unknown step",
            EXAMPLE_TAIL.replace("given step charge", "given step nope"),
        ),
        (
            "an undo of a step without compensate",
            EXAMPLE_TAIL.replace("given undo hold", "given undo confirm"),
        ),
        (
            "a step twice",
            EXAMPLE_TAIL.replace(
                "given undo hold = ok",
                "given step hold = { id: \"7a1c1a7e-0000-4000-8000-000000000001\" }",
            ),
        ),
        (
            "an error the callee cannot answer",
            EXAMPLE_TAIL.replace(
                "PaymentDeclined { reason: \"funds\" }",
                "RoomFull { capacity: 1 }",
            ),
        ),
        (
            "expect on a step without compensate",
            EXAMPLE_TAIL.replace("expect compensated hold", "expect compensated confirm"),
        ),
    ] {
        let found = codes(&format!("{}{bad}", with(CHECKOUT)));
        assert!(
            found.contains(&"E0951") || found.contains(&"E0952"),
            "{what}: {found:?}"
        );
    }
}

#[test]
fn saga_lines_on_an_endpoint_that_is_not_a_saga_are_e0952() {
    let src = format!(
        "{BASE}example \"e\" {{\n  given step a = {{ id: \"7a1c1a7e-0000-4000-8000-000000000001\" }}\n  hold(body: {{ roomId: \"7a1c1a7e-0000-4000-8000-000000000002\" }}) -> ok\n}}\n"
    );
    assert_eq!(codes(&src), ["E0952"]);
}

#[test]
fn a_saga_example_without_saga_lines_is_fine() {
    let src = format!(
        "{}example \"all steps succeed\" {{\n  key \"k-1\"\n  checkout(body: {{ roomId: \"7a1c1a7e-0000-4000-8000-000000000002\" }}) -> ok\n}}\n",
        with(CHECKOUT)
    );
    let ir = build(&src);
    assert!(ir.examples.iter().all(|e| e.saga.is_none()));
}
