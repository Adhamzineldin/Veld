//! Serves the generated test handler of the service `Notices` (no authentication: no caller) of
//! `tests/rules/endpoints/visibility.veld.pending` on a free port and prints `PORT=<n>`.

use guests_server::guests::guests::Booking;
use guests_server::guests::notices_server::{NoticesService, PublicBookingRequest};
use guests_server::guests::notices_test_server::{NoticesTestHandler, router};
use guests_server::veld::Error;
use guests_server::veld::server::RequestContext;

struct Stub;

impl NoticesService for Stub {
    async fn public_booking(&self, _request: PublicBookingRequest, _ctx: &RequestContext) -> Result<Booking, Error> {
        Err(Error::other("the stub handler was reached"))
    }
}

#[tokio::main]
async fn main() {
    let app = router(NoticesTestHandler::plain(Stub));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    println!("PORT={}", listener.local_addr().unwrap().port());
    axum::serve(listener, app).await.unwrap();
}
