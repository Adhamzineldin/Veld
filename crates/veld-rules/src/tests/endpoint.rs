//! The pipeline of section 9.5 against the hand-built bookings service.

use serde_json::{Value as Json, json};
use veld_ir::{Expr, Root};

use super::bookings::{Bookings, CLOCK_MS, ROOM_ID, bookings};
use super::kit::{ge, i, int, sub};
use super::service_kit::{old, rp};
use crate::eval::{Env, eval};
use crate::helpers::num::EvalError;
use crate::value::Value;
use crate::{Before, ErrorAnswer, Outcome, Pipeline, Request, RunError, save_old};

fn run(b: &Bookings, endpoint: &str, req: &Request) -> Outcome {
    let p = Pipeline::new(&b.ir, b.service(), endpoint).unwrap();
    p.run(req).unwrap()
}

fn error(o: Outcome) -> ErrorAnswer {
    match o {
        Outcome::Error(a) => a,
        Outcome::Passes => panic!("expected an error answer, the request passed"),
    }
}

#[test]
fn a_good_request_passes_every_step() {
    let b = bookings();
    let mut req = b.request();
    assert_eq!(run(&b, "createBooking", &req), Outcome::Passes);
    req.result = Some(b.result_json("pending", 2));
    assert_eq!(run(&b, "createBooking", &req), Outcome::Passes);
}

#[test]
fn an_unknown_endpoint_is_an_error_not_a_panic() {
    let b = bookings();
    let err = Pipeline::new(&b.ir, b.service(), "nope").unwrap_err();
    assert!(matches!(err, RunError::UnknownEndpoint { .. }));
}

#[test]
fn invariants_on_the_parameters_come_first_and_win_over_a_missing_fact() {
    let b = bookings();
    let mut req = b.request();
    req.params.insert(
        "body".to_owned(),
        json!({ "roomId": ROOM_ID, "guests": 2,
                "start": "2026-01-03T00:00:00Z", "end": "2026-01-02T00:00:00Z" }),
    );
    req.facts.insert("room".to_owned(), None);
    let a = error(run(&b, "createBooking", &req));
    assert_eq!((a.name.as_str(), a.status), ("ValidationError", 422));
    assert_eq!(a.violations.len(), 1);
    let v = &a.violations[0];
    assert_eq!(v.path, "body");
    assert_eq!(v.fields, ["end", "start"]);
    assert_eq!(v.rule_name, "endAfterStart");
    assert_eq!(
        a.data,
        json!({ "issues": [
            { "path": "body", "code": "rule", "message": "violates rule endAfterStart" } ] })
    );
    assert!(a.hook.is_empty());
}

#[test]
fn a_missing_required_fact_is_not_found_and_wins_over_a_require() {
    let b = bookings();
    let mut req = b.request();
    req.facts.remove("room");
    // this request would also fail `guests <= capacity` if the room existed
    req.params.insert(
        "body".to_owned(),
        json!({ "roomId": ROOM_ID, "guests": 99,
                "start": "2026-01-02T00:00:00Z", "end": "2026-01-03T00:00:00Z" }),
    );
    let a = error(run(&b, "createBooking", &req));
    assert_eq!((a.name.as_str(), a.status), ("NotFound", 404));
    assert_eq!(a.data, json!({ "resource": "room" }));
    assert!(a.violations.is_empty());
    assert_eq!(a.decl, b.id(&b.not_found));
}

#[test]
fn the_first_failing_require_wins() {
    let b = bookings();
    let mut req = b.request();
    // closed room, too many guests, and a start in the past: three failures
    req.facts.insert(
        "room".to_owned(),
        Some(json!({ "id": ROOM_ID, "capacity": 1, "closed": true, "openedAt": super::bookings::CLOCK })),
    );
    req.params.insert(
        "body".to_owned(),
        json!({ "roomId": ROOM_ID, "guests": 3,
                "start": "2025-12-01T00:00:00Z", "end": "2025-12-02T00:00:00Z" }),
    );
    assert_eq!(error(run(&b, "createBooking", &req)).name, "RoomClosed");

    req.facts.insert(
        "room".to_owned(),
        Some(json!({ "id": ROOM_ID, "capacity": 1, "closed": false, "openedAt": super::bookings::CLOCK })),
    );
    assert_eq!(error(run(&b, "createBooking", &req)).name, "RoomFull");

    req.facts.insert("room".to_owned(), Some(b.room_json()));
    req.params.insert(
        "body".to_owned(),
        json!({ "roomId": ROOM_ID, "guests": 1,
                "start": "2025-12-01T00:00:00Z", "end": "2025-12-02T00:00:00Z" }),
    );
    assert_eq!(
        error(run(&b, "createBooking", &req)).name,
        "ValidationError"
    );
}

#[test]
fn an_else_error_carries_its_evaluated_payload_and_one_violation() {
    let b = bookings();
    let mut req = b.request();
    req.facts.insert(
        "room".to_owned(),
        Some(json!({ "id": ROOM_ID, "capacity": 2, "closed": false, "openedAt": super::bookings::CLOCK })),
    );
    req.params.insert(
        "body".to_owned(),
        json!({ "roomId": ROOM_ID, "guests": 3,
                "start": "2026-01-02T00:00:00Z", "end": "2026-01-03T00:00:00Z" }),
    );
    let a = error(run(&b, "createBooking", &req));
    assert_eq!((a.name.as_str(), a.status), ("RoomFull", 409));
    assert_eq!(a.decl, b.id(&b.room_full));
    assert_eq!(a.data, json!({ "capacity": 2 }));
    assert_eq!(a.violations.len(), 1);
    let v = &a.violations[0];
    assert_eq!(v.path, "request");
    assert_eq!(v.fields, ["body"]);
    assert_eq!(v.rule_name, "require");
    assert_eq!(v.message, "precondition failed");
    assert_eq!(v.error, None);
}

#[test]
fn a_require_without_else_is_a_validation_error_with_its_message() {
    let b = bookings();
    let mut req = b.request();
    req.params.insert(
        "body".to_owned(),
        json!({ "roomId": ROOM_ID, "guests": 1,
                "start": "2025-12-31T00:00:00Z", "end": "2026-01-03T00:00:00Z" }),
    );
    let a = error(run(&b, "createBooking", &req));
    assert_eq!(a.name, "ValidationError");
    let v = &a.violations[0];
    assert_eq!(v.rule_name, "notInPast");
    assert_eq!(v.message, "A booking cannot start in the past");
    assert_eq!(v.path, "request");
    assert_eq!(
        a.data,
        json!({ "issues": [
            { "path": "request", "code": "rule", "message": "A booking cannot start in the past" } ] })
    );
}

#[test]
fn an_evaluation_error_in_a_require_has_failed() {
    let b = bookings();
    let a = error(run(&b, "risky", &b.request()));
    assert_eq!(a.name, "ValidationError");
    let v = &a.violations[0];
    assert_eq!(v.error, Some(EvalError::DivisionByZero));
    assert_eq!(v.message, "rule ratio could not be evaluated");
    assert_eq!(v.rule_name, "ratio");
    assert_eq!(a.data["issues"][0]["code"], "rule_error");
}

#[test]
fn an_optional_fact_whose_key_is_absent_is_absent_and_has_is_false() {
    let b = bookings();
    // no couponId: the loader is not called, even if a value was offered
    let mut req = b.request();
    req.facts
        .insert("coupon".to_owned(), Some(json!({ "percent": 99 })));
    assert_eq!(run(&b, "createBooking", &req), Outcome::Passes);

    // couponId present but the loader finds nothing: absent, `has` is false
    let mut req = b.request();
    req.params.insert(
        "body".to_owned(),
        json!({ "roomId": ROOM_ID, "guests": 2, "couponId": ROOM_ID,
                "start": "2026-01-02T00:00:00Z", "end": "2026-01-03T00:00:00Z" }),
    );
    assert_eq!(run(&b, "createBooking", &req), Outcome::Passes);

    // found, and over the limit
    req.facts
        .insert("coupon".to_owned(), Some(json!({ "percent": 80 })));
    let a = error(run(&b, "createBooking", &req));
    assert_eq!(a.violations[0].rule_name, "couponLimit");
    req.facts
        .insert("coupon".to_owned(), Some(json!({ "percent": 20 })));
    assert_eq!(run(&b, "createBooking", &req), Outcome::Passes);
}

#[test]
fn the_output_rules_are_strict_and_hidden_from_the_caller() {
    let b = bookings();
    let mut req = b.request();

    // an invariant of the result (guests > 0), reported to the hook only
    req.result = Some(b.result_json("pending", 0));
    let a = error(run(&b, "createBooking", &req));
    assert_eq!((a.name.as_str(), a.status), ("Internal", 500));
    assert!(a.violations.is_empty());
    assert_eq!(a.data, json!({}));
    assert_eq!(a.hook.len(), 1);
    assert_eq!(a.hook[0].rule_name, "guestsPositive");
    assert_eq!(a.hook[0].path, "result");

    // an ensure
    req.result = Some(b.result_json("paid", 2));
    let a = error(run(&b, "createBooking", &req));
    assert_eq!(a.status, 500);
    let names: Vec<&str> = a.hook.iter().map(|v| v.rule_name.as_str()).collect();
    assert_eq!(names, ["ensure"]);

    // every failing ensure is reported
    req.result = Some(b.result_json("paid", 9));
    let a = error(run(&b, "createBooking", &req));
    let names: Vec<&str> = a.hook.iter().map(|v| v.rule_name.as_str()).collect();
    assert_eq!(names, ["ensure", "guestsKept", "seatsLeft"]);
    assert!(a.hook.iter().all(|v| v.path == "request"));
}

#[test]
fn old_is_saved_before_the_handler_and_cannot_be_changed_by_it() {
    let b = bookings();
    let capacity = || rp(Root::Fact("room".to_owned()), &[("capacity", false)], int());
    let expr: Expr = ge(sub(old(capacity()), i(5)), i(0));
    let mut env = Env::new(&b.ir);
    env.facts
        .insert("room".to_owned(), json!({ "capacity": 5 }));
    let saved = save_old(&env, &[&expr]).unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].1, Ok(Value::Int(5)));

    // the handler changes the loaded object in place
    env.facts
        .insert("room".to_owned(), json!({ "capacity": 1 }));
    env.old = saved;
    assert_eq!(eval(&env, &expr), Ok(Value::Bool(true)));
    // without the saved value the same expression has nothing to read
    env.old.clear();
    assert!(eval(&env, &expr).is_err());
}

#[test]
fn old_that_overflowed_fails_the_ensure_closed() {
    let b = bookings();
    let mut env = Env::new(&b.ir);
    env.params.insert("n".to_owned(), json!(i32::MAX));
    let n = || rp(Root::Param("n".to_owned()), &[], int());
    let expr = old(super::kit::add(n(), super::kit::i(1)));
    let saved = save_old(&env, &[&expr]).unwrap();
    assert_eq!(saved[0].1, Err(EvalError::Overflow));
    env.old = saved;
    let verdict = eval(&env, &expr);
    assert_eq!(verdict, Err(crate::Failure::Eval(EvalError::Overflow)));
}

#[test]
fn before_and_after_can_be_driven_by_a_real_handler() {
    let b = bookings();
    let p = Pipeline::new(&b.ir, b.service(), "createBooking").unwrap();
    let Before::Ready(prepared) = p.before(&b.request()).unwrap() else {
        panic!("the request should pass steps 5 and 6");
    };
    assert_eq!(
        prepared.after(&b.result_json("pending", 2)).unwrap(),
        Outcome::Passes
    );
    assert!(matches!(
        prepared.after(&b.result_json("paid", 2)).unwrap(),
        Outcome::Error(_)
    ));
    let mut bad = b.request();
    bad.facts.remove("room");
    assert!(matches!(p.before(&bad).unwrap(), Before::Rejected(_)));
}

#[test]
fn malformed_input_is_an_error_and_never_a_panic() {
    let b = bookings();
    let p = Pipeline::new(&b.ir, b.service(), "createBooking").unwrap();
    let junk = [
        Json::Null,
        json!(5),
        json!("x"),
        json!([]),
        json!({}),
        json!({ "roomId": 1, "guests": "x", "start": [], "end": {} }),
        json!({ "roomId": ROOM_ID, "guests": 2, "start": "nope", "end": "nope" }),
    ];
    for body in &junk {
        let mut req = b.request();
        req.params.insert("body".to_owned(), body.clone());
        let _ = p.run(&req);
        let mut req = b.request();
        req.result = Some(body.clone());
        let _ = p.run(&req);
        let mut req = b.request();
        req.facts.insert("room".to_owned(), Some(body.clone()));
        let _ = p.run(&req);
    }
    let mut empty = Request::default();
    assert!(p.run(&empty).is_err());
    empty.params.insert("body".to_owned(), Json::Null);
    assert!(p.run(&empty).is_err());
    let mut req = b.request();
    req.now = CLOCK_MS;
    req.result = Some(json!({ "id": 5 }));
    assert!(p.run(&req).is_err());
}
