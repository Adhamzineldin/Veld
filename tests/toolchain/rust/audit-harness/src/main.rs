//! The audit log over the production handler (`spec/platform.md` section 4), generated from
//! `tests/toolchain/contract/platform_p1/audit.veld`: a success, a body, a fact and the one reading of
//! the clock, a handler that fails and one that panics, a deadline, a `401`, an endpoint that is not
//! audited, a hook that fails, an event that is owed when the request future is dropped, and none
//! when it is dropped before the answer was decided. With `validate = false` too. The program prints
//! `audit http ok`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

use bookings_server::bookings::bookings::{Note, User};
use bookings_server::bookings::notes_server::{
    AddNoteRequest, BoomRequest, GetNoteRequest, NotesDispatch, NotesFactLoaders, NotesHandler, NotesHooks, NotesService,
    QuietRequest, SlowRequest, ViaFactFacts, ViaFactRequest,
};
use bookings_server::veld::Error;
use bookings_server::veld::platform::AuditEvent;
use bookings_server::veld::server::{Body, Handler, Request, RequestContext};
use serde_json::{Value, json};

const ID: &str = "00000000-0000-4000-8000-000000000001";
const NOW_MS: u64 = 1_781_092_800_007;

#[derive(Default)]
struct World {
    /// How long a handler waits, and what it does after.
    delay_ms: Mutex<u64>,
    panic: Mutex<bool>,
    /// The hook: how long it takes, whether it fails.
    hook_ms: Mutex<u64>,
    hook_fails: Mutex<bool>,
    /// What the hook received and what `on_error` heard.
    events: Mutex<Vec<AuditEvent>>,
    errors: Mutex<Vec<String>>,
    clock_reads: AtomicUsize,
}

fn note() -> Note {
    Note { id: ID.into(), text: "t".into(), secret: Some("s3cr3t".into()) }
}

struct Impl(Arc<World>);

impl Impl {
    async fn wait(&self) {
        let ms = *self.0.delay_ms.lock().unwrap();
        if ms > 0 {
            tokio::time::sleep(Duration::from_millis(ms)).await;
        }
        assert!(!*self.0.panic.lock().unwrap(), "the handler panics");
    }
}

impl NotesService for Impl {
    async fn get_note(&self, _r: GetNoteRequest, _c: &RequestContext) -> Result<Note, Error> {
        self.wait().await;
        Ok(note())
    }

    async fn add_note(&self, request: AddNoteRequest, _c: &RequestContext) -> Result<Note, Error> {
        Ok(request.body)
    }

    async fn via_fact(&self, _r: ViaFactRequest, facts: ViaFactFacts, _c: &RequestContext) -> Result<Note, Error> {
        Ok(facts.note)
    }

    async fn boom(&self, _r: BoomRequest, _c: &RequestContext) -> Result<Note, Error> {
        Err(Error::other("boom"))
    }

    async fn slow(&self, _r: SlowRequest, _c: &RequestContext) -> Result<Note, Error> {
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(note())
    }

    async fn quiet(&self, _r: QuietRequest, _c: &RequestContext) -> Result<Note, Error> {
        Ok(note())
    }
}

impl NotesFactLoaders for Impl {
    async fn note(&self, _key: String, _ctx: &RequestContext) -> Result<Option<Note>, Error> {
        Ok(Some(note()))
    }
}

struct Hooks(Arc<World>, bool);

impl NotesHooks for Hooks {
    async fn auth_bearer<'a>(&'a self, token: &'a str, _ctx: &'a RequestContext) -> Option<User> {
        (token == "t").then(|| User { id: ID.into(), email: "a@example.test".into() })
    }

    async fn audit<'a>(&'a self, event: &'a AuditEvent) -> Result<(), Error> {
        let ms = *self.0.hook_ms.lock().unwrap();
        if ms > 0 {
            tokio::time::sleep(Duration::from_millis(ms)).await;
        }
        if *self.0.hook_fails.lock().unwrap() {
            return Err(Error::other("the log is down"));
        }
        self.0.events.lock().unwrap().push(event.clone());
        Ok(())
    }

    fn on_error(&self, err: &Error, _ctx: &RequestContext) {
        self.0.errors.lock().unwrap().push(err.to_string());
    }

    fn validate_requests(&self) -> bool {
        self.1
    }

    fn clock(&self, _ctx: &RequestContext) -> std::time::SystemTime {
        self.0.clock_reads.fetch_add(1, Ordering::SeqCst);
        UNIX_EPOCH + Duration::from_millis(NOW_MS)
    }
}

type Server = Handler<NotesDispatch<Impl, Hooks, Impl>>;

fn serve(world: &Arc<World>, validate: bool) -> Arc<Server> {
    Arc::new(NotesHandler::create(Impl(world.clone()), Hooks(world.clone(), validate), Impl(world.clone()), None).unwrap())
}

fn request(method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> Request {
    let mut headers: Vec<(String, String)> = token.map(|t| vec![("authorization".to_string(), format!("Bearer {t}"))]).unwrap_or_default();
    let body = body.map(|b| b.to_string().into_bytes()).unwrap_or_default();
    if !body.is_empty() {
        headers.push(("content-type".into(), "application/json".into()));
    }
    Request { method: method.into(), path: path.into(), query: String::new(), headers, body }
}

async fn call(server: &Server, req: Request) -> (u16, Value) {
    let response = server.handle(req).await.expect("a route");
    let doc = match response.body {
        Body::Bytes(bytes) => serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        _ => Value::Null,
    };
    (response.status, doc)
}

fn reset(world: &World) {
    world.events.lock().unwrap().clear();
    world.errors.lock().unwrap().clear();
    world.clock_reads.store(0, Ordering::SeqCst);
    *world.delay_ms.lock().unwrap() = 0;
    *world.hook_ms.lock().unwrap() = 0;
    *world.hook_fails.lock().unwrap() = false;
    *world.panic.lock().unwrap() = false;
}

fn only(world: &World) -> Value {
    let events = world.events.lock().unwrap();
    assert_eq!(events.len(), 1, "one event: {events:?}");
    events[0].to_json()
}

/// A success: the result and the principal are in the event without their `@secret` and `@pii`,
/// and the time is the one reading of the clock.
async fn a_success(world: &Arc<World>, server: &Server) {
    reset(world);
    let (status, _) = call(server, request("GET", &format!("/notes/{ID}"), Some("t"), None)).await;
    assert_eq!(status, 200);
    let event = only(world);
    assert_eq!(event["outcome"], json!({ "status": 200 }));
    assert_eq!(event["time"], json!("2026-06-10T12:00:00.007Z"));
    assert_eq!(event["result"], json!({ "id": ID, "text": "t" }));
    assert_eq!(event["principal"], json!({ "id": ID }));
    assert_eq!(event["redacted"], json!(["principal.email", "result.secret"]));
    assert_eq!(event["hidden"], json!([]));
    assert_eq!(event["parameters"], json!({ "id": ID }));
    assert_eq!(event["facts"], json!({}));
    assert!(event["operation"].as_str().unwrap().starts_with("veld:bookings/Notes.getNote@"));
}

/// A body parameter and a fact: both without their secret, and the clock is read once.
async fn a_body_and_a_fact(world: &Arc<World>, server: &Server) {
    reset(world);
    let body = json!({ "id": ID, "text": "t", "secret": "s3cr3t" });
    assert_eq!(call(server, request("POST", "/notes", Some("t"), Some(body))).await.0, 201);
    let event = only(world);
    assert_eq!(event["parameters"], json!({ "body": { "id": ID, "text": "t" } }));
    assert!(event["redacted"].as_array().unwrap().contains(&json!("parameters.body.secret")));
    reset(world);
    assert_eq!(call(server, request("GET", &format!("/notes/fact/{ID}"), Some("t"), None)).await.0, 200);
    let event = only(world);
    assert_eq!(event["facts"], json!({ "note": { "id": ID, "text": "t" } }));
    assert!(event["redacted"].as_array().unwrap().contains(&json!("facts.note.secret")));
    assert_eq!(world.clock_reads.load(Ordering::SeqCst), 1, "the clock is read once per request");
}

/// A handler that fails is `500`, with no result; a refused request and one that is not audited.
async fn failures_and_early_answers(world: &Arc<World>, server: &Server) {
    reset(world);
    assert_eq!(call(server, request("GET", &format!("/notes/boom/{ID}"), Some("t"), None)).await.0, 500);
    let event = only(world);
    assert_eq!(event["outcome"], json!({ "status": 500, "error": "Internal" }));
    assert!(event.get("result").is_none(), "{event}");
    reset(world);
    assert_eq!(call(server, request("GET", &format!("/notes/{ID}"), None, None)).await.0, 401);
    let event = only(world);
    assert_eq!(event["outcome"], json!({ "status": 401, "error": "Unauthorized" }));
    assert!(event.get("principal").is_none() && event.get("result").is_none(), "{event}");
    assert_eq!((event["parameters"].clone(), event["facts"].clone()), (json!({}), json!({})));
    reset(world);
    assert_eq!(call(server, request("GET", &format!("/notes/quiet/{ID}"), Some("t"), None)).await.0, 200);
    assert!(world.events.lock().unwrap().is_empty(), "an endpoint that is not audited has no event");
}

/// The deadline wraps the audit: the `504` is recorded.
async fn a_deadline(world: &Arc<World>, server: &Server) {
    reset(world);
    assert_eq!(call(server, request("GET", &format!("/notes/slow/{ID}"), Some("t"), None)).await.0, 504);
    assert_eq!(only(world)["outcome"], json!({ "status": 504, "error": "DeadlineExceeded" }));
}

/// A hook that fails changes nothing for the caller, and `on_error` hears of it with the event.
async fn a_hook_that_fails(world: &Arc<World>, server: &Server) {
    reset(world);
    *world.hook_fails.lock().unwrap() = true;
    assert_eq!(call(server, request("GET", &format!("/notes/{ID}"), Some("t"), None)).await.0, 200);
    let errors = world.errors.lock().unwrap().clone();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].contains("the log is down") && errors[0].contains("\"outcome\""), "{}", errors[0]);
}

/// A request whose future is dropped while the hook runs still has its event (the answer was
/// decided); one dropped before is not recorded; a handler that panics is recorded as `500`.
async fn a_request_that_is_dropped(world: &Arc<World>, server: &Arc<Server>) {
    reset(world);
    *world.hook_ms.lock().unwrap() = 100;
    let path = format!("/notes/{ID}");
    let cut = tokio::time::timeout(Duration::from_millis(30), call(server, request("GET", &path, Some("t"), None))).await;
    assert!(cut.is_err(), "the request was cut while the hook ran");
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(only(world)["outcome"], json!({ "status": 200 }));

    reset(world);
    *world.delay_ms.lock().unwrap() = 150;
    let cut = tokio::time::timeout(Duration::from_millis(30), call(server, request("GET", &path, Some("t"), None))).await;
    assert!(cut.is_err());
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(world.events.lock().unwrap().is_empty(), "a request dropped before its answer has no outcome to record");

    reset(world);
    *world.panic.lock().unwrap() = true;
    let task = {
        let server = server.clone();
        tokio::spawn(async move { call(&server, request("GET", &format!("/notes/{ID}"), Some("t"), None)).await })
    };
    assert!(task.await.is_err(), "the handler panicked");
    tokio::time::sleep(Duration::from_millis(200)).await;
    let event = only(world);
    assert_eq!(event["outcome"], json!({ "status": 500, "error": "Internal" }));
    assert_eq!(event["principal"], json!({ "id": ID }));
}

#[tokio::main]
async fn main() {
    for validate in [true, false] {
        let world = Arc::new(World::default());
        let server = serve(&world, validate);
        a_success(&world, &server).await;
        a_body_and_a_fact(&world, &server).await;
        failures_and_early_answers(&world, &server).await;
        a_deadline(&world, &server).await;
        a_hook_that_fails(&world, &server).await;
        a_request_that_is_dropped(&world, &server).await;
    }
    println!("audit http ok");
}
