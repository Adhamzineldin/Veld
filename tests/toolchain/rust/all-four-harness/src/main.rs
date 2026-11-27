//! The four features of platform phase P1 together over the production handler, generated from
//! `tests/toolchain/contract/platform_p1/all_four.veld`: a field hidden for one caller and present
//! for another and what the audit event says of it (`hidden` wins over `redacted`), a fact of
//! another tenant (`404`, audited), a result of another tenant (`500`), and the eleventh call of a
//! caller (`429`, audited with the caller). The program prints `all four ok`.

use std::sync::{Arc, Mutex};

use bookings_server::bookings::bookings::{Booking, User};
use bookings_server::bookings::bookings_server::{
    BookingsFactLoaders, BookingsHandler, BookingsHooks, BookingsService, GetBookingFacts, GetBookingRequest, ListBookingsRequest,
    LiveRequest, PayFacts, PayRequest,
};
use bookings_server::veld::platform::{AuditEvent, Limiter, MemoryLimiter};
use bookings_server::veld::server::{Body, Request, RequestContext};
use bookings_server::veld::stream::EventStream;
use bookings_server::veld::Error;
use serde_json::{Value, json};

const T1: &str = "00000000-0000-4000-8000-0000000000a1";
const T2: &str = "00000000-0000-4000-8000-0000000000b2";
const ID: &str = "00000000-0000-4000-8000-000000000001";
const GUEST: &str = "00000000-0000-4000-8000-0000000000c1";

/// What a test controls: the fact the loader returns, the tenant of what the handler returns.
struct World {
    fact_tenant: Mutex<String>,
    returns_tenant: Mutex<String>,
    limiter: MemoryLimiter,
    events: Mutex<Vec<AuditEvent>>,
}

fn booking(tenant: &str) -> Booking {
    Booking { id: ID.into(), tenant_id: tenant.into(), guest_id: GUEST.into(), title: "t".into(), phone: Some("+201".into()), card: Some("4111".into()) }
}

struct Impl(Arc<World>);

impl BookingsService for Impl {
    async fn get_booking(&self, _r: GetBookingRequest, _f: GetBookingFacts, _c: &RequestContext) -> Result<Booking, Error> {
        Ok(booking(&self.0.returns_tenant.lock().unwrap()))
    }

    async fn pay(&self, _r: PayRequest, f: PayFacts, _c: &RequestContext) -> Result<Booking, Error> {
        Ok(f.booking)
    }

    async fn list_bookings(&self, _r: ListBookingsRequest, _c: &RequestContext) -> Result<Vec<Booking>, Error> {
        Ok(vec![])
    }

    async fn live(&self, _r: LiveRequest, _c: &RequestContext) -> Result<EventStream<Booking>, Error> {
        Err(Error::other("the harness does not stream"))
    }
}

impl BookingsFactLoaders for Impl {
    async fn booking(&self, _key: String, _ctx: &RequestContext) -> Result<Option<Booking>, Error> {
        Ok(Some(booking(&self.0.fact_tenant.lock().unwrap())))
    }
}

struct Hooks(Arc<World>);

impl BookingsHooks for Hooks {
    async fn auth_bearer<'a>(&'a self, token: &'a str, _ctx: &'a RequestContext) -> Option<User> {
        let (id, roles) = match token {
            "guest" => (GUEST, vec![]),
            "other" => ("00000000-0000-4000-8000-0000000000d2", vec![]),
            "support" => ("00000000-0000-4000-8000-0000000000e3", vec!["support".to_string()]),
            _ => return None,
        };
        Some(User { id: id.into(), roles, email: "a@example.test".into(), tenant_id: Some(T1.into()) })
    }

    async fn audit<'a>(&'a self, event: &'a AuditEvent) -> Result<(), Error> {
        self.0.events.lock().unwrap().push(event.clone());
        Ok(())
    }

    fn limiter(&self) -> Option<&dyn Limiter> {
        Some(&self.0.limiter)
    }

    fn address(&self, _ctx: &RequestContext) -> Result<String, Error> {
        Ok("203.0.113.9".into())
    }

    fn caller(&self, principal: &User, _ctx: &RequestContext) -> Result<String, Error> {
        Ok(principal.id.clone())
    }
}

fn get(handler: &bookings_server::veld::server::Handler<impl bookings_server::veld::server::Endpoints>, token: &str) -> (u16, Value) {
    let request = Request {
        method: "GET".into(),
        path: format!("/bookings/{ID}"),
        query: String::new(),
        headers: vec![("authorization".into(), format!("Bearer {token}"))],
        body: Vec::new(),
    };
    let response = bookings_server::veld::block_on(handler.handle(request)).expect("a route");
    let doc = match response.body {
        Body::Bytes(bytes) => serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        _ => Value::Null,
    };
    (response.status, doc)
}

fn last(world: &World) -> Value {
    world.events.lock().unwrap().last().expect("an event").to_json()
}

fn main() {
    // The clock of the limiter stands still: every call is in the first window.
    let limiter = MemoryLimiter::with_clock(|| 0);
    let world = Arc::new(World { fact_tenant: Mutex::new(T1.into()), returns_tenant: Mutex::new(T1.into()), limiter, events: Mutex::new(Vec::new()) });
    let handler = BookingsHandler::create(Impl(world.clone()), Hooks(world.clone()), Impl(world.clone()), None).unwrap();

    // The guest sees the phone; the audit event leaves it out as `@pii` and lists it.
    let (status, doc) = get(&handler, "guest");
    assert_eq!((status, doc.get("phone").is_some(), doc.get("card").is_some()), (200, true, true), "{doc}");
    let event = last(&world);
    assert_eq!(event["hidden"], json!([]));
    for path in ["facts.booking.card", "facts.booking.phone", "principal.email", "result.card", "result.phone"] {
        assert!(event["redacted"].as_array().unwrap().contains(&json!(path)), "{path}: {event}");
    }
    assert!(event["result"].get("phone").is_none() && event["result"].get("card").is_none(), "{event}");

    // Another caller of the same tenant does not: `hidden` lists it, and it is not in `redacted`.
    let (status, doc) = get(&handler, "other");
    assert_eq!((status, doc.get("phone").is_some()), (200, false), "{doc}");
    let event = last(&world);
    assert_eq!(event["hidden"], json!(["phone"]));
    assert!(!event["redacted"].as_array().unwrap().contains(&json!("result.phone")), "{event}");
    assert!(event["redacted"].as_array().unwrap().contains(&json!("facts.booking.phone")), "the fact had it: {event}");
    assert_eq!(get(&handler, "support").1.get("phone").is_some(), true);

    // A fact of another tenant is not found, and the event has no fact; a result of another tenant is `500`.
    *world.fact_tenant.lock().unwrap() = T2.into();
    assert_eq!(get(&handler, "guest").0, 404);
    let event = last(&world);
    assert_eq!((event["outcome"].clone(), event["facts"].clone()), (json!({ "status": 404, "error": "NotFound" }), json!({})), "{event}");
    *world.fact_tenant.lock().unwrap() = T1.into();
    *world.returns_tenant.lock().unwrap() = T2.into();
    let (status, doc) = get(&handler, "guest");
    assert_eq!((status, doc["data"].clone()), (500, json!({})), "{doc}");
    assert!(last(&world).get("result").is_none());
    *world.returns_tenant.lock().unwrap() = T1.into();

    // The limit: ten calls a minute for each caller (this one has made five already).
    let mut refused = None;
    for i in 0..12 {
        let (status, _) = get(&handler, "support");
        if status == 429 {
            refused = Some(i);
            break;
        }
    }
    assert!(refused.is_some(), "the eleventh call of the caller is refused");
    let event = last(&world);
    assert_eq!(event["outcome"], json!({ "status": 429, "error": "TooManyRequests" }));
    assert!(event.get("principal").is_some() && event["parameters"] == json!({}), "{event}");
    println!("all four ok");
}
