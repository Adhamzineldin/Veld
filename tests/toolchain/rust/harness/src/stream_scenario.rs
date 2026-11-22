//! The streaming client scenario, run by the Rust client against every server: the Rust server and
//! the Node server. Server-side observations need a [`Probe`]; without one those checks are
//! skipped. Mirrors `tests/toolchain/java/StreamScenario.java`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use live_client::live::live::{NoSuchRoom, QuotaExceeded, Tick};
use live_client::live::live_client::{LiveClient, RoomEventsRequest, TicksRequest};
use live_client::live::vault_client::VaultClient;
use live_client::prelude::Unauthorized;
use live_client::veld::client::{ClientOptions, Reconnect, SseOptions};
use live_client::veld::stream::{BoxStream, next};
use live_client::veld::{DeclaredError, Error};

use crate::check::{equal, that};
use crate::stream_impl::Probe;

pub type Rt = live_client::veld::transport_reqwest::ReqwestTransport;

fn live(base: &str) -> LiveClient<Rt> {
    LiveClient::reqwest(ClientOptions::new().base_url(base)).unwrap_or_else(|e| panic!("client: {e}"))
}

fn vault(base: &str, token: Option<&str>) -> VaultClient<Rt> {
    let mut o = ClientOptions::new().base_url(base);
    if let Some(t) = token {
        let t = t.to_string();
        o = o.bearer(move || t.clone());
    }
    VaultClient::reqwest(o).unwrap_or_else(|e| panic!("client: {e}"))
}

/// Everything the stream yields, then its error, if any.
pub async fn drain(mut s: BoxStream<Result<Tick, Error>>) -> (Vec<i32>, Option<Error>) {
    let mut ns = Vec::new();
    while let Some(item) = next(&mut s).await {
        match item {
            Ok(t) => ns.push(t.n),
            Err(e) => return (ns, Some(e)),
        }
    }
    (ns, None)
}

async fn until(what: &str, mut cond: impl FnMut() -> bool) {
    for _ in 0..500 {
        if cond() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("timed out waiting for: {what}");
}

async fn get(url: &str) -> (u16, String, String) {
    let resp = reqwest::get(url).await.unwrap_or_else(|e| panic!("{url}: {e}"));
    let status = resp.status().as_u16();
    let ct = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    (status, ct, resp.text().await.unwrap_or_default())
}

pub async fn sse(base: &str, probe: Option<&Arc<Probe>>) {
    let live = live(base);

    // Ordered items, decoded to the contract type.
    let s = live.ticks(TicksRequest { count: 5, fail_at: None }).await.unwrap_or_else(|e| panic!("ticks: {e}"));
    let mut s = s;
    let mut ticks = Vec::new();
    while let Some(t) = next(&mut s).await {
        ticks.push(t.unwrap_or_else(|e| panic!("tick: {e}")));
    }
    equal(vec![0, 1, 2, 3, 4], ticks.iter().map(|t| t.n).collect::<Vec<_>>(), "ordered items");
    equal("tick 2", ticks[2].label.as_str(), "label");
    equal("2024-05-01T12:00:00Z", ticks[0].when.as_str(), "datetime decoded");
    let (none, err) = drain(live.ticks(TicksRequest { count: 0, fail_at: None }).await.unwrap_or_else(|e| panic!("{e}"))).await;
    that(none.is_empty() && err.is_none(), "an empty stream");

    // A declared error before the first event is a normal problem response, returned from the call.
    let early = live.ticks(TicksRequest { count: -1, fail_at: None }).await.err().unwrap_or_else(|| panic!("expected an early error"));
    let q = QuotaExceeded::from_error(&early).unwrap_or_else(|| panic!("typed error before the first event: {early}"));
    equal(0, q.limit, "early data");
    equal(Some(429), early.status(), "early status");
    that(early.op().is_some_and(|op| op.contains("Live.ticks")), "op carries the operation");

    // After streaming has started the error is the final `event: error`, the stream's last item.
    let (got, late) = drain(live.ticks(TicksRequest { count: 10, fail_at: Some(2) }).await.unwrap_or_else(|e| panic!("{e}"))).await;
    equal(vec![0, 1], got, "items before the error");
    let late = late.unwrap_or_else(|| panic!("expected a final error"));
    equal(2, QuotaExceeded::from_error(&late).map_or(-1, |q| q.limit), "late data");
    that(QuotaExceeded::matches(&late), "matches");
    that(!live_client::veld::same(&early, &late), "different data is not the same error");

    // Dropping the stream mid-way closes the request: the server sees the client go away.
    let mut forever = live.forever().await.unwrap_or_else(|e| panic!("forever: {e}"));
    let mut first = Vec::new();
    for _ in 0..3 {
        if let Some(Ok(t)) = next(&mut forever).await {
            first.push(t.n);
        }
    }
    equal(vec![0, 1, 2], first, "forever items");
    drop(forever);
    if let Some(p) = probe {
        until("the server notices the closed stream", || p.forever_closed.load(std::sync::atomic::Ordering::SeqCst) >= 1).await;
    }

    // Event names, ids and resuming with Last-Event-ID.
    let ids = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&ids);
    let options = SseOptions::default().on_event_id(move |id| sink.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(id.to_string()));
    let events = live.room_events_with(RoomEventsRequest { room: "r".into() }, options).await.unwrap_or_else(|e| panic!("{e}"));
    equal(vec![0, 1, 2], drain(events).await.0, "room events");
    equal(vec!["0".to_string(), "1".into(), "2".into()], ids.lock().map(|i| i.clone()).unwrap_or_default(), "event ids");
    let resumed = live.room_events_with(RoomEventsRequest { room: "r".into() }, SseOptions::resume_after("2")).await.unwrap_or_else(|e| panic!("{e}"));
    equal(vec![3, 4, 5], drain(resumed).await.0, "resumed");
    let missing = live.room_events(RoomEventsRequest { room: "nope".into() }).await.err().unwrap_or_else(|| panic!("expected NoSuchRoom"));
    equal("nope", NoSuchRoom::from_error(&missing).map_or(String::new(), |m| m.room).as_str(), "room");

    wire(base).await;
}

/// The bytes on the wire: framing, canonical JSON, ids and event names, the final error event.
async fn wire(base: &str) {
    let (status, ct, body) = get(&format!("{base}/live/ticks?count=2")).await;
    equal(200, status, "status");
    that(ct.starts_with("text/event-stream"), "content type");
    equal(
        "data: {\"label\":\"tick 0\",\"n\":0,\"when\":\"2024-05-01T12:00:00Z\"}\n\ndata: {\"label\":\"tick 1\",\"n\":1,\"when\":\"2024-05-01T12:00:00Z\"}\n\n",
        body.as_str(),
        "frames",
    );
    let (_, _, named) = get(&format!("{base}/live/rooms/r/events")).await;
    that(named.starts_with("id: 0\nevent: tick\ndata: {\"label\""), &named);
    let (_, _, failed) = get(&format!("{base}/live/ticks?count=3&failAt=1")).await;
    that(failed.contains("\n\nevent: error\ndata: {") && failed.contains("\"data\":{\"limit\":1}") && failed.ends_with("}\n\n"), &failed);
    let (status, ct, _) = get(&format!("{base}/live/ticks?count=-1")).await;
    equal(429, status, "refused status");
    equal("application/problem+json", ct.as_str(), "problem type");
    equal(422, get(&format!("{base}/live/ticks?count=x")).await.0, "bad query");
}

pub async fn auth(base: &str) {
    let denied = vault(base, None).feed().await.err().unwrap_or_else(|| panic!("expected 401"));
    that(Unauthorized::matches(&denied), "no token");
    equal(Some(401), denied.status(), "status");
    let wrong = vault(base, Some("bad")).feed().await.err().unwrap_or_else(|| panic!("expected 401"));
    that(Unauthorized::matches(&wrong), "bad token");
    let feed = vault(base, Some("good")).feed().await.unwrap_or_else(|e| panic!("feed: {e}"));
    equal(vec![0, 1], drain(feed).await.0, "authenticated feed");
}

async fn seen(base: &str, key: &str) -> Vec<serde_json::Value> {
    let (_, _, body) = get(&format!("{base}/__seen?key={key}")).await;
    serde_json::from_str::<Vec<serde_json::Value>>(&body).unwrap_or_default()
}

/// Reconnect with exponential backoff, against the flaky Node server (`tests/toolchain/ts/flaky.ts`),
/// which drops connections on purpose.
pub async fn reconnect(base: &str) {
    let fast = Reconnect { initial_ms: 30, max_ms: 100, retries: 5 };
    let plain = live(base);
    let live = LiveClient::reqwest(ClientOptions::new().base_url(base).reconnect(fast)).unwrap_or_else(|e| panic!("{e}"));

    // Off by default: the broken stream is a transport error and nothing is retried.
    let (got, err) = drain(plain.room_events(RoomEventsRequest { room: "resume-plain".into() }).await.unwrap_or_else(|e| panic!("{e}"))).await;
    equal(vec![0, 1], got, "items before the break");
    that(matches!(err, Some(Error::Transport(_))), "a broken stream is a transport error");
    equal(1, seen(base, "resume-plain").await.len(), "no reconnect by default");

    // With a policy: resumes with Last-Event-ID, one continuous stream.
    let ids = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&ids);
    let opts = SseOptions::default().on_event_id(move |id| sink.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(id.to_string()));
    let s = live.room_events_with(RoomEventsRequest { room: "resume-a".into() }, opts).await.unwrap_or_else(|e| panic!("{e}"));
    let (got, err) = drain(s).await;
    equal(vec![0, 1, 2, 3], got, "continuous stream");
    that(err.is_none(), "no error");
    equal(vec!["0".to_string(), "1".into(), "2".into(), "3".into()], ids.lock().map(|i| i.clone()).unwrap_or_default(), "event ids");
    let conns = seen(base, "resume-a").await;
    equal(2, conns.len(), "two connections");
    that(conns[0]["lastEventId"].is_null(), "the first had no Last-Event-ID");
    equal(Some("1"), conns[1]["lastEventId"].as_str(), "resumed after the last event");

    // A per-call policy on a client that has none.
    let per_call = plain.room_events_with(RoomEventsRequest { room: "resume-b".into() }, SseOptions::default().reconnect(fast)).await.unwrap_or_else(|e| panic!("{e}"));
    equal(vec![0, 1, 2, 3], drain(per_call).await.0, "per-call policy");

    // Backoff and exhaustion: the first attempt plus `retries`.
    let three = LiveClient::reqwest(ClientOptions::new().base_url(base).reconnect(Reconnect { initial_ms: 30, max_ms: 100, retries: 3 })).unwrap_or_else(|e| panic!("{e}"));
    let (_, err) = drain(three.room_events(RoomEventsRequest { room: "always-a".into() }).await.unwrap_or_else(|e| panic!("{e}"))).await;
    that(matches!(err, Some(Error::Transport(_))), "retries exhausted");
    let always = seen(base, "always-a").await;
    equal(4, always.len(), "first attempt and three retries");
    let (t0, t3) = (always[0]["at"].as_f64().unwrap_or(0.0), always[3]["at"].as_f64().unwrap_or(0.0));
    that(t3 - t0 >= f64::from(30 + 60 + 100 - 15), &format!("backoff 30, 60, 100: {}", t3 - t0));

    // A typed error is an answer: no retry.
    let missing = live.room_events(RoomEventsRequest { room: "missing-a".into() }).await.err().unwrap_or_else(|| panic!("expected NoSuchRoom"));
    that(NoSuchRoom::matches(&missing), "typed error");
    equal(1, seen(base, "missing-a").await.len(), "a typed error is never retried");

    ws_reconnect(base, &plain, &live).await;
}

/// WebSocket reconnect: off by default, on with a policy, exhaustion, typed error frames.
async fn ws_reconnect(base: &str, plain: &LiveClient<Rt>, live: &LiveClient<Rt>) {
    use live_client::live::live_client::ChatRequest;
    let room = |r: &str| ChatRequest { room: r.into(), nick: None };

    // Off by default: the abnormal close is a transport error.
    let mut c = plain.chat(room("resume-plain")).await.unwrap_or_else(|e| panic!("{e}"));
    equal(0, c.recv().await.and_then(Result::ok).map_or(-1, |h| h.seq), "frames before the drop");
    that(matches!(c.recv().await, Some(Err(Error::Transport(_)))), "abnormal close");
    equal(1, seen(base, "ws:resume-plain").await.len(), "no reconnect by default");

    // On: reopened, the iteration continues.
    let mut c = live.chat(room("resume-a")).await.unwrap_or_else(|e| panic!("{e}"));
    let mut heard = Vec::new();
    while let Some(h) = c.recv().await {
        heard.push(h.unwrap_or_else(|e| panic!("{e}")).seq);
    }
    equal(vec![0, 1, 2], heard, "continuous channel");
    equal(2, seen(base, "ws:resume-a").await.len(), "two connections");

    // Retries exhausted; a typed error frame is never retried.
    let two = LiveClient::reqwest(ClientOptions::new().base_url(base).reconnect(Reconnect { initial_ms: 20, max_ms: 5000, retries: 2 })).unwrap_or_else(|e| panic!("{e}"));
    let mut c = two.chat(room("always-a")).await.unwrap_or_else(|e| panic!("{e}"));
    that(matches!(c.recv().await, Some(Err(Error::Transport(_)))), "exhausted");
    equal(3, seen(base, "ws:always-a").await.len(), "first attempt and two retries");
    let mut c = live.chat(room("nosuch-a")).await.unwrap_or_else(|e| panic!("{e}"));
    let err = c.recv().await.and_then(Result::err).unwrap_or_else(|| panic!("expected NoSuchRoom"));
    that(NoSuchRoom::matches(&err), "typed error frame");
    equal(1, seen(base, "ws:nosuch-a").await.len(), "never retried");
}

// ---------------------------------------------------------------------------
// WebSocket
// ---------------------------------------------------------------------------

use futures_util::{SinkExt, StreamExt};
use live_client::live::live::{Heard, Say};
use live_client::live::live_client::ChatRequest;
use live_client::veld::Error as VeldError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

fn say(text: &str, loud: Option<bool>) -> Say {
    Say { text: text.into(), loud }
}

fn heard(from: &str, text: &str, seq: i32) -> Heard {
    Heard { from: from.into(), text: text.into(), seq }
}

/// A raw WebSocket, to see the frames and close codes themselves.
struct Raw {
    socket: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
}

impl Raw {
    async fn open(url: &str, headers: &[(&str, &str)]) -> Result<Raw, String> {
        let mut request = url.into_client_request().map_err(|e| e.to_string())?;
        for (k, v) in headers {
            if let (Ok(k), Ok(v)) = (
                tokio_tungstenite::tungstenite::http::HeaderName::from_bytes(k.as_bytes()),
                tokio_tungstenite::tungstenite::http::HeaderValue::from_str(v),
            ) {
                request.headers_mut().insert(k, v);
            }
        }
        let (socket, _) = tokio_tungstenite::connect_async(request).await.map_err(|e| e.to_string())?;
        Ok(Raw { socket })
    }

    async fn send(&mut self, text: &str) {
        self.socket.send(Message::Text(text.into())).await.unwrap_or_else(|e| panic!("raw send: {e}"));
    }

    /// The next text frame.
    async fn frame(&mut self) -> String {
        loop {
            match self.socket.next().await {
                Some(Ok(Message::Text(t))) => return t.to_string(),
                Some(Ok(_)) => {}
                other => panic!("expected a text frame, got {other:?}"),
            }
        }
    }

    /// The close code the server ends with.
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

fn ws_base(base: &str) -> String {
    base.replacen("http", "ws", 1)
}


pub async fn ws(base: &str, probe: Option<&Arc<Probe>>, closes_4404: bool) {
    let live = live(base);

    // Bidirectional chat: replies are typed.
    let before = probe.map_or(0, |p| p.chat_ended.load(std::sync::atomic::Ordering::SeqCst));
    let mut chat = live.chat(ChatRequest { room: "r1".into(), nick: Some("ada".into()) }).await.unwrap_or_else(|e| panic!("chat: {e}"));
    chat.send(say("hi", None)).await.unwrap_or_else(|e| panic!("send: {e}"));
    chat.send(say("yo", Some(true))).await.unwrap_or_else(|e| panic!("send: {e}"));
    equal(heard("ada", "hi", 0), chat.recv().await.and_then(Result::ok).unwrap_or_else(|| panic!("first reply")), "first reply");
    equal(heard("ada", "YO", 1), chat.recv().await.and_then(Result::ok).unwrap_or_else(|| panic!("second reply")), "second reply");
    chat.close().await;
    if let Some(p) = probe {
        until("the server sees the chat end", || p.chat_ended.load(std::sync::atomic::Ordering::SeqCst) > before).await;
    }

    // The client validates outgoing messages.
    let mut guarded = live.chat(ChatRequest { room: "r2".into(), nick: None }).await.unwrap_or_else(|e| panic!("{e}"));
    let invalid = guarded.send(say("", None)).await.err().unwrap_or_else(|| panic!("expected a local ValidationError"));
    that(live_client::prelude::ValidationError::matches(&invalid), "local validation");
    guarded.close().await;

    // The server validates incoming messages: a bad one comes back as a typed ValidationError.
    let lax = LiveClient::reqwest(ClientOptions::new().base_url(base).validate(false)).unwrap_or_else(|e| panic!("{e}"));
    let mut bad = lax.chat(ChatRequest { room: "r3".into(), nick: None }).await.unwrap_or_else(|e| panic!("{e}"));
    bad.send(say("", None)).await.unwrap_or_else(|e| panic!("{e}"));
    let failure = bad.recv().await.and_then(Result::err).unwrap_or_else(|| panic!("expected a ValidationError frame"));
    that(live_client::prelude::ValidationError::matches(&failure), "server validation");
    equal(Some(422), failure.status(), "remote status");
    that(failure.problem().is_some_and(|p| p.data.to_string().contains("\"path\":\"message")), "issue paths start with message");
    bad.close().await;

    // A declared error closes the socket after a problem frame, typed on the client.
    let mut locked = live.chat(ChatRequest { room: "locked".into(), nick: None }).await.unwrap_or_else(|e| panic!("{e}"));
    let err = locked.recv().await.and_then(Result::err).unwrap_or_else(|| panic!("expected NoSuchRoom"));
    equal("locked", NoSuchRoom::from_error(&err).map_or(String::new(), |e| e.room).as_str(), "room");
    that(err.op().is_some_and(|op| op.contains("Live.chat")), "op carries the operation");

    // Server-only and client-only endpoints.
    let mut announce = live.announce().await.unwrap_or_else(|e| panic!("{e}"));
    let mut announced = Vec::new();
    while let Some(t) = announce.recv().await {
        announced.push(t.unwrap_or_else(|e| panic!("announce: {e}")).n);
    }
    equal(vec![0, 1, 2], announced, "announced");
    let mut sink = live.sink().await.unwrap_or_else(|e| panic!("{e}"));
    sink.send(say("a", None)).await.unwrap_or_else(|e| panic!("{e}"));
    sink.send(say("b", None)).await.unwrap_or_else(|e| panic!("{e}"));
    that(sink.recv().await.is_none(), "the server ends the sink normally");
    if let Some(p) = probe {
        equal(vec!["a".to_string(), "b".into()], p.sink.lock().map(|s| s.clone()).unwrap_or_default(), "the sink saw both");
    }

    raw(base, closes_4404).await;

    // An unreachable server is a transport error, not a hang.
    let dead = LiveClient::reqwest(ClientOptions::new().base_url("http://127.0.0.1:1")).unwrap_or_else(|e| panic!("{e}"));
    that(matches!(dead.announce().await, Err(VeldError::Transport(_))), "unreachable");
}

/// Raw frames and close codes: envelopes of canonical JSON, 4404 (or a refused handshake), 1008.
async fn raw(base: &str, closes_4404: bool) {
    let ws = ws_base(base);
    if closes_4404 {
        let mut nowhere = Raw::open(&format!("{ws}/nowhere"), &[]).await.unwrap_or_else(|e| panic!("{e}"));
        equal(4404, nowhere.close_code().await, "not a ws endpoint");
    } else {
        that(Raw::open(&format!("{ws}/nowhere"), &[]).await.is_err(), "the handshake of a path that is not a ws endpoint is refused");
    }
    let mut raw = Raw::open(&format!("{ws}/live/chat/raw?nick=zed"), &[]).await.unwrap_or_else(|e| panic!("{e}"));
    raw.send("{\"loud\":false,\"text\":\"a\"}").await;
    equal("{\"data\":{\"from\":\"zed\",\"seq\":0,\"text\":\"a\"}}", raw.frame().await.as_str(), "envelope");
    raw.send("not json").await;
    let problem = raw.frame().await;
    that(problem.starts_with("{\"error\":{") && problem.contains("\"title\":\"ValidationError\"") && problem.contains("\"status\":422"), &problem);
    equal(1008, raw.close_code().await, "close code of a client-side error");
}

/// WebSocket auth: the same hook, a problem frame and close 1008; the token travels in a header.
pub async fn ws_auth(base: &str) {
    let anonymous = vault(base, None);
    let mut rejected = anonymous.echo().await.unwrap_or_else(|e| panic!("{e}"));
    let _ = rejected.send(say("hi", None)).await;
    let denied = rejected.recv().await.and_then(Result::err).unwrap_or_else(|| panic!("expected Unauthorized"));
    that(Unauthorized::matches(&denied), "ws no token");
    let wrong = vault(base, Some("bad"));
    let mut bad = wrong.echo().await.unwrap_or_else(|e| panic!("{e}"));
    let denied = bad.recv().await.and_then(Result::err).unwrap_or_else(|| panic!("expected Unauthorized"));
    that(Unauthorized::matches(&denied), "ws bad token");
    let good = vault(base, Some("good"));
    let mut echo = good.echo().await.unwrap_or_else(|e| panic!("{e}"));
    echo.send(say("one", None)).await.unwrap_or_else(|e| panic!("{e}"));
    echo.send(say("two", None)).await.unwrap_or_else(|e| panic!("{e}"));
    equal(heard("vault", "one", 0), echo.recv().await.and_then(Result::ok).unwrap_or_else(|| panic!("first echo")), "first echo");
    equal(heard("vault", "two", 1), echo.recv().await.and_then(Result::ok).unwrap_or_else(|| panic!("second echo")), "second echo");
    echo.close().await;
    // The token also travels as `?access_token=` (browsers cannot set headers on a WebSocket).
    let mut query = Raw::open(&format!("{}/vault/echo?access_token=good", ws_base(base)), &[]).await.unwrap_or_else(|e| panic!("{e}"));
    query.send("{\"text\":\"q\"}").await;
    equal("{\"data\":{\"from\":\"vault\",\"seq\":0,\"text\":\"q\"}}", query.frame().await.as_str(), "echo with the token in the query");
    let mut raw = Raw::open(&format!("{}/vault/echo", ws_base(base)), &[("authorization", "Bearer good")]).await.unwrap_or_else(|e| panic!("{e}"));
    raw.send("{\"text\":\"h\"}").await;
    equal("{\"data\":{\"from\":\"vault\",\"seq\":0,\"text\":\"h\"}}", raw.frame().await.as_str(), "raw echo");
}
