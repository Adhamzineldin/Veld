//! Policies (section 11): the verdict of one policy, and `allow` in the pipeline. The contract
//! is compiled from source, so the IR is the one the compiler writes.

use std::collections::BTreeMap;

use serde_json::{Value as Json, json};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind, Policy};

use crate::{ErrorAnswer, NOT_PERMITTED, Outcome, Pipeline, Request, RunError, eval_policy};

const SRC: &str = r#"
enum Status { pending, paid, cancelled
  flow pending -> paid
  flow pending -> cancelled
  final paid, cancelled }

model User { id: uuid, roles: string[], suspended: bool }
principal User

model Booking { id: uuid, guestId: uuid, status: Status, total: decimal }
model Payment { amount: decimal, count: int }
model Window { until: datetime }

policy ManageBooking(user: User, booking: Booking) {
  permit when booking.guestId == user.id
  permit when "support" in user.roles
  forbid when user.suspended
    message "A suspended account cannot manage bookings"
}

policy AnyRole(user: User) {
  permit when len(user.roles) > 0
}

policy Fragile(n: int) {
  permit when 10 / n > 1
  forbid when 100 / n > 1000
    message "too small"
}

policy FragileForbidWithoutMessage(n: int) {
  permit when true
  forbid when 100 / n > 1000
}

policy NoPermit(user: User) {
  forbid when user.suspended message "suspended"
}

policy TwoForbids(user: User) {
  permit when true
  forbid when user.suspended
  forbid when len(user.roles) == 0 message "no roles"
}

policy Until(deadline: datetime) {
  permit when now <= deadline
}

@auth(bearer)
service Bookings at "/bookings" {
  post "/{id}/pay" pay(id: uuid, body: Payment) -> Booking | AmountMismatch {
    fact booking: Booking = id
    allow ManageBooking(user, booking)
    transition booking.status: pending -> paid
    require body.amount == booking.total else AmountMismatch { expected: booking.total }
  }

  get "/{id}" get(id: uuid) -> Booking {
    fact booking: Booking = id
    allow ManageBooking(user, booking)
    allow AnyRole(user)
  }

  post "/frail" frail(body: Payment) -> Booking {
    allow Fragile(body.count)
  }

  post "/frail2" frail2(body: Payment) -> Booking {
    allow FragileForbidWithoutMessage(body.count)
  }

  post "/arg" arg(body: Payment) -> Booking {
    allow Fragile(100 / body.count)
  }

  post "/nopermit" nopermit(body: Payment) -> Booking {
    allow NoPermit(user)
  }

  post "/two" two(body: Payment) -> Booking {
    allow TwoForbids(user)
  }

  post "/clock" clock(body: Window) -> Booking {
    allow Until(body.until)
  }

  post "/ensured" ensured(body: Payment) -> Booking {
    ensure not user.suspended
  }
}

error AmountMismatch 409 { expected: decimal }
"#;

const GUEST: &str = "00000000-0000-4000-8000-0000000000aa";
const OTHER: &str = "00000000-0000-4000-8000-0000000000bb";
const NOW: i64 = 1_767_225_600_000;

fn ir() -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "policies.veld".into(),
                text: SRC.to_owned(),
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    let text = out.render();
    out.ir.unwrap_or_else(|| panic!("{text}"))
}

fn policy<'a>(ir: &'a Ir, name: &str) -> &'a Policy {
    ir.policies
        .iter()
        .find(|p| p.name == name)
        .unwrap_or_else(|| panic!("no policy {name}"))
}

fn user(id: &str, roles: &[&str], suspended: bool) -> Json {
    json!({ "id": id, "roles": roles, "suspended": suspended })
}

fn booking(status: &str) -> Json {
    json!({ "id": GUEST, "guestId": GUEST, "status": status, "total": "10.00" })
}

fn verdict(ir: &Ir, name: &str, args: &[Json]) -> (bool, Option<String>) {
    let v = eval_policy(ir, policy(ir, name), args, NOW).unwrap();
    (v.permitted, v.message)
}

fn run(ir: &Ir, endpoint: &str, req: &Request) -> Outcome {
    let service = &ir.services[0];
    Pipeline::new(ir, service, endpoint)
        .unwrap()
        .run(req)
        .unwrap()
}

fn denied(o: Outcome) -> ErrorAnswer {
    match o {
        Outcome::Error(a) => a,
        Outcome::Passes => panic!("the request passed"),
    }
}

fn pay_request(principal: Json, status: &str, amount: &str) -> Request {
    let mut req = Request {
        principal: Some(principal),
        now: NOW,
        ..Request::default()
    };
    req.params.insert("id".into(), json!(GUEST));
    req.params
        .insert("body".into(), json!({ "amount": amount, "count": 1 }));
    req.facts.insert("booking".into(), Some(booking(status)));
    req
}

// ---- the verdict of a policy -----------------------------------------------------------------

#[test]
fn a_policy_permits_when_one_permit_is_true() {
    let ir = ir();
    let b = booking("pending");
    let owner = user(GUEST, &[], false);
    assert_eq!(
        verdict(&ir, "ManageBooking", &[owner, b.clone()]),
        (true, None)
    );
    let support = user(OTHER, &["support"], false);
    assert_eq!(
        verdict(&ir, "ManageBooking", &[support, b.clone()]),
        (true, None)
    );
    let stranger = user(OTHER, &[], false);
    assert_eq!(verdict(&ir, "ManageBooking", &[stranger, b]), (false, None));
}

#[test]
fn a_forbid_beats_every_permit_and_brings_its_message() {
    let ir = ir();
    let b = booking("pending");
    let both = user(GUEST, &["support"], true);
    assert_eq!(
        verdict(&ir, "ManageBooking", &[both, b]),
        (
            false,
            Some("A suspended account cannot manage bookings".to_owned())
        )
    );
}

#[test]
fn the_order_of_the_clauses_does_not_matter() {
    let ir = ir();
    let mut p = policy(&ir, "ManageBooking").clone();
    p.clauses.reverse();
    let b = booking("pending");
    for (u, expect) in [
        (user(GUEST, &["support"], true), false),
        (user(GUEST, &[], false), true),
        (user(OTHER, &[], false), false),
    ] {
        let v = eval_policy(&ir, &p, &[u, b.clone()], NOW).unwrap();
        assert_eq!(v.permitted, expect);
    }
}

#[test]
fn a_policy_with_no_permit_permits_nobody() {
    let ir = ir();
    assert_eq!(
        verdict(&ir, "NoPermit", &[user(GUEST, &[], false)]),
        (false, None)
    );
    // A forbid that is true still brings its message.
    assert_eq!(
        verdict(&ir, "NoPermit", &[user(GUEST, &[], true)]),
        (false, Some("suspended".to_owned()))
    );
}

#[test]
fn a_permit_that_cannot_be_evaluated_does_not_permit() {
    let ir = ir();
    // 10 / 0 is a division by zero: the permit fails closed.
    assert_eq!(
        verdict(&ir, "Fragile", &[json!(0)]),
        (false, Some("too small".to_owned()))
    );
    // The same policy permits when the clause can be evaluated and holds.
    assert_eq!(verdict(&ir, "Fragile", &[json!(5)]), (true, None));
}

#[test]
fn a_forbid_that_cannot_be_evaluated_forbids() {
    let ir = ir();
    // The permit of this policy is fine, the forbid divides by zero: it forbids.
    assert_eq!(
        verdict(&ir, "FragileForbidWithoutMessage", &[json!(0)]),
        (false, None)
    );
    assert_eq!(
        verdict(&ir, "FragileForbidWithoutMessage", &[json!(7)]),
        (true, None)
    );
}

#[test]
fn the_message_is_the_one_of_the_first_forbid_that_was_true() {
    let ir = ir();
    // Both forbids are true: the first has no message, so the denial says `not permitted`.
    let both = user(GUEST, &[], true);
    let v = eval_policy(&ir, policy(&ir, "TwoForbids"), &[both], NOW).unwrap();
    assert!(!v.permitted);
    assert_eq!(v.message, None);
    assert_eq!(v.denial_message(), NOT_PERMITTED);
    // Only the second is true.
    let roleless = user(GUEST, &[], false);
    let v = eval_policy(&ir, policy(&ir, "TwoForbids"), &[roleless], NOW).unwrap();
    assert_eq!(v.denial_message(), "no roles");
}

#[test]
fn a_policy_sees_now() {
    let ir = ir();
    // NOW is 2026-01-01T00:00:00Z.
    assert_eq!(
        verdict(&ir, "Until", &[json!("2026-06-01T00:00:00Z")]),
        (true, None)
    );
    assert_eq!(
        verdict(&ir, "Until", &[json!("2025-06-01T00:00:00Z")]),
        (false, None)
    );
}

#[test]
fn wrong_arguments_are_an_input_error_not_a_verdict() {
    let ir = ir();
    let p = policy(&ir, "AnyRole");
    assert!(eval_policy(&ir, p, &[], NOW).is_err());
    assert!(eval_policy(&ir, p, &[json!(1)], NOW).is_err());
}

// ---- allow in the pipeline -------------------------------------------------------------------

#[test]
fn a_permitted_request_passes() {
    let ir = ir();
    let req = pay_request(user(GUEST, &[], false), "pending", "10.00");
    assert_eq!(run(&ir, "pay", &req), Outcome::Passes);
}

#[test]
fn a_denial_is_a_403_with_one_violation() {
    let ir = ir();
    let req = pay_request(user(OTHER, &[], false), "pending", "10.00");
    let a = denied(run(&ir, "pay", &req));
    assert_eq!((a.name.as_str(), a.status), ("Forbidden", 403));
    assert_eq!(a.data, json!({ "message": "not permitted" }));
    assert_eq!(a.violations.len(), 1);
    let v = &a.violations[0];
    assert_eq!(v.rule_name, "ManageBooking");
    assert_eq!(v.rule_id, policy(&ir, "ManageBooking").id);
    assert_eq!(v.path, "request");
    assert!(v.fields.is_empty());
    assert_eq!(v.message, "not permitted");
    assert_eq!(v.error, None);
}

#[test]
fn the_message_of_a_forbid_is_the_message_of_the_error_and_of_the_violation() {
    let ir = ir();
    let req = pay_request(user(GUEST, &["support"], true), "pending", "10.00");
    let a = denied(run(&ir, "pay", &req));
    let text = "A suspended account cannot manage bookings";
    assert_eq!(a.data, json!({ "message": text }));
    assert_eq!(a.violations[0].message, text);
}

#[test]
fn the_fields_of_a_violation_are_the_parameters_the_arguments_read() {
    let ir = ir();
    let mut req = Request {
        now: NOW,
        ..Request::default()
    };
    req.params
        .insert("body".into(), json!({ "amount": "1.00", "count": 0 }));
    let a = denied(run(&ir, "frail", &req));
    assert_eq!(a.violations[0].fields, ["body"]);
    assert_eq!(a.violations[0].rule_name, "Fragile");
}

#[test]
fn a_missing_fact_wins_over_a_denial() {
    let ir = ir();
    let mut req = pay_request(user(OTHER, &[], false), "pending", "10.00");
    req.facts.insert("booking".into(), None);
    let a = denied(run(&ir, "pay", &req));
    assert_eq!((a.name.as_str(), a.status), ("NotFound", 404));
}

#[test]
fn a_denial_wins_over_the_transition_guard_and_over_require() {
    let ir = ir();
    // The booking is already paid (the guard would refuse) and the amount is wrong (require
    // would refuse): the caller is not permitted, so that is the answer.
    let req = pay_request(user(OTHER, &[], false), "paid", "99.00");
    let a = denied(run(&ir, "pay", &req));
    assert_eq!(a.name, "Forbidden");
    // A permitted caller reaches the guard, then require.
    let req = pay_request(user(GUEST, &[], false), "paid", "99.00");
    assert_eq!(denied(run(&ir, "pay", &req)).name, "InvalidTransition");
    let req = pay_request(user(GUEST, &[], false), "pending", "99.00");
    assert_eq!(denied(run(&ir, "pay", &req)).name, "AmountMismatch");
}

#[test]
fn several_allows_all_must_permit_and_the_first_denial_ends_the_request() {
    let ir = ir();
    let mut req = Request {
        principal: Some(user(GUEST, &[], false)),
        now: NOW,
        ..Request::default()
    };
    req.params.insert("id".into(), json!(GUEST));
    req.facts.insert("booking".into(), Some(booking("pending")));
    // The owner passes the first policy but has no role: the second denies.
    let a = denied(run(&ir, "get", &req));
    assert_eq!(a.violations[0].rule_name, "AnyRole");
    // With a role both permit.
    req.principal = Some(user(GUEST, &["member"], false));
    assert_eq!(run(&ir, "get", &req), Outcome::Passes);
    // Suspended: the first policy denies with its message and the second is never asked.
    req.principal = Some(user(GUEST, &[], true));
    let a = denied(run(&ir, "get", &req));
    assert_eq!(a.violations[0].rule_name, "ManageBooking");
    assert_eq!(
        a.data,
        json!({ "message": "A suspended account cannot manage bookings" })
    );
}

#[test]
fn an_argument_that_cannot_be_evaluated_fails_closed() {
    let ir = ir();
    let mut req = Request {
        now: NOW,
        ..Request::default()
    };
    // `100 / body.count` divides by zero before the policy is even asked.
    req.params
        .insert("body".into(), json!({ "amount": "1.00", "count": 0 }));
    let a = denied(run(&ir, "arg", &req));
    assert_eq!((a.name.as_str(), a.status), ("Forbidden", 403));
    assert_eq!(a.data, json!({ "message": "not permitted" }));
}

#[test]
fn a_policy_over_now_uses_the_clock_of_the_request() {
    let ir = ir();
    let mut req = Request {
        now: NOW,
        ..Request::default()
    };
    req.params
        .insert("body".into(), json!({ "until": "2026-06-01T00:00:00Z" }));
    assert_eq!(run(&ir, "clock", &req), Outcome::Passes);
    // A year later the same deadline has passed.
    req.now = 1_798_761_600_000;
    assert_eq!(denied(run(&ir, "clock", &req)).name, "Forbidden");
}

#[test]
fn a_missing_principal_is_an_input_error() {
    let ir = ir();
    let mut req = pay_request(user(GUEST, &[], false), "pending", "10.00");
    req.principal = None;
    let err = Pipeline::new(&ir, &ir.services[0], "pay")
        .unwrap()
        .run(&req)
        .unwrap_err();
    assert!(matches!(err, RunError::Input(_)), "{err:?}");
}

#[test]
fn ensure_may_read_the_principal() {
    let ir = ir();
    let mut req = Request {
        principal: Some(user(GUEST, &[], false)),
        now: NOW,
        result: Some(booking("pending")),
        ..Request::default()
    };
    req.params
        .insert("body".into(), json!({ "amount": "1.00", "count": 1 }));
    assert_eq!(run(&ir, "ensured", &req), Outcome::Passes);
    req.principal = Some(user(GUEST, &[], true));
    let a = denied(run(&ir, "ensured", &req));
    assert_eq!((a.name.as_str(), a.status), ("Internal", 500));
    assert_eq!(a.hook.len(), 1);
}
