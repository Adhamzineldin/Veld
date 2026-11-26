//! The server of `tests/toolchain/contract/platform_p2/service_levels.veld` that the programs
//! `deadline` and `slo` drive: handlers that wait for as long as a test says, and hooks that record
//! what they see.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use rooms_server::rooms::rooms::Room;
use rooms_server::rooms::rooms_server::{BuildRequest, GetRoomRequest, ListRoomsRequest, RoomsHandler, RoomsHooks, RoomsService};
use rooms_server::prelude::NotFound;
use rooms_server::veld::Error;
use rooms_server::veld::idempotency::MemoryIdempotencyStore;
use rooms_server::veld::server::{Metric, RequestContext};

pub const ID: &str = "00000000-0000-4000-8000-000000000001";

/// What a test controls and what it reads back.
#[derive(Default)]
pub struct World {
    /// How long the handlers wait before they answer.
    pub delay_ms: Mutex<u64>,
    /// What the next `getRoom` answers instead of a room: `missing` is a declared `NotFound`, `boom`
    /// an error that becomes `Internal`.
    pub fail: Mutex<Option<&'static str>>,
    /// The handlers that ran to their end, by name (a cancelled handler never gets there).
    pub finished: Mutex<Vec<String>>,
    /// What the handlers saw of the deadline: `<endpoint>:<has a deadline>:<expired at the start>`.
    pub saw: Mutex<Vec<String>>,
    /// How often `build` was entered.
    pub builds: Mutex<usize>,
    pub store: MemoryIdempotencyStore,
    /// The metrics the hook received.
    pub metrics: Mutex<Vec<Metric>>,
    /// The hook panics after it recorded the metric.
    pub metrics_panic: Mutex<bool>,
    /// What went to `on_error`.
    pub errors: Mutex<usize>,
}

pub struct Impl(pub Arc<World>);

fn room() -> Room {
    Room { id: ID.into(), name: "A".into() }
}

async fn wait(world: &World) {
    let ms = *world.delay_ms.lock().unwrap();
    if ms > 0 {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }
}

impl RoomsService for Impl {
    async fn get_room(&self, _request: GetRoomRequest, ctx: &RequestContext) -> Result<Room, Error> {
        self.0.saw.lock().unwrap().push(format!("getRoom:{}:{}", ctx.deadline.is_some(), ctx.expired()));
        wait(&self.0).await;
        self.0.finished.lock().unwrap().push("getRoom".into());
        match self.0.fail.lock().unwrap().take() {
            Some("missing") => Err(NotFound { resource: Some("room".into()) }.into()),
            Some(_) => Err(Error::other("boom")),
            None => Ok(room()),
        }
    }

    async fn list_rooms(&self, _request: ListRoomsRequest, ctx: &RequestContext) -> Result<Vec<Room>, Error> {
        self.0.saw.lock().unwrap().push(format!("listRooms:{}:{}", ctx.deadline.is_some(), ctx.expired()));
        wait(&self.0).await;
        self.0.finished.lock().unwrap().push("listRooms".into());
        Ok(vec![room()])
    }

    async fn build(&self, _request: BuildRequest, _ctx: &RequestContext) -> Result<Room, Error> {
        *self.0.builds.lock().unwrap() += 1;
        wait(&self.0).await;
        self.0.finished.lock().unwrap().push("build".into());
        Ok(room())
    }
}

pub struct Hooks(pub Arc<World>);

impl RoomsHooks for Hooks {
    type IdempotencyStore = MemoryIdempotencyStore;

    fn idempotency_store(&self) -> &MemoryIdempotencyStore {
        &self.0.store
    }

    fn on_error(&self, _err: &Error, _ctx: &RequestContext) {
        *self.0.errors.lock().unwrap() += 1;
    }

    fn metrics(&self, metric: &Metric) {
        self.0.metrics.lock().unwrap().push(metric.clone());
        assert!(!*self.0.metrics_panic.lock().unwrap(), "the metrics hook fails");
    }
}

pub struct Server {
    pub base: String,
    pub world: Arc<World>,
}

/// Serves the production handler on a free port.
pub async fn serve() -> Server {
    let world = Arc::new(World::default());
    let handler = RoomsHandler::create(Impl(world.clone()), Hooks(world.clone()));
    let router = rooms_server::adapters::axum::router(handler);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Server { base, world }
}

pub struct Answer {
    pub status: u16,
    pub content_type: String,
    pub replayed: bool,
    pub doc: serde_json::Value,
}

pub async fn send(s: &Server, method: &str, path: &str, key: Option<&str>, body: Option<serde_json::Value>) -> Answer {
    let client = reqwest::Client::new();
    let url = format!("{}{path}", s.base);
    let mut req = if method == "POST" { client.post(url) } else { client.get(url) };
    if let Some(key) = key {
        req = req.header("idempotency-key", key);
    }
    if let Some(body) = body {
        req = req.header("content-type", "application/json").body(body.to_string());
    }
    let response = req.send().await.unwrap();
    let status = response.status().as_u16();
    let content_type = response.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or_default().to_string();
    let replayed = response.headers().contains_key("veld-idempotent-replay");
    let text = response.text().await.unwrap();
    Answer { status, content_type, replayed, doc: serde_json::from_str(&text).unwrap_or(serde_json::Value::Null) }
}
