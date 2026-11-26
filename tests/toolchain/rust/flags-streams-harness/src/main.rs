//! Flagged invariants on the messages of a WebSocket and the items of a stream
//! (`spec/platform.md` section 11, `spec/targets/rust.md` section 17.2), generated from
//! `tests/toolchain/contract/platform_p2/flags_streams.veld`, over real sockets: the checks run
//! with the flags the request read, after the call that opened the conversation or the stream has
//! returned.

#![allow(clippy::result_large_err)]

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use chat_server::chat::chat::{Heard, Say};
use chat_server::chat::chat_server::{ChatHandler, ChatHooks, ChatService, TalkRequest, TicksRequest};
use chat_server::veld::Error;
use chat_server::veld::flags::{FlagContext, FlagProvider};
use chat_server::veld::server::{RequestContext, WsSession};
use chat_server::veld::stream::{EventStream, SseEvent, iter};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

/// A provider that records every question; `failing` makes it fail.
#[derive(Default)]
struct Provider {
    on: Mutex<BTreeSet<String>>,
    failing: Mutex<bool>,
    asked: Mutex<Vec<String>>,
}

impl FlagProvider for Provider {
    async fn is_on<'a>(&'a self, name: &'a str, _context: &'a FlagContext) -> Result<bool, Error> {
        self.asked.lock().unwrap().push(name.to_string());
        if *self.failing.lock().unwrap() {
            return Err(Error::other("the flag service is down"));
        }
        Ok(self.on.lock().unwrap().contains(name))
    }
}

struct World {
    provider: Provider,
    errors: Mutex<usize>,
}

struct Impl;

impl ChatService for Impl {
    async fn ticks(&self, request: TicksRequest, _ctx: &RequestContext) -> Result<EventStream<Heard>, Error> {
        let items: Vec<Result<SseEvent<Heard>, Error>> = (0..2).map(|_| Ok(SseEvent::of(Heard { text: request.text.clone() }))).collect();
        Ok(Box::pin(iter(items)))
    }

    async fn talk(&self, _request: TalkRequest, _ctx: &RequestContext, mut ws: WsSession<Say, Heard>) -> Result<(), Error> {
        while let Some(message) = ws.recv().await {
            let message = message?;
            ws.send(Heard { text: message.text })?;
        }
        Ok(())
    }
}

struct Hooks(Arc<World>);

impl ChatHooks for Hooks {
    type Flags = Provider;

    fn flags(&self) -> &Provider {
        &self.0.provider
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
    let w = Arc::new(World { provider: Provider::default(), errors: Mutex::new(0) });
    let handler = ChatHandler::create(Impl, Hooks(w.clone()), None).unwrap();
    let router = chat_server::adapters::axum::router(handler);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Server { base, w }
}

fn turn(s: &Server, on: &[&str]) {
    *s.w.provider.on.lock().unwrap() = on.iter().map(ToString::to_string).collect();
}

/// How many times a flag was asked since the last call of this function.
fn asked(s: &Server, name: &str) -> usize {
    let mut log = s.w.provider.asked.lock().unwrap();
    let n = log.iter().filter(|n| n.as_str() == name).count();
    log.clear();
    n
}

async fn ticks(s: &Server, text: &str) -> (u16, String) {
    let response = reqwest::get(format!("{}/chat/ticks?text={text}", s.base)).await.unwrap();
    (response.status().as_u16(), response.text().await.unwrap())
}

struct Raw {
    socket: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
}

impl Raw {
    async fn open(s: &Server) -> Raw {
        let url = format!("{}/chat/talk", s.base.replacen("http", "ws", 1));
        let (socket, _) = tokio_tungstenite::connect_async(url).await.unwrap();
        Raw { socket }
    }

    async fn say(&mut self, text: &str) {
        self.socket.send(Message::Text(format!(r#"{{"text":"{text}"}}"#).into())).await.unwrap();
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

/// The items of a stream: `loud` is checked on each of them, with the flags of the request.
async fn stream_items() {
    let s = serve().await;
    let (status, body) = ticks(&s, "a").await;
    assert_eq!(status, 200, "loud is off: a one-letter item goes out: {body}");
    assert_eq!(body.matches(r#""text":"a""#).count(), 2, "{body}");
    assert_eq!(asked(&s, "loud"), 1, "one reading of the flag per request");

    turn(&s, &["loud"]);
    let (status, body) = ticks(&s, "a").await;
    assert_eq!(status, 500, "loud is on: the first item violates its invariant: {body}");
    assert_eq!(asked(&s, "loud"), 1);
    let (status, body) = ticks(&s, "ok").await;
    assert_eq!(status, 200, "a value that holds goes out: {body}");

    // A provider that fails counts the invariant as on, and the failure reaches `on_error`.
    turn(&s, &[]);
    *s.w.provider.failing.lock().unwrap() = true;
    let before = *s.w.errors.lock().unwrap();
    let (status, _) = ticks(&s, "a").await;
    assert_eq!(status, 500, "a failing provider counts the flag as on");
    assert!(*s.w.errors.lock().unwrap() > before, "the failure reached on_error");
}

/// The messages a client sends: `shortOnly` is checked on each of them, with the flags the
/// upgrade request read.
async fn incoming_messages() {
    let s = serve().await;
    let mut socket = Raw::open(&s).await;
    socket.say("waytoolong").await;
    assert_eq!(socket.frame().await, r#"{"data":{"text":"waytoolong"}}"#, "shortOnly is off");
    assert_eq!(asked(&s, "shortOnly"), 1, "read once, when the conversation opened");
    socket.say("again-too-long").await;
    assert_eq!(socket.frame().await, r#"{"data":{"text":"again-too-long"}}"#, "the same reading serves the next message");
    assert_eq!(asked(&s, "shortOnly"), 0);

    turn(&s, &["shortOnly"]);
    let mut socket = Raw::open(&s).await;
    socket.say("ok").await;
    assert_eq!(socket.frame().await, r#"{"data":{"text":"ok"}}"#, "a short message passes");
    socket.say("waytoolong").await;
    let frame = socket.frame().await;
    assert!(frame.starts_with(r#"{"error":{"#) && frame.contains(r#""title":"ValidationError""#), "{frame}");
    assert!(frame.contains(r#""violations":[{"#) && frame.contains("too long") && frame.contains(r#""path":"message""#), "{frame}");
    assert_eq!(socket.close_code().await, 1008);
}

/// The messages the server sends: `loud` is checked on each `send`.
async fn outgoing_messages() {
    let s = serve().await;
    turn(&s, &["loud"]);
    let mut socket = Raw::open(&s).await;
    socket.say("ok").await;
    assert_eq!(socket.frame().await, r#"{"data":{"text":"ok"}}"#);
    socket.say("a").await;
    let frame = socket.frame().await;
    assert!(frame.contains(r#""title":"Internal""#), "a violating message is not sent: {frame}");
    assert_eq!(socket.close_code().await, 1011);

    turn(&s, &[]);
    let mut socket = Raw::open(&s).await;
    socket.say("a").await;
    assert_eq!(socket.frame().await, r#"{"data":{"text":"a"}}"#, "loud is off");
}

#[tokio::main]
async fn main() {
    stream_items().await;
    incoming_messages().await;
    outgoing_messages().await;
    println!("flags streams ok");
}
