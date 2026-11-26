//! Tenancy over the production handler (`spec/platform.md` section 3), generated from
//! `tests/toolchain/contract/platform_p1/tenancy.veld`: a fact of another tenant is not found (`404`,
//! never `403`), an optional one is absent, a value of another tenant in a result, a list, a page, a
//! declared error and a stream item is never sent (`500`, the hook hears one event per value), a
//! caller without a tenant owns nothing, and a request that lacks the tenant member or carries one.
//! The program prints `tenancy http ok`.

use std::sync::{Arc, Mutex};

use bookings_server::bookings::bookings::{Booking, Summary, User};
use bookings_server::bookings::tenants_server::{
    CopyBookingFacts, CopyBookingRequest, CreateBookingRequest, GetBookingFacts, GetBookingRequest, ListBookingsRequest,
    LiveBookingsRequest, MeRequest, PageBookingsRequest, SeenFacts, SeenRequest, TenantsDispatch, TenantsFactLoaders,
    TenantsHandler, TenantsHooks, TenantsService,
};
use bookings_server::veld::rules::{Outcome, Position, RuleEvent};
use bookings_server::veld::server::{Body, Handler, Request, RequestContext};
use bookings_server::veld::stream::{EventStream, SseEvent, iter, next};
use bookings_server::veld::{Error, Page, VeldDecl};
use serde_json::{Value, json};

const T1: &str = "00000000-0000-4000-8000-0000000000a1";
const T2: &str = "00000000-0000-4000-8000-0000000000b2";
const ID: &str = "00000000-0000-4000-8000-000000000001";

fn booking(tenant: &str, open: bool) -> Booking {
    Booking { id: ID.into(), tenant_id: tenant.into(), title: "t".into(), open }
}

/// What a test controls and reads back.
#[derive(Default)]
struct World {
    /// The tenant of the fact the loaders return, and whether it is open.
    fact: Mutex<(String, bool)>,
    /// The tenants of the bookings the handlers return.
    returns: Mutex<Vec<String>>,
    /// The tenant of the user `me` returns (`None`: it has none).
    me: Mutex<Option<String>>,
    /// How often a handler ran, and the tenant the last `createBooking` saw.
    ran: Mutex<usize>,
    created: Mutex<Option<String>>,
    events: Mutex<Vec<RuleEvent>>,
}

struct Impl(Arc<World>);

impl Impl {
    fn returned(&self) -> Vec<Booking> {
        self.0.returns.lock().unwrap().iter().map(|t| booking(t, true)).collect()
    }
}

impl TenantsService for Impl {
    async fn get_booking(&self, _r: GetBookingRequest, _f: GetBookingFacts, _c: &RequestContext) -> Result<Booking, Error> {
        *self.0.ran.lock().unwrap() += 1;
        Ok(self.returned().remove(0))
    }

    async fn seen(&self, _r: SeenRequest, _f: SeenFacts, _c: &RequestContext) -> Result<Summary, Error> {
        *self.0.ran.lock().unwrap() += 1;
        Ok(Summary { title: "seen".into() })
    }

    async fn list_bookings(&self, _r: ListBookingsRequest, _c: &RequestContext) -> Result<Vec<Booking>, Error> {
        Ok(self.returned())
    }

    async fn page_bookings(&self, _r: PageBookingsRequest, _c: &RequestContext) -> Result<Page<Booking>, Error> {
        let items = self.returned();
        Ok(Page { total: 2, page: 1, size: 2, items })
    }

    async fn me(&self, _r: MeRequest, _c: &RequestContext) -> Result<User, Error> {
        Ok(User { id: ID.into(), tenant_id: self.0.me.lock().unwrap().clone() })
    }

    async fn copy_booking(&self, _r: CopyBookingRequest, f: CopyBookingFacts, _c: &RequestContext) -> Result<Booking, Error> {
        Ok(f.booking)
    }

    async fn create_booking(&self, request: CreateBookingRequest, ctx: &RequestContext) -> Result<Booking, Error> {
        *self.0.created.lock().unwrap() = Some(request.body.tenant_id.clone());
        let tenant = ctx.principal::<User>().and_then(|u| u.tenant_id.clone()).unwrap_or_default();
        Ok(Booking { tenant_id: tenant, ..request.body })
    }

    async fn live_bookings(&self, _r: LiveBookingsRequest, _c: &RequestContext) -> Result<EventStream<Booking>, Error> {
        let items: Vec<Result<SseEvent<Booking>, Error>> = self.returned().into_iter().map(|b| Ok(SseEvent::of(b))).collect();
        Ok(Box::pin(iter(items)))
    }
}

impl TenantsFactLoaders for Impl {
    async fn booking(&self, _key: String, _ctx: &RequestContext) -> Result<Option<Booking>, Error> {
        let (tenant, open) = self.0.fact.lock().unwrap().clone();
        Ok(Some(booking(&tenant, open)))
    }

    async fn held(&self, _key: String, _ctx: &RequestContext) -> Result<Option<Booking>, Error> {
        let (tenant, open) = self.0.fact.lock().unwrap().clone();
        Ok(Some(booking(&tenant, open)))
    }
}

struct Hooks(Arc<World>);

impl TenantsHooks for Hooks {
    async fn auth_bearer<'a>(&'a self, token: &'a str, _ctx: &'a RequestContext) -> Option<User> {
        match token {
            "t1" => Some(User { id: ID.into(), tenant_id: Some(T1.into()) }),
            "t2" => Some(User { id: ID.into(), tenant_id: Some(T2.into()) }),
            "none" => Some(User { id: ID.into(), tenant_id: None }),
            _ => None,
        }
    }

    fn on_rule_violation(&self, event: &RuleEvent) {
        self.0.events.lock().unwrap().push(event.clone());
    }
}

type Server = Handler<TenantsDispatch<Impl, Hooks, Impl>>;

fn serve(world: &Arc<World>) -> Server {
    TenantsHandler::create(Impl(world.clone()), Hooks(world.clone()), Impl(world.clone()), None).unwrap()
}

fn call(server: &Server, method: &str, path: &str, token: &str, body: Option<Value>) -> (u16, Value, Body) {
    let mut headers = vec![("authorization".to_string(), format!("Bearer {token}"))];
    let body = body.map(|b| b.to_string().into_bytes()).unwrap_or_default();
    if !body.is_empty() {
        headers.push(("content-type".into(), "application/json".into()));
    }
    let request = Request { method: method.into(), path: path.into(), query: String::new(), headers, body };
    let response = bookings_server::veld::block_on(server.handle(request)).expect("a route");
    let doc = match &response.body {
        Body::Bytes(bytes) => serde_json::from_slice(bytes).unwrap_or(Value::Null),
        _ => Value::Null,
    };
    (response.status, doc, response.body)
}

fn reset(world: &World, fact_tenant: &str, returns: &[&str]) {
    *world.fact.lock().unwrap() = (fact_tenant.to_string(), true);
    *world.returns.lock().unwrap() = returns.iter().map(ToString::to_string).collect();
    *world.ran.lock().unwrap() = 0;
    world.events.lock().unwrap().clear();
}

/// The id of the tenant rule of `Booking`: what the hook is told.
fn tenant_rule() -> String {
    match <Booking as VeldDecl>::schema() {
        bookings_server::veld::schema::Schema::Model(m) => m.tenant.expect("a tenant mark").id.to_string(),
        _ => panic!("a model"),
    }
}

/// What the hook heard: `(rule, path, field, position, outcome)` of each event.
fn heard(world: &World) -> Vec<(String, String, String, Position, Outcome)> {
    world
        .events
        .lock()
        .unwrap()
        .iter()
        .map(|e| (e.rule.clone(), e.violation.path.clone(), e.violation.fields.join(","), e.position, e.outcome))
        .collect()
}

fn violation(path: &str) -> (String, String, String, Position, Outcome) {
    (tenant_rule(), path.into(), "tenantId".into(), Position::Output, Outcome::Rejected)
}

/// A fact of another tenant is `404` with its name, never `403`, and the handler is not reached;
/// the same fact of the caller's tenant is served.
fn facts(world: &Arc<World>, server: &Server) {
    let path = format!("/tenants/{ID}");
    reset(world, T1, &[T1]);
    let own = call(server, "GET", &path, "t1", None);
    assert_eq!(own.0, 200, "{}", own.1);
    reset(world, T2, &[T2]);
    let other = call(server, "GET", &path, "t1", None);
    assert_eq!(other.0, 404, "{}", other.1);
    assert_eq!(other.1["title"], json!("NotFound"));
    assert_eq!(other.1["data"]["resource"], json!("booking"));
    assert_eq!(*world.ran.lock().unwrap(), 0, "the handler was not reached");
    // An optional fact of another tenant is absent (`seen` answers `NotFound` for the absent one).
    let seen = format!("/tenants/{ID}/seen");
    reset(world, T1, &[]);
    assert_eq!(call(server, "GET", &seen, "t1", None).0, 200);
    reset(world, T2, &[]);
    let absent = call(server, "GET", &seen, "t1", None);
    assert_eq!((absent.0, absent.1["data"]["resource"].clone()), (404, json!("held")), "{}", absent.1);
}

/// A value of another tenant is never sent: `500` with no detail, one event per value with the
/// path of the value, the rule of the model and the position `output`.
fn answers(world: &Arc<World>, server: &Server) {
    reset(world, T1, &[T1]);
    // One value: the fact is the caller's, the handler returns another tenant's.
    *world.returns.lock().unwrap() = vec![T2.into()];
    let one = call(server, "GET", &format!("/tenants/{ID}"), "t1", None);
    assert_eq!(one.0, 500, "{}", one.1);
    assert_eq!(one.1["data"], json!({}), "no detail: {}", one.1);
    assert!(!one.1.to_string().contains(T2), "nothing of the value is sent");
    assert_eq!(heard(world), vec![violation("result")]);
    // A list: the second item.
    reset(world, T1, &[T1, T2]);
    let list = call(server, "GET", "/tenants", "t1", None);
    assert_eq!(list.0, 500);
    assert_eq!(heard(world), vec![violation("result[1]")]);
    // A list of the caller's own is sent.
    reset(world, T1, &[T1, T1]);
    assert_eq!(call(server, "GET", "/tenants", "t1", None).0, 200);
    // A page: the first item.
    reset(world, T1, &[T2, T1]);
    assert_eq!(call(server, "GET", "/tenants/page", "t1", None).0, 500);
    assert_eq!(heard(world), vec![violation("result.items[0]")]);
    // Two values of another tenant are two events.
    reset(world, T1, &[T2, T2]);
    assert_eq!(call(server, "GET", "/tenants", "t1", None).0, 500);
    assert_eq!(heard(world), vec![violation("result[0]"), violation("result[1]")]);
}

/// A declared error that holds a value of another tenant is never sent either.
fn a_declared_error(world: &Arc<World>, server: &Server) {
    reset(world, T1, &[]);
    *world.fact.lock().unwrap() = (T1.to_string(), false);
    let copy = |tenant: &str| json!({ "copy": { "id": ID, "tenantId": tenant, "title": "c", "open": true } });
    let path = format!("/tenants/{ID}/copy");
    let sent = call(server, "POST", &path, "t1", Some(copy(T1)));
    assert_eq!((sent.0, sent.1["title"].clone()), (409, json!("Dup")), "{}", sent.1);
    world.events.lock().unwrap().clear();
    let refused = call(server, "POST", &path, "t1", Some(copy(T2)));
    assert_eq!(refused.0, 500, "{}", refused.1);
    assert_eq!(heard(world), vec![violation("error.copy")]);
}

/// A caller whose principal has no tenant owns nothing, not even the principal's own model.
fn a_caller_without_a_tenant(world: &Arc<World>, server: &Server) {
    reset(world, T1, &[T1]);
    assert_eq!(call(server, "GET", &format!("/tenants/{ID}"), "none", None).0, 404);
    assert_eq!(call(server, "GET", "/tenants", "none", None).0, 500);
    *world.me.lock().unwrap() = None;
    assert_eq!(call(server, "GET", "/tenants/me", "none", None).0, 500, "a value with no tenant belongs to nobody");
    *world.me.lock().unwrap() = Some(T1.into());
    assert_eq!(call(server, "GET", "/tenants/me", "t1", None).0, 200);
    assert_eq!(call(server, "GET", "/tenants/me", "t2", None).0, 500);
}

/// A request may lack the tenant member, and one that carries it is kept as received (the
/// application sets the real tenant from the caller).
fn the_member_of_a_request(world: &Arc<World>, server: &Server) {
    reset(world, T1, &[]);
    let without = call(server, "POST", "/tenants", "t1", Some(json!({ "id": ID, "title": "x", "open": true })));
    assert_eq!(without.0, 200, "{}", without.1);
    assert_eq!(without.1["tenantId"], json!(T1));
    let carried = call(server, "POST", "/tenants", "t1", Some(json!({ "id": ID, "tenantId": T2, "title": "x", "open": true })));
    assert_eq!(carried.0, 200, "{}", carried.1);
    assert_eq!(world.created.lock().unwrap().as_deref(), Some(T2), "kept as received");
}

/// A stream item of another tenant is not sent: the stream ends with the error event; one that
/// is the first item is a `500`.
fn a_stream(world: &Arc<World>, server: &Server) {
    reset(world, T1, &[T1, T2]);
    let (status, _, body) = call(server, "GET", "/tenants/live", "t1", None);
    assert_eq!(status, 200);
    let Body::Sse(mut frames) = body else { panic!("a stream") };
    let first = String::from_utf8(bookings_server::veld::block_on(next(&mut frames)).expect("a frame")).unwrap();
    assert!(first.contains(T1) && !first.contains(T2), "{first}");
    let second = String::from_utf8(bookings_server::veld::block_on(next(&mut frames)).expect("a frame")).unwrap();
    assert!(second.starts_with("event: error"), "{second}");
    assert!(!second.contains(T2) && bookings_server::veld::block_on(next(&mut frames)).is_none());
    assert_eq!(heard(world), vec![violation("item")]);
    reset(world, T1, &[T2]);
    assert_eq!(call(server, "GET", "/tenants/live", "t1", None).0, 500);
}

fn main() {
    let world = Arc::new(World::default());
    let server = serve(&world);
    facts(&world, &server);
    answers(&world, &server);
    a_declared_error(&world, &server);
    a_caller_without_a_tenant(&world, &server);
    the_member_of_a_request(&world, &server);
    a_stream(&world, &server);
    println!("tenancy http ok");
}
