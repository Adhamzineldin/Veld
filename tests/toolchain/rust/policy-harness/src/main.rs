//! Policies and the principal over HTTP (`spec/rules.md` section 11): the production handler
//! generated from `crates/emitters/java/tests/fixtures/guarded.veld`, with a typed authentication
//! hook and application-written fact loaders, and `can_<endpoint>` of the generated client
//! checked against the server.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bookings_client::bookings::bookings::{Booking as ClientBooking, Payment as ClientPayment, User as ClientUser};
use bookings_client::bookings::bookings_client as can;
use bookings_server::bookings::bookings::{Booking, Status, User};
use bookings_server::bookings::bookings_server::{
    BookingsFactLoaders, BookingsHandler, BookingsHooks, BookingsService, CancelBookingFacts, CancelBookingRequest,
    DividesBookingFacts, DividesBookingRequest, GetBookingFacts, GetBookingRequest, PayBookingFacts,
    PayBookingRequest, PositiveBookingFacts, PositiveBookingRequest, RefundBookingFacts, RefundBookingRequest,
};
use bookings_server::veld::Error;
use bookings_server::veld::rules::{Outcome, Position, RuleEvent, Transition};
use bookings_server::veld::server::RequestContext;
use serde_json::{Value, json};

const ID: &str = "00000000-0000-4000-8000-000000000001";
const GUEST: &str = "00000000-0000-4000-8000-0000000000aa";
const OTHER: &str = "00000000-0000-4000-8000-0000000000bb";

/// The callers: a token and the user it stands for.
fn users() -> Vec<(&'static str, Value)> {
    vec![
        ("guest", json!({ "id": GUEST, "roles": [], "suspended": false })),
        ("member", json!({ "id": GUEST, "roles": ["member"], "suspended": false })),
        ("other", json!({ "id": OTHER, "roles": [], "suspended": false })),
        ("support", json!({ "id": OTHER, "roles": ["support"], "suspended": false })),
        ("suspended", json!({ "id": GUEST, "roles": ["support", "member"], "suspended": true })),
        ("three", json!({ "id": OTHER, "roles": ["a", "b", "c"], "suspended": false })),
        ("one", json!({ "id": OTHER, "roles": ["a"], "suspended": false })),
    ]
}

#[derive(Default)]
struct Db {
    bookings: Mutex<BTreeMap<String, Booking>>,
    /// What the handlers saw: `<endpoint>:<caller id>`, in order.
    calls: Mutex<Vec<String>>,
}

struct Loaders(Arc<Db>);

impl BookingsFactLoaders for Loaders {
    async fn booking(&self, key: String, _ctx: &RequestContext) -> Result<Option<Booking>, Error> {
        Ok(self.0.bookings.lock().unwrap().get(&key).cloned())
    }
}

struct Impl(Arc<Db>);

impl Impl {
    fn answer(&self, what: &str, ctx: &RequestContext, facts_booking: Booking) -> Result<Booking, Error> {
        let user = ctx.principal::<User>().expect("the handler sees the principal as the generated model");
        self.0.calls.lock().unwrap().push(format!("{what}:{}:{}", user.id, user.roles.join(",")));
        Ok(facts_booking)
    }
}

impl BookingsService for Impl {
    async fn divides_booking(&self, _r: DividesBookingRequest, f: DividesBookingFacts, ctx: &RequestContext) -> Result<Booking, Error> {
        self.answer("divides", ctx, f.booking)
    }

    async fn refund_booking(&self, _r: RefundBookingRequest, f: RefundBookingFacts, ctx: &RequestContext) -> Result<Booking, Error> {
        self.answer("refund", ctx, f.booking)
    }

    async fn positive_booking(&self, _r: PositiveBookingRequest, f: PositiveBookingFacts, ctx: &RequestContext) -> Result<Booking, Error> {
        self.answer("positive", ctx, f.booking)
    }

    async fn pay_booking(
        &self,
        _r: PayBookingRequest,
        f: PayBookingFacts,
        t: Transition<Status>,
        ctx: &RequestContext,
    ) -> Result<Booking, Error> {
        assert_eq!((t.field.as_str(), t.from, t.to), ("status", Status::Pending, Status::Paid));
        let mut booking = f.booking;
        booking.status = t.to;
        self.answer("pay", ctx, booking)
    }

    async fn get_booking(&self, _r: GetBookingRequest, f: GetBookingFacts, ctx: &RequestContext) -> Result<Booking, Error> {
        self.answer("get", ctx, f.booking)
    }

    async fn cancel_booking(
        &self,
        _r: CancelBookingRequest,
        f: CancelBookingFacts,
        t: Transition<Status>,
        ctx: &RequestContext,
    ) -> Result<Booking, Error> {
        let mut booking = f.booking;
        booking.status = t.to;
        self.answer("cancel", ctx, booking)
    }
}

type Events = Arc<Mutex<Vec<RuleEvent>>>;

struct Hooks {
    events: Events,
    validate: bool,
}

impl BookingsHooks for Hooks {
    async fn auth_bearer<'a>(&'a self, token: &'a str, _ctx: &'a RequestContext) -> Option<User> {
        let (_, wire) = users().into_iter().find(|(t, _)| *t == token)?;
        serde_json::from_value(wire).ok()
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

async fn serve(validate: bool) -> Server {
    let db = Arc::new(Db::default());
    let events: Events = Arc::default();
    let handler = BookingsHandler::create(Impl(db.clone()), Hooks { events: events.clone(), validate }, Loaders(db.clone()));
    let router = bookings_server::adapters::axum::router(handler);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Server { base, db, events }
}

fn put(s: &Server, status: Status) {
    let booking = Booking { id: ID.into(), guest_id: GUEST.into(), status, total: "50.00".into() };
    s.db.bookings.lock().unwrap().insert(ID.into(), booking);
}

struct Answer {
    status: u16,
    doc: Value,
}

async fn call(s: &Server, token: &str, method: &str, path: &str, body: Option<Value>) -> Answer {
    let client = reqwest::Client::new();
    let url = format!("{}/bookings/{ID}{path}", s.base);
    let mut req = if method == "GET" { client.get(url) } else { client.post(url) };
    req = req.header("authorization", format!("Bearer {token}"));
    if let Some(b) = body {
        req = req.header("content-type", "application/json").body(b.to_string());
    }
    let response = req.send().await.unwrap();
    let status = response.status().as_u16();
    let text = response.text().await.unwrap();
    Answer { status, doc: serde_json::from_str(&text).unwrap_or(Value::Null) }
}

fn pay(amount: &str) -> Option<Value> {
    Some(json!({ "amount": amount }))
}

fn assert_denied(a: &Answer, policy: &str, fields: &[&str], message: &str) {
    assert_eq!(a.status, 403, "{}", a.doc);
    assert_eq!(a.doc["title"], "Forbidden");
    assert_eq!(a.doc["data"], json!({ "message": message }));
    let all = a.doc["violations"].as_array().unwrap();
    assert_eq!(all.len(), 1, "{}", a.doc);
    assert!(all[0]["rule"].as_str().unwrap().starts_with(&format!("veld:bookings/{policy}@")), "{}", all[0]);
    assert_eq!(all[0]["path"], "request");
    assert_eq!(all[0]["fields"], json!(fields));
    assert_eq!(all[0]["message"], message);
    assert!(all[0].get("error").is_none());
}

// ---- scenarios -------------------------------------------------------------------------------

/// `permit` and `forbid`, the messages, and the principal the handler sees.
async fn permit_and_forbid() {
    let s = serve(true).await;
    put(&s, Status::Pending);
    let a = call(&s, "guest", "POST", "/pay", pay("50.00")).await;
    assert_eq!(a.status, 201, "{}", a.doc);
    assert_eq!(*s.db.calls.lock().unwrap(), [format!("pay:{GUEST}:")], "the handler sees the principal");

    put(&s, Status::Pending);
    assert_denied(&call(&s, "other", "POST", "/pay", pay("50.00")).await, "ManageBooking", &[], "not permitted");
    let heard = s.events.lock().unwrap().clone();
    assert_eq!(heard.len(), 1);
    assert_eq!((heard[0].position, heard[0].outcome), (Position::Input, Outcome::Rejected));
    assert_eq!(call(&s, "support", "POST", "/pay", pay("50.00")).await.status, 201);
    put(&s, Status::Pending);
    assert_denied(
        &call(&s, "suspended", "POST", "/pay", pay("50.00")).await,
        "ManageBooking",
        &[],
        "A suspended account cannot manage bookings",
    );
}

/// Several `allow` lines, and an argument that reads a request parameter.
async fn several_allows_and_parameters() {
    let s = serve(true).await;
    put(&s, Status::Pending);
    // The guest has no role: the second `allow` denies.
    assert_denied(&call(&s, "guest", "GET", "", None).await, "AnyRole", &[], "not permitted");
    assert_eq!(call(&s, "member", "GET", "", None).await.status, 200);
    // The first allow that denies ends the request.
    assert_denied(
        &call(&s, "suspended", "GET", "", None).await,
        "ManageBooking",
        &[],
        "A suspended account cannot manage bookings",
    );
    // The argument `body` is a parameter: the violation names it.
    assert_eq!(call(&s, "guest", "POST", "/refund", pay("50.00")).await.status, 201);
    assert_denied(&call(&s, "guest", "POST", "/refund", pay("500.00")).await, "SmallPayment", &["body"], "not permitted");
}

/// The order: a missing fact, then the policies, then the guard, then `require`.
async fn order() {
    let s = serve(true).await;
    let a = call(&s, "other", "POST", "/pay", pay("1.00")).await;
    assert_eq!(a.status, 404, "a missing fact wins over a denial: {}", a.doc);
    put(&s, Status::Paid);
    assert_denied(&call(&s, "other", "POST", "/pay", pay("1.00")).await, "ManageBooking", &[], "not permitted");
    let a = call(&s, "guest", "POST", "/pay", pay("1.00")).await;
    assert_eq!(a.status, 409, "a permitted caller meets the guard: {}", a.doc);
    assert_eq!(a.doc["title"], "InvalidTransition");
    put(&s, Status::Pending);
    let a = call(&s, "guest", "POST", "/pay", pay("1.00")).await;
    assert_eq!(a.status, 422, "{}", a.doc);
    assert_eq!(a.doc["title"], "ValidationError");
}

/// A clause or an argument that cannot be evaluated fails closed.
async fn fail_closed() {
    let s = serve(true).await;
    put(&s, Status::Pending);
    // `100 / len(roles)`: no role is a division by zero. The permit does not permit.
    assert_denied(&call(&s, "guest", "GET", "/divides", None).await, "Divides", &[], "not permitted");
    // One role: the `forbid` is true.
    assert_denied(&call(&s, "one", "GET", "/divides", None).await, "Divides", &[], "not permitted");
    assert_eq!(call(&s, "three", "GET", "/divides", None).await.status, 200);
    // An argument that cannot be evaluated denies with `not permitted`.
    assert_denied(&call(&s, "guest", "GET", "/positive", None).await, "Positive", &[], "not permitted");
    assert_eq!(call(&s, "one", "GET", "/positive", None).await.status, 200);
}

/// With request validation off the policies and the guard still run.
async fn validation_off() {
    let s = serve(false).await;
    put(&s, Status::Pending);
    assert_denied(&call(&s, "other", "POST", "/pay", pay("50.00")).await, "ManageBooking", &[], "not permitted");
    put(&s, Status::Paid);
    let a = call(&s, "guest", "POST", "/pay", pay("50.00")).await;
    assert_eq!(a.status, 409, "{}", a.doc);
    put(&s, Status::Pending);
    assert_eq!(call(&s, "guest", "POST", "/pay", pay("1.00")).await.status, 201, "`require` is off");
}

fn client_user(wire: &Value) -> ClientUser {
    serde_json::from_value(wire.clone()).unwrap()
}

fn client_booking(status: &str) -> ClientBooking {
    serde_json::from_value(json!({ "id": ID, "guestId": GUEST, "status": status, "total": "50.00" })).unwrap()
}

/// `can_<endpoint>` of the client agrees with the server for every caller and both states.
async fn can_agrees_with_the_server() {
    let s = serve(true).await;
    for state in [Status::Pending, Status::Paid] {
        let wire = if state == Status::Pending { "pending" } else { "paid" };
        let b = client_booking(wire);
        for (token, user) in users() {
            let u = client_user(&user);
            put(&s, state);
            let a = call(&s, token, "POST", "/pay", pay("50.00")).await;
            assert_eq!(can::can_pay_booking(&b, &u), a.status != 403 && a.status != 409, "pay {token} {wire}: {}", a.doc);
            put(&s, state);
            let a = call(&s, token, "POST", "/cancel", Some(json!({}))).await;
            assert_eq!(can::can_cancel_booking(&b), a.status != 409, "cancel {wire}");
            let a = call(&s, token, "GET", "", None).await;
            assert_eq!(can::can_get_booking(&b, &u), a.status != 403, "get {token}: {}", a.doc);
            let a = call(&s, token, "GET", "/divides", None).await;
            assert_eq!(can::can_divides_booking(&u), a.status != 403, "divides {token}: {}", a.doc);
            let a = call(&s, token, "GET", "/positive", None).await;
            assert_eq!(can::can_positive_booking(&u), a.status != 403, "positive {token}: {}", a.doc);
            for amount in ["50.00", "500.00"] {
                let body: ClientPayment = serde_json::from_value(json!({ "amount": amount })).unwrap();
                let a = call(&s, token, "POST", "/refund", pay(amount)).await;
                assert_eq!(can::can_refund_booking(&u, &body), a.status != 403, "refund {token} {amount}: {}", a.doc);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    permit_and_forbid().await;
    several_allows_and_parameters().await;
    order().await;
    fail_closed().await;
    validation_off().await;
    can_agrees_with_the_server().await;
    println!("policies ok");
}
