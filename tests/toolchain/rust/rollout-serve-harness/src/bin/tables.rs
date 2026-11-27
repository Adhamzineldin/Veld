//! Serves the generated test handler of `tests/rules/endpoints/rollout_error.veld` on a free port with
//! a stub application handler, and prints `PORT=<n>`. The examples are sent to it.

use rollout_server::rollout::rollout::Confirmation;
use rollout_server::rollout::tables_server::{BookRequest, TablesService};
use rollout_server::rollout::tables_test_server::{TablesTestHandler, router};
use rollout_server::veld::Error;
use rollout_server::veld::server::RequestContext;

/// The application handler is never reached: an example without `returns` stops after step 6 (the
/// dry run) and one with `returns` brings its result.
struct Stub;

impl TablesService for Stub {
    async fn book(&self, _request: BookRequest, _ctx: &RequestContext) -> Result<Confirmation, Error> {
        Err(Error::other("the stub handler was reached"))
    }
}

#[tokio::main]
async fn main() {
    let app = router(TablesTestHandler::plain(Stub));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    println!("PORT={}", listener.local_addr().unwrap().port());
    axum::serve(listener, app).await.unwrap();
}
