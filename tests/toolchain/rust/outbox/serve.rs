//! Serves the generated test handlers of `tests/events/outbox.veld.pending` behind one address:
//! the moves of `Bookings` (with stub application handlers: a call example has `returns`, so the
//! test handler does not call them) and the event endpoint of `Notifications`. Prints `PORT=<n>`;
//! the examples are sent to it with `veld_testing::Live`.
use axum::body::Bytes;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use events_server::bookings::bookings_events::{CancelEmit, PayEmit};
use events_server::bookings::bookings_server::{
    BookingsHooks, BookingsService, CancelFacts, CancelRequest, PayFacts, PayRequest, RefundFacts, RefundRequest,
};
use events_server::bookings::bookings_test_server::{self as bookings, BookingsTestHandler, BookingsTestHooks};
use events_server::bookings::notifications_test_server as notifications;
use events_server::bookings::t::{Booking, Status};
use events_server::veld::Error;
use events_server::veld::events::MemoryOutbox;
use events_server::veld::rules::Transition;
use events_server::veld::server::RequestContext;
use serde_json::{Value, json};

struct Stub;

impl BookingsService for Stub {
    async fn pay(&self, _r: PayRequest, _f: PayFacts, _t: Transition<Status>, _e: PayEmit<'_>, _ctx: &RequestContext) -> Result<Booking, Error> {
        Err(Error::other("the stub handler was reached"))
    }

    async fn cancel(&self, _r: CancelRequest, _f: CancelFacts, _t: Transition<Status>, _e: CancelEmit<'_>, _ctx: &RequestContext) -> Result<Booking, Error> {
        Err(Error::other("the stub handler was reached"))
    }

    async fn refund(&self, _r: RefundRequest, _f: RefundFacts, _t: Transition<Status>, _ctx: &RequestContext) -> Result<Booking, Error> {
        Err(Error::other("the stub handler was reached"))
    }
}

/// The test handler brings its own outbox and never asks the one of its hooks.
struct Hooks(MemoryOutbox);

impl BookingsHooks for Hooks {
    type Outbox = MemoryOutbox;

    fn outbox(&self) -> &MemoryOutbox {
        &self.0
    }
}

impl BookingsTestHooks for Hooks {}

fn merged_probe() -> String {
    let (mut digests, mut subscribes) = (serde_json::Map::new(), serde_json::Map::new());
    for text in [bookings::probe(), notifications::probe()] {
        let one: Value = serde_json::from_str(&text).unwrap();
        digests.extend(one["digests"].as_object().cloned().unwrap_or_default());
        subscribes.extend(one.get("subscribes").and_then(Value::as_object).cloned().unwrap_or_default());
    }
    json!({ "veldTestHandler": 1, "digests": digests, "subscribes": subscribes }).to_string()
}

async fn event(headers: HeaderMap, body: Bytes) -> axum::response::Response {
    let text = |name: &str| headers.get(name).and_then(|v| v.to_str().ok()).map(str::to_string);
    let Some(topic) = text("veld-test-topic").filter(|t| !t.is_empty()) else {
        return (StatusCode::BAD_REQUEST, [(header::CONTENT_TYPE, "application/json")], json!({ "error": "Veld-Test-Topic is missing or empty" }).to_string()).into_response();
    };
    let key = text("veld-test-key");
    let deliveries: u8 = if text("veld-test-deliveries").as_deref() == Some("2") { 2 } else { 1 };
    let outcome = notifications::deliver_event(&topic, key.as_deref(), deliveries, &body).await.unwrap();
    ([(header::CONTENT_TYPE, "application/json")], outcome.to_json().to_string()).into_response()
}

#[tokio::main]
async fn main() {
    let handler = BookingsTestHandler::create(Stub, Hooks(MemoryOutbox::new()));
    let app = events_server::adapters::axum::router(handler)
        .route("/__veld/test", get(|| async { ([(header::CONTENT_TYPE, "application/json")], merged_probe()) }))
        .route("/__veld/test/event", post(event));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    println!("PORT={}", listener.local_addr().unwrap().port());
    axum::serve(listener, app).await.unwrap();
}
