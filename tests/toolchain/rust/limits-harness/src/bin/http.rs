//! Rate limits over the production handler (`spec/platform.md` section 5), generated from
//! `tests/toolchain/contract/platform_p1/limits.veld`: the last allowed call, the first refused one
//! with its body and header, the next window, each of `user`, `tenant` (also a caller with none) and
//! `ip`, a limit a service gives its endpoints, the audit event of a refused call, a limiter and a
//! hook that fail (the call goes through and `on_error` hears), and a server with no limiter. The
//! program prints `limits http ok`.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use bookings_server::bookings::bookings::{Room, User};
use bookings_server::bookings::desks_server::{DesksHandler, DesksHooks, DesksService, MineRequest};
use bookings_server::bookings::public_server::{ListingRequest, PublicHandler, PublicHooks, PublicService};
use bookings_server::bookings::rooms_server::{
    ArchiveRequest, ByTenantRequest, ByUserRequest, CreateRequest, FreeRequest, RoomsDispatch, RoomsHandler, RoomsHooks,
    RoomsService,
};
use bookings_server::veld::idempotency::MemoryIdempotencyStore;
use bookings_server::veld::platform::{AuditEvent, Decision, Limiter, MemoryLimiter};
use bookings_server::veld::server::{Body, Handler, Request, RequestContext};
use bookings_server::veld::Error;
use serde_json::{Value, json};

const T1: &str = "00000000-0000-4000-8000-0000000000a1";
const ID: &str = "00000000-0000-4000-8000-000000000001";

/// How the limiter of the hooks behaves.
enum Mode {
    /// The memory limiter on the clock of the test.
    Memory(MemoryLimiter),
    /// A limiter that fails.
    Failing,
    /// None: the server counts in memory itself.
    None,
}

struct Failing;

impl Limiter for Failing {
    fn take<'a>(&'a self, _key: &'a str, _limit: u32, _window_seconds: u32) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Decision, Error>> + Send + 'a>> {
        Box::pin(std::future::ready(Err(Error::other("the limiter is down"))))
    }
}

struct World {
    clock: Arc<AtomicI64>,
    mode: Mode,
    failing: Failing,
    address: Mutex<Option<String>>,
    caller_fails: Mutex<bool>,
    errors: Mutex<Vec<String>>,
    events: Mutex<Vec<AuditEvent>>,
    store: MemoryIdempotencyStore,
}

impl World {
    fn new(failing_limiter: bool, none: bool) -> Arc<World> {
        let clock = Arc::new(AtomicI64::new(0));
        let mode = if none {
            Mode::None
        } else if failing_limiter {
            Mode::Failing
        } else {
            let clock = clock.clone();
            Mode::Memory(MemoryLimiter::with_clock(move || clock.load(Ordering::SeqCst)))
        };
        Arc::new(World {
            clock,
            mode,
            failing: Failing,
            address: Mutex::new(Some("203.0.113.9".into())),
            caller_fails: Mutex::new(false),
            errors: Mutex::new(Vec::new()),
            events: Mutex::new(Vec::new()),
            store: MemoryIdempotencyStore::new(),
        })
    }

    fn limiter(&self) -> Option<&dyn Limiter> {
        match &self.mode {
            Mode::Memory(l) => Some(l),
            Mode::Failing => Some(&self.failing),
            Mode::None => None,
        }
    }

    fn address(&self) -> Result<String, Error> {
        self.address.lock().unwrap().clone().ok_or_else(|| Error::other("no address"))
    }
}

struct Impl;

impl RoomsService for Impl {
    async fn by_user(&self, _r: ByUserRequest, _c: &RequestContext) -> Result<Room, Error> {
        Ok(Room { id: ID.into() })
    }

    async fn by_tenant(&self, _r: ByTenantRequest, _c: &RequestContext) -> Result<Vec<Room>, Error> {
        Ok(vec![])
    }

    async fn archive(&self, _r: ArchiveRequest, _c: &RequestContext) -> Result<(), Error> {
        Ok(())
    }

    async fn create(&self, request: CreateRequest, _c: &RequestContext) -> Result<Room, Error> {
        Ok(request.body)
    }

    async fn free(&self, _r: FreeRequest, _c: &RequestContext) -> Result<Vec<Room>, Error> {
        Ok(vec![])
    }
}

impl PublicService for Impl {
    async fn listing(&self, _r: ListingRequest, _c: &RequestContext) -> Result<Vec<Room>, Error> {
        Ok(vec![])
    }
}

impl DesksService for Impl {
    async fn mine(&self, _r: MineRequest, _c: &RequestContext) -> Result<Vec<Room>, Error> {
        Ok(vec![])
    }
}

fn user(token: &str) -> Option<User> {
    match token {
        "t1" => Some(User { id: "u1".into(), tenant_id: Some(T1.into()) }),
        "n1" => Some(User { id: "n1".into(), tenant_id: None }),
        "n2" => Some(User { id: "n2".into(), tenant_id: None }),
        _ => None,
    }
}

struct Hooks(Arc<World>);

impl RoomsHooks for Hooks {
    type IdempotencyStore = MemoryIdempotencyStore;

    async fn auth_bearer<'a>(&'a self, token: &'a str, _ctx: &'a RequestContext) -> Option<User> {
        user(token)
    }

    fn idempotency_store(&self) -> &MemoryIdempotencyStore {
        &self.0.store
    }

    fn idempotency_caller(&self, principal: &User, _ctx: &RequestContext) -> String {
        principal.id.clone()
    }

    async fn audit<'a>(&'a self, event: &'a AuditEvent) -> Result<(), Error> {
        self.0.events.lock().unwrap().push(event.clone());
        Ok(())
    }

    fn limiter(&self) -> Option<&dyn Limiter> {
        self.0.limiter()
    }

    fn address(&self, _ctx: &RequestContext) -> Result<String, Error> {
        self.0.address()
    }

    fn on_error(&self, err: &Error, _ctx: &RequestContext) {
        self.0.errors.lock().unwrap().push(err.to_string());
    }
}

impl PublicHooks for Hooks {
    fn limiter(&self) -> Option<&dyn Limiter> {
        self.0.limiter()
    }

    fn address(&self, _ctx: &RequestContext) -> Result<String, Error> {
        self.0.address()
    }
}

impl DesksHooks for Hooks {
    async fn auth_bearer<'a>(&'a self, token: &'a str, _ctx: &'a RequestContext) -> Option<User> {
        user(token)
    }

    fn limiter(&self) -> Option<&dyn Limiter> {
        self.0.limiter()
    }

    fn caller(&self, principal: &User, _ctx: &RequestContext) -> Result<String, Error> {
        if *self.0.caller_fails.lock().unwrap() { Err(Error::other("no caller")) } else { Ok(principal.id.clone()) }
    }

    fn on_error(&self, err: &Error, _ctx: &RequestContext) {
        self.0.errors.lock().unwrap().push(err.to_string());
    }
}

struct Answer {
    status: u16,
    doc: Value,
    retry_after: Option<String>,
}

fn call<E: bookings_server::veld::server::Endpoints>(h: &Handler<E>, method: &str, path: &str, token: Option<&str>) -> Answer {
    let headers = token.map(|t| vec![("authorization".to_string(), format!("Bearer {t}"))]).unwrap_or_default();
    let request = Request { method: method.into(), path: path.into(), query: String::new(), headers, body: Vec::new() };
    let response = bookings_server::veld::block_on(h.handle(request)).expect("a route");
    let retry_after = response.headers.iter().find(|(k, _)| k == "retry-after").map(|(_, v)| v.clone());
    let doc = match response.body {
        Body::Bytes(bytes) => serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        _ => Value::Null,
    };
    Answer { status: response.status, doc, retry_after }
}

fn rooms(world: &Arc<World>) -> Handler<RoomsDispatch<Impl, Hooks>> {
    RoomsHandler::create(Impl, Hooks(world.clone()))
}

/// The last allowed call, the first refused one with its body and header, and the next window.
fn by_user(world: &Arc<World>) {
    let rooms = rooms(world);
    let path = format!("/rooms/{ID}");
    for _ in 0..2 {
        assert_eq!(call(&rooms, "GET", &path, Some("t1")).status, 200);
    }
    world.clock.store(20_000, Ordering::SeqCst);
    let refused = call(&rooms, "GET", &path, Some("t1"));
    assert_eq!(refused.status, 429, "{}", refused.doc);
    assert_eq!(refused.doc["title"], json!("TooManyRequests"));
    assert_eq!(refused.doc["data"]["retryAfter"], json!(40), "a JSON number: {}", refused.doc);
    assert_eq!(refused.retry_after.as_deref(), Some("40"));
    // Another caller has a count of its own; the refused call did not count; the next window opens.
    assert_eq!(call(&rooms, "GET", &path, Some("n1")).status, 200);
    world.clock.store(60_000, Ordering::SeqCst);
    assert_eq!(call(&rooms, "GET", &path, Some("t1")).status, 200);
    // The refused call was audited: the caller is known, nothing was bound, nothing loaded.
    let events = world.events.lock().unwrap();
    let event = events.iter().find(|e| e.outcome.status == 429).expect("the refused call is audited").to_json();
    assert_eq!((event["parameters"].clone(), event["facts"].clone()), (json!({}), json!({})), "{event}");
    assert!(event.get("principal").is_some() && event.get("result").is_none(), "{event}");
    assert_eq!(event["outcome"], json!({ "status": 429, "error": "TooManyRequests" }));
}

/// `tenant`: one count for the callers of a tenant, and one for all the callers that have none.
fn by_tenant(world: &Arc<World>) {
    let rooms = rooms(world);
    for token in ["t1", "t1", "t1"] {
        assert_eq!(call(&rooms, "GET", "/rooms/tenant", Some(token)).status, 200);
    }
    assert_eq!(call(&rooms, "GET", "/rooms/tenant", Some("t1")).status, 429);
    // `n1` and `n2` have no tenant: they are counted together (the limit is 3 per hour).
    for token in ["n1", "n2", "n1"] {
        assert_eq!(call(&rooms, "GET", "/rooms/tenant", Some(token)).status, 200);
    }
    assert_eq!(call(&rooms, "GET", "/rooms/tenant", Some("n2")).status, 429);
}

/// `ip`: the address hook; a `post` of the service has its own limit; a service's limit; the
/// endpoint without a limit is never refused.
fn by_ip(world: &Arc<World>) {
    let rooms = rooms(world);
    let path = format!("/rooms/{ID}/archive");
    assert_eq!(call(&rooms, "POST", &path, Some("t1")).status, 204);
    assert_eq!(call(&rooms, "POST", &path, Some("t1")).status, 429);
    *world.address.lock().unwrap() = Some("203.0.113.10".into());
    assert_eq!(call(&rooms, "POST", &path, Some("t1")).status, 204, "another address, another count");
    for _ in 0..5 {
        assert_eq!(call(&rooms, "GET", "/rooms/free", Some("t1")).status, 200);
    }
    // The limit of a service: two per second for each address.
    *world.address.lock().unwrap() = Some("203.0.113.9".into());
    let public = PublicHandler::create(Impl, Hooks(world.clone()));
    assert_eq!(call(&public, "GET", "/public", None).status, 200);
    assert_eq!(call(&public, "GET", "/public", None).status, 200);
    assert_eq!(call(&public, "GET", "/public", None).status, 429);
}

/// A caller hook of its own (a service with no idempotent endpoint), and a failing hook.
fn a_service_with_its_own_caller_hook(world: &Arc<World>) {
    let desks = DesksHandler::create(Impl, Hooks(world.clone()));
    assert_eq!(call(&desks, "GET", "/desks", Some("t1")).status, 200);
    assert_eq!(call(&desks, "GET", "/desks", Some("t1")).status, 429);
    *world.caller_fails.lock().unwrap() = true;
    assert_eq!(call(&desks, "GET", "/desks", Some("t1")).status, 200, "a failing hook lets the call through");
    assert!(world.errors.lock().unwrap().iter().any(|e| e.contains("no caller")));
}

/// A limiter that fails and an address hook that fails let the call through; `on_error` hears.
fn failures() {
    let world = World::new(true, false);
    let rooms = rooms(&world);
    let path = format!("/rooms/{ID}");
    for _ in 0..4 {
        assert_eq!(call(&rooms, "GET", &path, Some("t1")).status, 200);
    }
    assert!(world.errors.lock().unwrap().iter().any(|e| e.contains("the limiter is down")));
    let world = World::new(false, false);
    *world.address.lock().unwrap() = None;
    let rooms = self::rooms(&world);
    let archive = format!("/rooms/{ID}/archive");
    for _ in 0..3 {
        assert_eq!(call(&rooms, "POST", &archive, Some("t1")).status, 204);
    }
    assert!(world.errors.lock().unwrap().iter().any(|e| e.contains("no address")));
}

/// A server whose hooks give no limiter counts in memory (and says so once when it is created).
fn no_limiter() {
    let world = World::new(false, true);
    let rooms = rooms(&world);
    let path = format!("/rooms/{ID}");
    assert_eq!(call(&rooms, "GET", &path, Some("t1")).status, 200);
    assert_eq!(call(&rooms, "GET", &path, Some("t1")).status, 200);
    assert_eq!(call(&rooms, "GET", &path, Some("t1")).status, 429);
}

fn main() {
    by_user(&World::new(false, false));
    by_tenant(&World::new(false, false));
    by_ip(&World::new(false, false));
    a_service_with_its_own_caller_hook(&World::new(false, false));
    failures();
    no_limiter();
    println!("limits http ok");
}
