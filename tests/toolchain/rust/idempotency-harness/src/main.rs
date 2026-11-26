//! `@idempotent` over HTTP (`spec/rules.md` sections 11.8 to 11.12): the production handler
//! generated from `tests/rules/endpoints/idempotency.veld.pending`, with an application-written
//! store; the key, the replay, the conflicts, what is stored, the scope of a key, a store that
//! fails, the fingerprint goldens of the reference and what the generated client sends.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use bookings_client::bookings::bookings::NewBooking as ClientNewBooking;
use bookings_client::bookings::bookings_client::{
    BookMineRequest as ClientBookMine, BookRequest as ClientBook, BookingsClient,
    PayRequest as ClientPay, ReplaceRequest as ClientReplace,
};
use bookings_client::veld::Error as ClientError;
use bookings_client::veld::client::{ClientOptions, HttpRequest, HttpResponse, Transport};
use bookings_server::bookings::bookings::{Booking, Room, Shelf};
use bookings_server::bookings::bookings_server::{
    BookFacts, BookMineRequest, BookRequest, BookingsFactLoaders, BookingsHandler, BookingsHooks,
    BookingsService, CancelRequest, PayRequest, ReplaceRequest, SaveRoomFacts, SaveRoomRequest,
};
use bookings_server::veld::Error;
use bookings_server::veld::idempotency::{
    Begin, IdempotencyStore, MemoryIdempotencyStore, StoredAnswer, fingerprint,
};
use bookings_server::veld::server::{Principal, RequestContext};
use serde_json::{Map, Value, json};

const ID: &str = "00000000-0000-4000-8000-000000000001";
/// A uuid for the `n`-th thing a handler made: the server checks its answers against the contract.
fn uid(n: usize) -> String {
    format!("00000000-0000-4000-8000-{n:012}")
}

const OTHER_MESSAGE: &str = "this idempotency key was used with other parameters";
const RUNNING_MESSAGE: &str = "a request with this idempotency key is still running";
const KEY_MESSAGE: &str = "must be 1 to 255 visible ASCII characters";

/// The store of the application: a memory store that says what it was asked and can be made to fail.
#[derive(Default)]
struct Store {
    inner: MemoryIdempotencyStore,
    /// `begin:<scope>|<key>|<fingerprint>`, `complete:<status>`, `release`, in order.
    log: Mutex<Vec<String>>,
    fail_begin: AtomicBool,
    fail_complete: AtomicBool,
    fail_release: AtomicBool,
}

struct StoreRef(Arc<Store>);

impl IdempotencyStore for StoreRef {
    fn begin<'a>(
        &'a self,
        scope: &'a str,
        key: &'a str,
        fingerprint: &'a str,
    ) -> impl std::future::Future<Output = Result<Begin, Error>> + Send + 'a {
        let s = &self.0;
        s.log.lock().unwrap().push(format!("begin:{scope}|{key}|{fingerprint}"));
        let found = if s.fail_begin.load(Ordering::SeqCst) {
            Err(Error::other("the store is down"))
        } else {
            s.inner.begin_now(scope, key, fingerprint)
        };
        std::future::ready(found)
    }

    fn complete<'a>(
        &'a self,
        scope: &'a str,
        key: &'a str,
        answer: StoredAnswer,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + 'a {
        let s = &self.0;
        s.log.lock().unwrap().push(format!("complete:{}", answer.status));
        let done = if s.fail_complete.load(Ordering::SeqCst) {
            Err(Error::other("cannot complete"))
        } else {
            s.inner.complete_now(scope, key, answer)
        };
        std::future::ready(done)
    }

    fn release<'a>(
        &'a self,
        scope: &'a str,
        key: &'a str,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + 'a {
        let s = &self.0;
        s.log.lock().unwrap().push("release".to_string());
        let done = if s.fail_release.load(Ordering::SeqCst) {
            Err(Error::other("cannot release"))
        } else {
            s.inner.release_now(scope, key)
        };
        std::future::ready(done)
    }
}

#[derive(Default)]
struct App {
    store: Arc<Store>,
    /// How many times each handler ran.
    runs: Mutex<BTreeMap<String, usize>>,
    loads: AtomicUsize,
    capacity: AtomicI32,
    /// The next `book` fails with an error that is not declared (`500`).
    fail_next: AtomicBool,
    /// `book` waits while this is on, and says when it has started.
    hold: AtomicBool,
    entered: AtomicBool,
    /// The next `book` panics.
    panic_next: AtomicBool,
    /// What `on_error` heard.
    errors: Mutex<Vec<String>>,
}

impl App {
    fn ran(&self, what: &str) -> usize {
        let mut runs = self.runs.lock().unwrap();
        let n = runs.entry(what.to_string()).or_default();
        *n += 1;
        *n
    }

    fn count(&self, what: &str) -> usize {
        self.runs.lock().unwrap().get(what).copied().unwrap_or(0)
    }
}

struct Impl(Arc<App>);

impl BookingsService for Impl {
    async fn book(&self, request: BookRequest, _facts: BookFacts, _ctx: &RequestContext) -> Result<Booking, Error> {
        let n = self.0.ran("book");
        self.0.entered.store(true, Ordering::SeqCst);
        while self.0.hold.load(Ordering::SeqCst) {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        if self.0.fail_next.swap(false, Ordering::SeqCst) {
            return Err(Error::other("boom"));
        }
        if self.0.panic_next.swap(false, Ordering::SeqCst) {
            panic!("the handler panics");
        }
        Ok(Booking { id: uid(n), guests: request.body.guests })
    }

    async fn pay(&self, request: PayRequest, _ctx: &RequestContext) -> Result<Booking, Error> {
        let n = self.0.ran("pay");
        Ok(Booking { id: uid(100 + n), guests: request.body.guests })
    }

    async fn replace(&self, request: ReplaceRequest, _ctx: &RequestContext) -> Result<Booking, Error> {
        let n = self.0.ran("replace");
        Ok(Booking { id: request.id, guests: request.body.guests + i32::try_from(n).unwrap() })
    }

    async fn cancel(&self, _request: CancelRequest, _ctx: &RequestContext) -> Result<(), Error> {
        self.0.ran("cancel");
        Ok(())
    }

    async fn book_mine(&self, request: BookMineRequest, _ctx: &RequestContext) -> Result<Booking, Error> {
        let n = self.0.ran("mine");
        Ok(Booking { id: uid(200 + n), guests: request.body.guests })
    }

    async fn save_room(
        &self,
        request: SaveRoomRequest,
        facts: SaveRoomFacts,
        _version: Option<i64>,
        _ctx: &RequestContext,
    ) -> Result<Shelf, Error> {
        self.0.ran("save");
        Ok(Shelf { id: facts.shelf.id, name: request.body.name, revision: facts.shelf.revision + 1 })
    }
}

struct Loaders(Arc<App>);

impl BookingsFactLoaders for Loaders {
    async fn room(&self, _ctx: &RequestContext) -> Result<Option<Room>, Error> {
        self.0.loads.fetch_add(1, Ordering::SeqCst);
        Ok(Some(Room { id: ID.into(), capacity: self.0.capacity.load(Ordering::SeqCst) }))
    }

    async fn shelf(&self, key: String, _ctx: &RequestContext) -> Result<Option<Shelf>, Error> {
        self.0.loads.fetch_add(1, Ordering::SeqCst);
        Ok(Some(Shelf { id: key, name: "Old".into(), revision: 7 }))
    }
}

struct Hooks(Arc<App>, StoreRef);

impl BookingsHooks for Hooks {
    type IdempotencyStore = StoreRef;

    fn idempotency_store(&self) -> &StoreRef {
        &self.1
    }

    /// The caller is the token the authentication hook accepted.
    fn idempotency_caller(&self, principal: &Principal, _ctx: &RequestContext) -> String {
        principal.get::<String>().cloned().unwrap_or_default()
    }

    async fn auth_bearer<'a>(&'a self, token: &'a str, _ctx: &'a RequestContext) -> Option<Principal> {
        Some(Principal::new(token.to_string()))
    }

    fn on_error(&self, err: &Error, _ctx: &RequestContext) {
        self.0.errors.lock().unwrap().push(err.to_string());
    }
}

struct Server {
    base: String,
    app: Arc<App>,
}

async fn serve() -> Server {
    let app = Arc::new(App::default());
    app.capacity.store(9, Ordering::SeqCst);
    let hooks = Hooks(app.clone(), StoreRef(app.store.clone()));
    let handler = BookingsHandler::create(Impl(app.clone()), hooks, Loaders(app.clone()), None).unwrap();
    let router = bookings_server::adapters::axum::router(handler);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Server { base, app }
}

#[derive(Debug)]
struct Answer {
    status: u16,
    headers: BTreeMap<String, String>,
    text: String,
    doc: Value,
}

impl Answer {
    fn replayed(&self) -> bool {
        self.headers.get("veld-idempotent-replay").map(String::as_str) == Some("true")
    }
}

async fn call(
    s: &Server,
    method: reqwest::Method,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<Value>,
) -> Answer {
    let mut req = reqwest::Client::new().request(method, format!("{}{path}", s.base));
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    if let Some(body) = body {
        req = req.header("content-type", "application/json").body(body.to_string());
    }
    let response = req.send().await.unwrap();
    let status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap().to_string()))
        .collect();
    let text = response.text().await.unwrap();
    let doc = serde_json::from_str(&text).unwrap_or(Value::Null);
    Answer { status, headers, text, doc }
}

async fn book(s: &Server, key: Option<&str>, guests: i32) -> Answer {
    let headers: Vec<(&str, &str)> = key.map(|k| ("idempotency-key", k)).into_iter().collect();
    call(s, reqwest::Method::POST, "/bookings", &headers, Some(json!({ "guests": guests }))).await
}

fn is_conflict(a: &Answer, message: &str) {
    assert_eq!(a.status, 409, "{}", a.text);
    assert_eq!(a.doc["title"], "Conflict");
    assert_eq!(a.doc["data"], json!({ "message": message }));
    assert!(a.doc.get("violations").is_none());
    assert!(!a.replayed());
}

fn log(s: &Server) -> Vec<String> {
    s.app.store.log.lock().unwrap().clone()
}

// ---- scenarios -------------------------------------------------------------------------------

/// A new key runs and stores; the same key and parameters replays the stored answer: same status,
/// same bytes, the replay header, the handler not run again and no fact loaded.
async fn new_key_then_replay() {
    let s = serve().await;
    let first = book(&s, Some("k-1"), 2).await;
    assert_eq!(first.status, 201, "{}", first.text);
    assert_eq!(first.doc, json!({ "id": uid(1), "guests": 2 }));
    assert!(!first.replayed(), "the first answer is not a replay");
    assert_eq!((s.app.count("book"), s.app.loads.load(Ordering::SeqCst)), (1, 1));
    let log1 = log(&s);
    assert_eq!(log1.len(), 2, "{log1:?}");
    // The scope is the operation signature; an endpoint without auth has no caller in it. The
    // fingerprint is the reference's golden for `{ "body": { "guests": 2 } }`.
    assert!(log1[0].starts_with("begin:veld:bookings/Bookings.book@"), "{}", log1[0]);
    assert!(!log1[0].contains('\n'), "{}", log1[0]);
    assert!(log1[0].ends_with("|k-1|74ff6d8fbb076cfc"), "{}", log1[0]);
    assert_eq!(log1[1], "complete:201");

    let again = book(&s, Some("k-1"), 2).await;
    assert!(again.replayed(), "{again:?}");
    assert_eq!(again.status, 201);
    assert_eq!(again.text, first.text, "the stored bytes");
    assert_eq!(again.headers.get("content-type"), first.headers.get("content-type"));
    assert_eq!((s.app.count("book"), s.app.loads.load(Ordering::SeqCst)), (1, 1), "nothing ran");
    assert_eq!(log(&s).len(), 3, "a replay asks the store once and stores nothing");

    // A request without a key is not idempotent: it runs every time and touches no store.
    let before = log(&s).len();
    assert_eq!(book(&s, None, 2).await.status, 201);
    assert_eq!(book(&s, None, 2).await.status, 201);
    assert_eq!((s.app.count("book"), log(&s).len()), (3, before));

    // Other parameters under another key give the other golden.
    book(&s, Some("k-2"), 3).await;
    assert!(log(&s).iter().any(|l| l.ends_with("|k-2|19b1b33ecd669a6f")), "{:?}", log(&s));
}

/// The same key with other parameters is `409`, and nothing runs.
async fn other_parameters() {
    let s = serve().await;
    assert_eq!(book(&s, Some("k"), 2).await.status, 201);
    let a = book(&s, Some("k"), 3).await;
    is_conflict(&a, OTHER_MESSAGE);
    assert_eq!((s.app.count("book"), s.app.loads.load(Ordering::SeqCst)), (1, 1));
}

/// A second request with the key while the first is running is `409`; with other parameters too.
/// Once the first has finished its answer is replayed.
async fn running() {
    let s = Arc::new(serve().await);
    s.app.hold.store(true, Ordering::SeqCst);
    let first = {
        let s = s.clone();
        tokio::spawn(async move { book(&s, Some("slow"), 1).await })
    };
    while !s.app.entered.load(Ordering::SeqCst) {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    is_conflict(&book(&s, Some("slow"), 1).await, RUNNING_MESSAGE);
    is_conflict(&book(&s, Some("slow"), 4).await, OTHER_MESSAGE);
    assert_eq!(s.app.count("book"), 1);
    s.app.hold.store(false, Ordering::SeqCst);
    let first = first.await.unwrap();
    assert_eq!(first.status, 201, "{}", first.text);
    let third = book(&s, Some("slow"), 1).await;
    assert!(third.replayed());
    assert_eq!(third.text, first.text);
    assert_eq!(s.app.count("book"), 1);
}

/// Sends `book` with a key and drops the request after `ms` milliseconds, as a client that gives up.
async fn book_and_hang_up(s: &Server, key: &str, ms: u64) {
    let sent = reqwest::Client::new()
        .post(format!("{}/bookings", s.base))
        .header("idempotency-key", key)
        .header("content-type", "application/json")
        .body(json!({ "guests": 1 }).to_string())
        .timeout(std::time::Duration::from_millis(ms))
        .send()
        .await;
    assert!(sent.is_err(), "the request was meant to be abandoned");
}

/// The same key and the same parameters, until the key is not "running" any more (the release is
/// handed to a task of its own, so it follows the drop by a moment).
async fn book_when_released(s: &Server, key: &str) -> Answer {
    for _ in 0..200 {
        let a = book(s, Some(key), 1).await;
        if a.status != 409 {
            return a;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("the key `{key}` stayed running");
}

/// A request whose future is dropped while the handler runs (the client hangs up) gives its key
/// back: the same key runs again instead of answering `409` (`spec/rules.md` section 11.12).
async fn a_dropped_request_releases_its_key() {
    let s = serve().await;
    s.app.hold.store(true, Ordering::SeqCst);
    book_and_hang_up(&s, "gone", 150).await;
    assert!(s.app.entered.load(Ordering::SeqCst), "the handler had started");
    s.app.hold.store(false, Ordering::SeqCst);
    let again = book_when_released(&s, "gone").await;
    assert_eq!(again.status, 201, "{}", again.text);
    assert!(!again.replayed(), "nothing was stored by the abandoned request");
    assert_eq!(s.app.count("book"), 2);
    assert!(log(&s).iter().any(|l| l == "release"), "{:?}", log(&s));
}

/// A handler that panics gives the key back too.
async fn a_panicking_handler_releases_its_key() {
    let s = serve().await;
    s.app.panic_next.store(true, Ordering::SeqCst);
    let first = reqwest::Client::new()
        .post(format!("{}/bookings", s.base))
        .header("idempotency-key", "boom")
        .header("content-type", "application/json")
        .body(json!({ "guests": 1 }).to_string())
        .send()
        .await;
    assert!(first.is_err() || first.is_ok_and(|r| r.status().as_u16() >= 500));
    let again = book_when_released(&s, "boom").await;
    assert_eq!(again.status, 201, "{}", again.text);
    assert_eq!(s.app.count("book"), 2);
}

/// A declared error is stored and replayed (the room is full: `RoomFull`, from step 6).
async fn declared_error() {
    let s = serve().await;
    s.app.capacity.store(1, Ordering::SeqCst);
    let first = book(&s, Some("full"), 2).await;
    assert_eq!((first.status, first.doc["title"].as_str()), (409, Some("RoomFull")), "{}", first.text);
    assert!(!first.replayed());
    s.app.capacity.store(99, Ordering::SeqCst);
    let again = book(&s, Some("full"), 2).await;
    assert!(again.replayed(), "{again:?}");
    assert_eq!((again.status, again.text.as_str()), (409, first.text.as_str()));
    assert_eq!(s.app.loads.load(Ordering::SeqCst), 1, "no fact is loaded for a replay");
    assert_eq!(s.app.count("book"), 0, "the handler never ran");
}

/// A `500` is not stored: the entry is released and the next request with the key runs.
async fn five_hundred_is_not_stored() {
    let s = serve().await;
    s.app.fail_next.store(true, Ordering::SeqCst);
    let first = book(&s, Some("f"), 2).await;
    assert_eq!(first.status, 500, "{}", first.text);
    assert_eq!(log(&s).last().map(String::as_str), Some("release"));
    let second = book(&s, Some("f"), 2).await;
    assert_eq!(second.status, 201, "{}", second.text);
    assert!(!second.replayed());
    assert_eq!(s.app.count("book"), 2, "the second request ran");
    assert!(book(&s, Some("f"), 2).await.replayed(), "and its answer is stored");
}

/// The key is checked at step 5: a required one that is missing, one that is not a key, and a
/// header issue next to a rule violation. Nothing runs, the store is not asked.
async fn the_key_is_checked() {
    let s = serve().await;
    let pay = |headers: Vec<(&'static str, String)>, guests: i32| {
        let s = &s;
        async move {
            let h: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
            call(s, reqwest::Method::POST, "/bookings/paid", &h, Some(json!({ "guests": guests }))).await
        }
    };
    let a = pay(vec![], 1).await;
    assert_eq!(a.status, 422, "{}", a.text);
    assert_eq!(
        a.doc["data"],
        json!({ "issues": [{ "path": "header.Idempotency-Key", "code": "required", "message": "is required" }] })
    );
    assert!(a.doc.get("violations").is_none());
    for bad in ["", "two words", &"k".repeat(256)] {
        let a = pay(vec![("idempotency-key", bad.to_string())], 1).await;
        assert_eq!(a.status, 422, "{bad:?}: {}", a.text);
        assert_eq!(
            a.doc["data"],
            json!({ "issues": [{ "path": "header.Idempotency-Key", "code": "type", "message": KEY_MESSAGE }] })
        );
    }
    // A key of the longest legal length is a key.
    assert_eq!(pay(vec![("idempotency-key", "k".repeat(255))], 1).await.status, 201);
    let before = log(&s).len();
    // Header issues come first; the rule violation stays, and it is the only violation.
    let a = pay(vec![], 0).await;
    assert_eq!(a.status, 422, "{}", a.text);
    let issues = a.doc["data"]["issues"].as_array().unwrap();
    assert_eq!(issues.len(), 2, "{}", a.text);
    assert_eq!(issues[0]["path"], "header.Idempotency-Key");
    assert_eq!(issues[1]["code"], "rule");
    assert_eq!(a.doc["violations"].as_array().unwrap().len(), 1);
    assert_eq!(log(&s).len(), before, "a request that is not valid never reaches the store");
    assert_eq!(s.app.count("pay"), 1, "only the valid one ran");
    // The key of an endpoint with a header of its own is read from that header.
    let path = format!("/bookings/{ID}");
    let put = |name: &'static str| {
        let (s, path) = (&s, path.clone());
        async move {
            call(s, reqwest::Method::PUT, &path, &[(name, "x")], Some(json!({ "guests": 1 }))).await
        }
    };
    assert_eq!(put("idempotency-key").await.status, 200);
    assert!(!put("idempotency-key").await.replayed(), "the other header is ignored");
    assert_eq!(put("x-request-id").await.status, 200);
    assert!(put("x-request-id").await.replayed());
    assert_eq!(s.app.count("replace"), 3, "two runs under the ignored header, one under the key header");
}

/// Two callers with the same key do not replay each other: the scope has the caller in it.
async fn callers_do_not_share_keys() {
    let s = serve().await;
    let mine = |token: &'static str| {
        let s = &s;
        async move {
            let bearer = format!("Bearer {token}");
            call(
                s,
                reqwest::Method::POST,
                "/bookings/mine",
                &[("authorization", &bearer), ("idempotency-token", "k")],
                Some(json!({ "guests": 2 })),
            )
            .await
        }
    };
    let alice = mine("alice").await;
    assert_eq!((alice.status, alice.replayed()), (201, false), "{}", alice.text);
    let bob = mine("bob").await;
    assert_eq!((bob.status, bob.replayed()), (201, false), "{}", bob.text);
    assert_ne!(alice.text, bob.text);
    assert!(mine("alice").await.replayed());
    assert_eq!(s.app.count("mine"), 2);
    let log = log(&s);
    assert!(log.iter().any(|l| l.contains("Bookings.bookMine@") && l.contains("\nalice|k|")), "{log:?}");
    assert!(log.iter().any(|l| l.contains("Bookings.bookMine@") && l.contains("\nbob|k|")), "{log:?}");
}

/// The `ETag` of a stored success is replayed with it; a `204` replays without a body.
async fn etag_and_no_content() {
    let s = serve().await;
    let path = format!("/bookings/rooms/{ID}");
    let save = || {
        call(&s, reqwest::Method::PUT, &path, &[("idempotency-key", "s")], Some(json!({ "name": "New" })))
    };
    let first = save().await;
    assert_eq!(first.status, 200, "{}", first.text);
    assert_eq!(first.headers.get("etag").map(String::as_str), Some("\"8\""));
    let again = save().await;
    assert!(again.replayed());
    assert_eq!(again.headers.get("etag").map(String::as_str), Some("\"8\""));
    assert_eq!(again.text, first.text);
    assert_eq!((s.app.count("save"), s.app.loads.load(Ordering::SeqCst)), (1, 1));

    let gone = format!("/bookings/{ID}");
    let cancel = || call(&s, reqwest::Method::DELETE, &gone, &[("idempotency-key", "c")], None);
    let first = cancel().await;
    assert_eq!((first.status, first.text.as_str()), (204, ""));
    let again = cancel().await;
    assert_eq!((again.status, again.text.as_str(), again.replayed()), (204, "", true));
    assert_eq!(s.app.count("cancel"), 1);
}

/// A store that fails: `begin` is `500` and the handler does not run; `complete` and `release` do
/// not change the answer, and the failure goes to `on_error`.
async fn a_store_that_fails() {
    let s = serve().await;
    s.app.store.fail_begin.store(true, Ordering::SeqCst);
    let a = book(&s, Some("down"), 2).await;
    assert_eq!(a.status, 500, "{}", a.text);
    assert_eq!(s.app.count("book"), 0, "running without the store could run it twice");
    assert!(s.app.errors.lock().unwrap().iter().any(|e| e.contains("the idempotency store failed")));
    s.app.store.fail_begin.store(false, Ordering::SeqCst);

    s.app.store.fail_complete.store(true, Ordering::SeqCst);
    let a = book(&s, Some("c"), 2).await;
    assert_eq!(a.status, 201, "the answer is sent as it is: {}", a.text);
    assert!(s.app.errors.lock().unwrap().iter().any(|e| e == "cannot complete"));
    s.app.store.fail_complete.store(false, Ordering::SeqCst);

    s.app.store.fail_release.store(true, Ordering::SeqCst);
    s.app.fail_next.store(true, Ordering::SeqCst);
    let a = book(&s, Some("r"), 2).await;
    assert_eq!(a.status, 500, "{}", a.text);
    assert!(s.app.errors.lock().unwrap().iter().any(|e| e == "cannot release"));
}

/// Of many concurrent `begin` calls for a key the store does not know, exactly one is `Started`.
async fn begin_is_atomic() {
    let store = Arc::new(MemoryIdempotencyStore::new());
    let mut tasks = Vec::new();
    for _ in 0..32 {
        let store = store.clone();
        tasks.push(tokio::spawn(async move { store.begin("scope", "k", "fp").await.unwrap() }));
    }
    let mut started = 0;
    for task in tasks {
        match task.await.unwrap() {
            Begin::Started => started += 1,
            Begin::Running => {}
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(started, 1);
    // Another fingerprint is `Other`, running or stored.
    assert_eq!(store.begin("scope", "k", "fp2").await.unwrap(), Begin::Other);
    let answer = StoredAnswer { status: 201, body: b"{}".to_vec(), content_type: None, etag: None, location: None };
    store.complete("scope", "k", answer.clone()).await.unwrap();
    assert_eq!(store.begin("scope", "k", "fp").await.unwrap(), Begin::Replay(answer));
    assert_eq!(store.begin("scope", "k", "fp2").await.unwrap(), Begin::Other);
    // Another scope is another key; a released key is unknown again.
    assert_eq!(store.begin("other-scope", "k", "fp").await.unwrap(), Begin::Started);
    store.release("other-scope", "k").await.unwrap();
    assert_eq!(store.begin("other-scope", "k", "fp").await.unwrap(), Begin::Started);
}

/// The goldens of the reference evaluator (`crates/veld-rules`), computed by the generated runtime.
fn fingerprint_goldens() {
    let of = |v: Value| -> String {
        let Value::Object(map) = v else { panic!("not an object") };
        fingerprint(&map)
    };
    assert_eq!(fingerprint(&Map::new()), "44136fa355b3678a");
    assert_eq!(of(json!({ "body": { "guests": 2 } })), "74ff6d8fbb076cfc");
    assert_eq!(of(json!({ "body": { "guests": 3 } })), "19b1b33ecd669a6f");
    // After the defaults, with an optional member left out, and with one that was sent.
    assert_eq!(of(json!({ "id": ID, "body": { "n": 1, "note": "none" } })), "bc508ee59e0a79b7");
    assert_eq!(
        of(json!({ "id": ID, "dry": true, "body": { "n": 1, "note": "none", "tag": "a" } })),
        "62c08a8b20a7323e"
    );
    // The order of the keys does not matter.
    assert_eq!(of(json!({ "b": 2, "a": "x" })), of(json!({ "a": "x", "b": 2 })));
}

/// A recording transport: what the generated client puts on the wire.
#[derive(Clone, Default)]
struct Wire(Arc<Mutex<Vec<HttpRequest>>>);

impl Transport for Wire {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse, ClientError> {
        self.0.lock().unwrap().push(req);
        Ok(HttpResponse {
            status: 201,
            headers: vec![],
            body: json!({ "id": ID, "guests": 2 }).to_string().into_bytes(),
        })
    }
}

fn sent(wire: &Wire) -> HttpRequest {
    wire.0.lock().unwrap().last().cloned().unwrap()
}

fn header<'a>(req: &'a HttpRequest, name: &str) -> Option<&'a str> {
    req.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

/// The client sends the key it is given in the header the annotation names, and never invents one;
/// a required key is required by the type.
async fn client_headers() {
    let wire = Wire::default();
    let client = BookingsClient::new(wire.clone(), ClientOptions::new().base_url("http://x")).unwrap();
    let body = ClientNewBooking { guests: 2 };
    client
        .book(ClientBook { body: body.clone(), idempotency_key: Some("k-1".into()) })
        .await
        .unwrap();
    let req = sent(&wire);
    assert_eq!(header(&req, "Idempotency-Key"), Some("k-1"));
    assert_eq!(serde_json::from_slice::<Value>(req.body.as_ref().unwrap()).unwrap(), json!({ "guests": 2 }));
    client.book(ClientBook { body: body.clone(), idempotency_key: None }).await.unwrap();
    assert_eq!(header(&sent(&wire), "Idempotency-Key"), None, "the client never invents a key");
    // `pay` has `required: true`: the field is a `String`, so the type requires it.
    client
        .pay(ClientPay { body: body.clone(), idempotency_key: "paid-1".into() })
        .await
        .unwrap();
    assert_eq!(header(&sent(&wire), "Idempotency-Key"), Some("paid-1"));
    // Headers of their own.
    client
        .replace(ClientReplace { id: ID.into(), body: body.clone(), idempotency_key: Some("r".into()) })
        .await
        .unwrap();
    assert_eq!(header(&sent(&wire), "X-Request-Id"), Some("r"));
    assert_eq!(header(&sent(&wire), "Idempotency-Key"), None);
    client
        .book_mine(ClientBookMine { body, idempotency_key: "t".into() })
        .await
        .unwrap();
    assert_eq!(header(&sent(&wire), "Idempotency-Token"), Some("t"));
}

#[tokio::main]
async fn main() {
    fingerprint_goldens();
    begin_is_atomic().await;
    new_key_then_replay().await;
    other_parameters().await;
    running().await;
    declared_error().await;
    five_hundred_is_not_stored().await;
    the_key_is_checked().await;
    callers_do_not_share_keys().await;
    etag_and_no_content().await;
    a_store_that_fails().await;
    a_dropped_request_releases_its_key().await;
    a_panicking_handler_releases_its_key().await;
    client_headers().await;
    println!("idempotency ok");
}
