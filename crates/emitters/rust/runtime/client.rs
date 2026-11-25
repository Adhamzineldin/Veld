//! The client core: request building, validation, typed errors, base URL resolution, streaming.
//!
//! The generated clients are generic over [`Transport`], so the HTTP stack is yours: implement
//! the trait for what you use, or take one of the opt-in adapters (`reqwest`, `ureq`).

use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use super::canonical::es_number;
use super::error::{Error, Issue, Problem};
use super::spec::{AuthKind, Binding, EndpointSpec, StdSigs};
use super::util::{base64_encode, pct_encode};
// @if sse
use std::collections::VecDeque;
use std::marker::PhantomData;

use super::stream::{BoxStream, SseMessage, SseParser, next, unfold};
// @end
// @if ws_client
use std::pin::Pin;
// @end

/// An HTTP request as a transport sees it.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    /// Applies to the whole exchange, or to connecting only for a stream.
    pub timeout: Duration,
}

/// An HTTP response with its whole body.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

// @if sse
/// An HTTP response whose body is still arriving.
pub struct HttpStream {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    /// Body chunks. An `Err` is a broken connection; the end of the stream is a clean end.
    pub body: BoxStream<Result<Vec<u8>, Error>>,
}

impl fmt::Debug for HttpStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HttpStream({})", self.status)
    }
}
// @end

/// The HTTP stack of a generated client. Implement it for the library you already use.
pub trait Transport: Send + Sync + 'static {
    /// Sends one request and reads the whole response. A non-2xx status is a normal response;
    /// only a failure to talk to the server is an [`Error::Transport`].
    fn send(&self, req: HttpRequest) -> impl Future<Output = Result<HttpResponse, Error>> + Send;

    // @if sse
    /// Sends one request and returns as soon as the headers are there.
    fn open(&self, req: HttpRequest) -> impl Future<Output = Result<HttpStream, Error>> + Send;

    /// Waits; used between reconnect attempts.
    fn sleep(&self, d: Duration) -> impl Future<Output = ()> + Send;
    // @end

    // @if ws_client
    /// Opens a WebSocket (`ws://` or `wss://`) with these headers.
    fn connect_ws(&self, url: String, headers: Vec<(String, String)>, timeout: Duration) -> impl Future<Output = Result<WsConn, Error>> + Send;
    // @end
}

// @if sse
/// Reconnect policy of streams: only connection failures are retried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reconnect {
    pub initial_ms: u64,
    pub max_ms: u64,
    pub retries: u32,
}

impl Default for Reconnect {
    /// 1000 ms doubling to 30000 ms, 10 consecutive failures.
    fn default() -> Self {
        Reconnect { initial_ms: 1000, max_ms: 30_000, retries: 10 }
    }
}

impl Reconnect {
    fn delay(&self, failures: u32) -> Duration {
        let shift = failures.saturating_sub(1).min(20);
        Duration::from_millis(self.initial_ms.saturating_mul(1u64 << shift).min(self.max_ms))
    }
}

/// Options of one server-sent-events call.
#[derive(Clone, Default)]
pub struct SseOptions {
    /// Resume after this event id (sent as `Last-Event-ID`).
    pub last_event_id: Option<String>,
    /// Called with every event id as it is delivered.
    pub on_event_id: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    /// Overrides the client's reconnect policy for this call.
    pub reconnect: Option<Reconnect>,
}

impl SseOptions {
    /// Resumes after `id`.
    #[must_use]
    pub fn resume_after(id: impl Into<String>) -> Self {
        SseOptions { last_event_id: Some(id.into()), ..Self::default() }
    }

    /// Reports every event id.
    #[must_use]
    pub fn on_event_id(mut self, f: impl Fn(&str) + Send + Sync + 'static) -> Self {
        self.on_event_id = Some(Arc::new(f));
        self
    }

    /// Reconnects after connection failures.
    #[must_use]
    pub fn reconnect(mut self, policy: Reconnect) -> Self {
        self.reconnect = Some(policy);
        self
    }
}

impl fmt::Debug for SseOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SseOptions").field("last_event_id", &self.last_event_id).finish_non_exhaustive()
    }
}
// @end

type Supplier<T> = Arc<dyn Fn() -> T + Send + Sync>;

/// Options of a generated client.
#[derive(Clone)]
pub struct ClientOptions {
    /// The server's base URL. Else `VELD_<SERVICE>_URL`, else the contract's `base-url`.
    pub base_url: Option<String>,
    /// Headers sent with every request.
    pub headers: Vec<(String, String)>,
    header_supplier: Option<Supplier<Vec<(String, String)>>>,
    bearer: Option<Supplier<String>>,
    basic: Option<Supplier<(String, String)>>,
    api_key: Option<Supplier<String>>,
    /// Validate requests before sending them. Unset: the default of the generated crate (`true`,
    /// or `false` for a target built with `validate = false`).
    pub validate: Option<bool>,
    /// The timeout of one request (default 30 s).
    pub timeout: Duration,
    // #if rules
    /// Check the invariants of a request body and of a `query` object before sending (default
    /// `true`). A violation is a local `ValidationError`; nothing is sent.
    pub check_rules: bool,
    // #endif
    // @if sse
    /// Reconnect policy of streams (default: none).
    pub reconnect: Option<Reconnect>,
    // @end
}

impl Default for ClientOptions {
    fn default() -> Self {
        ClientOptions {
            base_url: None,
            headers: Vec::new(),
            header_supplier: None,
            bearer: None,
            basic: None,
            api_key: None,
            validate: None,
            timeout: Duration::from_secs(30),
            // #if rules
            check_rules: true,
            // #endif
            // @if sse
            reconnect: None,
            // @end
        }
    }
}

impl fmt::Debug for ClientOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientOptions").field("base_url", &self.base_url).field("validate", &self.validate).finish_non_exhaustive()
    }
}

impl ClientOptions {
    /// Options with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the base URL.
    #[must_use]
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Adds a header to every request.
    #[must_use]
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Headers computed for every request.
    #[must_use]
    pub fn header_supplier(mut self, f: impl Fn() -> Vec<(String, String)> + Send + Sync + 'static) -> Self {
        self.header_supplier = Some(Arc::new(f));
        self
    }

    /// The bearer token, read for every request that needs one.
    #[must_use]
    pub fn bearer(mut self, f: impl Fn() -> String + Send + Sync + 'static) -> Self {
        self.bearer = Some(Arc::new(f));
        self
    }

    /// The user name and password of basic auth.
    #[must_use]
    pub fn basic(mut self, f: impl Fn() -> (String, String) + Send + Sync + 'static) -> Self {
        self.basic = Some(Arc::new(f));
        self
    }

    /// The API key.
    #[must_use]
    pub fn api_key(mut self, f: impl Fn() -> String + Send + Sync + 'static) -> Self {
        self.api_key = Some(Arc::new(f));
        self
    }

    /// Switches request validation on or off. Off, the types stay and nothing is checked locally.
    #[must_use]
    pub fn validate(mut self, on: bool) -> Self {
        self.validate = Some(on);
        self
    }

    /// Sets the request timeout.
    #[must_use]
    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }
    // #if rules

    /// Switches the local check of invariants on or off. Off, the server decides alone.
    #[must_use]
    pub fn check_rules(mut self, on: bool) -> Self {
        self.check_rules = on;
        self
    }
    // #endif

    // @if sse
    /// Reconnects streams after connection failures.
    #[must_use]
    pub fn reconnect(mut self, policy: Reconnect) -> Self {
        self.reconnect = Some(policy);
        self
    }
    // @end
}

fn scalar_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.as_i64().map_or_else(|| es_number(n.as_f64().unwrap_or(0.0)), |i| i.to_string()),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        Value::Array(_) | Value::Object(_) => super::canonical::canonical(v),
    }
}

fn push_query(query: &mut Vec<(String, String)>, key: &str, wire: &Value) {
    match wire {
        Value::Null => {}
        Value::Array(items) => {
            for item in items {
                query.push((key.to_string(), scalar_text(item)));
            }
        }
        other => query.push((key.to_string(), scalar_text(other))),
    }
}

/// The part of a client that does not depend on the contract.
pub struct Core<T: Transport> {
    transport: Arc<T>,
    base: String,
    options: ClientOptions,
    std: StdSigs,
    validate: bool,
}

impl<T: Transport> Clone for Core<T> {
    fn clone(&self) -> Self {
        Core {
            transport: Arc::clone(&self.transport),
            base: self.base.clone(),
            options: self.options.clone(),
            std: self.std,
            validate: self.validate,
        }
    }
}

impl<T: Transport> fmt::Debug for Core<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Core({})", self.base)
    }
}

/// An outgoing request, built and checked.
struct Built {
    request: HttpRequest,
}

impl<T: Transport> Core<T> {
    /// Resolves the base URL: `options.base_url`, else the environment variable `env`, else
    /// `default_base` (the contract's `base-url`).
    ///
    /// # Errors
    /// [`Error::Transport`] when none of the three is set.
    pub fn new(
        transport: T,
        env: &str,
        default_base: Option<&str>,
        std: StdSigs,
        validate_default: bool,
        options: ClientOptions,
    ) -> Result<Self, Error> {
        let base = options
            .base_url
            .clone()
            .filter(|b| !b.is_empty())
            .or_else(|| std::env::var(env).ok().filter(|b| !b.is_empty()))
            .or_else(|| default_base.map(str::to_string))
            .ok_or_else(|| Error::Transport(format!("no base URL: pass `base_url`, set {env} or give the contract a base-url")))?;
        let validate = options.validate.unwrap_or(validate_default);
        Ok(Core { transport: Arc::new(transport), base: base.trim_end_matches('/').to_string(), options, std, validate })
    }

    /// The transport.
    #[must_use]
    pub fn transport(&self) -> &T {
        &self.transport
    }

    fn validation_error(&self, ep: &EndpointSpec, issues: &[Issue]) -> Error {
        let spec = ep.errors.iter().find(|e| e.sig == self.std.validation);
        let data = serde_json::json!({ "issues": serde_json::to_value(issues).unwrap_or(Value::Null) });
        Error::Problem(Problem {
            sig: self.std.validation.to_string(),
            title: spec.map_or("ValidationError", |s| s.name).to_string(),
            status: 422,
            op: Some(ep.sig.to_string()),
            data,
            schema: spec.map(|s| s.schema),
            // #if rules
            rulings: super::rules::Rulings::boxed(Vec::new(), true),
            // #endif
        })
    }
    // #if rules

    /// The local `ValidationError` of rule violations: one issue each, plus the violations.
    fn rule_error(&self, ep: &EndpointSpec, violations: Vec<super::rules::Violation>) -> Error {
        let spec = ep.errors.iter().find(|e| e.sig == self.std.validation);
        Error::Problem(Problem {
            sig: self.std.validation.to_string(),
            title: spec.map_or("ValidationError", |s| s.name).to_string(),
            status: 422,
            op: Some(ep.sig.to_string()),
            data: super::rules::rejection_data(&violations),
            schema: spec.map(|s| s.schema),
            rulings: super::rules::Rulings::boxed(violations, true),
        })
    }
    // #endif

    fn auth_headers(&self, ep: &EndpointSpec, out: &mut Vec<(String, String)>) {
        let Some(auth) = &ep.auth else {
            return;
        };
        match auth.kind {
            AuthKind::Bearer => {
                if let Some(b) = &self.options.bearer {
                    out.push(("authorization".into(), format!("Bearer {}", b())));
                }
            }
            AuthKind::Basic => {
                if let Some(b) = &self.options.basic {
                    let (user, password) = b();
                    out.push(("authorization".into(), format!("Basic {}", base64_encode(format!("{user}:{password}").as_bytes()))));
                }
            }
            AuthKind::ApiKey => {
                if let Some(k) = &self.options.api_key {
                    out.push((auth.header.unwrap_or("X-API-Key").to_string(), k()));
                }
            }
        }
    }

    fn build(&self, ep: &EndpointSpec, req: &Value, accept: &str) -> Result<Built, Error> {
        let empty = Map::new();
        let args = req.as_object().unwrap_or(&empty);
        let mut path = ep.path.to_string();
        let mut query: Vec<(String, String)> = Vec::new();
        let mut headers: Vec<(String, String)> = Vec::new();
        let mut body: Option<Value> = None;
        let mut issues = Vec::new();
        // #if rules
        let mut violations = Vec::new();
        // #endif
        for p in ep.params.iter().filter(|p| !p.stream) {
            let Some(typed) = args.get(p.name).filter(|v| !v.is_null()) else {
                if !p.optional && self.validate {
                    issues.push(Issue { path: p.location(), code: "required".into(), message: "is required".into() });
                }
                continue;
            };
            let wire = p.schema.to_wire(typed, true, false);
            if self.validate {
                issues.extend(p.schema.validate(&wire, &p.location(), true));
            }
            // #if rules
            if self.options.check_rules && issues.is_empty() && matches!(p.binding, Binding::Body | Binding::QueryObject) {
                violations.extend(super::rules::check_wire(p.schema, &wire, &p.location(), super::rules::Mode::Request));
            }
            // #endif
            match p.binding {
                Binding::Path => path = path.replace(&format!("{{{}}}", p.name), &pct_encode(&scalar_text(&wire))),
                Binding::Query => push_query(&mut query, p.name, &wire),
                Binding::QueryObject => {
                    for (k, v) in wire.as_object().unwrap_or(&empty) {
                        push_query(&mut query, k, v);
                    }
                }
                Binding::Header => headers.push((p.header.unwrap_or(p.name).to_string(), scalar_text(&wire))),
                Binding::Body => body = Some(wire),
            }
        }
        if !issues.is_empty() {
            return Err(self.validation_error(ep, &issues));
        }
        // #if rules
        if !violations.is_empty() {
            return Err(self.rule_error(ep, violations));
        }
        // #endif
        query.sort_by(|a, b| a.0.cmp(&b.0));
        let mut url = format!("{}{path}", self.base);
        if !query.is_empty() {
            let pairs: Vec<String> = query.iter().map(|(k, v)| format!("{}={}", pct_encode(k), pct_encode(v))).collect();
            url.push('?');
            url.push_str(&pairs.join("&"));
        }
        let mut all = vec![("accept".to_string(), accept.to_string())];
        all.extend(self.options.headers.iter().cloned());
        if let Some(supplier) = &self.options.header_supplier {
            all.extend(supplier());
        }
        self.auth_headers(ep, &mut all);
        all.extend(headers);
        let body = body.map(|b| serde_json::to_vec(&b).unwrap_or_default());
        if body.is_some() {
            all.push(("content-type".into(), "application/json".into()));
        }
        Ok(Built {
            request: HttpRequest { method: ep.method.to_string(), url, headers: all, body, timeout: self.options.timeout },
        })
    }

    /// The typed error of a non-2xx response: a declared problem, else [`Error::Http`].
    fn decode_error(ep: &EndpointSpec, status: u16, body: &[u8]) -> Error {
        let http = || Error::Http { status, body: String::from_utf8_lossy(body).into_owned() };
        let Some(problem) = serde_json::from_slice::<Value>(body).ok().and_then(|v| Problem::from_json(&v)) else {
            return http();
        };
        let Some(spec) = ep.errors.iter().find(|e| e.sig == problem.sig) else {
            return http();
        };
        let data = spec.schema.to_wire(&spec.schema.from_wire(&problem.data, false), false, false);
        Error::Problem(Problem {
            sig: problem.sig,
            title: spec.name.to_string(),
            status,
            op: problem.op.or_else(|| Some(ep.sig.to_string())),
            data,
            schema: Some(spec.schema),
            // #if rules
            rulings: problem.rulings,
            // #endif
        })
    }

    fn decode_success(ep: &EndpointSpec, body: &[u8]) -> Result<Option<Value>, Error> {
        let Some(schema) = ep.success else {
            return Ok(None);
        };
        let wire: Value = serde_json::from_slice(body).map_err(|e| Error::Transport(format!("{}: invalid response body: {e}", ep.name)))?;
        Ok(Some(schema.from_wire(&wire, false)))
    }

    async fn exchange(&self, ep: &EndpointSpec, req: &Value) -> Result<Option<Value>, Error> {
        let built = self.build(ep, req, "application/json")?;
        let resp = self.transport.send(built.request).await?;
        if (200..300).contains(&resp.status) {
            Self::decode_success(ep, &resp.body)
        } else {
            Err(Self::decode_error(ep, resp.status, &resp.body))
        }
    }

    /// Calls an endpoint with a success value.
    ///
    /// # Errors
    /// A declared [`Error::Problem`], [`Error::Http`] or [`Error::Transport`].
    pub async fn call<Req: Serialize, Res: DeserializeOwned>(&self, ep: &'static EndpointSpec, req: &Req) -> Result<Res, Error> {
        let value = serde_json::to_value(req).map_err(Error::other)?;
        let typed = self
            .exchange(ep, &value)
            .await?
            .ok_or_else(|| Error::Transport(format!("{}: the endpoint has no response body", ep.name)))?;
        serde_json::from_value(typed).map_err(|e| Error::Transport(format!("{}: unreadable response: {e}", ep.name)))
    }

    /// Calls an endpoint that answers without a body.
    ///
    /// # Errors
    /// A declared [`Error::Problem`], [`Error::Http`] or [`Error::Transport`].
    pub async fn call_unit<Req: Serialize>(&self, ep: &'static EndpointSpec, req: &Req) -> Result<(), Error> {
        let value = serde_json::to_value(req).map_err(Error::other)?;
        self.exchange(ep, &value).await.map(|_| ())
    }

    // @if sse
    /// Opens a server-sent-events endpoint. The HTTP call is eager: a non-2xx response is the
    /// typed error of this method; later errors arrive as the stream's last item.
    ///
    /// # Errors
    /// The typed error of a failed first response.
    pub async fn stream<Req: Serialize, Item: DeserializeOwned + Send + 'static>(
        &self,
        ep: &'static EndpointSpec,
        req: &Req,
        opts: SseOptions,
    ) -> Result<BoxStream<Result<Item, Error>>, Error> {
        let value = serde_json::to_value(req).map_err(Error::other)?;
        let built = self.build(ep, &value, "text/event-stream")?;
        let reconnect = opts.reconnect.or(self.options.reconnect);
        let mut state = SseState {
            core: self.clone(),
            ep,
            request: built.request,
            conn: None,
            last_id: opts.last_event_id,
            on_event_id: opts.on_event_id,
            reconnect,
            failures: 0,
            done: false,
            item: PhantomData,
        };
        state.connect_first().await?;
        let body = unfold(state, |st: SseState<T, Item>| Box::pin(st.step()));
        Ok(Box::pin(body))
    }
    // @end
}

// @if sse
struct Connection {
    body: BoxStream<Result<Vec<u8>, Error>>,
    parser: SseParser,
    pending: VecDeque<SseMessage>,
}

/// Opens one connection. A non-2xx response is the typed error of the call.
async fn connect<T: Transport>(core: Core<T>, ep: &'static EndpointSpec, request: HttpRequest) -> Result<Connection, Error> {
    let resp = core.transport.open(request).await?;
    if (200..300).contains(&resp.status) {
        return Ok(Connection { body: resp.body, parser: SseParser::new(), pending: VecDeque::new() });
    }
    let mut body = resp.body;
    let mut all = Vec::new();
    while let Some(Ok(chunk)) = next(&mut body).await {
        all.extend(chunk);
    }
    Err(Core::<T>::decode_error(ep, resp.status, &all))
}

struct SseState<T: Transport, Item> {
    core: Core<T>,
    ep: &'static EndpointSpec,
    request: HttpRequest,
    conn: Option<Connection>,
    last_id: Option<String>,
    on_event_id: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    reconnect: Option<Reconnect>,
    failures: u32,
    done: bool,
    item: PhantomData<fn() -> Item>,
}

impl<T: Transport, Item: DeserializeOwned + Send + 'static> SseState<T, Item> {
    /// The request of the next connection: resumes after the last event id.
    fn next_request(&self) -> HttpRequest {
        let mut request = self.request.clone();
        if let Some(id) = &self.last_id {
            request.headers.push(("last-event-id".into(), id.clone()));
        }
        request
    }


    /// The first connection: retried like any other when a reconnect policy is set.
    async fn connect_first(&mut self) -> Result<(), Error> {
        loop {
            match connect(self.core.clone(), self.ep, self.next_request()).await {
                Ok(c) => {
                    self.conn = Some(c);
                    return Ok(());
                }
                Err(e @ Error::Transport(_)) => self.backoff(e).await?,
                Err(e) => return Err(e),
            }
        }
    }

    /// Sleeps before the next attempt, or gives `err` back when the attempts ran out.
    async fn backoff(&mut self, err: Error) -> Result<(), Error> {
        match self.reconnect {
            Some(r) if self.failures < r.retries => {
                self.failures += 1;
                self.core.transport.sleep(r.delay(self.failures)).await;
                Ok(())
            }
            Some(_) | None => Err(err),
        }
    }

    fn deliver(&mut self, msg: &SseMessage) -> Result<Item, Error> {
        if msg.event == "error" {
            let status = serde_json::from_str::<Value>(&msg.data)
                .ok()
                .and_then(|v| v.get("status").and_then(Value::as_u64))
                .and_then(|s| u16::try_from(s).ok())
                .unwrap_or(0);
            return Err(Core::<T>::decode_error(self.ep, status, msg.data.as_bytes()));
        }
        let wire: Value = serde_json::from_str(&msg.data).map_err(|e| Error::Transport(format!("invalid event data: {e}")))?;
        let schema = self.ep.success.ok_or_else(|| Error::Transport("a stream needs an item type".into()))?;
        serde_json::from_value(schema.from_wire(&wire, false)).map_err(|e| Error::Transport(format!("unreadable event: {e}")))
    }

    async fn step(mut self) -> Option<(Result<Item, Error>, Self)> {
        loop {
            if self.done {
                return None;
            }
            let Some(conn) = self.conn.as_mut() else {
                match connect(self.core.clone(), self.ep, self.next_request()).await {
                    Ok(c) => self.conn = Some(c),
                    Err(e @ Error::Transport(_)) => {
                        if let Err(e) = self.backoff(e).await {
                            self.done = true;
                            return Some((Err(e), self));
                        }
                    }
                    Err(e) => {
                        self.done = true;
                        return Some((Err(e), self));
                    }
                }
                continue;
            };
            if let Some(msg) = conn.pending.pop_front() {
                if let Some(id) = &msg.id {
                    self.last_id = Some(id.clone());
                    if let Some(f) = &self.on_event_id {
                        f(id);
                    }
                }
                let item = self.deliver(&msg);
                if item.is_err() {
                    self.done = true;
                } else {
                    self.failures = 0;
                }
                return Some((item, self));
            }
            match next(&mut conn.body).await {
                Some(Ok(chunk)) => {
                    let events = conn.parser.push(&chunk);
                    conn.pending.extend(events);
                }
                None => return None,
                Some(Err(e)) => {
                    self.conn = None;
                    if let Err(e) = self.backoff(e).await {
                        self.done = true;
                        return Some((Err(e), self));
                    }
                }
            }
        }
    }
}
// @end

// @if ws_client
/// What a connected WebSocket reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsEvent {
    Text(String),
    /// The peer closed the socket with this code.
    Closed(u16),
    /// The connection broke.
    Failed(String),
}

/// The writing half of a WebSocket.
pub trait WsSink: Send {
    fn send<'a>(&'a mut self, text: String) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;

    fn close<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
}

/// A connected WebSocket: where to write, and what arrives.
pub struct WsConn {
    pub sink: Box<dyn WsSink>,
    pub events: BoxStream<WsEvent>,
}

impl fmt::Debug for WsConn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WsConn")
    }
}

/// Options of one WebSocket call.
#[derive(Debug, Clone, Copy, Default)]
pub struct WsOptions {
    /// Overrides the client's reconnect policy for this call.
    pub reconnect: Option<Reconnect>,
}

impl WsOptions {
    /// Reconnects after connection failures.
    #[must_use]
    pub fn reconnect(mut self, policy: Reconnect) -> Self {
        self.reconnect = Some(policy);
        self
    }
}

fn ws_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = url.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        url.to_string()
    }
}

impl<T: Transport> Core<T> {
    /// Opens a WebSocket endpoint. The channel is connected when this returns; a refused
    /// connection is retried like any connection failure when a reconnect policy is set.
    ///
    /// # Errors
    /// [`Error::Transport`] when the connection cannot be made, or a local `ValidationError`.
    pub async fn ws<Req: Serialize, In: DeserializeOwned, Out: Serialize>(
        &self,
        ep: &'static EndpointSpec,
        req: &Req,
        opts: WsOptions,
    ) -> Result<WsChannel<T, In, Out>, Error> {
        let value = serde_json::to_value(req).map_err(Error::other)?;
        let built = self.build(ep, &value, "*/*")?;
        let headers = built
            .request
            .headers
            .into_iter()
            .filter(|(k, _)| !k.eq_ignore_ascii_case("accept") && !k.eq_ignore_ascii_case("content-type"))
            .collect();
        let mut channel = WsChannel {
            core: self.clone(),
            ep,
            url: ws_url(&built.request.url),
            headers,
            conn: None,
            reconnect: opts.reconnect.or(self.options.reconnect),
            failures: 0,
            queue: VecDeque::new(),
            done: false,
            types: PhantomData,
        };
        channel.establish().await?;
        Ok(channel)
    }
}

/// A WebSocket, from the client's side: `In` messages arrive from the server, `Out` messages are
/// sent to it. `()` marks a side that has no messages.
///
/// Reconnecting is driven by [`recv`](WsChannel::recv): when the connection breaks, `recv` waits
/// (the policy's backoff), opens a new socket (the server starts a new conversation) and sends
/// what was queued in the meantime. Drop the channel to stop.
pub struct WsChannel<T: Transport, In, Out> {
    core: Core<T>,
    ep: &'static EndpointSpec,
    url: String,
    headers: Vec<(String, String)>,
    conn: Option<WsConn>,
    reconnect: Option<Reconnect>,
    failures: u32,
    queue: VecDeque<String>,
    done: bool,
    types: PhantomData<fn(Out) -> In>,
}

impl<T: Transport, In, Out> fmt::Debug for WsChannel<T, In, Out> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WsChannel({})", self.url)
    }
}

impl<T: Transport, In: DeserializeOwned, Out: Serialize> WsChannel<T, In, Out> {
    /// Connects, retrying per the policy, and sends what is queued.
    async fn establish(&mut self) -> Result<(), Error> {
        loop {
            let connected = self.core.transport.connect_ws(self.url.clone(), self.headers.clone(), self.core.options.timeout).await;
            match connected {
                Ok(conn) => {
                    self.conn = Some(conn);
                    if self.flush().await {
                        return Ok(());
                    }
                    self.conn = None;
                    self.backoff(Error::Transport("the connection broke while sending".into())).await?;
                }
                Err(e) => self.backoff(e).await?,
            }
        }
    }

    /// Sleeps before the next attempt, or gives `err` back when the attempts ran out.
    async fn backoff(&mut self, err: Error) -> Result<(), Error> {
        match self.reconnect {
            Some(r) if self.failures < r.retries => {
                self.failures += 1;
                self.core.transport.sleep(r.delay(self.failures)).await;
                Ok(())
            }
            Some(_) | None => Err(err),
        }
    }

    /// Writes the queued messages; `false` when the connection broke.
    async fn flush(&mut self) -> bool {
        while let Some(text) = self.queue.pop_front() {
            let Some(conn) = self.conn.as_mut() else {
                self.queue.push_front(text);
                return false;
            };
            if conn.sink.send(text.clone()).await.is_err() {
                self.queue.push_front(text);
                return false;
            }
        }
        true
    }

    /// Sends a message. It is validated first (a local `ValidationError` whose paths start with
    /// `message`) unless validation is off. While the connection is down it is queued.
    ///
    /// # Errors
    /// A local `ValidationError`, or [`Error::Other`] when the message cannot be encoded.
    pub async fn send(&mut self, message: Out) -> Result<(), Error> {
        let typed = serde_json::to_value(&message).map_err(Error::other)?;
        let schema = self.ep.params.iter().find(|p| p.stream).map(|p| p.schema);
        let text = match schema {
            Some(schema) => {
                if self.core.validate {
                    let issues = schema.validate(&schema.to_wire(&typed, false, false), "message", true);
                    if !issues.is_empty() {
                        return Err(self.core.validation_error(self.ep, &issues));
                    }
                }
                String::from_utf8_lossy(&schema.canonical_typed(&typed)).into_owned()
            }
            None => super::canonical::canonical(&typed),
        };
        self.queue.push_back(text);
        if self.conn.is_some() && !self.flush().await {
            self.conn = None;
        }
        Ok(())
    }

    /// The next message from the server. `None` when the server ended the conversation normally
    /// (or after `close`); `Err` with the typed error of an error frame (never retried), or
    /// [`Error::Transport`] when the connection fails and no (more) reconnects are allowed.
    pub async fn recv(&mut self) -> Option<Result<In, Error>> {
        loop {
            if self.done {
                return None;
            }
            if self.conn.is_none() {
                if let Err(e) = self.establish().await {
                    self.done = true;
                    return Some(Err(e));
                }
            }
            let Some(conn) = self.conn.as_mut() else {
                continue;
            };
            let failure = match next(&mut conn.events).await {
                Some(WsEvent::Text(text)) => match self.frame(&text) {
                    Some(item) => return Some(item),
                    None => continue,
                },
                Some(WsEvent::Closed(1000 | 1001 | 1005)) => {
                    self.done = true;
                    return None;
                }
                Some(WsEvent::Closed(code)) => format!("the server closed the socket with code {code}"),
                Some(WsEvent::Failed(why)) => why,
                None => "the connection ended without a close frame".to_string(),
            };
            self.conn = None;
            if let Err(e) = self.backoff(Error::Transport(failure)).await {
                self.done = true;
                return Some(Err(e));
            }
        }
    }

    /// Decodes one text frame: `{"data": ...}` is a message, `{"error": ...}` the typed error that
    /// ends the conversation.
    fn frame(&mut self, text: &str) -> Option<Result<In, Error>> {
        let Ok(Value::Object(mut obj)) = serde_json::from_str::<Value>(text) else {
            return Some(Err(Error::Transport("unreadable WebSocket frame".into())));
        };
        if let Some(problem) = obj.remove("error") {
            self.done = true;
            let status = problem.get("status").and_then(Value::as_u64).and_then(|s| u16::try_from(s).ok()).unwrap_or(0);
            let body = serde_json::to_vec(&problem).unwrap_or_default();
            return Some(Err(Core::<T>::decode_error(self.ep, status, &body)));
        }
        let wire = obj.remove("data").unwrap_or(Value::Null);
        self.failures = 0;
        let typed = match self.ep.success {
            Some(schema) => schema.from_wire(&wire, false),
            None => wire,
        };
        Some(serde_json::from_value(typed).map_err(|e| Error::Transport(format!("unreadable message: {e}"))))
    }

    /// Closes the socket; `recv` then returns `None`.
    pub async fn close(&mut self) {
        self.done = true;
        if let Some(mut conn) = self.conn.take() {
            conn.sink.close().await;
        }
    }
}
// @end
