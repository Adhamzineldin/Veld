//! Endpoint rules over HTTP (`spec/rules.md` section 9): the production handler generated from
//! `tests/rules/endpoints/bookings.veld`, with application-written fact loaders, and the generated
//! client for the rules digest.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bookings_client::bookings::bookings::NewBooking;
use bookings_client::bookings::bookings_client::{BookingsClient, CreateBookingRequest as ClientCreate};
use bookings_client::veld::client::ClientOptions;
use bookings_server::bookings::bookings::{Booking, Coupon, Room, Status};
use bookings_server::bookings::bookings_server::{
    BookingsFactLoaders, BookingsHandler, BookingsHooks, BookingsService, CreateBookingFacts, CreateBookingRequest,
    PayBookingFacts, PayBookingRequest,
};
use bookings_server::veld::Error;
use bookings_server::veld::rules::{Outcome, Position, RuleEvent, RuleOutput};
use bookings_server::veld::server::RequestContext;
use serde_json::{Value, json};

const ROOM: &str = "00000000-0000-4000-8000-000000000001";
const BOOKING: &str = "00000000-0000-4000-8000-000000000002";
const FAR: (&str, &str) = ("2099-01-01T00:00:00Z", "2099-01-02T00:00:00Z");

// ---- the application ------------------------------------------------------------------------

#[derive(Default)]
struct Db {
    rooms: Mutex<BTreeMap<String, Room>>,
    coupons: Mutex<BTreeMap<String, Coupon>>,
    bookings: Mutex<BTreeMap<String, Booking>>,
    /// What the application was asked, in order: `room:<key>`, `coupon:<key>`, `handler:...`.
    calls: Mutex<Vec<String>>,
}

impl Db {
    fn note(&self, what: impl Into<String>) {
        self.calls.lock().unwrap().push(what.into());
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn clear(&self) {
        self.calls.lock().unwrap().clear();
    }
}

struct Loaders(Arc<Db>);

impl BookingsFactLoaders for Loaders {
    async fn room(&self, key: String, _ctx: &RequestContext) -> Result<Option<Room>, Error> {
        self.0.note(format!("room:{key}"));
        Ok(self.0.rooms.lock().unwrap().get(&key).cloned())
    }

    async fn coupon(&self, key: String, _ctx: &RequestContext) -> Result<Option<Coupon>, Error> {
        self.0.note(format!("coupon:{key}"));
        Ok(self.0.coupons.lock().unwrap().get(&key).cloned())
    }

    async fn booking(&self, key: String, _ctx: &RequestContext) -> Result<Option<Booking>, Error> {
        self.0.note(format!("booking:{key}"));
        Ok(self.0.bookings.lock().unwrap().get(&key).cloned())
    }
}

/// What the handlers do wrong, when told to.
#[derive(Clone, Copy, PartialEq)]
enum Fault {
    None,
    /// `createBooking` answers a paid booking.
    Paid,
    /// `createBooking` answers a booking in another room.
    OtherRoom,
    /// `payBooking` changes the loaded booking in place and answers its changed total.
    ChangesLoaded,
}

struct Impl {
    db: Arc<Db>,
    fault: Fault,
}

impl BookingsService for Impl {
    async fn create_booking(
        &self,
        request: CreateBookingRequest,
        facts: CreateBookingFacts,
        _ctx: &RequestContext,
    ) -> Result<Booking, Error> {
        self.db.note("handler:create");
        let body = request.body;
        Ok(Booking {
            room_id: if self.fault == Fault::OtherRoom { BOOKING.into() } else { facts.room.id },
            guests: body.guests,
            start: body.start,
            end: body.end,
            coupon_code: body.coupon_code,
            id: BOOKING.into(),
            status: if self.fault == Fault::Paid { Status::Paid } else { Status::Pending },
            total: "10.00".into(),
        })
    }

    async fn pay_booking(
        &self,
        _request: PayBookingRequest,
        facts: PayBookingFacts,
        _ctx: &RequestContext,
    ) -> Result<Booking, Error> {
        self.db.note("handler:pay");
        let mut booking = facts.booking;
        if self.fault == Fault::ChangesLoaded {
            booking.total = "99.00".into();
        }
        booking.status = Status::Paid;
        Ok(booking)
    }
}

type Events = Arc<Mutex<Vec<RuleEvent>>>;

struct Hooks {
    mode: RuleOutput,
    events: Events,
    clock: Option<SystemTime>,
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

    fn clock(&self, _ctx: &RequestContext) -> SystemTime {
        self.clock.unwrap_or_else(SystemTime::now)
    }
}

struct Server {
    base: String,
    db: Arc<Db>,
    events: Events,
    hits: Arc<AtomicUsize>,
}

#[derive(Clone, Copy)]
struct Setup {
    mode: RuleOutput,
    fault: Fault,
    clock: Option<SystemTime>,
    validate: bool,
}

impl Setup {
    const fn new() -> Self {
        Setup { mode: RuleOutput::Strict, fault: Fault::None, clock: None, validate: true }
    }
}

async fn serve(setup: Setup) -> Server {
    let db = Arc::new(Db::default());
    db.rooms.lock().unwrap().insert(
        ROOM.into(),
        Room { id: ROOM.into(), capacity: 2, closed: false, nightly: "50.00".into() },
    );
    let events: Events = Arc::default();
    let hits = Arc::new(AtomicUsize::new(0));
    let handler = BookingsHandler::create(
        Impl { db: db.clone(), fault: setup.fault },
        Hooks { mode: setup.mode, events: events.clone(), clock: setup.clock, validate: setup.validate },
        Loaders(db.clone()),
    );
    let counter = hits.clone();
    let router = bookings_server::adapters::axum::router(handler).layer(axum::middleware::from_fn(
        move |req: axum::extract::Request, next: axum::middleware::Next| {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                next.run(req).await
            }
        },
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Server { base, db, events, hits }
}

// ---- requests --------------------------------------------------------------------------------

struct Answer {
    status: u16,
    digest: Option<String>,
    doc: Value,
}

async fn post(base: &str, path: &str, body: &Value) -> Answer {
    let response = reqwest::Client::new()
        .post(format!("{base}{path}"))
        .header("content-type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    let digest = response.headers().get("veld-rules").map(|v| v.to_str().unwrap().to_string());
    let text = response.text().await.unwrap();
    Answer { status, digest, doc: serde_json::from_str(&text).unwrap_or(Value::Null) }
}

fn booking_body(room: &str, guests: i32, (start, end): (&str, &str)) -> Value {
    json!({ "roomId": room, "guests": guests, "start": start, "end": end })
}

async fn create(server: &Server, body: &Value) -> Answer {
    post(&server.base, "/bookings", body).await
}

fn rules_of(a: &Answer) -> Vec<&Value> {
    a.doc["violations"].as_array().map(|v| v.iter().collect()).unwrap_or_default()
}

fn events_of(server: &Server) -> Vec<RuleEvent> {
    server.events.lock().unwrap().clone()
}

// ---- scenarios -------------------------------------------------------------------------------

/// A required fact that is not found is `404 NotFound`, and an optional one is absent.
async fn facts() {
    let s = serve(Setup::new()).await;
    let other = "00000000-0000-4000-8000-0000000000ff";
    let a = create(&s, &booking_body(other, 1, FAR)).await;
    assert_eq!(a.status, 404, "{}", a.doc);
    assert_eq!(a.doc["title"], "NotFound");
    assert_eq!(a.doc["data"], json!({ "resource": "room" }));
    assert!(a.doc.get("violations").is_none());
    assert_eq!(s.db.calls(), [format!("room:{other}")], "the handler and the coupon loader are not called");

    // The coupon loader is called only when the booking names a coupon; a coupon it does not find is absent.
    s.db.clear();
    let a = create(&s, &booking_body(ROOM, 2, FAR)).await;
    assert_eq!(a.status, 201, "{}", a.doc);
    assert_eq!(s.db.calls(), [format!("room:{ROOM}"), "handler:create".to_string()]);
    s.db.clear();
    let mut named = booking_body(ROOM, 2, FAR);
    named["couponCode"] = json!("GONE");
    let a = create(&s, &named).await;
    assert_eq!(a.status, 201, "{}", a.doc);
    assert_eq!(s.db.calls(), [format!("room:{ROOM}"), "coupon:GONE".to_string(), "handler:create".to_string()]);
}

fn assert_one_violation(a: &Answer, id_part: &str, fields: &[&str], message: &str) {
    let all = rules_of(a);
    assert_eq!(all.len(), 1, "{}", a.doc);
    let v = all[0];
    assert!(v["rule"].as_str().unwrap().contains(id_part), "{v}");
    assert_eq!(v["path"], "request");
    assert_eq!(v["fields"], json!(fields));
    assert_eq!(v["message"], message);
    assert!(v.get("error").is_none());
}

/// `require` with `else`: the declared error with its payload and one violation; the first failure wins.
async fn requires() {
    let s = serve(Setup::new()).await;
    let a = create(&s, &booking_body(ROOM, 3, FAR)).await;
    assert_eq!(a.status, 409, "{}", a.doc);
    assert_eq!(a.doc["title"], "RoomFull");
    assert_eq!(a.doc["data"], json!({ "capacity": 2 }));
    assert_one_violation(&a, "Bookings.createBooking.require@", &["body"], "precondition failed");
    assert_eq!(s.db.calls(), [format!("room:{ROOM}")], "the handler did not run");
    let heard = events_of(&s);
    assert_eq!(heard.len(), 1);
    assert_eq!((heard[0].position, heard[0].outcome), (Position::Input, Outcome::Rejected));

    // A closed room: the first failing `require` wins, though the second would fail too.
    s.db.rooms.lock().unwrap().insert(
        ROOM.into(),
        Room { id: ROOM.into(), capacity: 1, closed: true, nightly: "50.00".into() },
    );
    let a = create(&s, &booking_body(ROOM, 5, FAR)).await;
    assert_eq!(a.status, 409, "{}", a.doc);
    assert_eq!(a.doc["title"], "RoomClosed");
    assert_eq!(a.doc["data"], json!({}));
    assert_one_violation(&a, "Bookings.createBooking.require@", &[], "precondition failed");
}

/// Without `else`: `422 ValidationError` with one `rule` issue; named rules keep their message.
async fn validation_errors() {
    let past = Setup { clock: Some(UNIX_EPOCH + Duration::from_secs(1_780_272_000)), ..Setup::new() }; // 2026-06-01
    let s = serve(past).await;
    let early = booking_body(ROOM, 2, ("2026-05-01T00:00:00Z", "2026-05-03T00:00:00Z"));
    let a = create(&s, &early).await;
    assert_eq!(a.status, 422, "{}", a.doc);
    assert_eq!(a.doc["title"], "ValidationError");
    assert_eq!(a.doc["data"]["issues"].as_array().unwrap().len(), 1);
    assert_eq!(a.doc["data"]["issues"][0]["code"], "rule");
    assert_eq!(a.doc["data"]["issues"][0]["path"], "request");
    assert_one_violation(&a, "Bookings.createBooking.notInPast@", &["body"], "A booking cannot start in the past");
    // The clock option: exactly now is not in the past.
    let now = booking_body(ROOM, 2, ("2026-06-01T00:00:00Z", "2026-06-02T00:00:00Z"));
    assert_eq!(create(&s, &now).await.status, 201);

    // A coupon above fifty percent.
    s.db.coupons.lock().unwrap().insert("BIG".into(), Coupon { code: "BIG".into(), percent: 60 });
    let mut big = booking_body(ROOM, 2, FAR);
    big["couponCode"] = json!("BIG");
    let a = create(&s, &big).await;
    assert_eq!(a.status, 422, "{}", a.doc);
    assert_one_violation(&a, "Bookings.createBooking.couponWorks@", &[], "violates rule couponWorks");
}

/// The order of the pipeline: step 5 before step 6, the facts before the rules, `now` from the clock option.
async fn order() {
    let s = serve(Setup::new()).await;
    let backwards = booking_body("00000000-0000-4000-8000-0000000000ff", 1, ("2099-02-03T00:00:00Z", "2099-02-01T00:00:00Z"));
    let a = create(&s, &backwards).await;
    assert_eq!(a.status, 422, "{}", a.doc);
    assert_eq!(a.doc["violations"][0]["path"], "body");
    assert!(s.db.calls().is_empty(), "no loader runs for an invalid body");
    // The system clock by default: the past is refused, the far future is not.
    let a = create(&s, &booking_body(ROOM, 1, ("2020-01-01T00:00:00Z", "2020-01-02T00:00:00Z"))).await;
    assert_eq!(a.status, 422, "{}", a.doc);
    assert_eq!(create(&s, &booking_body(ROOM, 1, FAR)).await.status, 201);
    // With request validation off, nothing but the loading of the facts runs.
    let off = serve(Setup { validate: false, ..Setup::new() }).await;
    off.db.rooms.lock().unwrap().insert(
        ROOM.into(),
        Room { id: ROOM.into(), capacity: 1, closed: true, nightly: "50.00".into() },
    );
    let a = create(&off, &booking_body(ROOM, 9, ("2020-01-01T00:00:00Z", "2020-01-02T00:00:00Z"))).await;
    assert_eq!(a.status, 201, "{}", a.doc);
    assert_eq!(off.db.calls(), [format!("room:{ROOM}"), "handler:create".to_string()]);
}

/// `ensure` in `strict`, `audit` and `off`.
async fn ensures() {
    // strict: a result that breaks `startsPending` is `500 Internal` with no detail.
    let strict = serve(Setup { fault: Fault::Paid, ..Setup::new() }).await;
    let a = create(&strict, &booking_body(ROOM, 2, FAR)).await;
    assert_eq!(a.status, 500, "{}", a.doc);
    assert_eq!(a.doc["title"], "Internal");
    assert_eq!(a.doc["data"], json!({}));
    assert!(a.doc.get("violations").is_none());
    let heard = events_of(&strict);
    assert_eq!(heard.len(), 1);
    assert_eq!((heard[0].position, heard[0].outcome), (Position::Output, Outcome::Rejected));
    assert!(heard[0].rule.contains("Bookings.createBooking.startsPending@"), "{}", heard[0].rule);
    assert_eq!(strict.db.calls().last().map(String::as_str), Some("handler:create"));
    // The other statement, in source order: both are reported when both fail.
    let other = serve(Setup { fault: Fault::OtherRoom, ..Setup::new() }).await;
    let a = create(&other, &booking_body(ROOM, 2, FAR)).await;
    assert_eq!(a.status, 500, "{}", a.doc);
    assert!(events_of(&other)[0].rule.contains("keepsRoom@"));

    // audit: sent unchanged, reported.
    let audit = serve(Setup { fault: Fault::Paid, mode: RuleOutput::Audit, ..Setup::new() }).await;
    let a = create(&audit, &booking_body(ROOM, 2, FAR)).await;
    assert_eq!(a.status, 201, "{}", a.doc);
    assert_eq!(a.doc["status"], "paid");
    let heard = events_of(&audit);
    assert_eq!((heard[0].position, heard[0].outcome), (Position::Output, Outcome::Audited));

    // off: not evaluated.
    let off = serve(Setup { fault: Fault::Paid, mode: RuleOutput::Off, ..Setup::new() }).await;
    assert_eq!(create(&off, &booking_body(ROOM, 2, FAR)).await.status, 201);
    assert!(events_of(&off).is_empty());
}

/// `old(...)` is the value from before the handler, and a required fact is `404`.
async fn old_values() {
    let pay = |amount: &str| json!({ "amount": amount });
    let path = format!("/bookings/{BOOKING}/pay");
    let seed = |s: &Server| {
        s.db.bookings.lock().unwrap().insert(
            BOOKING.into(),
            Booking {
                room_id: ROOM.into(),
                guests: 1,
                start: FAR.0.into(),
                end: FAR.1.into(),
                coupon_code: None,
                id: BOOKING.into(),
                status: Status::Pending,
                total: "120.00".into(),
            },
        );
    };
    let honest = serve(Setup::new()).await;
    assert_eq!(post(&honest.base, &path, &pay("120.00")).await.status, 404);
    seed(&honest);
    let wrong = post(&honest.base, &path, &pay("100.00")).await;
    assert_eq!(wrong.status, 422, "{}", wrong.doc);
    assert_eq!(wrong.doc["title"], "AmountMismatch");
    assert_eq!(wrong.doc["data"], json!({ "expected": "120.00" }));
    assert_one_violation(&wrong, "Bookings.payBooking.require@", &["body"], "precondition failed");
    // The handler keeps the total: both `ensure`s hold.
    let paid = post(&honest.base, &path, &pay("120.00")).await;
    assert_eq!(paid.status, 201, "{}", paid.doc);
    // The handler changes the loaded booking in place: the saved copy is what it was before.
    let cheat = serve(Setup { fault: Fault::ChangesLoaded, ..Setup::new() }).await;
    seed(&cheat);
    let a = post(&cheat.base, &path, &pay("120.00")).await;
    assert_eq!(a.status, 500, "{}", a.doc);
    let heard = events_of(&cheat);
    assert_eq!(heard.len(), 1);
    assert!(heard[0].rule.contains("Bookings.payBooking.ensure@"), "{}", heard[0].rule);
}

/// `Veld-Rules` on every response.
async fn digest_header() {
    let s = serve(Setup::new()).await;
    let ok = create(&s, &booking_body(ROOM, 2, FAR)).await;
    let missing = create(&s, &booking_body("00000000-0000-4000-8000-0000000000ff", 1, FAR)).await;
    let refused = create(&s, &booking_body(ROOM, 3, FAR)).await;
    let invalid = create(&s, &booking_body(ROOM, 1, ("2099-02-03T00:00:00Z", "2099-02-01T00:00:00Z"))).await;
    let digest = ok.digest.clone().expect("a digest on a success");
    assert_eq!(digest.len(), 16);
    for a in [&missing, &refused, &invalid] {
        assert_eq!(a.digest.as_deref(), Some(digest.as_str()), "errors carry it too: {}", a.doc);
    }
    let nothing = reqwest::get(format!("{}/nowhere", s.base)).await.unwrap();
    assert!(nothing.headers().get("veld-rules").is_none(), "a request no route matches gets none");
}

// ---- the client and the digest ---------------------------------------------------------------

fn new_booking(start: &str, end: &str) -> NewBooking {
    NewBooking { room_id: ROOM.into(), guests: 1, start: start.into(), end: end.into(), coupon_code: None }
}

/// A server that answers every booking with a chosen `Veld-Rules` header (or none).
async fn fake(digest: Arc<Mutex<Option<String>>>) -> String {
    let router = axum::Router::new().route(
        "/bookings",
        axum::routing::post(move || {
            let digest = digest.clone();
            async move {
                let mut headers = axum::http::HeaderMap::new();
                headers.insert("content-type", "application/json".parse().unwrap());
                if let Some(d) = digest.lock().unwrap().clone() {
                    headers.insert("veld-rules", d.parse().unwrap());
                }
                let body = json!({
                    "roomId": ROOM, "guests": 1, "start": FAR.0, "end": FAR.1,
                    "id": BOOKING, "status": "pending", "total": "10.00"
                });
                (axum::http::StatusCode::CREATED, headers, body.to_string())
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    base
}

async fn client_digest() {
    let backwards = || ClientCreate { body: new_booking("2099-02-03T00:00:00Z", "2099-02-01T00:00:00Z") };
    // The same digest: the first request that fails the local check is sent anyway, the next is not.
    let s = serve(Setup::new()).await;
    let seen: Arc<Mutex<Vec<String>>> = Arc::default();
    let log = seen.clone();
    let opts = ClientOptions::new()
        .base_url(&s.base)
        .on_contract_mismatch(move |m| log.lock().unwrap().push(format!("{} {} {}", m.service, m.client_digest, m.server_digest)));
    let client = BookingsClient::reqwest(opts).unwrap();
    let before = s.hits.load(Ordering::SeqCst);
    let first = client.create_booking(backwards()).await.unwrap_err();
    assert!(!first.is_local(), "before the first response the server decides: {first:?}");
    assert_eq!(first.status(), Some(422));
    assert_eq!(s.hits.load(Ordering::SeqCst), before + 1, "it was sent");
    let second = client.create_booking(backwards()).await.unwrap_err();
    assert!(second.is_local(), "the digest matches: rejected locally");
    assert_eq!(s.hits.load(Ordering::SeqCst), before + 1, "nothing was sent");
    assert!(seen.lock().unwrap().is_empty(), "a matching digest is not a mismatch");
    let own = post(&s.base, "/bookings", &booking_body(ROOM, 2, FAR)).await.digest.unwrap();

    // A different digest stops the local checks and is reported once per digest.
    let header = Arc::new(Mutex::new(Some("ffffffffffffffff".to_string())));
    let base = fake(header.clone()).await;
    let log = seen.clone();
    let opts = ClientOptions::new()
        .base_url(&base)
        .on_contract_mismatch(move |m| log.lock().unwrap().push(format!("{} {} {}", m.service, m.client_digest, m.server_digest)));
    let client = BookingsClient::reqwest(opts).unwrap();
    assert!(client.create_booking(backwards()).await.is_ok(), "the first request is sent");
    assert!(client.create_booking(backwards()).await.is_ok(), "a different digest: no local rejection");
    assert_eq!(*seen.lock().unwrap(), [format!("Bookings {own} ffffffffffffffff")], "reported once");
    *header.lock().unwrap() = Some("eeeeeeeeeeeeeeee".into());
    assert!(client.create_booking(backwards()).await.is_ok());
    assert!(client.create_booking(backwards()).await.is_ok());
    assert_eq!(seen.lock().unwrap().len(), 2, "once for each digest it had not seen");
    // The header missing: the server decides, and nothing is reported.
    *header.lock().unwrap() = None;
    assert!(client.create_booking(backwards()).await.is_ok());
    assert!(client.create_booking(backwards()).await.is_ok());
    assert_eq!(seen.lock().unwrap().len(), 2);
    // Its own digest again: the local check is back.
    *header.lock().unwrap() = Some(own);
    assert!(client.create_booking(backwards()).await.is_ok(), "the response that brings the digest back");
    assert!(client.create_booking(backwards()).await.unwrap_err().is_local());
    // Local checks off: always sent.
    let opts = ClientOptions::new().base_url(&s.base).check_rules(false);
    let remote = BookingsClient::reqwest(opts).unwrap();
    assert!(!remote.create_booking(backwards()).await.unwrap_err().is_local());
}

#[tokio::main]
async fn main() {
    facts().await;
    requires().await;
    validation_errors().await;
    order().await;
    ensures().await;
    old_values().await;
    digest_header().await;
    client_digest().await;
    println!("endpoint rules ok");
}
