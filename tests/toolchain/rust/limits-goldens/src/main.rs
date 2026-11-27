//! The goldens of `tests/wire/limits/cases.json`: the `windows` through the generated memory
//! limiter with a clock the program controls (each call at its time, each decision as the case
//! says), and the `keys` through the production handler of each service (the key the limiter is
//! asked about, for a caller text, a tenant and an address). Usage: `limits-goldens <cases.json>`;
//! prints `limits goldens ok: <windows> windows, <keys> keys`.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use bookings_server::bookings::bookings::{Room, User};
use bookings_server::bookings::public_server::{HotRequest, ListingRequest, PublicHandler, PublicHooks, PublicService};
use bookings_server::bookings::rooms_server::{
    ArchiveRequest, ByIpRequest, ByTenantRequest, ByUserRequest, RoomsHandler, RoomsHooks, RoomsService,
};
use bookings_server::veld::Error;
use bookings_server::veld::platform::{Decision, Limiter, MemoryLimiter};
use bookings_server::veld::server::{Request, RequestContext};
use serde_json::Value;

/// What the stubs say, and the keys the limiter was asked about.
#[derive(Default)]
struct World {
    caller: Mutex<String>,
    address: Mutex<String>,
    principal: Mutex<Option<User>>,
    asked: Mutex<Vec<String>>,
}

struct Impl;

impl RoomsService for Impl {
    async fn by_user(&self, _r: ByUserRequest, _c: &RequestContext) -> Result<Room, Error> {
        Ok(Room { id: "r".into() })
    }

    async fn by_tenant(&self, _r: ByTenantRequest, _c: &RequestContext) -> Result<Vec<Room>, Error> {
        Ok(vec![])
    }

    async fn by_ip(&self, _r: ByIpRequest, _c: &RequestContext) -> Result<Vec<Room>, Error> {
        Ok(vec![])
    }

    async fn archive(&self, _r: ArchiveRequest, _c: &RequestContext) -> Result<(), Error> {
        Ok(())
    }
}

impl PublicService for Impl {
    async fn listing(&self, _r: ListingRequest, _c: &RequestContext) -> Result<Vec<Room>, Error> {
        Ok(vec![])
    }

    async fn hot(&self, _r: HotRequest, _c: &RequestContext) -> Result<Vec<Room>, Error> {
        Ok(vec![])
    }
}

/// A limiter that records the key and allows every call.
struct Recorder(Arc<World>);

impl Limiter for Recorder {
    fn take<'a>(&'a self, key: &'a str, _limit: u32, _window_seconds: u32) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Decision, Error>> + Send + 'a>> {
        self.0.asked.lock().unwrap().push(key.to_string());
        Box::pin(std::future::ready(Ok(Decision { allowed: true, retry_after_seconds: 0 })))
    }
}

struct Hooks(Arc<World>, Recorder);

impl RoomsHooks for Hooks {
    async fn auth_bearer<'a>(&'a self, _token: &'a str, _ctx: &'a RequestContext) -> Option<User> {
        self.0.principal.lock().unwrap().clone()
    }

    fn limiter(&self) -> Option<&dyn Limiter> {
        Some(&self.1)
    }

    fn address(&self, _ctx: &RequestContext) -> Result<String, Error> {
        Ok(self.0.address.lock().unwrap().clone())
    }

    fn caller(&self, _principal: &User, _ctx: &RequestContext) -> Result<String, Error> {
        Ok(self.0.caller.lock().unwrap().clone())
    }
}

impl PublicHooks for Hooks {
    fn limiter(&self) -> Option<&dyn Limiter> {
        Some(&self.1)
    }

    fn address(&self, _ctx: &RequestContext) -> Result<String, Error> {
        Ok(self.0.address.lock().unwrap().clone())
    }
}

fn windows(cases: &[Value]) {
    for case in cases {
        let name = case["name"].as_str().unwrap_or("?");
        let now = Arc::new(AtomicI64::new(0));
        let limiter = MemoryLimiter::with_clock({
            let now = now.clone();
            move || now.load(Ordering::SeqCst)
        });
        let limit = u32::try_from(case["limit"].as_u64().unwrap()).unwrap();
        let window = u32::try_from(case["windowSeconds"].as_u64().unwrap()).unwrap();
        let calls = case["calls"].as_array().unwrap();
        let decisions = case["decisions"].as_array().unwrap();
        assert_eq!(calls.len(), decisions.len(), "{name}");
        for (i, (call, want)) in calls.iter().zip(decisions).enumerate() {
            now.store(call["t"].as_i64().unwrap(), Ordering::SeqCst);
            let got = bookings_server::veld::block_on(limiter.take(call["key"].as_str().unwrap(), limit, window)).unwrap();
            assert_eq!(got.allowed, want["allowed"].as_bool().unwrap(), "{name}: call {i} allowed");
            assert_eq!(u64::from(got.retry_after_seconds), want["retryAfterSeconds"].as_u64().unwrap(), "{name}: call {i} retryAfter");
        }
    }
}

/// Sends the endpoint `name` to the handler of its service and returns the key the limiter was
/// asked about.
fn key_of(case: &Value) -> String {
    let world = Arc::new(World::default());
    *world.caller.lock().unwrap() = case["caller"].as_str().unwrap_or_default().to_string();
    *world.address.lock().unwrap() = case["address"].as_str().unwrap_or_default().to_string();
    *world.principal.lock().unwrap() = Some(match case.get("principal") {
        Some(p) => User { id: p["id"].as_str().unwrap().into(), tenant_id: p["tenantId"].as_str().map(str::to_string) },
        None => User { id: "00000000-0000-4000-8000-000000000001".into(), tenant_id: None },
    });
    let hooks = || Hooks(world.clone(), Recorder(world.clone()));
    let name = case["endpoint"].as_str().unwrap();
    let id = "00000000-0000-4000-8000-000000000001";
    let (method, path, public) = match name {
        "byUser" => ("GET", format!("/rooms/{id}"), false),
        "byTenant" => ("GET", "/rooms/tenant".to_string(), false),
        "byIp" => ("GET", "/rooms/ip".to_string(), false),
        "archive" => ("POST", format!("/rooms/{id}/archive"), false),
        "listing" => ("GET", "/public".to_string(), true),
        "hot" => ("GET", "/public/hot".to_string(), true),
        other => panic!("the contract has no endpoint `{other}`"),
    };
    let request = Request { method: method.into(), path, query: String::new(), headers: vec![("authorization".into(), "Bearer t".into())], body: Vec::new() };
    let status = if public {
        bookings_server::veld::block_on(PublicHandler::create(Impl, hooks()).handle(request)).map(|r| r.status)
    } else {
        bookings_server::veld::block_on(RoomsHandler::create(Impl, hooks()).handle(request)).map(|r| r.status)
    };
    assert!(status.is_some_and(|s| s < 300), "{name}: the call is answered, not refused: {status:?}");
    let asked = world.asked.lock().unwrap().clone();
    assert_eq!(asked.len(), 1, "{name}: the limiter is asked once: {asked:?}");
    asked[0].clone()
}

fn main() {
    let path = std::env::args().nth(1).expect("the path of cases.json");
    let file: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("cases.json")).expect("JSON");
    let (w, k) = (file["windows"].as_array().unwrap(), file["keys"].as_array().unwrap());
    windows(w);
    for case in k {
        assert_eq!(key_of(case), case["key"].as_str().unwrap(), "the key of {}", case["endpoint"]);
    }
    println!("limits goldens ok: {} windows, {} keys", w.len(), k.len());
}
