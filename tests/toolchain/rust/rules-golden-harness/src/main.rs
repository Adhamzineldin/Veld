//! The `422` golden (`tests/wire/rules`): the generated Rust server answers the request of
//! `request.json` with a problem document whose RFC 8785 bytes are `problem.json`.
//!
//! The directory of the golden files is the first argument.

use std::path::Path;

use golden_server::bookings::bookings::{Booking, Report};
use golden_server::bookings::bookings_server::{
    BookingsHandler, BookingsService, CreateRequest, ReportRequest, SearchRequest,
};
use golden_server::veld::Error;
use golden_server::veld::canonical::canonical;
use golden_server::veld::server::RequestContext;

struct Impl;

impl BookingsService for Impl {
    async fn create(&self, _request: CreateRequest, _ctx: &RequestContext) -> Result<Booking, Error> {
        Err(Error::other("the golden request never reaches the handler"))
    }

    async fn search(&self, _request: SearchRequest, _ctx: &RequestContext) -> Result<i32, Error> {
        Err(Error::other("unused"))
    }

    async fn report(&self, _request: ReportRequest, _ctx: &RequestContext) -> Result<Report, Error> {
        Err(Error::other("unused"))
    }
}

#[tokio::main]
async fn main() {
    let dir = std::env::args().nth(1).expect("the directory of the golden files");
    let dir = Path::new(&dir);
    let request: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("request.json")).unwrap()).unwrap();
    let golden = std::fs::read_to_string(dir.join("problem.json")).unwrap();

    let router = golden_server::adapters::axum::router(BookingsHandler::plain(Impl));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    let method = request["method"].as_str().unwrap();
    let url = format!("{base}{}", request["path"].as_str().unwrap());
    let mut call = reqwest::Client::new().request(method.parse().unwrap(), url);
    for (name, value) in request["headers"].as_object().unwrap() {
        call = call.header(name.as_str(), value.as_str().unwrap());
    }
    let response = call.body(serde_json::to_vec(&request["body"]).unwrap()).send().await.unwrap();
    assert_eq!(u64::from(response.status().as_u16()), request["status"].as_u64().unwrap());
    let content_type = response.headers()["content-type"].to_str().unwrap().to_string();
    assert!(content_type.starts_with(request["contentType"].as_str().unwrap()), "{content_type}");
    let body: serde_json::Value = serde_json::from_str(&response.text().await.unwrap()).unwrap();
    // Readers ignore a trailing line break.
    assert_eq!(canonical(&body), golden.trim_end_matches(['\r', '\n']));
    println!("rules golden ok");
}
