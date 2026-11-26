//! The server of `tests/toolchain/contract/platform_p1/visibility.veld` that the program `http`
//! drives in process (`Handler::handle`, the entry point every adapter calls).

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use guests_server::guests::desk_server::{
    CancelBookingFacts, CancelBookingRequest, CheckedBookingRequest, DeskFactLoaders, DeskHandler, DeskHooks,
    DeskService, GetBookingRequest, ListBookingsRequest, LiveBookingsRequest, PageBookingsRequest,
};
use guests_server::guests::guests::{Booking, Guest, User};
use guests_server::guests::notices_server::{NoticesHandler, NoticesService, PublicBookingRequest};
use guests_server::runtime_config::{GuestsServerRuntime, RuleOutputMode};
use guests_server::veld::server::{Body, Handler, Request, RequestContext};
use guests_server::veld::stream::EventStream;
use guests_server::veld::{Error, Page};

pub const ALICE: &str = "00000000-0000-4000-8000-0000000000a1";
pub const BOB: &str = "00000000-0000-4000-8000-0000000000b2";
pub const OPENS: &str = "2026-06-10T12:00:00Z";
/// `OPENS` in milliseconds since the epoch.
pub const OPENS_MS: u64 = 1781092800000;

/// What a test controls.
pub struct World {
    /// The divisor of the bookings the handlers return (zero: the expression of `ratio` fails).
    pub divisor: Mutex<i32>,
    /// Whether the bookings have a note (an `ensure` reads it).
    pub note: Mutex<bool>,
    /// The clock of the hooks, in milliseconds since the epoch.
    pub clock_ms: Mutex<u64>,
    /// Whether the hooks validate requests.
    pub validate: bool,
}

pub fn booking(world: &World, id: &str, guest: &str) -> Booking {
    Booking {
        id: id.into(),
        guest: Guest { id: guest.into(), name: "G".into(), phone: Some("+201000000001".into()) },
        divisor: *world.divisor.lock().unwrap(),
        opens_at: OPENS.into(),
        note: (*world.note.lock().unwrap()).then(|| "n".to_string()),
        ratio: Some("r".into()),
        early: Some("e".into()),
    }
}

const ID: &str = "00000000-0000-4000-8000-000000000001";
const ID2: &str = "00000000-0000-4000-8000-000000000002";

pub struct Impl(pub Arc<World>);

impl DeskService for Impl {
    async fn get_booking(&self, _request: GetBookingRequest, _ctx: &RequestContext) -> Result<Booking, Error> {
        Ok(booking(&self.0, ID, ALICE))
    }

    async fn list_bookings(&self, _request: ListBookingsRequest, _ctx: &RequestContext) -> Result<Vec<Booking>, Error> {
        Ok(vec![booking(&self.0, ID, ALICE), booking(&self.0, ID2, BOB)])
    }

    async fn page_bookings(&self, _request: PageBookingsRequest, _ctx: &RequestContext) -> Result<Page<Booking>, Error> {
        Ok(Page { items: vec![booking(&self.0, ID, ALICE), booking(&self.0, ID2, BOB)], total: 2, page: 1, size: 2 })
    }

    async fn checked_booking(&self, _request: CheckedBookingRequest, _ctx: &RequestContext) -> Result<Booking, Error> {
        Ok(booking(&self.0, ID, ALICE))
    }

    async fn cancel_booking(&self, _request: CancelBookingRequest, facts: CancelBookingFacts, _ctx: &RequestContext) -> Result<Booking, Error> {
        Ok(facts.booking)
    }

    async fn live_bookings(&self, _request: LiveBookingsRequest, _ctx: &RequestContext) -> Result<EventStream<Booking>, Error> {
        Err(Error::other("the harness does not stream"))
    }
}

impl DeskFactLoaders for Impl {
    async fn booking(&self, _key: String, _ctx: &RequestContext) -> Result<Option<Booking>, Error> {
        Ok(Some(booking(&self.0, ID, ALICE)))
    }
}

impl NoticesService for Impl {
    async fn public_booking(&self, _request: PublicBookingRequest, _ctx: &RequestContext) -> Result<Booking, Error> {
        Ok(booking(&self.0, ID, ALICE))
    }
}

/// Who a bearer token is: `alice` and `bob` are guests, `support` has the role.
pub fn caller(token: &str) -> Option<User> {
    match token {
        "alice" => Some(User { id: ALICE.into(), roles: vec![] }),
        "bob" => Some(User { id: BOB.into(), roles: vec![] }),
        "support" => Some(User { id: "00000000-0000-4000-8000-0000000000c3".into(), roles: vec!["support".into()] }),
        _ => None,
    }
}

pub struct Hooks(pub Arc<World>);

impl DeskHooks for Hooks {
    async fn auth_bearer<'a>(&'a self, token: &'a str, _ctx: &'a RequestContext) -> Option<User> {
        caller(token)
    }

    fn validate_requests(&self) -> bool {
        self.0.validate
    }

    fn clock(&self, _ctx: &RequestContext) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(*self.0.clock_ms.lock().unwrap())
    }
}

impl guests_server::guests::notices_server::NoticesHooks for Hooks {
    fn validate_requests(&self) -> bool {
        self.0.validate
    }
}

pub fn runtime(off: bool) -> GuestsServerRuntime {
    let mut runtime = GuestsServerRuntime::default();
    if off {
        runtime.rules.output = RuleOutputMode::Off;
    }
    runtime
}

pub struct Servers {
    pub world: Arc<World>,
    pub desk: Handler<guests_server::guests::desk_server::DeskDispatch<Impl, Hooks, Impl>>,
    pub notices: Handler<guests_server::guests::notices_server::NoticesDispatch<Impl, Hooks>>,
}

/// The two production handlers. `validate` and `off` are the settings that must change nothing.
pub fn serve(validate: bool, off: bool) -> Servers {
    let world = Arc::new(World { divisor: Mutex::new(1), note: Mutex::new(true), clock_ms: Mutex::new(OPENS_MS + 1), validate });
    let rt = runtime(off);
    let desk = DeskHandler::create(Impl(world.clone()), Hooks(world.clone()), Impl(world.clone()), Some(&rt)).unwrap();
    let notices = NoticesHandler::create(Impl(world.clone()), Hooks(world.clone()), Some(&rt)).unwrap();
    Servers { world, desk, notices }
}

pub struct Answer {
    pub status: u16,
    pub doc: serde_json::Value,
}

/// Sends a GET (or a POST without a body) to `handler` as `token` and reads the JSON answer.
pub fn send<E: guests_server::veld::server::Endpoints>(handler: &Handler<E>, method: &str, path: &str, token: Option<&str>) -> Answer {
    let headers = token.map(|t| vec![("authorization".to_string(), format!("Bearer {t}"))]).unwrap_or_default();
    let request = Request { method: method.into(), path: path.into(), query: String::new(), headers, body: Vec::new() };
    let response = guests_server::veld::block_on(handler.handle(request)).expect("a route");
    let doc = match response.body {
        Body::Bytes(bytes) => serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
        _ => serde_json::Value::Null,
    };
    Answer { status: response.status, doc }
}
