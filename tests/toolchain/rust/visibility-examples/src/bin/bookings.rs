//! Serves the generated test handler of the service `Bookings` of
//! `tests/rules/endpoints/visibility.veld.pending` on a free port with stub application handlers,
//! and prints `PORT=<n>`. The examples of that service are sent to it. (`Notices` is the other
//! binary: a workspace has one test handler per service.)

use guests_server::guests::bookings_server::{
    BookingsHooks, BookingsService, CancelBookingFacts, CancelBookingRequest, CheckedBookingRequest,
    GetBookingRequest, ListBookingsRequest, PageBookingsRequest,
};
use guests_server::guests::bookings_test_server::{BookingsTestHandler, BookingsTestHooks, router};
use guests_server::guests::guests::{Booking, User};
use guests_server::veld::server::RequestContext;
use guests_server::veld::{Error, Page};

/// The application handlers are never reached: an example without `returns` stops after step 6
/// (the dry run), and one with it answers what it says.
struct Stub;

fn reached<T>() -> Result<T, Error> {
    Err(Error::other("the stub handler was reached"))
}

impl BookingsService for Stub {
    async fn get_booking(&self, _request: GetBookingRequest, _ctx: &RequestContext) -> Result<Booking, Error> {
        reached()
    }

    async fn list_bookings(&self, _request: ListBookingsRequest, _ctx: &RequestContext) -> Result<Vec<Booking>, Error> {
        reached()
    }

    async fn page_bookings(&self, _request: PageBookingsRequest, _ctx: &RequestContext) -> Result<Page<Booking>, Error> {
        reached()
    }

    async fn checked_booking(&self, _request: CheckedBookingRequest, _ctx: &RequestContext) -> Result<Booking, Error> {
        reached()
    }

    async fn cancel_booking(
        &self,
        _request: CancelBookingRequest,
        _facts: CancelBookingFacts,
        _ctx: &RequestContext,
    ) -> Result<Booking, Error> {
        reached()
    }
}

/// The test handler never calls the authentication hook.
struct Hooks;

impl BookingsTestHooks for Hooks {}

impl BookingsHooks for Hooks {
    async fn auth_bearer<'a>(&'a self, _token: &'a str, _ctx: &'a RequestContext) -> Option<User> {
        None
    }
}

#[tokio::main]
async fn main() {
    let app = router(BookingsTestHandler::create(Stub, Hooks));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    println!("PORT={}", listener.local_addr().unwrap().port());
    axum::serve(listener, app).await.unwrap();
}
