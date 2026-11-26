//! Derived fields (`spec/rules.md` section 17.2) in generated Rust, over HTTP and over the memory
//! transport: the contract is `edges.veld` next to this crate. A value a caller sent is replaced
//! (a malformed one is dropped, not a `422`), a failing expression is a `422` and stops the
//! request, a fact and the principal are given theirs, the handler's value is overwritten, an
//! error and a stream item are computed, the idempotency fingerprint covers the computed values,
//! output mode `off` still computes, the client computes before it checks and never sends them,
//! and an event is computed by the publisher and again by the subscriber. Prints `derived ok`.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use edges_client::edges::edges::{Line as ClientLine, Order as ClientOrder};
use edges_client::edges::shop_client::{PlaceRequest as ClientPlace, ShopClient};
use edges_client::veld::Error as ClientError;
use edges_client::veld::client::{ClientOptions, HttpRequest, HttpResponse, HttpStream, Transport as ClientTransport};
use edges_server::edges::billing_events::create_billing_publisher;
use edges_server::edges::edges::{Gauge, Line, Order, Paid, Tick, User, Wallet};
use edges_server::edges::live_server::{FeedRequest, LiveHandler, LiveService};
use edges_server::edges::receipts_events::{ReceiptsEventHandlers, subscribe_receipts_events};
use edges_server::edges::shop_server::{
    ENDPOINTS, GaugeFacts, GaugeRequest, LineRequest, PlaceRequest, ShopFactLoaders, ShopHandler, ShopHooks,
    ShopService, SpendFacts, SpendRequest,
};
use edges_server::runtime_config::EdgesServerRuntime;
use edges_server::veld::events::{
    BoxFuture, ChannelOptions, ConsumerOptions, Deliver, EventMeta, MemoryTransport, PublisherOptions, Transport, TransportSubscription,
};
use edges_server::veld::idempotency::{Begin, MemoryIdempotencyStore, fingerprint};
use edges_server::veld::server::RequestContext;
use edges_server::veld::stream::{EventStream, SseEvent, iter};
use edges_server::veld::{Error, block_on};
use serde_json::{Map, Value, json};

const ID: &str = "00000000-0000-4000-8000-000000000001";
const NO_GAUGE: &str = "00000000-0000-4000-8000-0000000000ff";

/// What the application heard: the handlers' views, in order.
#[derive(Default)]
struct App {
    orders: Mutex<Vec<Order>>,
    wallets: Mutex<Vec<Wallet>>,
    /// `rule/position/outcome` of every violation the hook heard.
    heard: Mutex<Vec<String>>,
    store: MemoryIdempotencyStore,
    /// The next `place` answers with no items, so that `share` cannot be computed on the way out.
    empty_answer: Mutex<bool>,
}

struct Impl(Arc<App>);

/// A stand-in of an `Order`: whatever is written for a derived field is replaced.
fn order(items: Vec<Line>) -> Order {
    Order { id: ID.into(), serial: 4, items, discount: 0, subtotal: -1, payable: -1, share: -1, next: -1 }
}

impl ShopService for Impl {
    async fn place(&self, request: PlaceRequest, _ctx: &RequestContext) -> Result<Order, Error> {
        self.0.orders.lock().unwrap().push(request.body.clone());
        let items = if std::mem::take(&mut *self.0.empty_answer.lock().unwrap()) {
            Vec::new()
        } else {
            request.body.items.iter().map(|l| Line { price: l.price, qty: l.qty, total: -7 }).collect()
        };
        Ok(order(items))
    }

    async fn spend(&self, _request: SpendRequest, facts: SpendFacts, _ctx: &RequestContext) -> Result<Wallet, Error> {
        self.0.wallets.lock().unwrap().push(facts.wallet.clone());
        Ok(Wallet { available: -1, ..facts.wallet })
    }

    async fn gauge(&self, _request: GaugeRequest, _facts: GaugeFacts, _ctx: &RequestContext) -> Result<(), Error> {
        Ok(())
    }

    async fn line(&self, _request: LineRequest, _ctx: &RequestContext) -> Result<(), Error> {
        Ok(())
    }
}

struct Loaders;

impl ShopFactLoaders for Loaders {
    async fn gauge(&self, key: String, _ctx: &RequestContext) -> Result<Option<Gauge>, Error> {
        let n = if key == NO_GAUGE { 0 } else { 4 };
        Ok(Some(Gauge { id: key, n, inverse: -1 }))
    }

    async fn wallet(&self, key: String, _ctx: &RequestContext) -> Result<Option<Wallet>, Error> {
        Ok(Some(Wallet { id: key, balance: 1000, held: 920, available: -1 }))
    }
}

/// The hooks; the second member turns the validation of requests off.
struct Hooks(Arc<App>, bool);

impl ShopHooks for Hooks {
    type IdempotencyStore = MemoryIdempotencyStore;

    fn idempotency_store(&self) -> &MemoryIdempotencyStore {
        &self.0.store
    }

    fn idempotency_caller(&self, _principal: &User, _ctx: &RequestContext) -> String {
        "u".into()
    }

    async fn auth_bearer<'a>(&'a self, _token: &'a str, _ctx: &'a RequestContext) -> Option<User> {
        // `limit` is derived: the server gives the principal its value.
        Some(User { id: ID.into(), base: 5, limit: -1 })
    }

    fn validate_requests(&self) -> bool {
        !self.1
    }

    fn on_rule_violation(&self, e: &edges_server::veld::rules::RuleEvent) {
        let name = e.rule.rsplit_once('@').map_or(e.rule.as_str(), |(head, _)| head);
        self.0.heard.lock().unwrap().push(format!("{name}/{:?}/{:?}", e.position, e.outcome));
    }
}

struct Feed;

impl LiveService for Feed {
    async fn feed(&self, _request: FeedRequest, _ctx: &RequestContext) -> Result<EventStream<Tick>, Error> {
        let items: Vec<Result<SseEvent<Tick>, Error>> = (1..=3).map(|n| Ok(SseEvent::of(Tick { n, twice: 0 }))).collect();
        Ok(Box::pin(iter(items)))
    }
}

struct Server {
    base: String,
    app: Arc<App>,
}

async fn serve(output: Option<&str>) -> Server {
    serve_with(output, true).await
}

async fn serve_with(output: Option<&str>, validate: bool) -> Server {
    let app = Arc::new(App::default());
    let runtime = output.map(|mode| {
        EdgesServerRuntime::from_map(&BTreeMap::from([("rules.output".to_string(), mode.to_string())])).unwrap()
    });
    let shop = ShopHandler::create(Impl(app.clone()), Hooks(app.clone(), !validate), Loaders, runtime.as_ref()).unwrap();
    let live = LiveHandler::plain(Feed, runtime.as_ref()).unwrap();
    let router = edges_server::adapters::axum::router(shop).merge(edges_server::adapters::axum::router(live));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Server { base, app }
}

struct Answer {
    status: u16,
    text: String,
    doc: Value,
}

async fn post(s: &Server, path: &str, extra: &[(&str, &str)], body: Option<Value>) -> Answer {
    let mut req = reqwest::Client::new().post(format!("{}{path}", s.base)).header("authorization", "Bearer t");
    for (name, value) in extra {
        req = req.header(*name, *value);
    }
    if let Some(body) = body {
        req = req.header("content-type", "application/json").body(body.to_string());
    }
    let response = req.send().await.unwrap();
    let status = response.status().as_u16();
    let text = response.text().await.unwrap();
    let doc = serde_json::from_str(&text).unwrap_or(Value::Null);
    Answer { status, text, doc }
}

fn heard(s: &Server) -> Vec<String> {
    s.app.heard.lock().unwrap().clone()
}

// ---- a request -----------------------------------------------------------------------------------

/// What a caller sent for a derived field is replaced, a malformed value is dropped and not a `422`,
/// the chain is computed in order, the handler receives the computed values and what it puts into
/// its answer is overwritten.
async fn a_request_is_computed_and_what_was_sent_is_replaced() {
    let s = serve(None).await;
    let a = post(
        &s,
        "/shop/orders",
        &[],
        Some(json!({
            "items": [{ "price": 2, "qty": 3, "total": "not a number" }, { "price": 5, "qty": 1, "total": 999 }],
            "discount": 1, "subtotal": 12345, "payable": "x", "share": null, "next": [1]
        })),
    )
    .await;
    assert_eq!(a.status, 201, "{}", a.text);
    // The handler saw the computed values (`next` reads a read-only field, so it holds a stand-in).
    let seen = s.app.orders.lock().unwrap()[0].clone();
    assert_eq!(seen.items.iter().map(|l| l.total).collect::<Vec<_>>(), vec![6, 5]);
    assert_eq!((seen.subtotal, seen.payable, seen.share), (11, 10, 50));
    // What it returned was overwritten on the way out: the totals of the items it gave were -7.
    assert_eq!(a.doc["items"][0]["total"], json!(6));
    assert_eq!(a.doc["subtotal"], json!(11));
    assert_eq!(a.doc["payable"], json!(11), "the answer has the discount of the handler's value, 0");
    assert_eq!(a.doc["share"], json!(50));
    assert_eq!(a.doc["next"], json!(5), "`serial` is 4 in the answer");
    assert_eq!(heard(&s), Vec::<String>::new());
}

/// An expression that fails is one violation, a `422`, and the handler is not called.
async fn a_failing_derived_field_is_a_validation_error() {
    let s = serve(None).await;
    let a = post(&s, "/shop/orders", &[], Some(json!({ "items": [] }))).await;
    assert_eq!(a.status, 422, "{}", a.text);
    let violations = a.doc["violations"].as_array().unwrap();
    assert_eq!(violations.len(), 1, "{}", a.text);
    assert!(violations[0]["rule"].as_str().unwrap().starts_with("veld:edges/Order.share@"));
    assert_eq!(violations[0]["error"], json!("division_by_zero"));
    assert_eq!(violations[0]["path"], json!("body"));
    assert!(s.app.orders.lock().unwrap().is_empty());
    assert_eq!(heard(&s).len(), 1);
    assert!(heard(&s)[0].ends_with("/Input/Rejected"), "{:?}", heard(&s));
}

/// A rule that reads a derived field sees the computed value: `fits` fails when the discount is
/// larger than the subtotal.
async fn an_invariant_reads_the_computed_value() {
    let s = serve(None).await;
    let a = post(&s, "/shop/orders", &[], Some(json!({ "items": [{ "price": 1, "qty": 1 }], "discount": 50 }))).await;
    assert_eq!(a.status, 422, "{}", a.text);
    let rules: Vec<&str> = a.doc["violations"].as_array().unwrap().iter().map(|v| v["rule"].as_str().unwrap()).collect();
    assert_eq!(rules.len(), 1, "{rules:?}");
    assert!(rules[0].starts_with("veld:edges/Order.fits@"));
}

/// With the validation of requests off the derived fields are still computed (they are values, not
/// rules) and a value that cannot be computed still rejects the request: the handler could not be
/// given the field.
async fn without_validation_the_fields_are_computed_and_a_failing_one_rejects() {
    let s = serve_with(None, false).await;
    let a = post(&s, "/shop/orders", &[], Some(json!({ "items": [{ "price": 2, "qty": 3, "total": "x" }], "discount": 1 }))).await;
    assert_eq!(a.status, 201, "{}", a.text);
    let seen = s.app.orders.lock().unwrap()[0].clone();
    assert_eq!((seen.items[0].total, seen.subtotal, seen.payable), (6, 6, 5));
    let b = post(&s, "/shop/orders", &[], Some(json!({ "items": [] }))).await;
    assert_eq!(b.status, 422, "{}", b.text);
    assert_eq!(b.doc["violations"].as_array().unwrap().len(), 1, "{}", b.text);
    assert_eq!(s.app.orders.lock().unwrap().len(), 1, "the handler was not called again");
}

// ---- a fact, the principal, an error ---------------------------------------------------------------

/// A fact is given its derived field after it is loaded and the principal before the block reads it;
/// the declared error of an `else` has its derived field computed.
async fn facts_the_principal_and_errors_are_computed() {
    let s = serve(None).await;
    // `available` is 80, `limit` is 10.
    let ok = post(&s, &format!("/shop/spend/{ID}"), &[], Some(json!({ "amount": 10 }))).await;
    assert_eq!(ok.status, 200, "{}", ok.text);
    assert_eq!(s.app.wallets.lock().unwrap()[0].available, 80, "the handler's fact");
    assert_eq!(ok.doc["available"], json!(80), "the answer is computed again");
    let denied = post(&s, &format!("/shop/spend/{ID}"), &[], Some(json!({ "amount": 11 }))).await;
    assert_eq!(denied.status, 422, "the principal's limit is 10: {}", denied.text);
    let short = post(&s, &format!("/shop/spend/{ID}"), &[], Some(json!({ "amount": 90 }))).await;
    assert_eq!(short.status, 422, "{}", short.text);
    assert_eq!(short.doc["title"], json!("TooMuch"));
    assert_eq!(short.doc["data"], json!({ "needed": 90, "have": 80, "short": 10 }));
}

/// A derived field of a fact that cannot be computed is the application's mistake: `500`, reported
/// like an output violation, and the handler is not called.
async fn a_fact_that_cannot_be_computed_is_internal() {
    let s = serve(None).await;
    let ok = post(&s, &format!("/shop/gauge/{ID}"), &[], None).await;
    assert_eq!(ok.status, 204, "{}", ok.text);
    let bad = post(&s, &format!("/shop/gauge/{NO_GAUGE}"), &[], None).await;
    assert_eq!(bad.status, 500, "{}", bad.text);
    let heard = heard(&s);
    assert_eq!(heard.len(), 1, "{heard:?}");
    assert!(heard[0].starts_with("veld:edges/Gauge.inverse/Output/Rejected"), "{heard:?}");
}

// ---- outputs ---------------------------------------------------------------------------------------

/// A stream item is computed before it is written.
async fn a_stream_item_is_computed() {
    let s = serve(None).await;
    let text = reqwest::get(format!("{}/live/feed", s.base)).await.unwrap().text().await.unwrap();
    for n in 1..=3 {
        assert!(text.contains(&format!("{{\"n\":{n},\"twice\":{}}}", n * 2)), "{text}");
    }
}

/// A result that cannot be computed on the way out is `500` in `strict` and is sent without the
/// field in `off`, which still computes the others and reports nothing.
async fn the_output_mode_decides_what_a_failing_result_does() {
    let body = json!({ "items": [{ "price": 2, "qty": 3 }] });
    let strict = serve(None).await;
    *strict.app.empty_answer.lock().unwrap() = true;
    let a = post(&strict, "/shop/orders", &[], Some(body.clone())).await;
    assert_eq!(a.status, 500, "{}", a.text);
    assert!(heard(&strict)[0].contains("Order.share/Output/Rejected"), "{:?}", heard(&strict));
    let off = serve(Some("off")).await;
    *off.app.empty_answer.lock().unwrap() = true;
    let a = post(&off, "/shop/orders", &[], Some(body.clone())).await;
    assert_eq!(a.status, 201, "{}", a.text);
    assert!(a.doc.get("share").is_none(), "{}", a.text);
    assert_eq!(a.doc["subtotal"], json!(0), "the others are computed (there are no items)");
    assert!(heard(&off).is_empty());
    // `audit` sends it too, and reports.
    let audit = serve(Some("audit")).await;
    *audit.app.empty_answer.lock().unwrap() = true;
    let a = post(&audit, "/shop/orders", &[], Some(body)).await;
    assert_eq!(a.status, 201, "{}", a.text);
    assert!(a.doc.get("share").is_none());
    assert!(heard(&audit)[0].ends_with("/Output/Audited"), "{:?}", heard(&audit));
}

// ---- the fingerprint ---------------------------------------------------------------------------------

/// The fingerprint of an idempotent request covers the computed values, and not what was sent for
/// them.
async fn the_fingerprint_covers_the_computed_values() {
    let s = serve(None).await;
    let sent = post(&s, "/shop/lines", &[("idempotency-key", "k1")], Some(json!({ "price": 2, "qty": 3, "total": 999 }))).await;
    assert_eq!(sent.status, 204, "{}", sent.text);
    let scope = format!("{}\nu", ENDPOINTS[3].sig);
    let of = |body: Value| -> String {
        let mut params = Map::new();
        params.insert("body".into(), body);
        fingerprint(&params)
    };
    let computed = of(json!({ "price": 2, "qty": 3, "total": 6 }));
    assert!(matches!(s.app.store.begin_now(&scope, "k1", &computed).unwrap(), Begin::Replay(_)), "the computed total is hashed");
    let as_sent = of(json!({ "price": 2, "qty": 3, "total": 999 }));
    assert!(matches!(s.app.store.begin_now(&scope, "k1", &as_sent).unwrap(), Begin::Other), "what was sent is not");
}

// ---- the public function and the client ------------------------------------------------------------

/// `Order::derive` is the code the server runs; `derive_request` leaves out what reads a read-only field.
fn the_public_function_computes() {
    let mut o = Order { id: ID.into(), serial: 4, items: vec![Line { price: 2, qty: 3, total: 0 }], discount: 1, subtotal: 0, payable: 0, share: 0, next: 0 };
    assert!(o.derive().is_empty());
    assert_eq!((o.items[0].total, o.subtotal, o.payable, o.share, o.next), (6, 6, 5, 100, 5));
    let mut r = o.clone();
    r.next = 0;
    assert!(r.derive_request().is_empty());
    assert_eq!(r.next, 0, "a request has no `serial`: `next` holds its stand-in");
    o.items.clear();
    let failed = o.derive();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].error.as_deref(), Some("division_by_zero"));
    assert!(failed[0].rule.starts_with("veld:edges/Order.share@"));
}

/// A transport that answers what it is told and records what was sent.
#[derive(Clone)]
struct Wire {
    sent: Arc<Mutex<Vec<HttpRequest>>>,
    /// The rules digest the answers carry: the client runs its local checks while it matches its own.
    digest: String,
}

impl ClientTransport for Wire {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse, ClientError> {
        self.sent.lock().unwrap().push(req);
        // The server's answer is believed as it is, whatever it says.
        let body = json!({
            "id": ID, "serial": 4, "items": [{ "price": 1, "qty": 1, "total": 31 }],
            "discount": 0, "subtotal": 777, "payable": 888, "share": 100, "next": 5
        });
        Ok(HttpResponse {
            status: 201,
            headers: vec![("veld-rules".to_string(), self.digest.clone())],
            body: body.to_string().into_bytes(),
        })
    }

    async fn open(&self, _req: HttpRequest) -> Result<HttpStream, ClientError> {
        Err(ClientError::Transport("this transport has no streams".into()))
    }

    async fn sleep(&self, _d: std::time::Duration) {}
}

fn client_order(items: Vec<ClientLine>, discount: i32) -> ClientOrder {
    ClientOrder { id: String::new(), serial: 0, items, discount, subtotal: 0, payable: 0, share: 0, next: 0 }
}

/// The client computes before it checks (a rule reads a derived value), never sends one, and keeps
/// the derived value of a response as it came.
async fn the_client_computes_before_it_checks_and_never_sends_the_field() {
    // The digest of the rules of the service, as the server says it.
    let s = serve(None).await;
    let probe = post(&s, "/shop/orders", &[], Some(json!({ "items": [{ "price": 1, "qty": 1 }] }))).await;
    assert_eq!(probe.status, 201, "{}", probe.text);
    let digest = reqwest::Client::new()
        .post(format!("{}/shop/orders", s.base))
        .header("authorization", "Bearer t")
        .header("content-type", "application/json")
        .body(json!({ "items": [{ "price": 1, "qty": 1 }] }).to_string())
        .send()
        .await
        .unwrap()
        .headers()
        .get("veld-rules")
        .expect("the server sends its rules digest")
        .to_str()
        .unwrap()
        .to_string();
    let wire = Wire { sent: Arc::default(), digest };
    let client = ShopClient::new(wire.clone(), ClientOptions::new().base_url("http://x")).unwrap();
    let line = ClientLine { price: 1, qty: 1, total: 0 };
    // A valid one is sent without any derived member, at any depth.
    let order = client.place(ClientPlace { body: client_order(vec![line.clone()], 0) }).await.unwrap();
    let sent: Value = serde_json::from_slice(wire.sent.lock().unwrap()[0].body.as_ref().unwrap()).unwrap();
    assert_eq!(sent, json!({ "items": [{ "price": 1, "qty": 1 }], "discount": 0 }));
    // The response is taken as it is.
    assert_eq!((order.subtotal, order.payable, order.items[0].total), (777, 888, 31));
    // `fits` reads `payable`, which only the computation makes: -49. Nothing more is sent.
    let err = client.place(ClientPlace { body: client_order(vec![line], 50) }).await.unwrap_err();
    assert!(err.is_local(), "{err:?}");
    assert!(err.violations().iter().any(|v| v.rule.starts_with("veld:edges/Order.fits@")), "{:?}", err.violations());
    assert_eq!(wire.sent.lock().unwrap().len(), 1);
}

// ---- events ------------------------------------------------------------------------------------------

type Log<T> = Arc<Mutex<Vec<T>>>;

struct Receipts(Log<Paid>);

impl ReceiptsEventHandlers for Receipts {
    fn record(&self, event: Paid, _: EventMeta) -> impl Future<Output = Result<(), Error>> + Send {
        self.0.lock().unwrap().push(event);
        std::future::ready(Ok(()))
    }
}

/// The memory transport that also keeps what is published.
struct Spy {
    inner: Arc<MemoryTransport>,
    sent: Log<Vec<u8>>,
}

impl Transport for Spy {
    fn publish<'a>(&'a self, topic: &'a str, key: Option<&'a str>, bytes: &'a [u8], options: ChannelOptions) -> BoxFuture<'a, Result<(), Error>> {
        self.sent.lock().unwrap().push(bytes.to_vec());
        self.inner.publish(topic, key, bytes, options)
    }

    fn subscribe<'a>(
        &'a self,
        topic: &'a str,
        group: &'a str,
        deliver: Deliver,
        options: ChannelOptions,
    ) -> BoxFuture<'a, Result<Box<dyn TransportSubscription>, Error>> {
        self.inner.subscribe(topic, group, deliver, options)
    }
}

/// A publisher computes the derived fields of an event before its invariants and one that fails stops
/// the publish; a subscriber computes its own and does not believe the message.
fn events_are_computed_by_both_sides() {
    let memory = Arc::new(MemoryTransport::new());
    let sent: Log<Vec<u8>> = Default::default();
    let got: Log<Paid> = Default::default();
    let spy: Arc<dyn Transport> = Arc::new(Spy { inner: memory.clone(), sent: sent.clone() });
    block_on(async {
        subscribe_receipts_events(spy.clone(), Arc::new(Receipts(got.clone())), ConsumerOptions::new()).await.unwrap();
        let publisher = create_billing_publisher(spy.clone(), PublisherOptions::new());
        let paid = Paid { order: ID.into(), amount: 100, fee: 30, net: -5, percent: -5 };
        publisher.payments().paid(&paid).await.unwrap();
        memory.idle().await;
        // What was sent has the computed values.
        let envelope: Value = serde_json::from_slice(&sent.lock().unwrap()[0]).unwrap();
        assert_eq!(envelope["data"], json!({ "order": ID, "amount": 100, "fee": 30, "net": 70, "percent": 30 }));
        assert_eq!(got.lock().unwrap().len(), 1);
        assert_eq!((got.lock().unwrap()[0].net, got.lock().unwrap()[0].percent), (70, 30));
        // A message that carries other values is recomputed by the subscriber.
        let mut forged = envelope.clone();
        forged["id"] = json!("forged-1");
        forged["data"]["net"] = json!(999);
        forged["data"]["percent"] = json!("many");
        memory.publish("edges.Payments", None, forged.to_string().as_bytes(), ChannelOptions::NONE).await.unwrap();
        memory.idle().await;
        assert_eq!(got.lock().unwrap().len(), 2);
        assert_eq!((got.lock().unwrap()[1].net, got.lock().unwrap()[1].percent), (70, 30));
        // A derived field that cannot be computed stops the publish.
        let before = sent.lock().unwrap().len();
        let broken = Paid { order: ID.into(), amount: 0, fee: 5, net: 0, percent: 0 };
        let err = publisher.payments().paid(&broken).await.unwrap_err();
        assert_eq!(err.problem().unwrap().title, "ValidationError");
        assert_eq!(err.violations().len(), 1, "{:?}", err.violations());
        assert_eq!(err.violations()[0].error.as_deref(), Some("division_by_zero"));
        assert_eq!(sent.lock().unwrap().len(), before, "nothing was sent");
    });
}

#[tokio::main]
async fn main() {
    a_request_is_computed_and_what_was_sent_is_replaced().await;
    a_failing_derived_field_is_a_validation_error().await;
    an_invariant_reads_the_computed_value().await;
    without_validation_the_fields_are_computed_and_a_failing_one_rejects().await;
    facts_the_principal_and_errors_are_computed().await;
    a_fact_that_cannot_be_computed_is_internal().await;
    a_stream_item_is_computed().await;
    the_output_mode_decides_what_a_failing_result_does().await;
    the_fingerprint_covers_the_computed_values().await;
    the_public_function_computes();
    the_client_computes_before_it_checks_and_never_sends_the_field().await;
    // The memory transport needs no runtime of its own: drive it on a plain thread.
    std::thread::spawn(events_are_computed_by_both_sides).join().unwrap();
    println!("derived ok");
}
