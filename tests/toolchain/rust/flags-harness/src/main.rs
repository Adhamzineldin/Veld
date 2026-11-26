//! Flags over the production handler (`spec/platform.md` section 11), generated from
//! `tests/toolchain/contract/platform_p2/flags.veld`: a flagged `require`, `permit`, `forbid`,
//! `flow` and `ensure` on, off and with a failing provider, one reading per request and flag, a
//! request refused before the flags are read, and the client's `can_<endpoint>` and policy function
//! with and without the function that answers the flags.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use rooms_client::rooms::rooms::{Phase as ClientPhase, Room as ClientRoom, User as ClientUser};
use rooms_client::rooms::rooms_client::{can_archive_room, can_book_room, can_publish_room};
use rooms_client::rooms::rooms_policies::manage as client_manage;
use rooms_server::rooms::rooms::{Phase, Room, User};
use rooms_server::rooms::rooms_server::{
    ArchiveRoomFacts, ArchiveRoomRequest, BookRoomFacts, BookRoomRequest, PublishRoomFacts, PublishRoomRequest, ResizeRoomFacts,
    ResizeRoomRequest, RoomsFactLoaders, RoomsHandler, RoomsHooks, RoomsService,
};
use rooms_server::veld::Error;
use rooms_server::veld::flags::{FlagContext, FlagProvider};
use rooms_server::veld::rules::Transition;
use rooms_server::veld::server::RequestContext;
use serde_json::Value;

const ID: &str = "00000000-0000-4000-8000-000000000001";

/// A provider that counts its calls: the flags that are on, the ones that fail, and every reading.
#[derive(Default)]
struct Provider {
    on: Mutex<BTreeSet<String>>,
    failing: Mutex<bool>,
    /// `name` and the operation of the context, for each call.
    calls: Mutex<Vec<(String, String, bool)>>,
}

impl FlagProvider for Provider {
    async fn is_on<'a>(&'a self, name: &'a str, context: &'a FlagContext) -> Result<bool, Error> {
        self.calls.lock().unwrap().push((name.to_string(), context.operation.clone(), context.principal.is_some()));
        if *self.failing.lock().unwrap() {
            return Err(Error::other("the flag service is down"));
        }
        Ok(self.on.lock().unwrap().contains(name))
    }
}

struct World {
    rooms: Mutex<BTreeMap<String, Room>>,
    roles: Mutex<Vec<String>>,
    provider: Provider,
    errors: Mutex<usize>,
}

struct Loaders(Arc<World>);

impl RoomsFactLoaders for Loaders {
    async fn room(&self, key: String, _ctx: &RequestContext) -> Result<Option<Room>, Error> {
        Ok(self.0.rooms.lock().unwrap().get(&key).cloned())
    }
}

struct Impl;

impl RoomsService for Impl {
    async fn book_room(&self, _request: BookRoomRequest, facts: BookRoomFacts, _ctx: &RequestContext) -> Result<Room, Error> {
        Ok(facts.room)
    }

    async fn publish_room(&self, _r: PublishRoomRequest, facts: PublishRoomFacts, t: Transition<Phase>, _ctx: &RequestContext) -> Result<Room, Error> {
        Ok(Room { phase: t.to, ..facts.room })
    }

    async fn archive_room(&self, _r: ArchiveRoomRequest, facts: ArchiveRoomFacts, t: Transition<Phase>, _ctx: &RequestContext) -> Result<Room, Error> {
        Ok(Room { phase: t.to, ..facts.room })
    }

    async fn resize_room(&self, _request: ResizeRoomRequest, facts: ResizeRoomFacts, _ctx: &RequestContext) -> Result<Room, Error> {
        Ok(facts.room)
    }
}

struct Hooks(Arc<World>);

impl RoomsHooks for Hooks {
    type Flags = Provider;

    fn flags(&self) -> &Provider {
        &self.0.provider
    }

    async fn auth_bearer<'a>(&'a self, token: &'a str, _ctx: &'a RequestContext) -> Option<User> {
        (token != "bad").then(|| User { id: ID.into(), roles: self.0.roles.lock().unwrap().clone() })
    }

    fn on_error(&self, _err: &Error, _ctx: &RequestContext) {
        *self.0.errors.lock().unwrap() += 1;
    }
}

struct Server {
    base: String,
    w: Arc<World>,
}

async fn serve() -> Server {
    let w = Arc::new(World {
        rooms: Mutex::default(),
        roles: Mutex::new(vec!["staff".into()]),
        provider: Provider::default(),
        errors: Mutex::new(0),
    });
    let handler = RoomsHandler::create(Impl, Hooks(w.clone()), Loaders(w.clone()), None).unwrap();
    let router = rooms_server::adapters::axum::router(handler);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Server { base, w }
}

fn room(open: bool, capacity: i32, phase: Phase) -> Room {
    Room { id: ID.into(), open, capacity, phase }
}

fn turn(s: &Server, on: &[&str]) {
    *s.w.provider.on.lock().unwrap() = on.iter().map(ToString::to_string).collect();
}

fn roles(s: &Server, roles: &[&str]) {
    *s.w.roles.lock().unwrap() = roles.iter().map(ToString::to_string).collect();
}

struct Answer {
    status: u16,
    doc: Value,
}

async fn call(s: &Server, route: &str, token: &str) -> Answer {
    let response = reqwest::Client::new()
        .post(format!("{}/rooms/{ID}/{route}", s.base))
        .header("authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    let text = response.text().await.unwrap();
    Answer { status, doc: serde_json::from_str(&text).unwrap_or(Value::Null) }
}

fn reset(s: &Server) {
    s.w.provider.calls.lock().unwrap().clear();
}

// ---- require, on, off and failing -----------------------------------------------------------------

/// `require room.open when flag("openOnly")`: off, the statement is not there; on, it is an ordinary
/// `require` (a closed room is `422`); a failing provider counts it as on, and the failure goes to
/// `on_error`.
async fn a_flagged_require_is_off_on_or_applies_when_the_provider_fails() {
    let s = serve().await;
    s.w.rooms.lock().unwrap().insert(ID.into(), room(false, 4, Phase::Draft));
    assert_eq!(call(&s, "book", "t").await.status, 200, "off: the statement is not there");
    turn(&s, &["openOnly"]);
    assert_eq!(call(&s, "book", "t").await.status, 422, "on: a closed room is refused");
    s.w.rooms.lock().unwrap().insert(ID.into(), room(true, 4, Phase::Draft));
    assert_eq!(call(&s, "book", "t").await.status, 200, "on: an open room passes");
    turn(&s, &[]);
    *s.w.provider.failing.lock().unwrap() = true;
    s.w.rooms.lock().unwrap().insert(ID.into(), room(false, 4, Phase::Draft));
    let before = *s.w.errors.lock().unwrap();
    assert_eq!(call(&s, "book", "t").await.status, 422, "failing: a require counts as on");
    assert!(*s.w.errors.lock().unwrap() > before, "the failure goes to on_error");
}

// ---- permit and forbid -------------------------------------------------------------------------------

/// A `permit` with a flag exists while the flag is on and not when the provider fails; a `forbid`
/// with a flag applies while it is on and also when the provider fails.
async fn a_flagged_permit_and_forbid_on_off_and_failing() {
    let s = serve().await;
    s.w.rooms.lock().unwrap().insert(ID.into(), room(true, 4, Phase::Draft));
    // A beta user is only permitted by the flagged clause.
    roles(&s, &["beta"]);
    assert_eq!(call(&s, "book", "t").await.status, 403, "betaDesk off: the permit is not there");
    turn(&s, &["betaDesk"]);
    assert_eq!(call(&s, "book", "t").await.status, 200, "betaDesk on: the permit grants");
    *s.w.provider.failing.lock().unwrap() = true;
    assert_eq!(call(&s, "book", "t").await.status, 403, "a failing provider: a permit counts as off");
    *s.w.provider.failing.lock().unwrap() = false;
    // A suspended staff member is stopped by the flagged forbid only while its flag is on.
    roles(&s, &["staff", "suspended"]);
    turn(&s, &[]);
    assert_eq!(call(&s, "book", "t").await.status, 200, "lockSuspended off: the forbid is not there");
    turn(&s, &["lockSuspended"]);
    let on = call(&s, "book", "t").await;
    assert_eq!((on.status, on.doc["data"]["message"].clone()), (403, "suspended".into()), "{}", on.doc);
    turn(&s, &[]);
    *s.w.provider.failing.lock().unwrap() = true;
    assert_eq!(call(&s, "book", "t").await.status, 403, "a failing provider: a forbid counts as on");
}

// ---- one reading ---------------------------------------------------------------------------------------

/// Each flag an endpoint can reach is read once per request, after authentication, with the
/// principal and the operation signature in the context; a request refused before that point reads
/// nothing, and an endpoint does not read a flag it cannot reach.
async fn each_flag_is_read_once_per_request_after_authentication() {
    let s = serve().await;
    s.w.rooms.lock().unwrap().insert(ID.into(), room(true, 4, Phase::Draft));
    assert_eq!(call(&s, "book", "t").await.status, 200);
    let calls = s.w.provider.calls.lock().unwrap().clone();
    let names: Vec<&str> = calls.iter().map(|(n, _, _)| n.as_str()).collect();
    assert_eq!(names, vec!["betaDesk", "lockSuspended", "openOnly"], "once each, sorted: {calls:?}");
    assert!(calls.iter().all(|(_, op, principal)| op.starts_with("veld:") && op.contains("Rooms.bookRoom") && *principal), "{calls:?}");
    reset(&s);
    assert_eq!(call(&s, "book", "bad").await.status, 401);
    assert!(s.w.provider.calls.lock().unwrap().is_empty(), "refused before the flags are read");
    reset(&s);
    assert_eq!(call(&s, "publish", "t").await.status, 200);
    let names: Vec<String> = s.w.provider.calls.lock().unwrap().iter().map(|(n, _, _)| n.clone()).collect();
    assert_eq!(names, vec!["archiving", "betaDesk", "lockSuspended"], "the edge and the policy, not `openOnly`");
}

// ---- the flow ----------------------------------------------------------------------------------------

/// `flow live -> archived when flag("archiving")` is not an edge while the flag is off or the
/// provider fails: the guard refuses with `InvalidTransition`; on, the move is taken.
async fn a_flagged_flow_edge_is_an_edge_only_while_its_flag_is_on() {
    let s = serve().await;
    s.w.rooms.lock().unwrap().insert(ID.into(), room(true, 4, Phase::Live));
    let off = call(&s, "archive", "t").await;
    assert_eq!(off.status, 409, "{}", off.doc);
    turn(&s, &["archiving"]);
    assert_eq!(call(&s, "archive", "t").await.status, 200);
    turn(&s, &[]);
    *s.w.provider.failing.lock().unwrap() = true;
    assert_eq!(call(&s, "archive", "t").await.status, 409, "a failing provider counts a flow as off");
}

// ---- the flagged ensure and its old(...) operand ---------------------------------------------------------

/// With the flag off the flagged `ensure` is not there, and neither is the `old(...)` operand only it
/// reads: a room of capacity 0 (the operand divides by it) is answered; with the flag on the
/// evaluation error fails the `ensure` and the answer is `500`.
async fn a_flagged_ensure_that_is_off_does_not_evaluate_its_old_operand() {
    let s = serve().await;
    s.w.rooms.lock().unwrap().insert(ID.into(), room(true, 0, Phase::Draft));
    assert_eq!(call(&s, "resize", "t").await.status, 200, "off: nothing is evaluated");
    turn(&s, &["capacityGuard"]);
    assert_eq!(call(&s, "resize", "t").await.status, 500, "on: the operand raises an evaluation error");
    s.w.rooms.lock().unwrap().insert(ID.into(), room(true, 4, Phase::Draft));
    assert_eq!(call(&s, "resize", "t").await.status, 200, "on, a capacity that divides");
}

// ---- the client --------------------------------------------------------------------------------------------

fn client_room(phase: ClientPhase) -> ClientRoom {
    ClientRoom { id: ID.into(), open: true, capacity: 4, phase }
}

/// `can_<endpoint>` and the policy function take the function that answers the flags: with it they
/// decide like a server that has the provider; without it they decide as a failing provider does (a
/// flagged `permit` and `flow` are off, a flagged `forbid` is on).
fn the_client_decides_with_and_without_the_flag_function() {
    let beta = ClientUser { id: ID.into(), roles: vec!["beta".into()] };
    let staff = ClientUser { id: ID.into(), roles: vec!["staff".into()] };
    let suspended = ClientUser { id: ID.into(), roles: vec!["staff".into(), "suspended".into()] };
    let room = client_room(ClientPhase::Draft);
    let on = |name: &str| name == "betaDesk";
    assert!(can_book_room(&room, &beta, Some(&on)), "the flag function says betaDesk is on");
    assert!(!can_book_room(&room, &beta, None), "without it a flagged permit is off");
    assert!(!can_book_room(&room, &beta, Some(&|_: &str| false)));
    assert!(can_book_room(&room, &staff, None), "an unflagged permit still grants");
    assert!(!can_book_room(&room, &suspended, None), "without it a flagged forbid applies");
    assert!(can_book_room(&room, &suspended, Some(&|_: &str| false)), "the flag function turns the forbid off");
    assert!(client_manage(&beta, &room, Some(&on)));
    assert!(!client_manage(&beta, &room, None));
    // The edge of `archiveRoom` is flagged: a live room can be archived only when the flag is on.
    let live = client_room(ClientPhase::Live);
    assert!(can_archive_room(&live, &staff, Some(&|n: &str| n == "archiving")));
    assert!(!can_archive_room(&live, &staff, None), "without the function a flagged flow is off");
    assert!(can_publish_room(&room, &staff, None));
    // `next_states` lists the flagged edge too: the server still decides.
    assert_eq!(ClientPhase::Live.next_states(), &[ClientPhase::Archived]);
}

#[tokio::main]
async fn main() {
    a_flagged_require_is_off_on_or_applies_when_the_provider_fails().await;
    a_flagged_permit_and_forbid_on_off_and_failing().await;
    each_flag_is_read_once_per_request_after_authentication().await;
    a_flagged_flow_edge_is_an_edge_only_while_its_flag_is_on().await;
    a_flagged_ensure_that_is_off_does_not_evaluate_its_old_operand().await;
    the_client_decides_with_and_without_the_flag_function();
    println!("flags ok");
}
