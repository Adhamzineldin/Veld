//! Serves the generated test handler of `tests/rules/endpoints/policies.veld` on a free
//! port with stub application handlers, and prints `PORT=<n>`. The examples are sent to it.

use policies_server::bookings::bookings::{Booking, User};
use policies_server::bookings::bookings_server::{
    BookingsHooks, BookingsService, GetBookingFacts, GetBookingRequest, PayBookingFacts,
    PayBookingRequest,
};
use policies_server::bookings::bookings_test_server::{BookingsTestHandler, router};
use policies_server::veld::Error;
use policies_server::veld::server::RequestContext;

/// The application handlers are never reached: an example without `returns` stops after step 6
/// (the dry run).
struct Stub;

impl BookingsService for Stub {
    async fn pay_booking(
        &self,
        _request: PayBookingRequest,
        _facts: PayBookingFacts,
        _ctx: &RequestContext,
    ) -> Result<Booking, Error> {
        Err(Error::other("the stub handler was reached"))
    }

    async fn get_booking(
        &self,
        _request: GetBookingRequest,
        _facts: GetBookingFacts,
        _ctx: &RequestContext,
    ) -> Result<Booking, Error> {
        Err(Error::other("the stub handler was reached"))
    }
}

/// The test handler never calls the authentication hook.
struct Hooks;

impl BookingsHooks for Hooks {
    async fn auth_bearer<'a>(&'a self, _token: &'a str, _ctx: &'a RequestContext) -> Option<User> {
        None
    }
}

#[tokio::main]
async fn main() {
    let app = router(BookingsTestHandler::create(Stub, Hooks, None).unwrap());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    println!("PORT={}", listener.local_addr().unwrap().port());
    axum::serve(listener, app).await.unwrap();
}
