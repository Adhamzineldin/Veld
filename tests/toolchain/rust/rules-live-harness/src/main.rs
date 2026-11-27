//! Rules beyond bodies (`spec/rules.md` section 5.2): a declared error with an invariant, SSE items
//! and WebSocket messages in both directions, in `strict`, `audit` and `off`. The generated server
//! and client of `tests/toolchain/contract/rules_live`, over real sockets.

use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use live_client::live::live::{Note as ClientNote, Reading as ClientReading};
use live_client::live::live_client::{LiveClient, ReadingsRequest as ClientReadings, SeatsRequest as ClientSeats};
use live_client::prelude::ValidationError;
use live_client::veld::DeclaredError;
use live_client::veld::client::ClientOptions;
use live_client::veld::stream::next;
use live_server::live::live::{Note, Overbooked, Reading};
use live_server::live::live_server::{LiveHandler, LiveHooks, LiveService, NotesRequest, ReadingsRequest, SeatsRequest};
use live_server::veld::Error;
use live_server::runtime_config::{LiveServerRuntime, RuleOutputMode, RulesSettings};
use live_server::veld::rules::{Outcome, Position, RuleEvent};
use live_server::veld::server::{RequestContext, WsSession};
use live_server::veld::stream::{EventStream, SseEvent, iter};
use tokio_tungstenite::tungstenite::Message;

type Events = Arc<Mutex<Vec<RuleEvent>>>;

struct Hooks {
    events: Events,
}

impl LiveHooks for Hooks {
    fn on_rule_violation(&self, event: &RuleEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

/// The texts of the notes the server accepted.
type Received = Arc<Mutex<Vec<String>>>;

struct Impl {
    received: Received,
}

impl LiveService for Impl {
    async fn readings(&self, request: ReadingsRequest, _ctx: &RequestContext) -> Result<EventStream<Reading>, Error> {
        let items: Vec<Result<SseEvent<Reading>, Error>> = (0..3)
            .map(|i| Ok(SseEvent::of(if request.bad == i { Reading { low: 5, high: 1 } } else { Reading { low: i, high: i + 1 } })))
            .collect();
        Ok(Box::pin(iter(items)))
    }

    async fn notes(&self, _request: NotesRequest, _ctx: &RequestContext, mut ws: WsSession<Note, Reading>) -> Result<(), Error> {
        while let Some(message) = ws.recv().await {
            let message = message?;
            self.received.lock().unwrap().push(message.text.clone());
            ws.send(if message.text == "bad" { Reading { low: 9, high: 1 } } else { Reading { low: 0, high: message.stars } })?;
        }
        Ok(())
    }

    async fn seats(&self, request: SeatsRequest, _ctx: &RequestContext) -> Result<i32, Error> {
        Err(Overbooked { seats: if request.ok { 12 } else { 3 }, limit: 10 }.into())
    }
}

struct Server {
    base: String,
    events: Events,
    received: Received,
}

async fn start(mode: RuleOutputMode) -> Server {
    let events: Events = Arc::default();
    let received: Received = Arc::default();
    let handler = LiveHandler::create(
        Impl { received: received.clone() },
        Hooks { events: events.clone() },
        Some(&LiveServerRuntime { rules: RulesSettings { output: mode } }),
    )
    .unwrap();
    let router = live_server::adapters::axum::router(handler);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Server { base, events, received }
}

fn client(base: &str) -> LiveClient<live_client::veld::transport_reqwest::ReqwestTransport> {
    LiveClient::reqwest(ClientOptions::new().base_url(base)).unwrap()
}

async fn get(url: &str) -> (u16, String) {
    let response = reqwest::get(url).await.unwrap();
    (response.status().as_u16(), response.text().await.unwrap())
}

fn last(events: &Events) -> RuleEvent {
    events.lock().unwrap().last().cloned().expect("an event")
}

async fn error_rules() {
    let strict = start(RuleOutputMode::Strict).await;
    let (status, text) = get(&format!("{}/live/seats?ok=true", strict.base)).await;
    assert_eq!(status, 409);
    assert_eq!(serde_json::from_str::<serde_json::Value>(&text).unwrap()["title"], "Overbooked");
    assert!(strict.events.lock().unwrap().is_empty());
    // strict: a declared error that violates its own rule is a 500 with no rule detail.
    let (status, text) = get(&format!("{}/live/seats?ok=false", strict.base)).await;
    assert_eq!(status, 500, "{text}");
    let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(doc["title"], "Internal");
    assert_eq!(doc["data"], serde_json::json!({}));
    assert!(doc.get("violations").is_none());
    assert!(!text.contains("overflow") && !text.contains("Overbooked"), "{text}");
    let event = last(&strict.events);
    assert_eq!(strict.events.lock().unwrap().len(), 1);
    assert_eq!((event.position, event.outcome), (Position::Output, Outcome::Rejected));
    assert!(event.rule.starts_with("veld:live/Overbooked.overflow@"), "{}", event.rule);
    assert!(event.operation.contains("Live.seats"));
    let err = client(&strict.base).seats(ClientSeats { ok: false }).await.unwrap_err();
    assert_eq!(err.status(), Some(500));

    // audit: sent unchanged, the hook gets the violation, and the client never checks what it decodes.
    let audit = start(RuleOutputMode::Audit).await;
    let (status, text) = get(&format!("{}/live/seats?ok=false", audit.base)).await;
    assert_eq!(status, 409);
    assert_eq!(serde_json::from_str::<serde_json::Value>(&text).unwrap()["data"]["seats"], 3);
    assert_eq!(audit.events.lock().unwrap().len(), 1);
    assert_eq!(last(&audit.events).outcome, Outcome::Audited);
    let err = client(&audit.base).seats(ClientSeats { ok: false }).await.unwrap_err();
    let overbooked = live_client::live::live::Overbooked::from_error(&err).expect("a typed Overbooked");
    assert_eq!(overbooked.seats, 3);
    assert!(err.violations().is_empty());
    assert!(!err.is_local());

    // off: not evaluated.
    let off = start(RuleOutputMode::Off).await;
    assert_eq!(get(&format!("{}/live/seats?ok=false", off.base)).await.0, 409);
    assert!(off.events.lock().unwrap().is_empty());
}

/// Every item of a stream the client reads, and the error it ends with.
async fn readings(base: &str, bad: i32) -> (Vec<(i32, i32)>, Option<live_client::veld::Error>) {
    let mut stream = match client(base).readings(ClientReadings { bad }).await {
        Ok(stream) => stream,
        Err(e) => return (Vec::new(), Some(e)),
    };
    let (mut got, mut error) = (Vec::new(), None);
    while let Some(item) = next(&mut stream).await {
        match item {
            Ok(ClientReading { low, high }) => got.push((low, high)),
            Err(e) => error = Some(e),
        }
    }
    (got, error)
}

async fn sse_rules() {
    let strict = start(RuleOutputMode::Strict).await;
    assert_eq!(readings(&strict.base, -1).await.0.len(), 3);
    assert!(strict.events.lock().unwrap().is_empty());
    // A violating item in the middle is not sent: the stream ends with a final error event.
    let (got, error) = readings(&strict.base, 1).await;
    assert_eq!(got, [(0, 1)]);
    assert_eq!(error.expect("the stream fails").status(), Some(500));
    let event = last(&strict.events);
    assert_eq!(event.outcome, Outcome::Rejected);
    assert!(event.rule.starts_with("veld:live/Reading.ordered@"));
    let (_, raw) = get(&format!("{}/live/readings?bad=1", strict.base)).await;
    assert!(raw.contains("event: error") && !raw.contains("\"low\":5"), "{raw}");
    // A violating first item is still a normal 500: nothing was sent yet.
    let (got, error) = readings(&strict.base, 0).await;
    assert!(got.is_empty());
    assert_eq!(error.expect("the call fails").status(), Some(500));
    assert_eq!(get(&format!("{}/live/readings?bad=0", strict.base)).await.0, 500);

    let audit = start(RuleOutputMode::Audit).await;
    let (got, error) = readings(&audit.base, 1).await;
    assert_eq!(got.iter().map(|r| r.0).collect::<Vec<_>>(), [0, 5, 2]);
    assert!(error.is_none());
    assert_eq!(audit.events.lock().unwrap().len(), 1);
    assert_eq!(last(&audit.events).outcome, Outcome::Audited);

    let off = start(RuleOutputMode::Off).await;
    assert_eq!(readings(&off.base, 1).await.0.len(), 3);
    assert!(off.events.lock().unwrap().is_empty());
}

struct Raw {
    socket: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
}

impl Raw {
    async fn open(base: &str) -> Raw {
        let url = format!("{}/live/notes", base.replacen("http", "ws", 1));
        let (socket, _) = tokio_tungstenite::connect_async(url).await.unwrap();
        Raw { socket }
    }

    async fn send(&mut self, text: &str) {
        self.socket.send(Message::Text(text.into())).await.unwrap();
    }

    async fn frame(&mut self) -> String {
        loop {
            match self.socket.next().await {
                Some(Ok(Message::Text(t))) => return t.to_string(),
                Some(Ok(_)) => {}
                other => panic!("expected a text frame, got {other:?}"),
            }
        }
    }

    async fn close_code(&mut self) -> u16 {
        loop {
            match self.socket.next().await {
                Some(Ok(Message::Close(frame))) => return frame.map_or(1005, |f| u16::from(f.code)),
                Some(Ok(_)) => {}
                other => panic!("expected a close frame, got {other:?}"),
            }
        }
    }
}

fn note(text: &str, stars: i32) -> ClientNote {
    ClientNote { text: text.into(), stars }
}

async fn websocket_rules() {
    let strict = start(RuleOutputMode::Strict).await;
    // Client to server, checked locally like a request body: nothing is written. The client has
    // not seen the server's rules digest yet (a WebSocket upgrade shows none), so one plain call
    // brings it: until a response says the rules are the ones it was generated from, it sends.
    let live = client(&strict.base);
    live.seats(ClientSeats { ok: true }).await.unwrap_err();
    let mut chan = live.notes().await.unwrap();
    let err = chan.send(note("x", 9)).await.unwrap_err();
    assert!(ValidationError::matches(&err));
    assert!(err.is_local());
    assert_eq!(err.violations()[0].path, "message");
    assert_eq!(err.violations()[0].fields, ["stars"]);
    assert_eq!(ValidationError::from_error(&err).unwrap().issues[0].code, "rule");
    chan.send(note("fine", 3)).await.unwrap();
    let reply = chan.recv().await.unwrap().unwrap();
    assert_eq!((reply.low, reply.high), (0, 3));
    chan.close().await;
    assert_eq!(*strict.received.lock().unwrap(), ["fine"]);

    // Local check off: the server rejects the message like one of the wrong shape, with violations.
    let remote = LiveClient::reqwest(ClientOptions::new().base_url(&strict.base).check_rules(false)).unwrap();
    let mut chan = remote.notes().await.unwrap();
    chan.send(note("sneaky", 9)).await.unwrap();
    let err = chan.recv().await.unwrap().unwrap_err();
    assert!(ValidationError::matches(&err), "{err:?}");
    assert!(!err.is_local());
    assert_eq!(err.violations().len(), 1);
    assert_eq!(err.violations()[0].path, "message");
    assert!(err.violations()[0].rule.starts_with("veld:live/Note.starRange@"));
    assert!(!strict.received.lock().unwrap().contains(&"sneaky".to_string()));
    let mut raw = Raw::open(&strict.base).await;
    raw.send(r#"{"stars":9,"text":"raw"}"#).await;
    let frame = raw.frame().await;
    assert!(frame.starts_with(r#"{"error":{"#) && frame.contains(r#""title":"ValidationError""#), "{frame}");
    assert!(frame.contains(r#""violations":[{"#) && frame.contains(r#""path":"message""#), "{frame}");
    assert_eq!(raw.close_code().await, 1008);
    assert!(strict.events.lock().unwrap().iter().any(|e| e.position == Position::Input && e.outcome == Outcome::Rejected));

    // Server to client: a violating message is not sent; the conversation ends with Internal.
    let mut chan = client(&strict.base).notes().await.unwrap();
    chan.send(note("bad", 1)).await.unwrap();
    let err = chan.recv().await.unwrap().unwrap_err();
    assert_eq!(err.status(), Some(500), "{err:?}");
    let event = last(&strict.events);
    assert_eq!((event.position, event.outcome), (Position::Output, Outcome::Rejected));
    assert!(event.rule.starts_with("veld:live/Reading.ordered@"));

    let audit = start(RuleOutputMode::Audit).await;
    let mut chan = client(&audit.base).notes().await.unwrap();
    chan.send(note("bad", 1)).await.unwrap();
    let reply = chan.recv().await.unwrap().unwrap();
    assert_eq!((reply.low, reply.high), (9, 1), "audit sends it");
    assert_eq!(last(&audit.events).outcome, Outcome::Audited);

    let off = start(RuleOutputMode::Off).await;
    let mut chan = client(&off.base).notes().await.unwrap();
    chan.send(note("bad", 1)).await.unwrap();
    assert_eq!(chan.recv().await.unwrap().unwrap().low, 9);
    assert!(off.events.lock().unwrap().is_empty());
}

#[tokio::main]
async fn main() {
    error_rules().await;
    sse_rules().await;
    websocket_rules().await;
    println!("rules live ok");
}
