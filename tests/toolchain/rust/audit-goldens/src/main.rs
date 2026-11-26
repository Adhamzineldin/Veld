//! The goldens of `tests/wire/audit/cases.json` through the production handlers of the generated
//! server: each case builds a request, stubs the hooks, the loaders and the handler the way the
//! case says, and the event the `audit` hook receives must have the canonical bytes of the case
//! (and the answer its status and error). The contract is the goldens' own with
//! `@limit(10, per: minute, by: user)` added to `payBooking`, so that the case of the limiter runs.
//! Usage: `audit-goldens <cases.json>`; prints `audit goldens ok: <n>`.

use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

use bookings_server::bookings::bookings::{Booking, User};
use bookings_server::bookings::bookings_server::{
    BookingsDispatch, BookingsFactLoaders, BookingsHandler, BookingsHooks, BookingsService, CreateBookingRequest,
    GetBookingFacts, GetBookingRequest, ListBookingsRequest, PageBookingsRequest, PayBookingFacts, PayBookingRequest,
    RemoveBookingRequest, SetTotalFacts, SetTotalRequest, TouchRequest,
};
use bookings_server::bookings::public_server::{PublicBookingRequest, PublicDispatch, PublicHandler, PublicHooks, PublicService};
use bookings_server::veld::platform::{AuditEvent, Decision, Limiter};
use bookings_server::veld::rules::Transition;
use bookings_server::veld::rules::instant::parse_datetime;
use bookings_server::veld::server::{Body, Handler, Request, RequestContext};
use bookings_server::veld::{Error, Page, VeldDecl};
use serde_json::{Map, Value};

/// What one case stubs.
#[derive(Default)]
struct World {
    principal: Mutex<Option<Value>>,
    facts: Mutex<Map<String, Value>>,
    returns: Mutex<Option<Value>>,
    limiter: Mutex<Option<Decision>>,
    clock_ms: Mutex<i64>,
    events: Mutex<Vec<String>>,
}

impl World {
    fn returned<T: VeldDecl>(&self) -> Result<T, Error> {
        match self.returns.lock().unwrap().as_ref() {
            Some(wire) => T::from_wire(wire),
            None => Err(Error::other("the case returns nothing, but the handler was reached")),
        }
    }

    fn items(&self) -> Result<Vec<Booking>, Error> {
        let wire = self.returns.lock().unwrap().clone().unwrap_or(Value::Null);
        let items = if wire.is_array() { wire } else { wire["items"].clone() };
        items.as_array().into_iter().flatten().map(Booking::from_wire).collect()
    }
}

struct Impl(Arc<World>);

impl BookingsService for Impl {
    async fn pay_booking(&self, _r: PayBookingRequest, _f: PayBookingFacts, _v: Option<i32>, _t: Transition<bookings_server::bookings::bookings::Status>, _c: &RequestContext) -> Result<Booking, Error> {
        self.0.returned()
    }

    async fn get_booking(&self, _r: GetBookingRequest, _f: GetBookingFacts, _v: Option<i32>, _c: &RequestContext) -> Result<Booking, Error> {
        self.0.returned()
    }

    async fn list_bookings(&self, _r: ListBookingsRequest, _c: &RequestContext) -> Result<Vec<Booking>, Error> {
        self.0.items()
    }

    async fn page_bookings(&self, _r: PageBookingsRequest, _c: &RequestContext) -> Result<Page<Booking>, Error> {
        let wire = self.0.returns.lock().unwrap().clone().unwrap_or(Value::Null);
        let n = |k: &str| wire[k].as_i64().unwrap_or(0);
        Ok(Page { items: self.0.items()?, total: n("total"), page: i32::try_from(n("page")).unwrap_or(0), size: i32::try_from(n("size")).unwrap_or(0) })
    }

    async fn create_booking(&self, _r: CreateBookingRequest, _c: &RequestContext) -> Result<Booking, Error> {
        self.0.returned()
    }

    async fn remove_booking(&self, _r: RemoveBookingRequest, _c: &RequestContext) -> Result<(), Error> {
        Ok(())
    }

    async fn set_total(&self, _r: SetTotalRequest, _f: SetTotalFacts, _v: Option<i32>, _c: &RequestContext) -> Result<Booking, Error> {
        self.0.returned()
    }

    async fn touch(&self, _r: TouchRequest, _c: &RequestContext) -> Result<(), Error> {
        Ok(())
    }
}

impl PublicService for Impl {
    async fn public_booking(&self, _r: PublicBookingRequest, _c: &RequestContext) -> Result<Booking, Error> {
        self.0.returned()
    }
}

impl BookingsFactLoaders for Impl {
    async fn booking(&self, _key: String, _ctx: &RequestContext) -> Result<Option<Booking>, Error> {
        match self.0.facts.lock().unwrap().get("booking") {
            Some(Value::Null) | None => Ok(None),
            Some(wire) => Booking::from_wire(wire).map(Some),
        }
    }
}

/// What `take` answers: the decision of the case, else allowed.
struct Stub(Arc<World>);

impl Limiter for Stub {
    fn take<'a>(&'a self, _key: &'a str, _limit: u32, _window_seconds: u32) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Decision, Error>> + Send + 'a>> {
        let decision = self.0.limiter.lock().unwrap().unwrap_or(Decision { allowed: true, retry_after_seconds: 0 });
        Box::pin(std::future::ready(Ok(decision)))
    }
}

struct Hooks(Arc<World>, Stub);

impl Hooks {
    fn record(&self, event: &AuditEvent) {
        self.0.events.lock().unwrap().push(event.canonical());
    }

    fn at(&self) -> std::time::SystemTime {
        UNIX_EPOCH + Duration::from_millis(u64::try_from(*self.0.clock_ms.lock().unwrap()).unwrap_or(0))
    }
}

impl BookingsHooks for Hooks {
    async fn auth_bearer<'a>(&'a self, _token: &'a str, _ctx: &'a RequestContext) -> Option<User> {
        self.0.principal.lock().unwrap().as_ref().and_then(|p| User::from_wire(p).ok())
    }

    async fn audit<'a>(&'a self, event: &'a AuditEvent) -> Result<(), Error> {
        self.record(event);
        Ok(())
    }

    fn limiter(&self) -> Option<&dyn Limiter> {
        Some(&self.1)
    }

    fn caller(&self, principal: &User, _ctx: &RequestContext) -> Result<String, Error> {
        Ok(principal.id.clone())
    }

    fn clock(&self, _ctx: &RequestContext) -> std::time::SystemTime {
        self.at()
    }
}

impl PublicHooks for Hooks {
    async fn audit<'a>(&'a self, event: &'a AuditEvent) -> Result<(), Error> {
        self.record(event);
        Ok(())
    }

    fn clock(&self, _ctx: &RequestContext) -> std::time::SystemTime {
        self.at()
    }
}

type Bookings = Handler<BookingsDispatch<Impl, Hooks, Impl>>;
type Public = Handler<PublicDispatch<Impl, Hooks>>;

fn text(value: &Value) -> String {
    bookings_server::veld::canonical::canonical(value)
}

fn request(case: &Value) -> Request {
    let r = &case["request"];
    let query: Vec<String> = r["query"].as_object().into_iter().flatten().map(|(k, v)| format!("{k}={}", v.as_str().unwrap_or_default())).collect();
    let mut headers: Vec<(String, String)> = r["headers"].as_object().into_iter().flatten().map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string())).collect();
    headers.push(("authorization".into(), "Bearer t".into()));
    let body = if r.get("body").is_some() { r["body"].to_string().into_bytes() } else { Vec::new() };
    if !body.is_empty() {
        headers.push(("content-type".into(), "application/json".into()));
    }
    Request { method: r["method"].as_str().unwrap().into(), path: r["path"].as_str().unwrap().into(), query: query.join("&"), headers, body }
}

fn main() {
    let path = std::env::args().nth(1).expect("the path of cases.json");
    let file: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("cases.json")).expect("JSON");
    let cases = file["cases"].as_array().expect("cases");
    let mut failures: Vec<String> = Vec::new();
    for case in cases {
        let name = case["name"].as_str().unwrap_or("?");
        let world = Arc::new(World::default());
        *world.principal.lock().unwrap() = Some(case["principal"].clone()).filter(|p| !p.is_null());
        *world.facts.lock().unwrap() = case["facts"].as_object().cloned().unwrap_or_default();
        *world.returns.lock().unwrap() = Some(case["returns"].clone()).filter(|r| !r.is_null());
        *world.limiter.lock().unwrap() = case["limiter"].as_object().map(|l| Decision { allowed: l["allowed"].as_bool().unwrap_or(true), retry_after_seconds: u32::try_from(l["retryAfterSeconds"].as_u64().unwrap_or(0)).unwrap_or(0) });
        *world.clock_ms.lock().unwrap() = parse_datetime(case["clock"].as_str().unwrap()).unwrap();
        let hooks = || Hooks(world.clone(), Stub(world.clone()));
        let response = if case["service"] == "Public" {
            let handler: Public = PublicHandler::create(Impl(world.clone()), hooks(), None).unwrap();
            bookings_server::veld::block_on(handler.handle(request(case)))
        } else {
            let handler: Bookings = BookingsHandler::create(Impl(world.clone()), hooks(), Impl(world.clone()), None).unwrap();
            bookings_server::veld::block_on(handler.handle(request(case)))
        };
        let response = response.unwrap_or_else(|| panic!("{name}: no route"));
        assert_eq!(u64::from(response.status), case["status"].as_u64().unwrap(), "{name}: the status");
        if let (Body::Bytes(bytes), Some(error)) = (&response.body, case["error"].as_str()) {
            let doc: Value = serde_json::from_slice(bytes).unwrap();
            assert_eq!(doc["title"], error, "{name}: the error");
        }
        let events = world.events.lock().unwrap().clone();
        match case["event"].as_str() {
            None => assert!(events.is_empty(), "{name}: an endpoint that is not audited has no event: {events:?}"),
            Some(golden) => {
                assert_eq!(events.len(), 1, "{name}: one event");
                let want: Value = serde_json::from_str(golden).unwrap();
                if events[0] != text(&want) {
                    failures.push(format!("{name}: the bytes of the event\n  got  {}\n  want {}", events[0], text(&want)));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{} cases differ:\n{}", failures.len(), failures.join("\n"));
    println!("audit goldens ok: {}", cases.len());
}
