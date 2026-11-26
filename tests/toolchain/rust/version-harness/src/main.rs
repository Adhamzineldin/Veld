//! `@version` over HTTP (`spec/rules.md` sections 11.7 to 11.12): the production handlers
//! generated from `tests/rules/endpoints/version.veld.pending`, with application-written fact
//! loaders; the `version` the handler receives, the `ETag`, `412` and the malformed `If-Match`,
//! the order of the checks, and what the generated client sends for `if_match`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rooms_client::rooms::rooms_client::{GetRoomRequest as ClientGetRoom, RenameRequest as ClientRename, RoomsClient, TickRequest as ClientTick};
use rooms_client::veld::client::{ClientOptions, HttpRequest, HttpResponse, Transport};
use rooms_client::veld::Error as ClientError;
use rooms_client::veld::conditional::IfMatch;
use rooms_server::rooms::orders_server::{
    OrdersFactLoaders, OrdersHandler, OrdersHooks, OrdersService, PayOrderFacts, PayOrderRequest,
};
use rooms_server::rooms::rooms::{Counter, Order, Room, Status, User};
use rooms_server::rooms::rooms_server::{
    GetRoomRequest, RenameFacts, RenameRequest, RoomsFactLoaders, RoomsHandler, RoomsHooks,
    RoomsService, TickFacts, TickRequest, TouchFacts, TouchRequest,
};
use rooms_server::veld::rules::{Outcome, Position, RuleEvent, Transition};
use rooms_server::veld::server::RequestContext;
use rooms_server::veld::Error;
use serde_json::{Value, json};

const ID: &str = "00000000-0000-4000-8000-000000000001";
const IF_MATCH_MESSAGE: &str = "must be * or one strong entity tag holding a version, such as \"3\"";

#[derive(Default)]
struct Db {
    rooms: Mutex<BTreeMap<String, Room>>,
    counters: Mutex<BTreeMap<String, Counter>>,
    orders: Mutex<BTreeMap<String, Order>>,
    /// What the handlers received, in order: `rename:Some(4)`.
    calls: Mutex<Vec<String>>,
    /// How many facts were loaded.
    loads: Mutex<usize>,
}

fn note(db: &Db, what: String) {
    db.calls.lock().unwrap().push(what);
}

fn bump(db: &Db) {
    *db.loads.lock().unwrap() += 1;
}

struct Loaders(Arc<Db>);

impl RoomsFactLoaders for Loaders {
    async fn counter(&self, key: String, _ctx: &RequestContext) -> Result<Option<Counter>, Error> {
        bump(&self.0);
        Ok(self.0.counters.lock().unwrap().get(&key).cloned())
    }

    async fn maybe(&self, key: String, _ctx: &RequestContext) -> Result<Option<Room>, Error> {
        bump(&self.0);
        Ok(self.0.rooms.lock().unwrap().get(&key).cloned())
    }

    async fn room(&self, key: String, _ctx: &RequestContext) -> Result<Option<Room>, Error> {
        bump(&self.0);
        Ok(self.0.rooms.lock().unwrap().get(&key).cloned())
    }
}

impl OrdersFactLoaders for Loaders {
    async fn order(&self, key: String, _ctx: &RequestContext) -> Result<Option<Order>, Error> {
        bump(&self.0);
        Ok(self.0.orders.lock().unwrap().get(&key).cloned())
    }
}

struct Impl(Arc<Db>);

impl RoomsService for Impl {
    async fn rename(
        &self,
        request: RenameRequest,
        facts: RenameFacts,
        version: Option<i64>,
        _ctx: &RequestContext,
    ) -> Result<Room, Error> {
        note(&self.0, format!("rename:{version:?}"));
        let mut room = facts.room;
        room.name = request.body.name;
        room.revision += 1;
        self.0.rooms.lock().unwrap().insert(room.id.clone(), room.clone());
        Ok(room)
    }

    async fn touch(
        &self,
        request: TouchRequest,
        facts: TouchFacts,
        version: Option<i64>,
        _ctx: &RequestContext,
    ) -> Result<Room, Error> {
        note(&self.0, format!("touch:{version:?}"));
        Ok(facts.maybe.unwrap_or(Room { id: request.id, name: "new".into(), revision: 1 }))
    }

    async fn tick(
        &self,
        _request: TickRequest,
        facts: TickFacts,
        version: Option<i32>,
        _ctx: &RequestContext,
    ) -> Result<Counter, Error> {
        note(&self.0, format!("tick:{version:?}"));
        let mut counter = facts.counter;
        counter.version += 1;
        Ok(counter)
    }

    async fn get_room(&self, request: GetRoomRequest, _ctx: &RequestContext) -> Result<Room, Error> {
        note(&self.0, "get".to_string());
        self.0.rooms.lock().unwrap().get(&request.id).cloned().ok_or_else(|| Error::other("no room"))
    }
}

impl OrdersService for Impl {
    async fn pay_order(
        &self,
        _request: PayOrderRequest,
        facts: PayOrderFacts,
        transition: Transition<Status>,
        version: Option<i64>,
        _ctx: &RequestContext,
    ) -> Result<Order, Error> {
        note(&self.0, format!("pay:{version:?}"));
        let mut order = facts.order;
        order.status = transition.to;
        order.revision += 1;
        Ok(order)
    }
}

type Events = Arc<Mutex<Vec<RuleEvent>>>;

struct Hooks {
    events: Events,
    validate: bool,
}

impl RoomsHooks for Hooks {
    fn validate_requests(&self) -> bool {
        self.validate
    }

    fn on_rule_violation(&self, event: &RuleEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

impl OrdersHooks for Hooks {
    async fn auth_bearer<'a>(&'a self, token: &'a str, _ctx: &'a RequestContext) -> Option<User> {
        match token {
            "staff" => Some(User { id: "u-1".into(), roles: vec!["staff".into()] }),
            "plain" => Some(User { id: "u-2".into(), roles: vec![] }),
            _ => None,
        }
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
    let rooms = RoomsHandler::create(
        Impl(db.clone()),
        Hooks { events: events.clone(), validate },
        Loaders(db.clone()),
    );
    let orders = OrdersHandler::create(
        Impl(db.clone()),
        Hooks { events: events.clone(), validate },
        Loaders(db.clone()),
    );
    let router = rooms_server::adapters::axum::router(rooms).merge(rooms_server::adapters::axum::router(orders));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Server { base, db, events }
}

fn put_room(s: &Server, revision: i64) {
    let room = Room { id: ID.into(), name: "Red".into(), revision };
    s.db.rooms.lock().unwrap().insert(ID.into(), room);
}

fn put_counter(s: &Server, version: i32) {
    s.db.counters.lock().unwrap().insert(ID.into(), Counter { id: ID.into(), total: 0, version });
}

fn put_order(s: &Server, status: Status, revision: i64) {
    s.db.orders.lock().unwrap().insert(ID.into(), Order { id: ID.into(), status, revision });
}

fn calls(s: &Server) -> Vec<String> {
    s.db.calls.lock().unwrap().clone()
}

struct Answer {
    status: u16,
    etag: Option<String>,
    doc: Value,
}

async fn send(
    s: &Server,
    method: reqwest::Method,
    path: &str,
    if_match: Option<&str>,
    bearer: Option<&str>,
    body: Option<Value>,
) -> Answer {
    let mut req = reqwest::Client::new().request(method, format!("{}{path}", s.base));
    if let Some(tag) = if_match {
        req = req.header("if-match", tag);
    }
    if let Some(token) = bearer {
        req = req.header("authorization", format!("Bearer {token}"));
    }
    if let Some(body) = body {
        req = req.header("content-type", "application/json").body(body.to_string());
    }
    let response = req.send().await.unwrap();
    let status = response.status().as_u16();
    let etag = response.headers().get("etag").map(|v| v.to_str().unwrap().to_string());
    let text = response.text().await.unwrap();
    Answer { status, etag, doc: serde_json::from_str(&text).unwrap_or(Value::Null) }
}

async fn rename(s: &Server, if_match: Option<&str>) -> Answer {
    let path = format!("/rooms/{ID}");
    send(s, reqwest::Method::PUT, &path, if_match, None, Some(json!({ "name": "Blue" }))).await
}

async fn pay(s: &Server, if_match: Option<&str>, token: &str) -> Answer {
    let path = format!("/orders/{ID}/pay");
    send(s, reqwest::Method::POST, &path, if_match, Some(token), Some(json!({ "text": "ok" }))).await
}

fn assert_stale(a: &Answer, expected: &str, actual: &str) {
    assert_eq!(a.status, 412, "{}", a.doc);
    assert_eq!(a.doc["title"], "PreconditionFailed");
    assert_eq!(a.doc["data"], json!({ "expected": expected, "actual": actual }));
    let all = a.doc["violations"].as_array().unwrap();
    assert_eq!(all.len(), 1, "{}", a.doc);
    assert!(all[0]["rule"].as_str().unwrap().contains(".version@"), "{}", all[0]);
    assert_eq!(all[0]["path"], "request");
    assert_eq!(all[0]["fields"], json!([]));
    assert_eq!(all[0]["message"], "violates rule version");
    assert!(all[0].get("error").is_none());
    assert!(a.etag.is_none(), "an error has no ETag");
}

// ---- scenarios -------------------------------------------------------------------------------

/// A tag that matches, a star and no header: what the handler receives and the `ETag` of the answer.
async fn matching_star_and_absent() {
    let s = serve(true).await;
    put_room(&s, 4);
    let a = rename(&s, Some("\"4\"")).await;
    assert_eq!(a.status, 200, "{}", a.doc);
    assert_eq!(a.doc["revision"], "5");
    assert_eq!(a.etag.as_deref(), Some("\"5\""), "the ETag is the version of the returned value");
    let a = rename(&s, Some("*")).await;
    assert_eq!((a.status, a.etag.as_deref()), (200, Some("\"6\"")));
    let a = rename(&s, None).await;
    assert_eq!((a.status, a.etag.as_deref()), (200, Some("\"7\"")));
    // Only a tag that was compared is handed over.
    assert_eq!(calls(&s), ["rename:Some(4)", "rename:None", "rename:None"]);
    // Version zero is a version; so is the largest long.
    put_room(&s, 0);
    assert_eq!(rename(&s, Some("\"0\"")).await.status, 200);
    put_room(&s, i64::MAX - 1);
    assert_eq!(rename(&s, Some("\"9223372036854775806\"")).await.status, 200);
}

/// A stale tag: `412`, its body, one violation, no handler, no `ETag`, and the hook is told.
async fn stale_tag() {
    let s = serve(true).await;
    put_room(&s, 4);
    let a = rename(&s, Some("\"3\"")).await;
    assert_stale(&a, "3", "4");
    // A tag ahead of the row is refused too.
    assert_stale(&rename(&s, Some("\"9\"")).await, "9", "4");
    assert!(calls(&s).is_empty(), "the handler did not run");
    let heard = s.events.lock().unwrap().clone();
    assert_eq!(heard.len(), 2);
    for event in &heard {
        assert_eq!((event.position, event.outcome), (Position::Input, Outcome::Rejected));
        assert!(event.operation.contains("rename"));
    }
    // The room did not change.
    assert_eq!(s.db.rooms.lock().unwrap()[ID].revision, 4);
}

/// Every malformed form is `422` with the one issue of section 11.10 and no `violations`; the
/// request is refused at step 5, before any fact is loaded.
async fn malformed_tags() {
    for validate in [true, false] {
        let s = serve(validate).await;
        put_room(&s, 4);
        for bad in [
            "3",
            "\"03\"",
            "W/\"3\"",
            "\"3\", \"4\"",
            "\"\"",
            "\"-1\"",
            "\"+1\"",
            "\"1.5\"",
            "\"abc\"",
            "'3'",
            "**",
            "\"9223372036854775808\"",
        ] {
            let a = rename(&s, Some(bad)).await;
            assert_eq!(a.status, 422, "{bad}: {}", a.doc);
            assert_eq!(a.doc["title"], "ValidationError");
            assert_eq!(
                a.doc["data"],
                json!({ "issues": [{ "path": "header.If-Match", "code": "type", "message": IF_MATCH_MESSAGE }] }),
                "{bad}"
            );
            assert!(a.doc.get("violations").is_none(), "{bad}: it is not a rule");
        }
        assert_eq!(*s.db.loads.lock().unwrap(), 0, "no fact was loaded");
        assert!(calls(&s).is_empty());
        // `validate = false` switches off neither the syntax nor the guard.
        assert_stale(&rename(&s, Some("\"3\"")).await, "3", "4");
        assert_eq!(rename(&s, Some("\"4\"")).await.status, 200);
    }
}

/// An `int` version, an absent optional fact and an endpoint without a guard.
async fn int_optional_and_unguarded() {
    let s = serve(true).await;
    put_counter(&s, 2);
    assert_stale(&send(&s, reqwest::Method::POST, &format!("/rooms/{ID}/tick"), Some("\"1\""), None, None).await, "1", "2");
    let a = send(&s, reqwest::Method::POST, &format!("/rooms/{ID}/tick"), Some("\"2\""), None, None).await;
    assert_eq!(a.status, 200, "{}", a.doc);
    assert_eq!(a.doc["version"], 3, "an int is a number");
    assert_eq!(a.etag.as_deref(), Some("\"3\""));
    assert_eq!(calls(&s), ["tick:Some(2)"]);
    // The optional fact is absent: nothing to compare, the request passes, nothing is handed over.
    s.db.calls.lock().unwrap().clear();
    let a = send(&s, reqwest::Method::PATCH, &format!("/rooms/{ID}/touch"), Some("\"1\""), None, None).await;
    assert_eq!(a.status, 200, "{}", a.doc);
    assert_eq!(calls(&s), ["touch:None"]);
    // An endpoint without a guard ignores `If-Match`, even when it is not a tag; its answer has the ETag.
    put_room(&s, 7);
    let a = send(&s, reqwest::Method::GET, &format!("/rooms/{ID}"), Some("not even a tag"), None, None).await;
    assert_eq!(a.status, 200, "{}", a.doc);
    assert_eq!(a.etag.as_deref(), Some("\"7\""));
}

/// The order of step 6: the policy, then the version, then the transition.
async fn order_of_the_checks() {
    let s = serve(true).await;
    // The caller is not staff: `Forbidden`, whatever the tag and the state say.
    put_order(&s, Status::Paid, 3);
    let a = pay(&s, Some("\"1\""), "plain").await;
    assert_eq!((a.status, a.doc["title"].as_str()), (403, Some("Forbidden")), "{}", a.doc);
    // Staff, a stale tag and an illegal move: the version answers first.
    assert_stale(&pay(&s, Some("\"1\""), "staff").await, "1", "3");
    // Staff, the right tag, an illegal move: the transition answers.
    let a = pay(&s, Some("\"3\""), "staff").await;
    assert_eq!((a.status, a.doc["title"].as_str()), (409, Some("InvalidTransition")), "{}", a.doc);
    // Staff, the right tag, a legal move: the handler gets the transition and the version.
    put_order(&s, Status::Pending, 3);
    let a = pay(&s, Some("\"3\""), "staff").await;
    assert_eq!(a.status, 200, "{}", a.doc);
    assert_eq!(a.etag.as_deref(), Some("\"4\""));
    assert_eq!(calls(&s), ["pay:Some(3)"]);
}

/// A recording transport: what the generated client puts on the wire.
#[derive(Clone, Default)]
struct Wire(Arc<Mutex<Vec<HttpRequest>>>);

impl Transport for Wire {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse, ClientError> {
        self.0.lock().unwrap().push(req);
        Ok(HttpResponse {
            status: 200,
            headers: vec![],
            body: json!({ "id": ID, "name": "Blue", "revision": "5" }).to_string().into_bytes(),
        })
    }
}

fn sent(wire: &Wire) -> HttpRequest {
    wire.0.lock().unwrap().last().cloned().unwrap()
}

fn header<'a>(req: &'a HttpRequest, name: &str) -> Option<&'a str> {
    req.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

/// The client sends `If-Match` as a version and as `*`, and sends nothing without it; the same
/// header is not part of the body.
async fn client_headers() {
    let wire = Wire::default();
    let client = RoomsClient::new(wire.clone(), ClientOptions::new().base_url("http://x")).unwrap();
    let request = ClientRename {
        id: ID.into(),
        body: rooms_client::rooms::rooms::Rename { name: "Blue".into() },
        if_match: Some(IfMatch::Version(4)),
    };
    let room = client.rename(request.clone()).await.unwrap();
    assert_eq!(room.revision, 5);
    let req = sent(&wire);
    assert_eq!(header(&req, "If-Match"), Some("\"4\""));
    assert_eq!(serde_json::from_slice::<Value>(req.body.as_ref().unwrap()).unwrap(), json!({ "name": "Blue" }));
    client.rename(ClientRename { if_match: Some(IfMatch::Any), ..request.clone() }).await.unwrap();
    assert_eq!(header(&sent(&wire), "If-Match"), Some("*"));
    client.rename(ClientRename { if_match: None, ..request }).await.unwrap();
    assert_eq!(header(&sent(&wire), "If-Match"), None);
    // The `int` version of `Counter` is typed `i32`; a request without a guard has no field.
    let _ = ClientTick { id: ID.into(), if_match: Some(IfMatch::<i32>::Version(2)) };
    let _ = ClientGetRoom { id: ID.into() };
}

#[tokio::main]
async fn main() {
    matching_star_and_absent().await;
    stale_tag().await;
    malformed_tags().await;
    int_optional_and_unguarded().await;
    order_of_the_checks().await;
    client_headers().await;
    println!("version ok");
}
