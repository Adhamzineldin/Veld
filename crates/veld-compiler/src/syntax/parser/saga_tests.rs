//! Parser tests for sagas (`spec/platform.md` sections 17 and 18): the block of a `saga post`
//! endpoint, its lines, and the saga lines of an example. The words are contextual.

use super::{Parsed, parse};
use crate::diagnostics::FileId;
use crate::syntax::ast::{
    DeclKind, Endpoint, ExampleBody, SagaAnswerLine, SagaExpectKind, SagaValueDecl,
};

fn clean(src: &str) -> Parsed {
    let r = parse(src, FileId(0));
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    r
}

fn codes(src: &str) -> Vec<&'static str> {
    parse(src, FileId(0))
        .diagnostics
        .iter()
        .map(|d| d.code)
        .collect()
}

fn endpoints(r: &Parsed) -> &[Endpoint] {
    let Some(DeclKind::Service(s)) = r.file.decls.first().map(|d| &d.kind) else {
        panic!("no service")
    };
    &s.endpoints
}

const CHECKOUT: &str = r#"
service Checkout at "/checkout" {
  saga post "/" checkout(body: CheckoutRequest) -> Booking | RoomFull | PaymentDeclined {
    step hold    = Rooms.hold(body: { roomId: body.roomId })       compensate Rooms.release(id: hold.id)
    step charge  = Payments.charge(body: body.payment)             compensate Payments.refund(id: charge.id)
    step confirm = Bookings.confirm(id: hold.bookingId)
    timeout minutes(5)
  }
}
"#;

#[test]
fn the_example_of_the_spec_parses() {
    let r = clean(CHECKOUT);
    let ep = &endpoints(&r)[0];
    assert_eq!(ep.name.name, "checkout");
    assert!(ep.block.is_none());
    let saga = ep.saga.as_ref().expect("a saga");
    assert_eq!(saga.steps.len(), 3);
    assert_eq!(saga.steps[0].name.name, "hold");
    let call = &saga.steps[0].call;
    assert_eq!(
        (call.service.name.as_str(), call.endpoint.name.as_str()),
        ("Rooms", "hold")
    );
    assert!(matches!(
        call.args[0].value,
        SagaValueDecl::Object { ref fields, .. } if fields[0].name.name == "roomId"
    ));
    let undo = saga.steps[0].compensate.as_ref().expect("compensate");
    assert_eq!(undo.endpoint.name, "release");
    assert!(saga.steps[2].compensate.is_none());
    assert!(saga.timeout.is_some());
}

#[test]
fn an_endpoint_starts_at_the_saga_word() {
    let r = clean(CHECKOUT);
    let ep = &endpoints(&r)[0];
    let start = ep.span.start as usize;
    assert!(CHECKOUT[start..].starts_with("saga post"));
}

#[test]
fn objects_nest_and_arguments_may_be_empty_or_end_with_a_comma() {
    let r = clean(
        "service S at \"/\" {\n saga post \"/\" a() -> int {\n  step x = T.f()\n  step y = T.g(a: { b: { c: 1 } }, d: x,)\n  timeout seconds(1)\n }\n}",
    );
    let saga = endpoints(&r)[0].saga.as_ref().unwrap();
    assert!(saga.steps[0].call.args.is_empty());
    assert_eq!(saga.steps[1].call.args.len(), 2);
}

#[test]
fn a_statement_of_an_endpoint_block_is_e1203() {
    for word in [
        "fact f: int",
        "require true",
        "ensure true",
        "allow P(x)",
        "transition a.b: c -> d",
    ] {
        let src = format!(
            "service S at \"/\" {{\n saga post \"/\" a() -> int {{\n  {word}\n  step x = T.f()\n  timeout seconds(1)\n }}\n}}"
        );
        let r = parse(&src, FileId(0));
        assert!(
            r.diagnostics.iter().any(|d| d.code == "E1203"),
            "{word}: {:?}",
            r.diagnostics
        );
        // The step after it still parses.
        let saga = endpoints(&r)[0].saga.as_ref().unwrap();
        assert_eq!(saga.steps.len(), 1, "{word}");
    }
}

#[test]
fn only_post_may_be_a_saga() {
    assert_eq!(
        codes(
            "service S at \"/\" {\n saga get \"/\" a() -> int {\n  step x = T.f()\n  timeout seconds(1)\n }\n}"
        ),
        ["E1203"]
    );
}

#[test]
fn a_saga_without_a_block_is_e1203() {
    assert_eq!(
        codes("service S at \"/\" {\n saga post \"/\" a() -> int\n}"),
        ["E1203"]
    );
}

#[test]
fn a_step_after_the_timeout_and_two_timeouts_are_e1203() {
    let after = "service S at \"/\" {\n saga post \"/\" a() -> int {\n  timeout seconds(1)\n  step x = T.f()\n }\n}";
    assert!(codes(after).contains(&"E1203"));
    let twice = "service S at \"/\" {\n saga post \"/\" a() -> int {\n  step x = T.f()\n  timeout seconds(1)\n  timeout seconds(2)\n }\n}";
    assert!(codes(twice).contains(&"E1203"));
}

#[test]
fn a_broken_step_is_reported_and_the_next_still_parses() {
    let r = parse(
        "service S at \"/\" {\n saga post \"/\" a() -> int {\n  step x = T.f(\n  step y = T.g()\n  timeout seconds(1)\n }\n  get \"/b\" b() -> int\n}",
        FileId(0),
    );
    assert!(r.diagnostics.iter().any(|d| d.code == "E0003"));
    assert_eq!(endpoints(&r).len(), 2);
}

#[test]
fn the_new_words_stay_ordinary_names() {
    let src = r#"
model saga { step: int, timeout: int, compensate: int, undo: int, stuck: bool }
enum E { saga, step, timeout, compensate, unavailable, compensated }
service step at "/s" {
  get "/a" saga(@query timeout: int) -> saga
  get "/b" timeout() -> int
  get "/c" compensate(@query step: int) -> int { require step > 0 }
  post "/d" unavailable() -> 204
}
example "e" {
  given step = absent
  given undo = absent
  saga(timeout: 1) -> ok
}
"#;
    let r = clean(src);
    assert_eq!(r.file.decls.len(), 3);
    assert!(endpoints(&r).iter().all(|e| e.saga.is_none()));
    assert!(r.file.examples[0].saga.is_none());
}

#[test]
fn saga_as_the_last_word_of_a_service_is_not_a_saga() {
    let r = parse(
        "service S at \"/\" {\n get \"/a\" a() -> int\n saga\n}",
        FileId(0),
    );
    assert!(!r.diagnostics.is_empty());
}

const EXAMPLE: &str = r#"
example "a declined payment releases the room" {
  given step hold = { id: "h-1", bookingId: "0b7c" }
  given step charge = PaymentDeclined { reason: "funds" }
  given step confirm = unavailable
  given undo hold = unavailable
  given undo charge = ok
  given undo confirm = Gone
  key "k-1"
  checkout(body: { roomId: "r" }) -> PaymentDeclined
  expect compensated hold, charge
}
"#;

#[test]
fn the_saga_lines_of_an_example_parse() {
    let r = clean(EXAMPLE);
    let ex = &r.file.examples[0];
    assert!(matches!(ex.body, ExampleBody::Call(_)));
    assert!(ex.givens.is_empty());
    let saga = ex.saga.as_ref().expect("saga lines");
    assert_eq!(saga.givens.len(), 6);
    assert!(!saga.givens[0].undo);
    assert!(matches!(saga.givens[0].answer, SagaAnswerLine::Value(_)));
    assert!(matches!(
        &saga.givens[1].answer,
        SagaAnswerLine::Outcome(o) if o.error.is_some() && o.value.is_some()
    ));
    assert!(matches!(
        saga.givens[2].answer,
        SagaAnswerLine::Unavailable(_)
    ));
    assert!(saga.givens[3].undo);
    assert!(matches!(
        &saga.givens[4].answer,
        SagaAnswerLine::Outcome(o) if o.error.is_none()
    ));
    assert!(matches!(
        &saga.givens[5].answer,
        SagaAnswerLine::Outcome(o) if o.error.is_some() && o.value.is_none()
    ));
    let expect = saga.expect.as_ref().expect("expect");
    let SagaExpectKind::Compensated(steps) = &expect.kind else {
        panic!("not compensated")
    };
    let names: Vec<&str> = steps.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["hold", "charge"]);
    // The `key` line still reaches the call.
    let ExampleBody::Call(c) = &ex.body else {
        panic!()
    };
    assert_eq!(c.key.as_ref().map(|k| k.key.value.as_str()), Some("k-1"));
}

#[test]
fn expect_stuck_and_a_call_that_answers_ok() {
    let r = clean("example \"e\" {\n given step a = unavailable\n s(x: 1) -> ok\n expect stuck\n}");
    let saga = r.file.examples[0].saga.as_ref().unwrap();
    assert!(matches!(
        saga.expect.as_ref().unwrap().kind,
        SagaExpectKind::Stuck
    ));
}

#[test]
fn an_example_without_saga_lines_has_none() {
    let r = clean("example \"e\" {\n s(x: 1) -> ok\n}");
    assert!(r.file.examples[0].saga.is_none());
}

#[test]
fn a_value_after_a_step_answer_on_the_next_line_is_not_swallowed() {
    // `key` is a word on the line after an error without a payload.
    let r = clean("example \"e\" {\n given step a = Gone\n key \"k\"\n s() -> ok\n}");
    let saga = r.file.examples[0].saga.as_ref().unwrap();
    assert!(matches!(
        &saga.givens[0].answer,
        SagaAnswerLine::Outcome(o) if o.value.is_none()
    ));
}
