//! `validate = false`: the generated server and client of the `Greeter` fixture skip run-time
//! request validation by default and keep the types. `validate(true)` turns it back on.

use greeter_client::greeter::greeter_client::{GreeterClient, HelloRequest};
use greeter_client::prelude::ValidationError;
use greeter_client::veld::client::ClientOptions;
use greeter_client::veld::DeclaredError;
use greeter_server::greeter::greeter::Greeting;
use greeter_server::greeter::greeter_server::{GreeterHandler, GreeterService, HelloRequest as ServerHello};
use greeter_server::veld::Error;
use greeter_server::veld::server::RequestContext;
use harness::check::{equal, that};

struct Impl;

impl GreeterService for Impl {
    async fn hello(&self, request: ServerHello, _ctx: &RequestContext) -> Result<Greeting, Error> {
        Ok(Greeting { text: format!("hi {}", request.name) })
    }
}

#[tokio::main]
async fn main() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap_or_else(|e| panic!("bind: {e}"));
    let base = format!("http://127.0.0.1:{}", listener.local_addr().map(|a| a.port()).unwrap_or(0));
    let router = greeter_server::adapters::axum::router(GreeterHandler::plain(Impl));
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    // Neither side validates by default: a name shorter than the contract's minimum goes through.
    let lax = GreeterClient::reqwest(ClientOptions::new().base_url(&base)).unwrap_or_else(|e| panic!("{e}"));
    let greeting = lax.hello(HelloRequest { name: "ab".into() }).await.unwrap_or_else(|e| panic!("{e}"));
    equal("hi ab", greeting.text.as_str(), "an invalid request is not rejected");
    let (status, _) = {
        let resp = reqwest::get(format!("{base}/greeter/hello?name=ab")).await.unwrap_or_else(|e| panic!("{e}"));
        (resp.status().as_u16(), ())
    };
    equal(200, status, "the server does not validate either");

    // `validate(true)` switches the client's validation back on: a local ValidationError.
    let strict = GreeterClient::reqwest(ClientOptions::new().base_url(&base).validate(true)).unwrap_or_else(|e| panic!("{e}"));
    let err = strict.hello(HelloRequest { name: "ab".into() }).await.err().unwrap_or_else(|| panic!("expected a local ValidationError"));
    that(ValidationError::matches(&err), "validate(true) validates locally");
    println!("no-validate ok");
}
