//! The idempotency fingerprint goldens (`tests/wire/idempotency/fingerprints.json`, read next to
//! `spec/rules.md` section 11.10) through the generated production server of
//! `tests/wire/idempotency/fingerprints.veld`: every case is sent over HTTP as a client would send
//! it, and the fingerprint the server gives to `IdempotencyStore::begin` is compared with the one
//! the reference printed. A case with `rejected` must be answered with that status and never reach
//! the store; `same_as` and `differs_from` are checked on what the server computed. The path of the
//! JSON file is the first argument. Prints `fingerprints ok <cases>`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bookings_server::bookings::fingerprints_server::{
    ByBoolRequest, ByDateRequest, ByDatetimeRequest, ByDecimalRequest, ByFloatRequest, ByIntRequest, ByLongRequest,
    ByTextRequest, ByTierRequest, ByUuidRequest, FilteredRequest, FingerprintsHandler, FingerprintsHooks,
    FingerprintsService, HeadedRequest, HeldRequest, ListedRequest, MappedRequest, MixedRequest, PostedRequest,
    QueriedRequest,
};
use bookings_server::veld::Error;
use bookings_server::veld::idempotency::{Begin, IdempotencyStore, StoredAnswer};
use bookings_server::veld::server::RequestContext;
use serde_json::Value;

/// The application: every endpoint answers `204`.
struct Impl;

macro_rules! stubs {
    ($($method:ident: $request:ident),* $(,)?) => {
        $(
            async fn $method(&self, _request: $request, _ctx: &RequestContext) -> Result<(), Error> {
                Ok(())
            }
        )*
    };
}

impl FingerprintsService for Impl {
    stubs!(
        by_uuid: ByUuidRequest,
        by_int: ByIntRequest,
        by_long: ByLongRequest,
        by_float: ByFloatRequest,
        by_bool: ByBoolRequest,
        by_date: ByDateRequest,
        by_datetime: ByDatetimeRequest,
        by_decimal: ByDecimalRequest,
        by_tier: ByTierRequest,
        by_text: ByTextRequest,
        queried: QueriedRequest,
        filtered: FilteredRequest,
        headed: HeadedRequest,
        posted: PostedRequest,
        held: HeldRequest,
        mixed: MixedRequest,
        listed: ListedRequest,
        mapped: MappedRequest,
    );
}

/// A store that says yes to every key and remembers the fingerprints it was asked about.
#[derive(Default)]
struct Store {
    asked: Mutex<Vec<String>>,
}

struct StoreRef(Arc<Store>);

impl IdempotencyStore for StoreRef {
    fn begin<'a>(
        &'a self,
        _scope: &'a str,
        _key: &'a str,
        fingerprint: &'a str,
    ) -> impl std::future::Future<Output = Result<Begin, Error>> + Send + 'a {
        self.0.asked.lock().unwrap().push(fingerprint.to_string());
        std::future::ready(Ok(Begin::Started))
    }

    fn complete<'a>(
        &'a self,
        _scope: &'a str,
        _key: &'a str,
        _answer: StoredAnswer,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + 'a {
        std::future::ready(Ok(()))
    }

    fn release<'a>(
        &'a self,
        _scope: &'a str,
        _key: &'a str,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + 'a {
        std::future::ready(Ok(()))
    }
}

struct Hooks(StoreRef);

impl FingerprintsHooks for Hooks {
    type IdempotencyStore = StoreRef;

    fn idempotency_store(&self) -> &StoreRef {
        &self.0
    }
}

/// Percent-encodes a decoded text for a path segment or a query key or value.
fn encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn url(base: &str, request: &Value) -> String {
    let path: Vec<String> = request["path"].as_str().unwrap().split('/').map(encode).collect();
    let mut pairs = Vec::new();
    for (key, value) in request["query"].as_object().unwrap() {
        let values: Vec<&Value> = match value {
            Value::Array(a) => a.iter().collect(),
            other => vec![other],
        };
        for v in values {
            pairs.push(format!("{}={}", encode(key), encode(v.as_str().unwrap())));
        }
    }
    let query = if pairs.is_empty() { String::new() } else { format!("?{}", pairs.join("&")) };
    format!("{base}{}{query}", path.join("/"))
}

#[tokio::main]
async fn main() {
    let file = std::env::args().nth(1).expect("the path of fingerprints.json");
    let cases: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    let store = Arc::new(Store::default());
    let handler = FingerprintsHandler::create(Impl, Hooks(StoreRef(store.clone())), None).unwrap();
    let router = bookings_server::adapters::axum::router(handler);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    let mut fingerprints: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut failures = Vec::new();
    for case in &cases {
        let name = case["name"].as_str().unwrap().to_string();
        let request = &case["request"];
        store.asked.lock().unwrap().clear();
        let mut req = reqwest::Client::new().request(reqwest::Method::from_bytes(request["method"].as_str().unwrap().as_bytes()).unwrap(), url(&base, request));
        for (key, value) in request["headers"].as_object().unwrap() {
            req = req.header(key.as_str(), value.as_str().unwrap());
        }
        if let Some(body) = request.get("body") {
            req = req.header("content-type", "application/json").body(body.to_string());
        } else if let Some(text) = request.get("body_text") {
            req = req.header("content-type", "application/json").body(text.as_str().unwrap().to_string());
        }
        let status = req.send().await.unwrap().status().as_u16();
        let asked = store.asked.lock().unwrap().clone();
        match case.get("rejected").and_then(Value::as_str) {
            Some(want) => {
                if status.to_string() != want || !asked.is_empty() {
                    failures.push(format!("{name}: rejected as {want}, got {status} and the store was asked {asked:?}"));
                }
                fingerprints.insert(name, None);
            }
            None => {
                let want = case["fingerprint"].as_str().unwrap();
                if status != 204 || asked != [want.to_string()] {
                    failures.push(format!("{name}: status {status}, the store was asked {asked:?}, the reference says {want} for {}", case["hashed"]));
                }
                fingerprints.insert(name, asked.first().cloned());
            }
        }
    }
    for case in &cases {
        let name = case["name"].as_str().unwrap();
        let mine = fingerprints.get(name).cloned().flatten();
        if let Some(other) = case.get("same_as").and_then(Value::as_str) {
            if mine != fingerprints.get(other).cloned().flatten() {
                failures.push(format!("{name}: must have the fingerprint of {other}"));
            }
        }
        if let Some(other) = case.get("differs_from").and_then(Value::as_str) {
            if mine == fingerprints.get(other).cloned().flatten() {
                failures.push(format!("{name}: must not have the fingerprint of {other}"));
            }
        }
    }
    assert!(failures.is_empty(), "{} of {} cases differ:\n{}", failures.len(), cases.len(), failures.join("\n"));
    println!("fingerprints ok {}", cases.len());
}
