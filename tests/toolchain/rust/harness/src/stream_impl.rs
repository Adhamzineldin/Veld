//! The Rust implementations of the streaming contract (`tests/toolchain/contract/streaming`), and
//! what the tests observe about them. Mirrors `tests/toolchain/java/StreamImpl.java`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use live_server::live::live::{Heard, NoSuchRoom, QuotaExceeded, Say, Tick};
use live_server::live::live_server::{
    AnnounceRequest, ChatRequest, ForeverRequest, LiveHandler, LiveService, RoomEventsRequest, SinkRequest, TicksRequest,
};
use live_server::live::vault_server::{EchoRequest, FeedRequest, VaultHandler, VaultHooks, VaultService};
use live_server::veld::server::{Principal, RequestContext, WsSession};
use live_server::veld::stream::{EventStream, SseEvent, iter, unfold};
use live_server::veld::{DeclaredError, Error};

/// What the tests can learn about the server.
#[derive(Default)]
pub struct Probe {
    pub forever_closed: AtomicUsize,
    pub room_events_closed: AtomicUsize,
    pub chat_ended: AtomicUsize,
    pub sink: Mutex<Vec<String>>,
    pub principals: Mutex<Vec<String>>,
}

pub fn tick(n: i32) -> Tick {
    Tick { n, label: format!("tick {n}"), when: "2024-05-01T12:00:00Z".into() }
}

/// Counts when the stream that owns it is dropped: the server stopped iterating.
struct Closed(Arc<Probe>, fn(&Probe) -> &AtomicUsize);

impl Drop for Closed {
    fn drop(&mut self) {
        (self.1)(&self.0).fetch_add(1, Ordering::SeqCst);
    }
}

#[derive(Clone)]
pub struct Live(pub Arc<Probe>);

impl LiveService for Live {
    async fn chat(&self, request: ChatRequest, _ctx: &RequestContext, mut ws: WsSession<Say, Heard>) -> Result<(), Error> {
        if request.room == "locked" {
            return Err(NoSuchRoom { room: request.room }.into_error());
        }
        let _ended = Closed(Arc::clone(&self.0), |p| &p.chat_ended);
        let nick = request.nick.unwrap_or_else(|| "anon".into());
        let mut seq = 0;
        while let Some(message) = ws.recv().await {
            let message = message?;
            let text = if message.loud == Some(true) { message.text.to_uppercase() } else { message.text };
            ws.send(Heard { from: nick.clone(), text, seq })?;
            seq += 1;
        }
        Ok(())
    }

    async fn announce(&self, _request: AnnounceRequest, _ctx: &RequestContext, ws: WsSession<(), Tick>) -> Result<(), Error> {
        for n in 0..3 {
            ws.send(tick(n))?;
        }
        Ok(())
    }

    async fn sink(&self, _request: SinkRequest, _ctx: &RequestContext, mut ws: WsSession<Say, ()>) -> Result<(), Error> {
        while let Some(message) = ws.recv().await {
            let message = message?;
            let mut seen = self.0.sink.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            seen.push(message.text);
            if seen.len() % 2 == 0 {
                return Ok(());
            }
        }
        Ok(())
    }

    async fn ticks(&self, request: TicksRequest, _ctx: &RequestContext) -> Result<EventStream<Tick>, Error> {
        if request.count < 0 {
            return Err(QuotaExceeded { limit: 0 }.into_error());
        }
        let (count, fail_at) = (request.count, request.fail_at);
        Ok(Box::pin(unfold(0, move |n: i32| {
            Box::pin(async move {
                if n >= count {
                    return None;
                }
                if fail_at == Some(n) {
                    return Some((Err(QuotaExceeded { limit: n }.into_error()), n + 1));
                }
                Some((Ok(SseEvent::of(tick(n))), n + 1))
            })
        })))
    }

    async fn forever(&self, _request: ForeverRequest, _ctx: &RequestContext) -> Result<EventStream<Tick>, Error> {
        let guard = Closed(Arc::clone(&self.0), |p| &p.forever_closed);
        Ok(Box::pin(unfold((0, guard), |(n, guard): (i32, Closed)| {
            Box::pin(async move {
                if n > 0 {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Some((Ok(SseEvent::of(tick(n))), (n + 1, guard)))
            })
        })))
    }

    async fn room_events(&self, request: RoomEventsRequest, ctx: &RequestContext) -> Result<EventStream<Tick>, Error> {
        if request.room == "nope" {
            return Err(NoSuchRoom { room: request.room }.into_error());
        }
        let from = ctx.header("last-event-id").and_then(|l| l.parse::<i32>().ok()).map_or(0, |l| l + 1);
        let guard = Closed(Arc::clone(&self.0), |p| &p.room_events_closed);
        Ok(Box::pin(unfold((from, guard), move |(n, guard): (i32, Closed)| {
            Box::pin(async move {
                if n >= from + 3 {
                    return None;
                }
                Some((Ok(SseEvent::of(tick(n)).event("tick").id(n.to_string())), (n + 1, guard)))
            })
        })))
    }
}

#[derive(Clone)]
pub struct Vault(pub Arc<Probe>);

impl VaultService for Vault {
    async fn echo(&self, _request: EchoRequest, ctx: &RequestContext, mut ws: WsSession<Say, Heard>) -> Result<(), Error> {
        if let Some(who) = ctx.principal::<String>() {
            self.0.principals.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(who.clone());
        }
        let mut seq = 0;
        while let Some(message) = ws.recv().await {
            ws.send(Heard { from: "vault".into(), text: message?.text, seq })?;
            seq += 1;
        }
        Ok(())
    }

    async fn feed(&self, _request: FeedRequest, ctx: &RequestContext) -> Result<EventStream<Tick>, Error> {
        if let Some(who) = ctx.principal::<String>() {
            self.0.principals.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(who.clone());
        }
        Ok(Box::pin(iter(vec![Ok(SseEvent::of(tick(0))), Ok(SseEvent::of(tick(1)))])))
    }
}

pub struct VaultAuth;

impl VaultHooks for VaultAuth {
    async fn auth_bearer(&self, token: &str, _ctx: &RequestContext) -> Option<Principal> {
        (token == "good").then(|| Principal::new("u1".to_string()))
    }
}

/// Both services behind one router, like the Node test server.
pub fn app(probe: &Arc<Probe>) -> axum::Router {
    let live = LiveHandler::plain(Live(Arc::clone(probe)));
    let vault = VaultHandler::create(Vault(Arc::clone(probe)), VaultAuth);
    live_server::adapters::axum::router(live).merge(live_server::adapters::axum::router(vault))
}
