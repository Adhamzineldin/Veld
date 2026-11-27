//! Serves the generated test handler of `tests/rules/endpoints/idempotency.veld.pending` on a free
//! port with stub application handlers, and prints `PORT=<n>`. The examples are sent to it.

use bookings_server::bookings::bookings::{Booking, Shelf};
use bookings_server::bookings::bookings_server::{
    BookFacts, BookMineRequest, BookRequest, BookingsHooks, BookingsService, CancelRequest,
    PayRequest, ReplaceRequest, SaveRoomFacts, SaveRoomRequest,
};
use bookings_server::bookings::bookings_test_server::{BookingsTestHandler, BookingsTestHooks, router};
use bookings_server::veld::Error;
use bookings_server::veld::idempotency::MemoryIdempotencyStore;
use bookings_server::veld::server::{Principal, RequestContext};

/// The application handlers are never reached: an example without `returns` stops after step 6
/// (the dry run), a replay and a conflict stop before it, and one with `returns` brings its result.
struct Stub;

fn reached<T>() -> Result<T, Error> {
    Err(Error::other("the stub handler was reached"))
}

impl BookingsService for Stub {
    async fn book(&self, _request: BookRequest, _facts: BookFacts, _ctx: &RequestContext) -> Result<Booking, Error> {
        reached()
    }

    async fn pay(&self, _request: PayRequest, _ctx: &RequestContext) -> Result<Booking, Error> {
        reached()
    }

    async fn replace(&self, _request: ReplaceRequest, _ctx: &RequestContext) -> Result<Booking, Error> {
        reached()
    }

    async fn cancel(&self, _request: CancelRequest, _ctx: &RequestContext) -> Result<(), Error> {
        reached()
    }

    async fn book_mine(&self, _request: BookMineRequest, _ctx: &RequestContext) -> Result<Booking, Error> {
        reached()
    }

    async fn save_room(
        &self,
        _request: SaveRoomRequest,
        _facts: SaveRoomFacts,
        _version: Option<i64>,
        _ctx: &RequestContext,
    ) -> Result<Shelf, Error> {
        reached()
    }
}

/// The test handler has a store of its own for every request; the hooks' is never asked.
struct Hooks(MemoryIdempotencyStore);

impl BookingsTestHooks for Hooks {}

impl BookingsHooks for Hooks {
    type IdempotencyStore = MemoryIdempotencyStore;

    fn idempotency_store(&self) -> &MemoryIdempotencyStore {
        &self.0
    }

    fn idempotency_caller(&self, _principal: &Principal, _ctx: &RequestContext) -> String {
        "never asked".to_string()
    }

    async fn auth_bearer<'a>(&'a self, _token: &'a str, _ctx: &'a RequestContext) -> Option<Principal> {
        None
    }
}

#[tokio::main]
async fn main() {
    let app = router(BookingsTestHandler::create(Stub, Hooks(MemoryIdempotencyStore::new())));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    println!("PORT={}", listener.local_addr().unwrap().port());
    axum::serve(listener, app).await.unwrap();
}
