//! Audited rules (`@rollout(audit)`, `spec/rules.md` section 17.1) over HTTP and SSE: the generated
//! Rust server and client of `tests/toolchain/contract/rollout/rollout.veld`. An audited rule is
//! evaluated where the enforced one would be, goes to `on_rule_violation` with the outcome `Audited`,
//! and never rejects, never answers and never shows in a response.

use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use audit_client::audit::audit::Party as ClientParty;
use audit_client::audit::audit_client::{AuditClient, BookRequest as ClientBook};
use audit_client::veld::client::ClientOptions;
use audit_server::audit::audit::{Party, Room, Tick};
use audit_server::audit::audit_server::{
    AnnounceRequest, AuditFactLoaders, AuditHandler, AuditHooks, AuditService, BookFacts, BookRequest, EchoRequest,
    TicksRequest,
};
use audit_server::veld::rules::{Outcome, Position, RuleEvent, RuleOutput};
use audit_server::veld::server::{RequestContext, WsSession};
use audit_server::veld::stream::{EventStream, SseEvent, unfold};
use audit_server::veld::Error;

const ROOM: &str = "00000000-0000-4000-8000-000000000001";

/// What the room fact says about its capacity.
static CAPACITY: AtomicI32 = AtomicI32::new(100);

type Events = Arc<Mutex<Vec<RuleEvent>>>;

struct Loaders;

impl AuditFactLoaders for Loaders {
    async fn room(&self, _key: String, _ctx: &RequestContext) -> Result<Option<Room>, Error> {
        Ok(Some(Room { id: ROOM.into(), capacity: CAPACITY.load(Ordering::SeqCst) }))
    }
}

/// Five guests come back as six, which breaks the audited `ensure`.
struct Impl;

impl AuditService for Impl {
    async fn book(&self, request: BookRequest, _facts: BookFacts, _ctx: &RequestContext) -> Result<Party, Error> {
        let guests = if request.body.guests == 5 { 6 } else { request.body.guests };
        Ok(Party { room_id: request.body.room_id, guests })
    }

    async fn announce(
        &self,
        _request: AnnounceRequest,
        _ctx: &RequestContext,
        ws: WsSession<(), Tick>,
    ) -> Result<(), Error> {
        for n in 0..3 {
            ws.send(Tick { n })?;
        }
        Ok(())
    }

    async fn echo(&self, _request: EchoRequest, _ctx: &RequestContext, mut ws: WsSession<Party, Tick>) -> Result<(), Error> {
        while let Some(message) = ws.recv().await {
            ws.send(Tick { n: message?.guests })?;
        }
        Ok(())
    }

    async fn ticks(&self, request: TicksRequest, _ctx: &RequestContext) -> Result<EventStream<Tick>, Error> {
        let count = request.count;
        Ok(Box::pin(unfold(0, move |n: i32| {
            Box::pin(async move {
                if n >= count {
                    return None;
                }
                Some((Ok(SseEvent::of(Tick { n })), n + 1))
            })
        })))
    }
}

struct Hooks {
    mode: RuleOutput,
    validate: bool,
    events: Events,
}

impl AuditHooks for Hooks {
    fn rules_output(&self) -> RuleOutput {
        self.mode
    }

    fn validate_requests(&self) -> bool {
        self.validate
    }

    fn on_rule_violation(&self, event: &RuleEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

struct Server {
    base: String,
    events: Events,
}

async fn serve(mode: RuleOutput, validate: bool) -> Server {
    let events: Events = Arc::default();
    let handler = AuditHandler::create(Impl, Hooks { mode, validate, events: events.clone() }, Loaders);
    let router = audit_server::adapters::axum::router(handler);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Server { base, events }
}

/// What the hook heard, as `outcome position rule path`, in order; the events are cleared.
fn heard(server: &Server) -> Vec<String> {
    let mut events = server.events.lock().unwrap();
    let out = events
        .iter()
        .map(|e| {
            let rule = e.rule.trim_start_matches("veld:audit/");
            let rule = rule.split('@').next().unwrap_or(rule);
            format!("{:?} {:?} {rule} {}", e.outcome, e.position, e.violation.path)
        })
        .collect();
    events.clear();
    out
}

async fn book(server: &Server, guests: i32) -> (u16, String) {
    let response = reqwest::Client::new()
        .post(format!("{}/audit/book", server.base))
        .header("content-type", "application/json")
        .body(format!("{{\"roomId\":\"{ROOM}\",\"guests\":{guests}}}"))
        .send()
        .await
        .unwrap();
    (response.status().as_u16(), response.text().await.unwrap())
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_string()).collect()
}

async fn invariants_requires_and_ensures() {
    let strict = serve(RuleOutput::Strict, true).await;
    // An audited invariant of the body and of the result: reported, and the request goes on.
    CAPACITY.store(100, Ordering::SeqCst);
    let (status, body) = book(&strict, 7).await;
    assert_eq!(status, 201, "{body}");
    assert!(!body.contains("violations") && !body.contains("smallParty"), "{body}");
    assert_eq!(
        heard(&strict),
        strings(&["Audited Input Party.smallParty body", "Audited Output Party.smallParty result"])
    );
    // A client skips audited rules: the checks of a value and the local check before a send.
    let draft = ClientParty { room_id: ROOM.into(), guests: 7 };
    assert!(draft.check().is_empty(), "{:?}", draft.check());
    assert!(draft.check_request().is_empty());
    let client = AuditClient::reqwest(ClientOptions::new().base_url(&strict.base)).unwrap();
    let sent = client.book(ClientBook { body: draft.clone() }).await.unwrap();
    assert_eq!(sent.guests, 7, "the local check does not block the send");
    assert_eq!(heard(&strict).len(), 2);

    // An audited `require ... else E` that fails is not answered: the payload divides by zero and is
    // never built.
    CAPACITY.store(2, Ordering::SeqCst);
    let (status, body) = book(&strict, 3).await;
    assert_eq!(status, 201, "{body}");
    assert!(!body.contains("RoomFull"), "{body}");
    assert_eq!(heard(&strict), strings(&["Audited Input Audit.book.require request"]));

    // An audited `ensure` that the result breaks leaves the outcome ok.
    CAPACITY.store(100, Ordering::SeqCst);
    let (status, body) = book(&strict, 5).await;
    assert_eq!(status, 201, "{body}");
    assert!(body.contains("\"guests\":6"), "{body}");
    assert_eq!(heard(&strict), strings(&["Audited Output Audit.book.ensure request"]));

    // A rule that holds is not reported.
    assert_eq!(book(&strict, 2).await.0, 201);
    assert!(heard(&strict).is_empty());
}

async fn modes() {
    // Output mode `off` skips an audited output rule like every output rule; an input rule is evaluated.
    let off = serve(RuleOutput::Off, true).await;
    CAPACITY.store(100, Ordering::SeqCst);
    assert_eq!(book(&off, 7).await.0, 201);
    assert_eq!(heard(&off), strings(&["Audited Input Party.smallParty body"]));
    assert_eq!(book(&off, 5).await.0, 201);
    assert!(heard(&off).is_empty());

    // `audit` evaluates and reports them too.
    let audit = serve(RuleOutput::Audit, true).await;
    assert_eq!(book(&audit, 5).await.0, 201);
    assert_eq!(heard(&audit), strings(&["Audited Output Audit.book.ensure request"]));

    // With request validation off an audited rule is off exactly where the enforced one would be: the
    // invariants of the body, `require` and `ensure` are not evaluated, the invariants of the result are.
    let loose = serve(RuleOutput::Strict, false).await;
    CAPACITY.store(2, Ordering::SeqCst);
    assert_eq!(book(&loose, 7).await.0, 201);
    assert_eq!(heard(&loose), strings(&["Audited Output Party.smallParty result"]));
    assert_eq!(book(&loose, 5).await.0, 201);
    assert!(heard(&loose).is_empty());
}

async fn sse_items() {
    // The items of a stream are outputs: the odd ones break the audited invariant, are sent all the
    // same, and are reported.
    let sse = serve(RuleOutput::Strict, true).await;
    let response = reqwest::get(format!("{}/audit/ticks?count=4", sse.base)).await.unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let text = response.text().await.unwrap();
    assert_eq!(text.lines().filter(|l| l.starts_with("data:")).count(), 4, "{text}");
    assert!(!text.contains("event: error"), "{text}");
    assert_eq!(
        heard(&sse),
        strings(&["Audited Output Tick.even result", "Audited Output Tick.even result"])
    );
}

#[tokio::main]
async fn main() {
    invariants_requires_and_ensures().await;
    modes().await;
    sse_items().await;
    let _ = (Outcome::Audited, Position::Input);
    println!("rollout ok");
}
