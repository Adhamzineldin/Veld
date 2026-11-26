//! Serves the generated test handler of `tests/rules/endpoints/rollout.veld` on a free port with stub
//! application handlers, and prints `PORT=<n>`. The examples are sent to it, and `veld test --live`
//! compares the `Veld-Test-Audited` header of each answer with the reference evaluator.

use rollout_server::rollout::parties_server::{
    BookingRequest, EnforcedFacts, EnforcedRequest, MixedRequest, PartiesService, ReceiptRequest, WatchedFacts,
    WatchedRequest,
};
use rollout_server::rollout::parties_test_server::{PartiesTestHandler, router};
use rollout_server::rollout::rollout::{Booking, Receipt};
use rollout_server::veld::Error;
use rollout_server::veld::server::RequestContext;

/// The application handlers are never reached: an example without `returns` stops after step 6
/// (the dry run) and one with `returns` brings its result.
struct Stub;

fn never<T>() -> Result<T, Error> {
    Err(Error::other("the stub handler was reached"))
}

impl PartiesService for Stub {
    async fn watched(&self, _r: WatchedRequest, _f: WatchedFacts, _c: &RequestContext) -> Result<Booking, Error> {
        never()
    }

    async fn enforced(&self, _r: EnforcedRequest, _f: EnforcedFacts, _c: &RequestContext) -> Result<Receipt, Error> {
        never()
    }

    async fn mixed(&self, _r: MixedRequest, _c: &RequestContext) -> Result<Booking, Error> {
        never()
    }

    async fn booking(&self, _r: BookingRequest, _c: &RequestContext) -> Result<Booking, Error> {
        never()
    }

    async fn receipt(&self, _r: ReceiptRequest, _c: &RequestContext) -> Result<Receipt, Error> {
        never()
    }
}

#[tokio::main]
async fn main() {
    let app = router(PartiesTestHandler::plain(Stub, None).unwrap());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    println!("PORT={}", listener.local_addr().unwrap().port());
    axum::serve(listener, app).await.unwrap();
}
