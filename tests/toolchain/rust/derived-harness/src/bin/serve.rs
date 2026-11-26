//! Serves the generated test handler of `tests/rules/endpoints/derived.veld.pending` (the service
//! `Shop`) on one free port with a stub application handler, and prints `PORT=<n>`. The examples
//! of the contract are sent to it: the derived fields of a request, a fact, a result and an error.

use axum::routing::get;
use derived_server::derived::derived::Receipt;
use derived_server::derived::shop_server::{PayFacts, PayRequest, ShopService};
use derived_server::derived::shop_test_server::ShopTestHandler;
use derived_server::veld::Error;
use derived_server::veld::server::RequestContext;

/// The application handler is never reached: an example without `returns` stops after step 6 (the
/// dry run) and one with `returns` brings its result.
struct Stub;

impl ShopService for Stub {
    async fn pay(&self, _request: PayRequest, _facts: PayFacts, _ctx: &RequestContext) -> Result<Receipt, Error> {
        Err(Error::other("the stub handler was reached"))
    }
}

const PROBE: &str = r#"{"veldTestHandler":1,"digests":{"Shop":null}}"#;

#[tokio::main]
async fn main() {
    let shop = derived_server::adapters::axum::router(ShopTestHandler::plain(Stub, None).unwrap());
    let app = shop.route(
        "/__veld/test",
        get(|| async { ([(axum::http::header::CONTENT_TYPE, "application/json")], PROBE) }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    println!("PORT={}", listener.local_addr().unwrap().port());
    axum::serve(listener, app).await.unwrap();
}
