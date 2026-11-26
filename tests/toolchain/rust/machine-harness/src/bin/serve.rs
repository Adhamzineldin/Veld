//! Serves the generated test handler of `tests/rules/endpoints/transitions.veld` on a free
//! port with stub application handlers, and prints `PORT=<n>`. The examples are sent to it.

use bookings_server::bookings::bookings::{Booking, Status};
use bookings_server::bookings::bookings_server::{
    BookingsService, CancelBookingFacts, CancelBookingRequest, CheckInFacts, CheckInRequest,
    PayBookingFacts, PayBookingRequest,
};
use bookings_server::bookings::bookings_test_server::{BookingsTestHandler, router};
use bookings_server::veld::Error;
use bookings_server::veld::rules::Transition;
use bookings_server::veld::server::RequestContext;

/// The application handlers are never reached: an example without `returns` stops after step 6
/// (the dry run) and one with `returns` brings its result.
struct Stub;

fn reached<T>() -> Result<T, Error> {
    Err(Error::other("the stub handler was reached"))
}

impl BookingsService for Stub {
    async fn pay_booking(
        &self,
        _request: PayBookingRequest,
        _facts: PayBookingFacts,
        _transition: Transition<Status>,
        _ctx: &RequestContext,
    ) -> Result<Booking, Error> {
        reached()
    }

    async fn cancel_booking(
        &self,
        _request: CancelBookingRequest,
        _facts: CancelBookingFacts,
        _transition: Transition<Status>,
        _ctx: &RequestContext,
    ) -> Result<Booking, Error> {
        reached()
    }

    async fn check_in(
        &self,
        _request: CheckInRequest,
        _facts: CheckInFacts,
        _transition: Transition<Status>,
        _ctx: &RequestContext,
    ) -> Result<(), Error> {
        reached()
    }
}

#[tokio::main]
async fn main() {
    let app = router(BookingsTestHandler::plain(Stub, None).unwrap());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    println!("PORT={}", listener.local_addr().unwrap().port());
    axum::serve(listener, app).await.unwrap();
}
