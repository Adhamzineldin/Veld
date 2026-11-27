//! Serves the generated test handlers of `tests/rules/endpoints/version.veld.pending` (the
//! services `Rooms` and `Orders`) on one free port with stub application handlers, and prints
//! `PORT=<n>`. The examples are sent to it.

use axum::routing::get;
use rooms_server::rooms::orders_server::{OrdersHooks, OrdersService, PayOrderFacts, PayOrderRequest};
use rooms_server::rooms::orders_test_server::{OrdersTestHandler, OrdersTestHooks};
use rooms_server::rooms::rooms::{Counter, Order, Room, Status, User};
use rooms_server::rooms::rooms_server::{
    GetRoomRequest, RenameFacts, RenameRequest, RoomsService, TickFacts, TickRequest, TouchFacts,
    TouchRequest,
};
use rooms_server::rooms::rooms_test_server::RoomsTestHandler;
use rooms_server::veld::rules::Transition;
use rooms_server::veld::server::RequestContext;
use rooms_server::veld::Error;

/// The application handlers are never reached: an example without `returns` stops after step 6
/// (the dry run) and one with `returns` brings its result.
struct Stub;

fn reached<T>() -> Result<T, Error> {
    Err(Error::other("the stub handler was reached"))
}

impl RoomsService for Stub {
    async fn rename(
        &self,
        _request: RenameRequest,
        _facts: RenameFacts,
        _version: Option<i64>,
        _ctx: &RequestContext,
    ) -> Result<Room, Error> {
        reached()
    }

    async fn touch(
        &self,
        _request: TouchRequest,
        _facts: TouchFacts,
        _version: Option<i64>,
        _ctx: &RequestContext,
    ) -> Result<Room, Error> {
        reached()
    }

    async fn tick(
        &self,
        _request: TickRequest,
        _facts: TickFacts,
        _version: Option<i32>,
        _ctx: &RequestContext,
    ) -> Result<Counter, Error> {
        reached()
    }

    async fn get_room(&self, _request: GetRoomRequest, _ctx: &RequestContext) -> Result<Room, Error> {
        reached()
    }
}

impl OrdersService for Stub {
    async fn pay_order(
        &self,
        _request: PayOrderRequest,
        _facts: PayOrderFacts,
        _version: Option<i64>,
        _transition: Transition<Status>,
        _ctx: &RequestContext,
    ) -> Result<Order, Error> {
        reached()
    }
}

/// The test handler never calls the authentication hook.
struct Hooks;

impl OrdersTestHooks for Hooks {}

impl OrdersHooks for Hooks {
    async fn auth_bearer<'a>(&'a self, _token: &'a str, _ctx: &'a RequestContext) -> Option<User> {
        None
    }
}

const PROBE: &str = r#"{"veldTestHandler":1,"digests":{"Rooms":null,"Orders":null}}"#;

#[tokio::main]
async fn main() {
    let rooms = rooms_server::adapters::axum::router(RoomsTestHandler::plain(Stub));
    let orders = rooms_server::adapters::axum::router(OrdersTestHandler::create(Stub, Hooks));
    let app = rooms.merge(orders).route(
        "/__veld/test",
        get(|| async { ([(axum::http::header::CONTENT_TYPE, "application/json")], PROBE) }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    println!("PORT={}", listener.local_addr().unwrap().port());
    axum::serve(listener, app).await.unwrap();
}
