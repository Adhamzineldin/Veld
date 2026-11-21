//! The client side of the round trip: every endpoint called through the generated Rust client.
//! It only assumes the behaviour of `tests/toolchain/ts/roundtrip.ts`'s implementation, so it runs
//! against the Rust server and against the Node server alike.

use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use orders_client::orders::catalog::{BankTransfer, Card, OrderItem, Payment, Status};
use orders_client::orders::orders::{CreateOrder, Order, OrderFilter, OutOfStock, QuotaExceeded};
use orders_client::orders::orders_client::*;
use orders_client::prelude::{Internal, NotFound, Unauthorized, ValidationError};
use orders_client::veld::client::ClientOptions;
use orders_client::veld::{DeclaredError, Error, VeldDecl};

use crate::check::{equal, that};
use crate::server_impl::State;

pub const USER_ID: &str = "11111111-1111-4111-8111-111111111111";

/// Operation signatures of the standard contract, written by the test as `sigs.json`.
pub fn sigs() -> BTreeMap<String, String> {
    let path = std::env::var("VELD_SIGS").unwrap_or_else(|_| "sigs.json".into());
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
    v["a"].as_object().map(|m| m.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect()).unwrap_or_default()
}

fn client(base: &str, validate: bool, token: Option<&str>) -> OrdersClient<orders_client::veld::transport_reqwest::ReqwestTransport> {
    let mut o = ClientOptions::new().base_url(base).validate(validate);
    if let Some(t) = token {
        let t = t.to_string();
        o = o.bearer(move || t.clone());
    }
    OrdersClient::reqwest(o).unwrap_or_else(|e| panic!("client: {e}"))
}

fn new_order(sku: &str, quantity: i32, payment: Option<Payment>) -> CreateOrder {
    CreateOrder {
        customer_id: USER_ID.into(),
        items: vec![OrderItem { sku: sku.into(), quantity }],
        note: None,
        payment,
    }
}

async fn raw(url: &str, method: &str, headers: &[(&str, &str)], body: Option<&str>) -> (u16, String) {
    let http = reqwest::Client::new();
    let m = reqwest::Method::from_bytes(method.as_bytes()).unwrap_or_else(|e| panic!("{e}"));
    let mut r = http.request(m, url);
    for (k, v) in headers {
        r = r.header(*k, *v);
    }
    if let Some(b) = body {
        r = r.body(b.to_string());
    }
    let resp = r.send().await.unwrap_or_else(|e| panic!("raw {url}: {e}"));
    let status = resp.status().as_u16();
    (status, resp.text().await.unwrap_or_default())
}

fn paths(e: &Error) -> Vec<String> {
    ValidationError::from_error(e).map(|v| v.issues.iter().map(|i| format!("{}:{}", i.path, i.code)).collect()).unwrap_or_default()
}

fn expect_err<T: std::fmt::Debug>(r: Result<T, Error>, what: &str) -> Error {
    match r {
        Err(e) => e,
        Ok(v) => panic!("FAILED: {what}: expected an error, got {v:?}"),
    }
}

/// Runs the whole scenario against `base`. `probe` is the server's state when it runs in this process.
pub async fn run(label: &str, base: &str, probe: Option<&Arc<State>>) {
    let sig = sigs();
    let op = |n: &str| sig.get(n).cloned().unwrap_or_else(|| panic!("no op signature {n}"));
    let c = client(base, true, Some("good"));
    let lax = client(base, false, Some("good"));
    let middleware_before = probe.map_or(0, |p| p.middleware_calls.load(Ordering::SeqCst));

    // health: no auth, 204
    c.health().await.unwrap_or_else(|e| panic!("{label}: health: {e}"));

    // createOrder: 201, defaults applied, header param, union body field
    let created = c
        .create_order(CreateOrderRequest { body: new_order("ABC-1", 2, Some(Payment::Card(Card { last4: "4242".into() }))), idempotency_key: Some("key-1".into()) })
        .await
        .unwrap_or_else(|e| panic!("{label}: create: {e}"));
    equal(Status::Pending, created.status, &format!("{label}: default status"));
    equal(Some("key-1"), created.note.as_deref(), &format!("{label}: header param reaches the implementation"));
    equal("Ada", created.customer.display_name.as_str(), &format!("{label}: customer"));
    equal(Some(Payment::Card(Card { last4: "4242".into() })), created.payment.clone(), &format!("{label}: union value"));
    equal(9_007_199_254_740_993_i64, created.seq, &format!("{label}: long keeps all its digits"));
    // The typed form keeps the instant as the server wrote it (UTC or an offset); canonical form is UTC.
    that(created.created_at == "2024-05-01T10:00:00Z" || created.created_at == "2024-05-01T12:00:00+02:00", &format!("{label}: datetime {}", created.created_at));

    // wire form uses @wire names
    let (status, text) = raw(&format!("{base}/orders/{}", created.id), "GET", &[("authorization", "Bearer good")], None).await;
    equal(200, status, &format!("{label}: raw get"));
    let body: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    that(body.get("created_at").is_some(), &format!("{label}: @wire name created_at"));
    equal(Some("Ada"), body["customer"]["display_name"].as_str(), &format!("{label}: display_name"));
    equal(Some("9007199254740993"), body["seq"].as_str(), &format!("{label}: long is a string on the wire"));

    // getOrder
    let fetched = c.get_order(GetOrderRequest { id: created.id.clone() }).await.unwrap_or_else(|e| panic!("{label}: get: {e}"));
    equal(created.clone(), fetched, &format!("{label}: fetched equals created"));

    declared_errors(label, &c, &op).await;
    queries(label, &c, probe).await;

    // pay: union body and union in the response
    let paid = c
        .pay_order(PayOrderRequest { id: created.id.clone(), body: Payment::Bank(BankTransfer { iban: "DE00".into() }) })
        .await
        .unwrap_or_else(|e| panic!("{label}: pay: {e}"));
    equal(Status::Paid, paid.status, &format!("{label}: paid"));
    equal(Some(Payment::Bank(BankTransfer { iban: "DE00".into() })), paid.payment, &format!("{label}: union arm round trip"));
    let missing = expect_err(c.pay_order(PayOrderRequest { id: "00000000-0000-4000-8000-0000000000ff".into(), body: Payment::Bank(BankTransfer { iban: "x".into() }) }).await, "pay unknown");
    that(NotFound::matches(&missing), &format!("{label}: pay NotFound"));

    // cancel: 204, then NotFound
    c.cancel_order(CancelOrderRequest { id: created.id.clone() }).await.unwrap_or_else(|e| panic!("{label}: cancel: {e}"));
    let twice = expect_err(c.cancel_order(CancelOrderRequest { id: created.id.clone() }).await, "cancel twice");
    that(NotFound::matches(&twice), &format!("{label}: cancel twice"));

    undeclared(label, base, &c, probe).await;
    validation(label, base, &c, &lax, probe, &op).await;
    auth(label, base, &op).await;
    if let Some(p) = probe {
        that(p.middleware_calls.load(Ordering::SeqCst) > middleware_before, &format!("{label}: middleware called"));
    }
}

type Rt = orders_client::veld::transport_reqwest::ReqwestTransport;

async fn declared_errors(label: &str, c: &OrdersClient<Rt>, op: &dyn Fn(&str) -> String) {
    let e = expect_err(c.get_order(GetOrderRequest { id: "00000000-0000-4000-8000-0000000000aa".into() }).await, "get unknown");
    let nf = NotFound::from_error(&e).unwrap_or_else(|| panic!("{label}: NotFound class: {e}"));
    equal(Some(op("Orders.getOrder").as_str()), e.op(), &format!("{label}: NotFound op"));
    equal(Some(404), e.status(), &format!("{label}: NotFound status"));
    equal(Some("order"), nf.resource.as_deref(), &format!("{label}: NotFound data"));
    that(NotFound::matches(&e) && !Unauthorized::matches(&e), &format!("{label}: matches"));

    let e = expect_err(c.create_order(CreateOrderRequest { body: new_order("OUT-1", 1, None), idempotency_key: None }).await, "out of stock");
    let oos = OutOfStock::from_error(&e).unwrap_or_else(|| panic!("{label}: OutOfStock: {e}"));
    equal(Some(op("Orders.createOrder").as_str()), e.op(), &format!("{label}: OutOfStock op"));
    equal(Some(409), e.status(), &format!("{label}: OutOfStock status"));
    equal("OUT-1", oos.sku.as_str(), &format!("{label}: OutOfStock data"));

    let e = expect_err(c.create_order(CreateOrderRequest { body: new_order("ABC-1", 1, None), idempotency_key: Some("quota".into()) }).await, "quota");
    let q = QuotaExceeded::from_error(&e).unwrap_or_else(|| panic!("{label}: QuotaExceeded: {e}"));
    equal(Some(429), e.status(), &format!("{label}: use-site status"));
    equal(5, q.limit, &format!("{label}: QuotaExceeded data"));
}

async fn queries(label: &str, c: &OrdersClient<Rt>, probe: Option<&Arc<State>>) {
    // list with a query object (enum, repeated list keys, int)
    let page = c
        .list_orders(ListOrdersRequest { query: OrderFilter { status: Some(Status::Pending), tag: Some(vec!["a".into(), "b c".into()]), limit: Some(10) } })
        .await
        .unwrap_or_else(|e| panic!("{label}: list: {e}"));
    equal(1, page.total, &format!("{label}: page total"));
    equal(1, page.items.len(), &format!("{label}: page items"));
    if let Some(p) = probe {
        that(p.seen.lock().map(|s| s.contains(&"tags=a|b c".to_string())).unwrap_or(false), &format!("{label}: repeated query keys"));
    }
    let none = c
        .list_orders(ListOrdersRequest { query: OrderFilter { status: Some(Status::Paid), tag: None, limit: None } })
        .await
        .unwrap_or_else(|e| panic!("{label}: list paid: {e}"));
    equal(0, none.items.len(), &format!("{label}: filter"));

    // search with single query params and awkward characters
    let found = c.search_orders(SearchOrdersRequest { q: "key-1".into(), limit: None }).await.unwrap_or_else(|e| panic!("{label}: search: {e}"));
    equal(1, found.len(), &format!("{label}: search"));
    c.create_order(CreateOrderRequest { body: new_order("ABC-1", 1, None), idempotency_key: Some("a b&c=d".into()) })
        .await
        .unwrap_or_else(|e| panic!("{label}: create awkward: {e}"));
    let awkward = c.search_orders(SearchOrdersRequest { q: "a b&c=d".into(), limit: None }).await.unwrap_or_else(|e| panic!("{label}: awkward: {e}"));
    equal(1, awkward.len(), &format!("{label}: query encoding round trip"));
    equal(Some("a b&c=d"), awkward[0].note.as_deref(), &format!("{label}: query encoding value"));
    c.search_orders(SearchOrdersRequest { q: "x".into(), limit: Some(3) }).await.unwrap_or_else(|e| panic!("{label}: limit: {e}"));
    if let Some(p) = probe {
        let seen = p.seen.lock().map(|s| s.clone()).unwrap_or_default();
        that(seen.contains(&"q=x;limit=3".to_string()), &format!("{label}: optional query param"));
        that(seen.contains(&"q=key-1;limit=null".to_string()), &format!("{label}: omitted optional query param"));
    }
}

/// An undeclared failure surfaces as the declared prelude `Internal`, without leaking its message.
async fn undeclared(label: &str, base: &str, c: &OrdersClient<Rt>, probe: Option<&Arc<State>>) {
    let e = expect_err(c.boom().await, "boom");
    let internal = Internal::from_error(&e).unwrap_or_else(|| panic!("{label}: boom is a declared Internal error: {e}"));
    that(Internal::matches(&e), &format!("{label}: Internal.matches"));
    equal(Some(500), e.status(), &format!("{label}: Internal status"));
    that(!internal.to_wire().to_string().contains("secret"), &format!("{label}: no leak in data"));
    let (status, text) = raw(&format!("{base}/orders/boom"), "GET", &[], None).await;
    equal(500, status, &format!("{label}: raw boom status"));
    that(!text.contains("secret"), &format!("{label}: no leak on the wire"));
    if let Some(p) = probe {
        let thrown = p.thrown.lock().map(|t| t.clone()).unwrap_or_default();
        that(thrown.iter().any(|t| t.contains("secret internal detail")), &format!("{label}: onError saw the original"));
    }
}

async fn validation(label: &str, base: &str, c: &OrdersClient<Rt>, lax: &OrdersClient<Rt>, probe: Option<&Arc<State>>, op: &dyn Fn(&str) -> String) {
    let bad = CreateOrder { customer_id: USER_ID.into(), items: vec![], note: None, payment: None };
    let bad_item = new_order("nope", 0, None);
    let sent = probe.map_or(0, |p| p.requests.load(Ordering::SeqCst));
    let local = expect_err(c.create_order(CreateOrderRequest { body: bad.clone(), idempotency_key: None }).await, "local validation");
    that(ValidationError::matches(&local), &format!("{label}: local ValidationError"));
    equal(Some(op("Orders.createOrder").as_str()), local.op(), &format!("{label}: local op"));
    that(!ValidationError::from_error(&local).map(|v| v.issues).unwrap_or_default().is_empty(), &format!("{label}: local issues"));
    let item = expect_err(c.create_order(CreateOrderRequest { body: bad_item.clone(), idempotency_key: None }).await, "local item");
    let local_paths = paths(&item);
    that(local_paths.contains(&"body.items[0].sku:pattern".to_string()) && local_paths.contains(&"body.items[0].quantity:min".to_string()), &format!("{label}: {local_paths:?}"));
    if let Some(p) = probe {
        equal(sent, p.requests.load(Ordering::SeqCst), &format!("{label}: no request for a locally invalid call"));
    }
    let remote = expect_err(lax.create_order(CreateOrderRequest { body: bad, idempotency_key: None }).await, "remote validation");
    equal(Some(422), remote.status(), &format!("{label}: remote status"));
    equal(Some(op("Orders.createOrder").as_str()), remote.op(), &format!("{label}: remote op"));
    that(paths(&remote).contains(&"body.items:min_length".to_string()), &format!("{label}: {:?}", paths(&remote)));
    let query = expect_err(lax.list_orders(ListOrdersRequest { query: OrderFilter { status: None, tag: None, limit: Some(1000) } }).await, "remote query");
    that(paths(&query).contains(&"query.limit:max".to_string()), &format!("{label}: query.limit max"));
    let (status, text) = raw(&format!("{base}/orders"), "POST", &[("authorization", "Bearer good"), ("content-type", "application/json")], Some("{not json")).await;
    equal(422, status, &format!("{label}: bad JSON"));
    let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    equal(Some("ValidationError"), v["title"].as_str(), &format!("{label}: bad JSON title"));
}

async fn auth(label: &str, base: &str, op: &dyn Fn(&str) -> String) {
    let anon = client(base, true, None);
    let e = expect_err(anon.get_order(GetOrderRequest { id: "00000000-0000-4000-8000-0000000000bb".into() }).await, "missing token");
    that(Unauthorized::matches(&e), &format!("{label}: missing token"));
    equal(Some(401), e.status(), &format!("{label}: 401"));
    equal(Some(op("Orders.getOrder").as_str()), e.op(), &format!("{label}: 401 op"));
    let wrong = client(base, true, Some("bad"));
    let e = expect_err(wrong.list_orders(ListOrdersRequest { query: OrderFilter { status: None, tag: None, limit: None } }).await, "wrong token");
    that(Unauthorized::matches(&e), &format!("{label}: rejected token"));
    anon.health().await.unwrap_or_else(|e| panic!("{label}: anonymous health: {e}"));
}

/// Keeps `Order` in the public API of this module (the scenario's result type).
pub type Created = Order;
