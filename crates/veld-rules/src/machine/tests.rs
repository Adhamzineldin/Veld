//! The state machine helpers, the guard and the target check, on contracts the compiler lowers.

use std::collections::BTreeMap;

use serde_json::{Value as Json, json};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{DeclKind, Enum, Ir, PackageKind};

use crate::{Before, ErrorAnswer, Outcome, Pipeline, Request, is_final, next_states};

const CONTRACT: &str = r#"
enum Status {
  pending, paid, checkedIn, cancelled, refunded
  flow pending -> paid -> checkedIn
  flow pending -> cancelled
  flow paid -> cancelled
  flow paid -> refunded
  final checkedIn, cancelled, refunded
}

enum Plain { a, b }

model Booking {
  id: uuid
  status: Status = .pending
  total: decimal
}

model Receipt {
  id: uuid
}

model Payment { amount: decimal }

error AmountMismatch 422 { expected: decimal }

service Bookings at "/bookings" {
  post "/{id}/pay" pay(id: uuid, body: Payment) -> Booking | AmountMismatch {
    fact booking: Booking = id
    transition booking.status: pending -> paid
    require body.amount == booking.total else AmountMismatch { expected: booking.total }
  }

  post "/{id}/cancel" cancel(id: uuid) -> Booking {
    fact booking: Booking = id
    transition booking.status: pending, paid -> cancelled
  }

  post "/{id}/receipt" receipt(id: uuid) -> Receipt {
    fact booking: Booking = id
    transition booking.status: pending -> paid
  }

  post "/{id}/log" log(id: uuid) -> Booking {
    fact booking: Booking = id
    transition booking.status: pending -> paid
    ensure result.total >= 0
  }
}
"#;

const ID: &str = "00000000-0000-4000-8000-000000000001";

fn ir() -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "m.veld".into(),
                text: CONTRACT.to_owned(),
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    let rendered = out.render();
    out.ir
        .unwrap_or_else(|| panic!("does not compile:\n{rendered}"))
}

fn machine(ir: &Ir, name: &str) -> Enum {
    ir.decls
        .iter()
        .filter(|d| d.name == name)
        .find_map(|d| match &d.kind {
            DeclKind::Enum(e) => Some(e.clone()),
            DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Union(_) | DeclKind::Const(_) => {
                None
            }
        })
        .unwrap()
}

fn booking(status: &str) -> Json {
    json!({ "id": ID, "status": status, "total": "50.00" })
}

fn request(status: &str, amount: &str) -> Request {
    Request {
        params: BTreeMap::from([
            ("id".to_owned(), json!(ID)),
            ("body".to_owned(), json!({ "amount": amount })),
        ]),
        facts: BTreeMap::from([("booking".to_owned(), Some(booking(status)))]),
        now: 0,
        ..Request::default()
    }
}

fn run(ir: &Ir, endpoint: &str, req: &Request) -> Outcome {
    let service = &ir.services[0];
    Pipeline::new(ir, service, endpoint)
        .unwrap()
        .run(req)
        .unwrap()
}

fn error(outcome: Outcome) -> ErrorAnswer {
    match outcome {
        Outcome::Error(e) => e,
        Outcome::Passes => panic!("expected an error"),
    }
}

// ---- helpers (section 10.3) ------------------------------------------------------------------

#[test]
fn next_states_follow_the_declaration_order_of_the_edges() {
    let ir = ir();
    let m = machine(&ir, "Status");
    assert_eq!(next_states(&m, "pending"), ["paid", "cancelled"]);
    assert_eq!(
        next_states(&m, "paid"),
        ["checkedIn", "cancelled", "refunded"]
    );
}

#[test]
fn a_final_state_has_no_next_states_and_is_final() {
    let ir = ir();
    let m = machine(&ir, "Status");
    for s in ["checkedIn", "cancelled", "refunded"] {
        assert!(is_final(&m, s), "{s}");
        assert!(next_states(&m, s).is_empty(), "{s}");
    }
    assert!(!is_final(&m, "pending"));
    assert!(!is_final(&m, "nonsense"));
    assert!(next_states(&m, "nonsense").is_empty());
}

#[test]
fn an_enum_without_flows_has_nothing() {
    let ir = ir();
    let m = machine(&ir, "Plain");
    assert!(!m.is_state_machine());
    assert!(next_states(&m, "a").is_empty());
    assert!(!is_final(&m, "a"));
}

// ---- the guard ----------------------------------------------------------------------------------

#[test]
fn the_source_state_passes_the_guard() {
    let ir = ir();
    assert_eq!(
        run(&ir, "pay", &request("pending", "50.00")),
        Outcome::Passes
    );
}

#[test]
fn each_of_several_sources_passes_the_guard() {
    let ir = ir();
    for s in ["pending", "paid"] {
        assert_eq!(run(&ir, "cancel", &request(s, "0")), Outcome::Passes, "{s}");
    }
}

#[test]
fn a_state_outside_the_sources_is_an_invalid_transition() {
    let ir = ir();
    let e = error(run(&ir, "cancel", &request("checkedIn", "0")));
    assert_eq!((e.name.as_str(), e.status), ("InvalidTransition", 409));
    assert_eq!(
        e.data,
        json!({ "field": "status", "from": "checkedIn", "to": "cancelled" })
    );
    assert!(e.hook.is_empty());
    assert_eq!(e.violations.len(), 1);
    let v = &e.violations[0];
    assert_eq!((v.path.as_str(), v.fields.len()), ("request", 0));
    assert!(
        v.rule_id.starts_with("veld:t/Bookings.cancel.transition@"),
        "{}",
        v.rule_id
    );
}

#[test]
fn the_guard_runs_before_require() {
    let ir = ir();
    // The amount is wrong, but the state is checked first.
    let e = error(run(&ir, "pay", &request("paid", "1.00")));
    assert_eq!(e.name, "InvalidTransition");
    assert_eq!(e.data["from"], json!("paid"));
    // A legal move with the wrong amount is refused by `require`.
    let e = error(run(&ir, "pay", &request("pending", "1.00")));
    assert_eq!(e.name, "AmountMismatch");
}

#[test]
fn a_missing_fact_is_not_found_before_the_guard() {
    let ir = ir();
    let mut req = request("paid", "1.00");
    req.facts.insert("booking".to_owned(), None);
    let e = error(run(&ir, "pay", &req));
    assert_eq!((e.name.as_str(), e.status), ("NotFound", 404));
}

#[test]
fn the_guard_is_in_the_prepared_state_only_when_it_passes() {
    let ir = ir();
    let p = Pipeline::new(&ir, &ir.services[0], "cancel").unwrap();
    assert!(matches!(
        p.before(&request("pending", "0")).unwrap(),
        Before::Ready(_)
    ));
    assert!(matches!(
        p.before(&request("refunded", "0")).unwrap(),
        Before::Rejected(_)
    ));
}

// ---- the target -----------------------------------------------------------------------------------

fn with_result(status: &str, result: Json) -> Request {
    Request {
        result: Some(result),
        ..request(status, "50.00")
    }
}

#[test]
fn a_result_in_the_target_state_passes() {
    let ir = ir();
    let req = with_result("pending", booking("paid"));
    assert_eq!(run(&ir, "pay", &req), Outcome::Passes);
}

#[test]
fn a_result_in_another_state_is_an_internal_error_for_the_hook() {
    let ir = ir();
    let req = with_result("pending", booking("pending"));
    let e = error(run(&ir, "pay", &req));
    assert_eq!((e.name.as_str(), e.status), ("Internal", 500));
    assert_eq!(e.data, json!({}));
    assert!(e.violations.is_empty());
    assert_eq!(e.hook.len(), 1);
    assert_eq!(e.hook[0].path, "request");
    assert!(e.hook[0].rule_id.contains(".pay.transition@"));
}

#[test]
fn the_target_is_not_checked_for_another_success_type() {
    let ir = ir();
    let req = with_result("pending", json!({ "id": ID }));
    assert_eq!(run(&ir, "receipt", &req), Outcome::Passes);
}

#[test]
fn the_target_comes_after_the_ensure_statements() {
    let ir = ir();
    let req = with_result(
        "pending",
        json!({ "id": ID, "status": "pending", "total": "-1.00" }),
    );
    let e = error(run(&ir, "log", &req));
    assert_eq!(e.hook.len(), 2);
    assert!(
        e.hook[0].rule_id.contains(".log.ensure@"),
        "{}",
        e.hook[0].rule_id
    );
    assert!(
        e.hook[1].rule_id.contains(".log.transition@"),
        "{}",
        e.hook[1].rule_id
    );
}

#[test]
fn a_refused_guard_never_reaches_the_target() {
    let ir = ir();
    let req = with_result("paid", booking("pending"));
    let e = error(run(&ir, "pay", &req));
    assert_eq!(e.name, "InvalidTransition");
}

// ---- examples (section 10.5) ----------------------------------------------------------------------

const WITH_EXAMPLES: &str = r#"
enum Phase { first, second, third
  flow first -> second -> third
  final third
}

model Job { id: uuid, phase: Phase = .second, other: Phase, n: int }

service Jobs at "/jobs" {
  post "/{id}/advance" advance(id: uuid) -> Job {
    fact job: Job = id
    transition job.phase: second -> third
  }
}

example "a left-out state is the default" {
  given job = { n: 1 }
  advance(id: job.id) -> ok
}

example "a state without a default is the first variant" {
  given job = { phase: .third }
  advance(id: job.id) -> InvalidTransition { field: "phase", from: "third", to: "third" }
}

example "an illegal move is an outcome like any other error" {
  given job = { phase: .first }
  advance(id: job.id) -> InvalidTransition { from: "first", to: "third" }
}

example "a wrong outcome fails" {
  given job = { phase: .first }
  advance(id: job.id) -> ok
}
"#;

fn examples_ir() -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "m.veld".into(),
                text: WITH_EXAMPLES.to_owned(),
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    let rendered = out.render();
    out.ir
        .unwrap_or_else(|| panic!("does not compile:\n{rendered}"))
}

#[test]
fn a_left_out_state_field_is_filled_in_like_any_enum() {
    let ir = examples_ir();
    let filled = crate::fill_example(&ir, &ir.examples[0]).unwrap();
    let job = filled.request.facts["job"].clone().unwrap();
    // `phase` has a default, `other` has none: the first variant.
    assert_eq!(job["phase"], json!("second"));
    assert_eq!(job["other"], json!("first"));
}

#[test]
fn invalid_transition_works_as_an_outcome() {
    let ir = examples_ir();
    let passed: Vec<(String, bool)> = ir
        .examples
        .iter()
        .map(|e| (e.name.clone(), crate::run_example(&ir, e).unwrap().passed()))
        .collect();
    for (name, ok) in &passed {
        assert_eq!(*ok, name != "a wrong outcome fails", "{name}");
    }
    assert_eq!(passed.len(), 4);
}
