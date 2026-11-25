//! State machines over HTTP (`spec/rules.md` section 10): the production handler generated from
//! `tests/rules/endpoints/transitions.veld.pending`, with application-written fact loaders, the
//! helpers of the machine and `can_<endpoint>` of the generated client.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bookings_client::bookings::bookings::{Booking as ClientBooking, Status as ClientStatus};
use bookings_client::bookings::bookings_client::{can_cancel_booking, can_check_in, can_pay_booking};
use bookings_server::bookings::bookings::{Booking, Status};
use bookings_server::bookings::bookings_server::{
    BookingsFactLoaders, BookingsHandler, BookingsHooks, BookingsService, CancelBookingFacts,
    CancelBookingRequest, CheckInFacts, CheckInRequest, PayBookingFacts, PayBookingRequest,
};
use bookings_server::veld::Error;
use bookings_server::veld::rules::{Outcome, Position, RuleEvent, RuleOutput, Transition};
use bookings_server::veld::server::RequestContext;
use serde_json::{Value, json};

const ID: &str = "00000000-0000-4000-8000-000000000001";

#[derive(Default)]
struct Db {
    bookings: Mutex<BTreeMap<String, Booking>>,
    /// What the handlers received: `pay:<from>-><to>:<field>`, in order.
    calls: Mutex<Vec<String>>,
}

struct Loaders(Arc<Db>);

impl BookingsFactLoaders for Loaders {
    async fn booking(&self, key: String, _ctx: &RequestContext) -> Result<Option<Booking>, Error> {
        Ok(self.0.bookings.lock().unwrap().get(&key).cloned())
    }
}

/// What the handlers do wrong, when told to: they answer the booking unmoved.
struct Impl {
    db: Arc<Db>,
    leave_unmoved: bool,
}

fn note(db: &Db, what: &str, t: &Transition<Status>) {
    db.calls.lock().unwrap().push(format!("{what}:{}->{}:{}", t.from.wire(), t.to.wire(), t.field));
}

impl BookingsService for Impl {
    async fn pay_booking(
        &self,
        _request: PayBookingRequest,
        facts: PayBookingFacts,
        transition: Transition<Status>,
        _ctx: &RequestContext,
    ) -> Result<Booking, Error> {
        note(&self.db, "pay", &transition);
        let mut booking = facts.booking;
        if !self.leave_unmoved {
            booking.status = transition.to;
        }
        Ok(booking)
    }

    async fn cancel_booking(
        &self,
        _request: CancelBookingRequest,
        facts: CancelBookingFacts,
        transition: Transition<Status>,
        _ctx: &RequestContext,
    ) -> Result<Booking, Error> {
        note(&self.db, "cancel", &transition);
        let mut booking = facts.booking;
        booking.status = transition.to;
        Ok(booking)
    }

    async fn check_in(
        &self,
        _request: CheckInRequest,
        _facts: CheckInFacts,
        transition: Transition<Status>,
        _ctx: &RequestContext,
    ) -> Result<(), Error> {
        note(&self.db, "checkIn", &transition);
        Ok(())
    }
}

type Events = Arc<Mutex<Vec<RuleEvent>>>;

struct Hooks {
    mode: RuleOutput,
    events: Events,
    validate: bool,
}

impl BookingsHooks for Hooks {
    fn rules_output(&self) -> RuleOutput {
        self.mode
    }

    fn validate_requests(&self) -> bool {
        self.validate
    }

    fn on_rule_violation(&self, event: &RuleEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

struct Server {
    base: String,
    db: Arc<Db>,
    events: Events,
}

async fn serve(mode: RuleOutput, leave_unmoved: bool, validate: bool) -> Server {
    let db = Arc::new(Db::default());
    let events: Events = Arc::default();
    let handler = BookingsHandler::create(
        Impl { db: db.clone(), leave_unmoved },
        Hooks { mode, events: events.clone(), validate },
        Loaders(db.clone()),
    );
    let router = bookings_server::adapters::axum::router(handler);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Server { base, db, events }
}

fn put(s: &Server, status: Status) {
    let booking = Booking { id: ID.into(), status, total: "50.00".into() };
    s.db.bookings.lock().unwrap().insert(ID.into(), booking);
}

struct Answer {
    status: u16,
    doc: Value,
}

async fn post(s: &Server, path: &str, body: &Value) -> Answer {
    let response = reqwest::Client::new()
        .post(format!("{}/bookings/{ID}/{path}", s.base))
        .header("content-type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    let text = response.text().await.unwrap();
    Answer { status, doc: serde_json::from_str(&text).unwrap_or(Value::Null) }
}

fn pay_body(amount: &str) -> Value {
    json!({ "amount": amount })
}

fn assert_refused(a: &Answer, from: &str, to: &str) {
    assert_eq!(a.status, 409, "{}", a.doc);
    assert_eq!(a.doc["title"], "InvalidTransition");
    assert_eq!(a.doc["data"], json!({ "field": "status", "from": from, "to": to }));
    let all = a.doc["violations"].as_array().unwrap();
    assert_eq!(all.len(), 1, "{}", a.doc);
    assert!(all[0]["rule"].as_str().unwrap().contains(".transition@"), "{}", all[0]);
    assert_eq!(all[0]["path"], "request");
    assert_eq!(all[0]["fields"], json!([]));
    assert_eq!(all[0]["message"], "violates rule transition");
    assert!(all[0].get("error").is_none());
}

// ---- scenarios -------------------------------------------------------------------------------

/// A legal move reaches the handler with the transition: wire path, typed states.
async fn legal_and_illegal() {
    let s = serve(RuleOutput::Strict, false, true).await;
    put(&s, Status::Pending);
    let a = post(&s, "pay", &pay_body("50.00")).await;
    assert_eq!(a.status, 201, "{}", a.doc);
    assert_eq!(a.doc["status"], "paid");
    assert_eq!(*s.db.calls.lock().unwrap(), ["pay:pending->paid:status"]);

    // The same move from `paid` is refused, and the handler does not run.
    s.db.calls.lock().unwrap().clear();
    put(&s, Status::Paid);
    let a = post(&s, "pay", &pay_body("50.00")).await;
    assert_refused(&a, "paid", "paid");
    assert!(s.db.calls.lock().unwrap().is_empty(), "the handler did not run");
    let heard = s.events.lock().unwrap().clone();
    assert_eq!(heard.len(), 1);
    assert_eq!((heard[0].position, heard[0].outcome), (Position::Input, Outcome::Rejected));
}

/// Several sources, a final state, and an endpoint without a result.
async fn sources_and_finals() {
    let s = serve(RuleOutput::Strict, false, true).await;
    for from in [Status::Pending, Status::Paid] {
        put(&s, from);
        let a = post(&s, "cancel", &json!({})).await;
        assert_eq!(a.status, 201, "{}", a.doc);
        assert_eq!(a.doc["status"], "cancelled");
    }
    put(&s, Status::CheckedIn);
    assert_refused(&post(&s, "cancel", &json!({})).await, "checkedIn", "cancelled");
    put(&s, Status::Refunded);
    assert_refused(&post(&s, "cancel", &json!({})).await, "refunded", "cancelled");

    put(&s, Status::Paid);
    let a = post(&s, "check-in", &json!({})).await;
    assert_eq!(a.status, 204, "{}", a.doc);
    put(&s, Status::Pending);
    assert_refused(&post(&s, "check-in", &json!({})).await, "pending", "checkedIn");
}

/// The order: a missing fact, then the guard, then `require`.
async fn order() {
    let s = serve(RuleOutput::Strict, false, true).await;
    // No booking: `404` before the guard.
    let a = post(&s, "pay", &pay_body("1.00")).await;
    assert_eq!(a.status, 404, "{}", a.doc);
    assert_eq!(a.doc["data"], json!({ "resource": "booking" }));
    // An illegal move with a wrong amount: the guard wins.
    put(&s, Status::Cancelled);
    assert_refused(&post(&s, "pay", &pay_body("1.00")).await, "cancelled", "paid");
    // A legal move with a wrong amount: `require` answers.
    put(&s, Status::Pending);
    let a = post(&s, "pay", &pay_body("1.00")).await;
    assert_eq!(a.status, 422, "{}", a.doc);
    assert_eq!(a.doc["title"], "AmountMismatch");
}

/// The target state of the result is an `ensure` under the output mode.
async fn target_check() {
    let strict = serve(RuleOutput::Strict, true, true).await;
    put(&strict, Status::Pending);
    let a = post(&strict, "pay", &pay_body("50.00")).await;
    assert_eq!(a.status, 500, "{}", a.doc);
    assert_eq!(a.doc["title"], "Internal");
    assert!(a.doc.get("violations").is_none(), "no rule detail on the wire");
    let heard = strict.events.lock().unwrap().clone();
    assert_eq!(heard.len(), 1);
    assert_eq!((heard[0].position, heard[0].outcome), (Position::Output, Outcome::Rejected));
    assert_eq!(
        heard[0].violation.message,
        "the result must hold the state `paid` in `status`, found `pending`"
    );
    assert!(heard[0].violation.rule.contains(".transition@"));

    let audit = serve(RuleOutput::Audit, true, true).await;
    put(&audit, Status::Pending);
    let a = post(&audit, "pay", &pay_body("50.00")).await;
    assert_eq!(a.status, 201, "{}", a.doc);
    assert_eq!(a.doc["status"], "pending", "sent unchanged");
    let heard = audit.events.lock().unwrap().clone();
    assert_eq!(heard.len(), 1);
    assert_eq!((heard[0].position, heard[0].outcome), (Position::Output, Outcome::Audited));

    let off = serve(RuleOutput::Off, true, true).await;
    put(&off, Status::Pending);
    assert_eq!(post(&off, "pay", &pay_body("50.00")).await.status, 201);
    assert!(off.events.lock().unwrap().is_empty());
}

/// With request validation off the guard still runs, and the handler still gets the transition.
async fn validation_off() {
    let s = serve(RuleOutput::Strict, false, false).await;
    put(&s, Status::Paid);
    assert_refused(&post(&s, "pay", &pay_body("50.00")).await, "paid", "paid");
    put(&s, Status::Pending);
    let a = post(&s, "pay", &pay_body("1.00")).await;
    assert_eq!(a.status, 201, "`require` is off: {}", a.doc);
    assert_eq!(*s.db.calls.lock().unwrap(), ["pay:pending->paid:status"]);
}

/// The helpers of the machine, in the order of the edges.
fn helpers() {
    assert_eq!(Status::Pending.next_states(), [Status::Paid, Status::Cancelled]);
    assert_eq!(Status::Paid.next_states(), [Status::CheckedIn, Status::Cancelled, Status::Refunded]);
    assert!(Status::CheckedIn.next_states().is_empty());
    assert!(Status::CheckedIn.is_final() && Status::Cancelled.is_final() && Status::Refunded.is_final());
    assert!(!Status::Pending.is_final() && !Status::Paid.is_final());
    assert_eq!(ClientStatus::Pending.next_states(), [ClientStatus::Paid, ClientStatus::Cancelled]);
    assert!(ClientStatus::Refunded.is_final());
}

/// `can_<endpoint>` of the client agrees with the server for every state.
async fn can_agrees_with_the_server() {
    let s = serve(RuleOutput::Strict, false, true).await;
    let all = [
        (Status::Pending, ClientStatus::Pending),
        (Status::Paid, ClientStatus::Paid),
        (Status::CheckedIn, ClientStatus::CheckedIn),
        (Status::Cancelled, ClientStatus::Cancelled),
        (Status::Refunded, ClientStatus::Refunded),
    ];
    for (server_state, client_state) in all {
        let client = ClientBooking { id: ID.into(), status: client_state, total: "50.00".into() };
        for (path, body, can) in [
            ("pay", pay_body("50.00"), can_pay_booking(&client)),
            ("cancel", json!({}), can_cancel_booking(&client)),
            ("check-in", json!({}), can_check_in(&client)),
        ] {
            put(&s, server_state);
            let answered = post(&s, path, &body).await;
            let allowed = answered.status != 409;
            assert_eq!(can, allowed, "{path} from {server_state:?}: {}", answered.doc);
        }
    }
}

#[tokio::main]
async fn main() {
    helpers();
    legal_and_illegal().await;
    sources_and_finals().await;
    order().await;
    target_check().await;
    validation_off().await;
    can_agrees_with_the_server().await;
    println!("state machines ok");
}
