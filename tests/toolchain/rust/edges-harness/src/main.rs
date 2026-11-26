//! The shapes of endpoint block that the bookings contract does not have
//! (`tests/toolchain/contract/rules_edges`): the generated crates build and lint, and the
//! production handler and the test handler answer them.

use edges_server::edges::edges::{Account, Receipt, Tier};
use edges_server::edges::edges_server::{
    EchoRequest, EdgesFactLoaders, EdgesHandler, EdgesService, MoveFacts, MoveRequest, PingRequest, WatchFacts,
    WatchRequest,
};
use edges_server::edges::edges_test_server::{EdgesTestHandler, probe, router};
use edges_server::edges::plain_server::{EchoRequest as PlainEcho, FeedRequest, PlainHandler, PlainService};
use edges_server::edges::plain_test_server::{PlainTestHandler, router as plain_router};
use edges_server::veld::Error;
use edges_server::veld::server::RequestContext;
use edges_server::veld::stream::{EventStream, iter};
use serde_json::{Value, json};

const ID: &str = "00000000-0000-4000-8000-000000000001";

fn receipt(count: i32) -> Receipt {
    Receipt { id: ID.into(), count }
}

struct Impl;

impl EdgesService for Impl {
    async fn echo(&self, request: EchoRequest, _ctx: &RequestContext) -> Result<Receipt, Error> {
        Ok(receipt(request.body.count))
    }

    async fn ping(&self, _request: PingRequest, _ctx: &RequestContext) -> Result<(), Error> {
        Ok(())
    }

    async fn r#move(&self, request: MoveRequest, _facts: MoveFacts, _ctx: &RequestContext) -> Result<Receipt, Error> {
        Ok(receipt(request.body.count))
    }

    async fn watch(&self, _request: WatchRequest, _facts: WatchFacts, _ctx: &RequestContext) -> Result<EventStream<Receipt>, Error> {
        let items: Vec<Result<edges_server::veld::stream::SseEvent<Receipt>, Error>> = Vec::new();
        Ok(Box::pin(iter(items)))
    }
}

struct Loaders;

impl EdgesFactLoaders for Loaders {
    async fn account(&self, _key: String, _ctx: &RequestContext) -> Result<Option<Account>, Error> {
        Ok(Some(Account { id: ID.into(), tier: Tier::Gold, limit: 3 }))
    }

    async fn quota(&self, key: i32, _ctx: &RequestContext) -> Result<Option<i32>, Error> {
        Ok(Some(if key == 3 { 1 } else { key }))
    }

    async fn settings(&self, _ctx: &RequestContext) -> Result<Option<Account>, Error> {
        Ok(Some(Account { id: ID.into(), tier: Tier::Basic, limit: 1 }))
    }
}

struct PlainImpl(i32);

impl PlainService for PlainImpl {
    async fn echo(&self, _request: PlainEcho, _ctx: &RequestContext) -> Result<Receipt, Error> {
        Ok(receipt(self.0))
    }

    async fn feed(&self, _request: FeedRequest, _ctx: &RequestContext) -> Result<EventStream<Receipt>, Error> {
        let items: Vec<Result<edges_server::veld::stream::SseEvent<Receipt>, Error>> = Vec::new();
        Ok(Box::pin(iter(items)))
    }
}

async fn start(router: axum::Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    base
}

/// Unpadded base64url.
fn b64(text: &str) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in text.as_bytes().chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..=chunk.len() {
            out.push(char::from(A[((n >> (18 - 6 * i)) & 63) as usize]));
        }
    }
    out
}

struct Reply {
    status: u16,
    passed: bool,
    doc: Value,
}

async fn send(method: &str, url: &str, headers: &[(&str, String)], body: Option<Value>) -> Reply {
    let mut call = reqwest::Client::new().request(method.parse().unwrap(), url);
    for (k, v) in headers {
        call = call.header(*k, v);
    }
    if let Some(b) = body {
        call = call.header("content-type", "application/json").body(b.to_string());
    }
    let response = call.send().await.unwrap();
    let status = response.status().as_u16();
    let passed = response.headers().get("veld-test-passed").is_some();
    let text = response.text().await.unwrap();
    Reply { status, passed, doc: serde_json::from_str(&text).unwrap_or(Value::Null) }
}

fn test_headers(facts: &Value, dry: bool) -> Vec<(&'static str, String)> {
    let mut h = vec![("veld-test-facts", b64(&facts.to_string()))];
    if dry {
        h.push(("veld-test-dry-run", "1".to_string()));
    }
    h
}

fn account(tier: &str) -> Value {
    json!({ "id": ID, "tier": tier, "limit": 3 })
}

async fn the_test_handler() {
    let base = start(router(EdgesTestHandler::plain(Impl, None).unwrap())).await;
    let probe_text = reqwest::get(format!("{base}/__veld/test")).await.unwrap().text().await.unwrap();
    let probe_doc: Value = serde_json::from_str(&probe_text).unwrap();
    assert_eq!(probe_doc, serde_json::from_str::<Value>(&probe()).unwrap());
    assert_eq!(probe_doc["veldTestHandler"], 1);
    assert!(probe_doc["digests"]["Edges"].is_null(), "no invariants, no digest");
    let url = format!("{base}/edges/{ID}/move");
    let facts = json!({ "settings": account("basic"), "quota": 5, "account": account("gold") });
    // Keyless, scalar and keyed facts load from the header: every check passes.
    let ok = send("POST", &url, &test_headers(&facts, true), Some(json!({ "count": 2 }))).await;
    assert_eq!((ok.status, ok.passed), (204, true), "{}", ok.doc);
    // The `else` payload: a literal, and fields computed from a fact and a parameter.
    let basic = json!({ "settings": account("basic"), "quota": 5, "account": account("basic") });
    let frozen = send("POST", &url, &test_headers(&basic, true), Some(json!({ "count": 2 }))).await;
    assert_eq!(frozen.status, 409, "{}", frozen.doc);
    assert_eq!(frozen.doc["data"], json!({ "reason": "not gold" }));
    let many = send("POST", &url, &test_headers(&facts, true), Some(json!({ "count": 9 }))).await;
    assert_eq!(many.status, 422, "{}", many.doc);
    assert_eq!(many.doc["data"], json!({ "allowed": 5, "asked": 18 }));
    assert_eq!(many.doc["violations"].as_array().unwrap().len(), 1);
    // A fact missing from the header is not found.
    let none = send("POST", &url, &test_headers(&json!({}), true), Some(json!({ "count": 2 }))).await;
    assert_eq!(none.status, 404, "{}", none.doc);
    assert_eq!(none.doc["data"], json!({ "resource": "settings" }));
    // An optional query parameter in a rule; no success body.
    let ping = |q: &str| format!("{base}/edges/ping{q}");
    assert_eq!(send("POST", &ping("?limit=11"), &test_headers(&json!({}), true), None).await.status, 409);
    let passed = send("POST", &ping("?limit=3"), &test_headers(&json!({}), true), None).await;
    assert_eq!((passed.status, passed.passed), (204, true));
    // A stream endpoint with a fact and a rule.
    let watch = format!("{base}/edges/{ID}/watch");
    let gold = json!({ "account": account("gold") });
    assert_eq!(send("GET", &watch, &test_headers(&gold, true), None).await.status, 204);
    let basic = json!({ "account": account("basic") });
    assert_eq!(send("GET", &watch, &test_headers(&basic, true), None).await.status, 409);
    // A service with blocks and no facts: the result comes from the header, and `ensure` judges it.
    let plain = start(plain_router(PlainTestHandler::plain(PlainImpl(0), None).unwrap())).await;
    let result = |count: i32| vec![("veld-test-result", b64(&json!({ "id": ID, "count": count }).to_string()))];
    let echo = format!("{plain}/plain/echo");
    let good = send("POST", &echo, &result(3), Some(json!({ "count": 3 }))).await;
    assert_eq!(good.status, 201, "{}", good.doc);
    let bad = send("POST", &echo, &result(4), Some(json!({ "count": 3 }))).await;
    assert_eq!(bad.status, 500, "{}", bad.doc);
}

async fn the_production_handler() {
    let base = start(edges_server::adapters::axum::router(EdgesHandler::plain(Impl, Loaders, None).unwrap())).await;
    // An ensure on an endpoint with no success body.
    let ping = |q: &str| format!("{base}/edges/ping{q}");
    assert_eq!(send("POST", &ping(""), &[], None).await.status, 204);
    assert_eq!(send("POST", &ping("?limit=3"), &[], None).await.status, 204);
    assert_eq!(send("POST", &ping("?limit=11"), &[], None).await.status, 409);
    assert_eq!(send("POST", &ping("?limit=0"), &[], None).await.status, 500, "ensure on no body");
    // The computed key reaches the scalar loader; a keyless fact is loaded.
    let moved = send("POST", &format!("{base}/edges/{ID}/move"), &[], Some(json!({ "count": 5 }))).await;
    assert_eq!(moved.status, 201, "{}", moved.doc);
    // count 2 makes the key 3, for which the loader answers a quota of 1.
    let too_many = send("POST", &format!("{base}/edges/{ID}/move"), &[], Some(json!({ "count": 2 }))).await;
    assert_eq!(too_many.status, 422, "{}", too_many.doc);
    assert_eq!(too_many.doc["data"], json!({ "allowed": 1, "asked": 4 }));
    let plain = start(edges_server::adapters::axum::router(PlainHandler::plain(PlainImpl(7), None).unwrap())).await;
    let echoed = send("POST", &format!("{plain}/plain/echo"), &[], Some(json!({ "count": 7 }))).await;
    assert_eq!(echoed.status, 201);
    let broken = send("POST", &format!("{plain}/plain/echo"), &[], Some(json!({ "count": 1 }))).await;
    assert_eq!(broken.status, 500);
}

#[tokio::main]
async fn main() {
    the_test_handler().await;
    the_production_handler().await;
    println!("edges ok");
}
