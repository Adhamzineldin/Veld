//! Serves the generated test handler of `tests/rules/endpoints/tenancy.veld.pending` on a free port
//! with stub application handlers, and prints `PORT=<n>`. The examples are sent to it.

use bookings_server::bookings::bookings::{Booking, Summary, User};
use bookings_server::bookings::bookings_server::{
    BookingsHooks, BookingsService, CopyBookingFacts, CopyBookingRequest, GetBookingFacts, GetBookingRequest,
    ListBookingsRequest, MeRequest, PageBookingsRequest, SeenFacts, SeenRequest,
};
use bookings_server::bookings::bookings_test_server::{BookingsTestHandler, BookingsTestHooks, router};
use bookings_server::veld::server::RequestContext;
use bookings_server::veld::{Error, Page};

/// The application handlers are never reached: an example without `returns` stops after step 6
/// (the dry run), and one with it answers what it says.
struct Stub;

fn reached<T>() -> Result<T, Error> {
    Err(Error::other("the stub handler was reached"))
}

impl BookingsService for Stub {
    async fn get_booking(&self, _request: GetBookingRequest, _facts: GetBookingFacts, _ctx: &RequestContext) -> Result<Booking, Error> {
        reached()
    }

    async fn seen(&self, _request: SeenRequest, _facts: SeenFacts, _ctx: &RequestContext) -> Result<Summary, Error> {
        reached()
    }

    async fn list_bookings(&self, _request: ListBookingsRequest, _ctx: &RequestContext) -> Result<Vec<Booking>, Error> {
        reached()
    }

    async fn page_bookings(&self, _request: PageBookingsRequest, _ctx: &RequestContext) -> Result<Page<Booking>, Error> {
        reached()
    }

    async fn me(&self, _request: MeRequest, _ctx: &RequestContext) -> Result<User, Error> {
        reached()
    }

    async fn copy_booking(&self, _request: CopyBookingRequest, _facts: CopyBookingFacts, _ctx: &RequestContext) -> Result<Booking, Error> {
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
