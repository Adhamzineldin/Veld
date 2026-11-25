//! The server core: routing, auth, middleware, binding, validation, declared-error mapping.
//!
//! It is framework agnostic. A generated `<Service>Handler` wires a service implementation and
//! its hooks into an [`Endpoints`] value; [`Handler`] turns a [`Request`] into a [`Response`].
//! Framework adapters (`adapters/*.rs`) only translate to and from these two types.

use std::any::Any;
use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use super::error::{Error, Issue, Problem};
use super::schema::{Scalar, Schema};
use super::spec::{
    AuthKind, AuthSpec, Binding, EndpointSpec, Kind, ParamSpec, StdSigs, path_without_trailing_slash,
};
use super::util::{base64_decode, pct_decode};
// @if sse
use super::stream::{BoxStream, EventStream, SseEvent, next, sse_frame, unfold};
// @end
// @if ws_server
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use serde::Serialize;

use super::stream::{Chan, Stream};
// @end

/// A request as the core sees it.
#[derive(Debug, Clone, Default)]
pub struct Request {
    pub method: String,
    /// The path without the query string, as sent (percent-encoded).
    pub path: String,
    /// The raw query string without `?`.
    pub query: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// The body of a [`Response`].
pub enum Body {
    Empty,
    Bytes(Vec<u8>),
    // @if sse
    /// Server-Sent Events: ready-made frames. Drop the stream when the client goes away.
    Sse(BoxStream<Vec<u8>>),
    // @end
}

impl fmt::Debug for Body {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Body::Empty => f.write_str("Body::Empty"),
            Body::Bytes(b) => write!(f, "Body::Bytes({} bytes)", b.len()),
            // @if sse
            Body::Sse(_) => f.write_str("Body::Sse"),
            // @end
        }
    }
}

/// A response as the core produces it.
#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Body,
}

/// Whatever a hook returns to identify the caller. Read it back with [`Principal::get`].
#[derive(Clone)]
pub struct Principal(Arc<dyn Any + Send + Sync>);

impl Principal {
    /// Wraps a value.
    #[must_use]
    pub fn new<T: Any + Send + Sync>(value: T) -> Self {
        Principal(Arc::new(value))
    }

    /// The value, when it is a `T`.
    #[must_use]
    pub fn get<T: Any>(&self) -> Option<&T> {
        self.0.downcast_ref::<T>()
    }
}

impl fmt::Debug for Principal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Principal")
    }
}

/// What a service method and a hook can see of the request.
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub method: String,
    pub path: String,
    /// Header names are lower case.
    pub headers: Vec<(String, String)>,
    /// Set by the auth hook of an endpoint that needs auth.
    pub principal: Option<Principal>,
}

impl RequestContext {
    /// The first header named `name` (case-insensitive).
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        header(&self.headers, name)
    }

    /// The principal, when the auth hook returned a `T`.
    #[must_use]
    pub fn principal<T: Any>(&self) -> Option<&T> {
        self.principal.as_ref().and_then(Principal::get::<T>)
    }
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

/// What a generated service supplies to the core.
pub trait Endpoints: Send + Sync + 'static {
    fn endpoints(&self) -> &'static [EndpointSpec];

    fn std_sigs(&self) -> StdSigs;

    /// Validate parameters and bodies against the contract (default `true`). Off, values are
    /// only decoded.
    fn validate_requests(&self) -> bool {
        true
    }

    /// Validate every response and stream item before it is sent (default `false`).
    fn validate_responses(&self) -> bool {
        false
    }
    // #if rules

    /// What happens to a violated invariant of the server's own output (default `Strict`).
    fn rules_output(&self) -> super::rules::RuleOutput {
        super::rules::RuleOutput::Strict
    }

    /// Observes every violated rule, of the input (rejected) and of the output (rejected or
    /// audited). It cannot change the response.
    fn on_rule_violation(&self, _event: &super::rules::RuleEvent) {}

    /// Whether a body, query object or WebSocket message of this service can reach a rule: with
    /// request validation off the server says once, when it is created, that they are not enforced.
    fn reaches_rules(&self) -> bool {
        false
    }
    // #endif

    /// Observes every error that becomes `Internal`. It cannot change the response.
    fn on_error(&self, _err: &Error, _ctx: &RequestContext) {}

    fn auth_bearer<'a>(
        &'a self,
        _token: &'a str,
        _ctx: &'a RequestContext,
    ) -> impl Future<Output = Option<Principal>> + Send + 'a {
        async { None }
    }

    fn auth_basic<'a>(
        &'a self,
        _user: &'a str,
        _password: &'a str,
        _ctx: &'a RequestContext,
    ) -> impl Future<Output = Option<Principal>> + Send + 'a {
        async { None }
    }

    fn auth_api_key<'a>(
        &'a self,
        _key: &'a str,
        _ctx: &'a RequestContext,
    ) -> impl Future<Output = Option<Principal>> + Send + 'a {
        async { None }
    }

    fn middleware<'a>(
        &'a self,
        _name: &'a str,
        _ctx: &'a RequestContext,
    ) -> impl Future<Output = Result<(), Error>> + Send + 'a {
        async { Ok(()) }
    }

    /// Calls the implementation of `endpoint`. `args` holds the decoded parameters by name; the
    /// result is the typed JSON of the success value.
    fn call<'a>(
        &'a self,
        endpoint: usize,
        args: Map<String, Value>,
        ctx: &'a RequestContext,
    ) -> impl Future<Output = Result<Value, Error>> + Send + 'a;

    // @if ws_server
    /// Runs a WebSocket endpoint: it returns when the conversation is over.
    fn call_ws<'a>(
        &'a self,
        _endpoint: usize,
        _args: Map<String, Value>,
        _ctx: &'a RequestContext,
        _io: WsIo,
    ) -> impl Future<Output = Result<(), Error>> + Send + 'a {
        async { Err(Error::other("not a WebSocket endpoint")) }
    }
    // @end

    // @if sse
    /// Calls a server-sent-events endpoint; the stream's items are typed JSON.
    fn call_sse<'a>(
        &'a self,
        endpoint: usize,
        args: Map<String, Value>,
        ctx: &'a RequestContext,
    ) -> impl Future<Output = Result<EventStream<Value>, Error>> + Send + 'a;
    // @end
}

/// Decodes the bound arguments into a request struct.
///
/// # Errors
/// [`Error::Invalid`] when they do not fit (only possible with `validate_requests` off).
pub fn decode_args<T: DeserializeOwned>(args: Map<String, Value>) -> Result<T, Error> {
    serde_json::from_value(Value::Object(args)).map_err(|e| {
        Error::Invalid(vec![Issue { path: "request".into(), code: "type".into(), message: e.to_string() }])
    })
}

/// Encodes a success value as typed JSON.
///
/// # Errors
/// [`Error::Other`] when the value cannot be serialised.
pub fn encode_value<T: serde::Serialize>(value: &T) -> Result<Value, Error> {
    serde_json::to_value(value).map_err(Error::other)
}

#[derive(Debug, Clone)]
struct Seg {
    pre: String,
    param: Option<String>,
    post: String,
}

#[derive(Debug, Clone)]
struct Route {
    index: usize,
    method: &'static str,
    segs: Vec<Seg>,
    params: usize,
}

fn split_path(path: &str) -> Vec<&str> {
    let p = path_without_trailing_slash(path);
    let t = p.strip_prefix('/').unwrap_or(p);
    if t.is_empty() { Vec::new() } else { t.split('/').collect() }
}

fn compile(index: usize, ep: &EndpointSpec) -> Route {
    let segs: Vec<Seg> = split_path(ep.path)
        .into_iter()
        .map(|s| match (s.find('{'), s.find('}')) {
            (Some(a), Some(b)) if a < b => Seg {
                pre: s[..a].to_string(),
                param: Some(s[a + 1..b].to_string()),
                post: s[b + 1..].to_string(),
            },
            _ => Seg { pre: s.to_string(), param: None, post: String::new() },
        })
        .collect();
    let params = segs.iter().filter(|s| s.param.is_some()).count();
    let method = if ep.kind == Kind::Ws { "WS" } else { ep.method };
    Route { index, method, segs, params }
}

fn matches(route: &Route, path: &str) -> Option<Vec<(String, String)>> {
    let parts = split_path(path);
    if parts.len() != route.segs.len() {
        return None;
    }
    let mut caps = Vec::new();
    for (seg, part) in route.segs.iter().zip(parts) {
        match &seg.param {
            None => {
                if seg.pre != part {
                    return None;
                }
            }
            Some(name) => {
                let inner = part.strip_prefix(seg.pre.as_str())?.strip_suffix(seg.post.as_str())?;
                if inner.is_empty() {
                    return None;
                }
                caps.push((name.clone(), inner.to_string()));
            }
        }
    }
    Some(caps)
}

struct Inner<E> {
    endpoints: E,
    routes: Vec<Route>,
}

/// Serves one service: `handle` runs the whole pipeline for a request.
pub struct Handler<E: Endpoints> {
    inner: Arc<Inner<E>>,
}

impl<E: Endpoints> Clone for Handler<E> {
    fn clone(&self) -> Self {
        Handler { inner: Arc::clone(&self.inner) }
    }
}

impl<E: Endpoints> fmt::Debug for Handler<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handler({} routes)", self.inner.routes.len())
    }
}

fn parse_query(qs: &str) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for pair in qs.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        if let (Some(k), Some(v)) = (pct_decode(k, true), pct_decode(v, true)) {
            out.entry(k).or_default().push(v);
        }
    }
    out
}

fn number(raw: &str) -> Option<Value> {
    let ok = !raw.is_empty()
        && raw.chars().all(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E'))
        && raw.chars().any(|c| c.is_ascii_digit());
    if !ok {
        return None;
    }
    if let Ok(i) = raw.parse::<i64>() {
        return Some(Value::from(i));
    }
    raw.parse::<f64>().ok().and_then(serde_json::Number::from_f64).map(Value::Number)
}

fn coerce(s: &Schema, raw: &str) -> Value {
    match s {
        Schema::Scalar(Scalar::Int | Scalar::Float, _) => number(raw).unwrap_or_else(|| Value::String(raw.into())),
        Schema::Scalar(Scalar::Bool, _) => match raw {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ => Value::String(raw.into()),
        },
        Schema::Scalar(..)
        | Schema::Enum(_)
        | Schema::List(..)
        | Schema::Map(..)
        | Schema::Page(_)
        | Schema::Model(_)
        | Schema::Union(_) => Value::String(raw.into()),
    }
}

fn coerce_many(s: &Schema, raw: &[String]) -> Value {
    match s.list_item() {
        Some(of) => Value::Array(raw.iter().map(|x| coerce(of, x)).collect()),
        None => coerce(s, raw.first().map_or("", String::as_str)),
    }
}

/// Collects what binding finds.
struct Bound {
    value: Map<String, Value>,
    issues: Vec<Issue>,
    check: bool,
    // #if rules
    violations: Vec<super::rules::Violation>,
    micros: u64,
    // #endif
}
// #if rules

impl Bound {
    /// Step 5: the invariants of a bound body or query object, once its shape is valid.
    fn rules(&mut self, p: &ParamSpec, wire: &Value) {
        let watch = super::rules::Stopwatch::start();
        let found = super::rules::check_wire(p.schema, wire, &p.location(), super::rules::Mode::Request);
        self.micros += watch.micros();
        self.violations.extend(found);
    }
}
// #endif

impl Bound {
    fn required(&mut self, p: &ParamSpec) {
        self.issues.push(Issue { path: p.location(), code: "required".into(), message: "is required".into() });
    }

    fn simple(&mut self, p: &ParamSpec, raw: Option<&[String]>) {
        let Some(raw) = raw.filter(|r| !r.is_empty()) else {
            if !p.optional {
                self.required(p);
            }
            return;
        };
        let wire = coerce_many(p.schema, raw);
        let issues = if self.check { p.schema.validate(&wire, &p.location(), false) } else { Vec::new() };
        if issues.is_empty() {
            self.value.insert(p.name.to_string(), p.schema.from_wire(&wire, false));
        }
        self.issues.extend(issues);
    }

    fn query_object(&mut self, p: &ParamSpec, query: &BTreeMap<String, Vec<String>>) {
        let Some(model) = p.schema.model() else {
            return;
        };
        let mut wire = Map::new();
        for f in model.fields {
            if let Some(raw) = query.get(f.wire) {
                wire.insert(f.wire.to_string(), coerce_many(f.schema, raw));
            }
        }
        let wire = Value::Object(wire);
        let issues = if self.check { p.schema.validate(&wire, &p.location(), true) } else { Vec::new() };
        // #if rules
        if self.check && issues.is_empty() {
            self.rules(p, &wire);
        }
        // #endif
        if issues.is_empty() {
            self.value.insert(p.name.to_string(), p.schema.from_wire(&wire, true));
        }
        self.issues.extend(issues);
    }

    fn body(&mut self, p: &ParamSpec, body: &[u8]) {
        if String::from_utf8_lossy(body).trim().is_empty() {
            if !p.optional {
                self.required(p);
            }
            return;
        }
        let Ok(raw) = serde_json::from_slice::<Value>(body) else {
            self.issues.push(Issue { path: "body".into(), code: "json".into(), message: "body is not valid JSON".into() });
            return;
        };
        let issues = if self.check { p.schema.validate(&raw, "body", true) } else { Vec::new() };
        // #if rules
        if self.check && issues.is_empty() {
            self.rules(p, &raw);
        }
        // #endif
        if issues.is_empty() {
            self.value.insert(p.name.to_string(), p.schema.from_wire(&raw, true));
        }
        self.issues.extend(issues);
    }
}

fn error_response(sig: &str, title: &str, status: u16, op: &str, data: Value) -> Response {
    let problem = Problem {
        sig: sig.to_string(),
        title: title.to_string(),
        status,
        op: Some(op.to_string()),
        data,
        schema: None,
        // #if rules
        rulings: None,
        // #endif
    };
    Response {
        status,
        headers: vec![("content-type".into(), "application/problem+json".into())],
        body: Body::Bytes(problem.to_text().into_bytes()),
    }
}

fn issues_json(issues: &[Issue]) -> Value {
    serde_json::json!({ "issues": serde_json::to_value(issues).unwrap_or(Value::Null) })
}

fn bearer_token(h: Option<&str>) -> Option<&str> {
    let (scheme, rest) = h?.trim().split_once(char::is_whitespace)?;
    let token = rest.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty() && !token.contains(char::is_whitespace)).then_some(token)
}

fn basic_credentials(h: Option<&str>) -> Option<(String, String)> {
    let (scheme, rest) = h?.trim().split_once(char::is_whitespace)?;
    if !scheme.eq_ignore_ascii_case("basic") {
        return None;
    }
    let decoded = String::from_utf8(base64_decode(rest.trim())?).ok()?;
    let (user, password) = decoded.split_once(':')?;
    Some((user.to_string(), password.to_string()))
}

impl<E: Endpoints> Handler<E> {
    /// A handler for `endpoints`.
    #[must_use]
    pub fn new(endpoints: E) -> Self {
        // #if rules
        if !endpoints.validate_requests() && endpoints.reaches_rules() {
            eprintln!("veld: request validation is off: the rules of request bodies, query objects and WebSocket messages are not enforced");
        }
        let output = endpoints.rules_output();
        if output != super::rules::RuleOutput::Strict {
            eprintln!("veld: output rules run in {output:?} mode");
        }
        // #endif
        let mut routes: Vec<Route> = endpoints.endpoints().iter().enumerate().map(|(i, ep)| compile(i, ep)).collect();
        // Literal routes before parameterised ones; the declared order breaks ties.
        routes.sort_by_key(|r| (r.params, r.index));
        Handler { inner: Arc::new(Inner { endpoints, routes }) }
    }

    /// The generated endpoints (for adapters that register routes).
    #[must_use]
    pub fn endpoints(&self) -> &E {
        &self.inner.endpoints
    }

    /// Serves a request; `None` when no route matches.
    pub async fn handle(&self, req: Request) -> Option<Response> {
        let method = req.method.to_ascii_uppercase();
        let (route, caps) = self
            .inner
            .routes
            .iter()
            .filter(|r| r.method == method)
            .find_map(|r| matches(r, &req.path).map(|c| (r, c)))?;
        Some(self.run(route.index, caps, req).await)
    }

    /// Runs the endpoint named `name` through the same pipeline without choosing it from the
    /// method; `None` when it is unknown or the path does not fit it. For frameworks that route
    /// themselves.
    pub async fn invoke(&self, name: &str, req: Request) -> Option<Response> {
        let route = self.inner.routes.iter().find(|r| self.inner.endpoints.endpoints()[r.index].name == name)?;
        let caps = matches(route, &req.path)?;
        Some(self.run(route.index, caps, req).await)
    }

    // @if ws_server
    /// Starts a WebSocket conversation on the endpoint whose `ws` path matches `req.path`; `None`
    /// when there is none (the adapter refuses the upgrade). The adapter pushes every text frame it
    /// receives into `input` and writes `frames` to the socket until they end.
    pub fn ws(&self, req: Request) -> Option<WsConversation> {
        let (route, caps) = self
            .inner
            .routes
            .iter()
            .filter(|r| r.method == "WS")
            .find_map(|r| matches(r, &req.path).map(|c| (r, c)))?;
        let (incoming, outgoing) = (Chan::new(), Chan::new());
        let io = WsIo { incoming: incoming.clone(), outgoing: outgoing.clone() };
        let (this, index) = (self.clone(), route.index);
        let conversation = Box::pin(async move { this.run_ws(index, caps, req, io).await });
        let frames = WsFrames { out: outgoing, conversation: Some(conversation) };
        Some(WsConversation { frames: Box::pin(frames), input: WsInput(incoming) })
    }

    /// The whole conversation: auth, middleware, binding, the implementation, the closing frames.
    async fn run_ws(&self, index: usize, caps: Vec<(String, String)>, req: Request, io: WsIo) {
        let e = &self.inner.endpoints;
        let ep = &e.endpoints()[index];
        let out = io.outgoing.clone();
        let mut ctx = self.context(ep, &req);
        Self::query_credentials(ep, &req, &mut ctx);
        let (status, body) = match self.guard(ep, &caps, &req, &mut ctx, true).await {
            Err(resp) => match resp.body {
                Body::Bytes(bytes) => (resp.status, bytes),
                Body::Empty | Body::Sse(_) => (resp.status, Vec::new()),
            },
            Ok(args) => match e.call_ws(index, args, &ctx, io).await {
                Ok(()) => {
                    out.push(WsFrame::Close(1000));
                    out.close();
                    return;
                }
                Err(err) => {
                    let (status, body) = self.problem_of(&err, ep, &ctx);
                    (status, body.to_text().into_bytes())
                }
            },
        };
        let text = format!("{{\"error\":{}}}", String::from_utf8_lossy(&body));
        out.push(WsFrame::Text(text));
        // 1011 for `Internal`, 1008 for declared errors, validation and auth.
        out.push(WsFrame::Close(if status >= 500 { 1011 } else { 1008 }));
        out.close();
    }

    /// Browsers cannot set headers on a WebSocket: a bearer token comes as `?access_token=` and an
    /// API key as `?api_key=` when the header is absent.
    fn query_credentials(ep: &EndpointSpec, req: &Request, ctx: &mut RequestContext) {
        let Some(auth) = &ep.auth else {
            return;
        };
        let query = parse_query(&req.query);
        let first = |k: &str| query.get(k).and_then(|v| v.first()).cloned();
        match auth.kind {
            AuthKind::Bearer if ctx.header("authorization").is_none() => {
                if let Some(t) = first("access_token") {
                    ctx.headers.push(("authorization".into(), format!("Bearer {t}")));
                }
            }
            AuthKind::ApiKey => {
                let name = auth.header.unwrap_or("X-API-Key").to_ascii_lowercase();
                if ctx.header(&name).is_none() {
                    if let Some(k) = first("api_key") {
                        ctx.headers.push((name, k));
                    }
                }
            }
            AuthKind::Bearer | AuthKind::Basic => {}
        }
    }
    // @end

    fn context(&self, ep: &EndpointSpec, req: &Request) -> RequestContext {
        RequestContext {
            method: ep.method.to_string(),
            path: path_without_trailing_slash(&req.path).to_string(),
            headers: req.headers.iter().map(|(k, v)| (k.to_ascii_lowercase(), v.clone())).collect(),
            principal: None,
        }
    }

    async fn run(&self, index: usize, caps: Vec<(String, String)>, req: Request) -> Response {
        let e = &self.inner.endpoints;
        let ep = &e.endpoints()[index];
        let mut ctx = self.context(ep, &req);
        let args = match self.guard(ep, &caps, &req, &mut ctx, false).await {
            Ok(args) => args,
            Err(resp) => return resp,
        };
        // @if sse
        if ep.kind == Kind::Sse {
            return match e.call_sse(index, args, &ctx).await {
                Ok(stream) => self.start_sse(index, ctx, stream).await,
                Err(err) => self.failure(&err, ep, &ctx),
            };
        }
        // @end
        match e.call(index, args, &ctx).await {
            Ok(value) => self.success(ep, &value, &ctx),
            Err(err) => self.failure(&err, ep, &ctx),
        }
    }

    /// Auth, middleware, binding and validation: the part of the pipeline every transport shares.
    async fn guard(
        &self,
        ep: &EndpointSpec,
        caps: &[(String, String)],
        req: &Request,
        ctx: &mut RequestContext,
        ws: bool,
    ) -> Result<Map<String, Value>, Response> {
        let e = &self.inner.endpoints;
        let std = e.std_sigs();
        if let Some(auth) = &ep.auth {
            ctx.principal = self.authenticate(auth, ctx).await;
            if ctx.principal.is_none() {
                return Err(error_response(std.unauthorized, "Unauthorized", 401, ep.sig, serde_json::json!({})));
            }
        }
        for name in ep.middleware {
            if let Err(err) = e.middleware(name, ctx).await {
                return Err(self.failure(&err, ep, ctx));
            }
        }
        let bound = self.bind(ep, caps, req, ctx, ws);
        // #if rules
        if bound.issues.is_empty() && !bound.violations.is_empty() {
            return Err(self.rejection(ep, std.validation, bound.violations, bound.micros));
        }
        // #endif
        if bound.issues.is_empty() {
            Ok(bound.value)
        } else {
            Err(error_response(std.validation, "ValidationError", 422, ep.sig, issues_json(&bound.issues)))
        }
    }

    async fn authenticate(&self, auth: &AuthSpec, ctx: &RequestContext) -> Option<Principal> {
        let e = &self.inner.endpoints;
        match auth.kind {
            AuthKind::Bearer => e.auth_bearer(bearer_token(ctx.header("authorization"))?, ctx).await,
            AuthKind::Basic => {
                let (user, password) = basic_credentials(ctx.header("authorization"))?;
                e.auth_basic(&user, &password, ctx).await
            }
            AuthKind::ApiKey => e.auth_api_key(ctx.header(auth.header.unwrap_or("X-API-Key"))?, ctx).await,
        }
    }

    fn bind(&self, ep: &EndpointSpec, caps: &[(String, String)], req: &Request, ctx: &RequestContext, ws: bool) -> Bound {
        // #if !rules
        let mut b = Bound { value: Map::new(), issues: Vec::new(), check: self.inner.endpoints.validate_requests() };
        // #endif
        // #if rules
        let mut b = Bound {
            value: Map::new(),
            issues: Vec::new(),
            check: self.inner.endpoints.validate_requests(),
            violations: Vec::new(),
            micros: 0,
        };
        // #endif
        let query = parse_query(&req.query);
        for p in ep.params.iter().filter(|p| !p.stream) {
            match p.binding {
                Binding::Path => {
                    let seg = caps.iter().find(|(n, _)| n == p.name).map(|(_, v)| v.replace('+', "%2B"));
                    let decoded = seg.and_then(|s| pct_decode(&s, false));
                    b.simple(p, decoded.map(|d| vec![d]).as_deref());
                }
                Binding::Query => b.simple(p, query.get(p.name).map(Vec::as_slice)),
                Binding::QueryObject => b.query_object(p, &query),
                Binding::Header => {
                    let name = p.header.unwrap_or(p.name);
                    // Browsers cannot set headers on a WebSocket: those endpoints also read the query string.
                    let found = ctx.header(name).map(str::to_string).or_else(|| if ws { query.get(name).and_then(|v| v.first().cloned()) } else { None });
                    b.simple(p, found.map(|h| vec![h]).as_deref());
                }
                Binding::Body => b.body(p, &req.body),
            }
        }
        b
    }

    fn success(&self, ep: &EndpointSpec, value: &Value, ctx: &RequestContext) -> Response {
        let Some(schema) = ep.success else {
            return Response { status: ep.status, headers: Vec::new(), body: Body::Empty };
        };
        let wire = schema.to_wire(value, false, false);
        // #if rules
        if let Some(resp) = self.output_rules(ep, schema, &wire, ctx) {
            return resp;
        }
        // #endif
        if self.inner.endpoints.validate_responses() {
            let issues = schema.validate(&wire, "$", false);
            if !issues.is_empty() {
                let err = Error::other(format!("{}: response failed validation: {issues:?}", ep.name));
                return self.failure(&err, ep, ctx);
            }
        }
        Response {
            status: ep.status,
            headers: vec![("content-type".into(), "application/json".into())],
            body: Body::Bytes(serde_json::to_vec(&wire).unwrap_or_default()),
        }
    }

    // #if rules
    /// Step 5 failed: `422 ValidationError` with one issue and one violation per broken rule.
    fn rejection(&self, ep: &EndpointSpec, sig: &str, violations: Vec<super::rules::Violation>, micros: u64) -> Response {
        for v in &violations {
            self.report(ep, v, super::rules::Position::Input, super::rules::Outcome::Rejected, micros);
        }
        let problem = Problem {
            sig: sig.to_string(),
            title: "ValidationError".to_string(),
            status: 422,
            op: Some(ep.sig.to_string()),
            data: super::rules::rejection_data(&violations),
            schema: None,
            rulings: super::rules::Rulings::boxed(violations, false),
        };
        Response {
            status: 422,
            headers: vec![("content-type".into(), "application/problem+json".into())],
            body: Body::Bytes(problem.to_text().into_bytes()),
        }
    }

    fn report(&self, ep: &EndpointSpec, v: &super::rules::Violation, position: super::rules::Position, outcome: super::rules::Outcome, micros: u64) {
        self.inner.endpoints.on_rule_violation(&super::rules::RuleEvent {
            rule: v.rule.clone(),
            operation: ep.sig.to_string(),
            position,
            outcome,
            violation: v.clone(),
            elapsed_micros: micros,
        });
    }

    /// Step 8: the invariants of the result, by output mode. `Some` replaces the response.
    fn output_rules(&self, ep: &EndpointSpec, schema: &'static Schema, wire: &Value, ctx: &RequestContext) -> Option<Response> {
        use super::rules::{Mode, Outcome, Position, RuleOutput};
        let mode = self.inner.endpoints.rules_output();
        if mode == RuleOutput::Off {
            return None;
        }
        let issues = schema.validate(wire, "$", false);
        if !issues.is_empty() {
            let err = Error::other(format!("{}: response failed validation: {issues:?}", ep.name));
            return Some(self.failure(&err, ep, ctx));
        }
        let watch = super::rules::Stopwatch::start();
        let violations = super::rules::check_wire(schema, wire, "$", Mode::Full);
        let micros = watch.micros();
        if violations.is_empty() {
            return None;
        }
        let outcome = if mode == RuleOutput::Strict { Outcome::Rejected } else { Outcome::Audited };
        for v in &violations {
            self.report(ep, v, Position::Output, outcome, micros);
        }
        if mode == RuleOutput::Strict {
            let err = Error::other(format!("{}: the result violates {} rule(s)", ep.name, violations.len()));
            return Some(self.failure(&err, ep, ctx));
        }
        None
    }

    // #endif
    /// A declared error is answered at its declared status; anything else becomes `Internal`
    /// without a message, and the original goes to `on_error`.
    fn failure(&self, err: &Error, ep: &EndpointSpec, ctx: &RequestContext) -> Response {
        let (status, body) = self.problem_of(err, ep, ctx);
        Response {
            status,
            headers: vec![("content-type".into(), "application/problem+json".into())],
            body: Body::Bytes(body.to_text().into_bytes()),
        }
    }

    fn problem_of(&self, err: &Error, ep: &EndpointSpec, ctx: &RequestContext) -> (u16, Problem) {
        let e = &self.inner.endpoints;
        let std = e.std_sigs();
        let problem = |sig: &str, title: &str, status: u16, data: Value| {
            let p = Problem {
                sig: sig.to_string(),
                title: title.to_string(),
                status,
                op: Some(ep.sig.to_string()),
                data,
                schema: None,
                // #if rules
                rulings: None,
                // #endif
            };
            (status, p)
        };
        match err {
            Error::Problem(p) => {
                if let Some(d) = ep.errors.iter().find(|d| d.sig == p.sig) {
                    return problem(d.sig, d.name, d.status, p.data.clone());
                }
            }
            Error::Invalid(issues) => return problem(std.validation, "ValidationError", 422, issues_json(issues)),
            Error::Http { .. } | Error::Transport(_) | Error::Other(_) => {}
        }
        e.on_error(err, ctx);
        problem(std.internal, "Internal", 500, serde_json::json!({}))
    }

    // @if sse
    fn frame(&self, ep: &EndpointSpec, ev: &SseEvent<Value>) -> Result<Vec<u8>, Error> {
        let schema = ep.success.ok_or_else(|| Error::other(format!("{}: a stream needs an item type", ep.name)))?;
        if self.inner.endpoints.validate_responses() {
            let issues = schema.validate(&schema.to_wire(&ev.data, false, false), "$", false);
            if !issues.is_empty() {
                return Err(Error::other(format!("{}: stream item failed validation: {issues:?}", ep.name)));
            }
        }
        let data = String::from_utf8_lossy(&schema.canonical_typed(&ev.data)).into_owned();
        sse_frame(&data, ev.id.as_deref(), ev.event.as_deref(), ev.retry).map(String::into_bytes)
    }

    /// The status line is long gone once streaming has started: an error is a final `error` event.
    fn error_frame(&self, err: &Error, ep: &EndpointSpec, ctx: &RequestContext) -> Vec<u8> {
        let (_, body) = self.problem_of(err, ep, ctx);
        let data = super::canonical::canonical(&body.to_json());
        sse_frame(&data, None, Some("error"), None).map(String::into_bytes).unwrap_or_default()
    }

    /// Pulls the first item before answering, so that an error raised before any event is a
    /// normal problem+json response.
    async fn start_sse(&self, index: usize, ctx: RequestContext, mut stream: EventStream<Value>) -> Response {
        let ep = &self.inner.endpoints.endpoints()[index];
        let first = match next(&mut stream).await {
            None => None,
            Some(Err(err)) => return self.failure(&err, ep, &ctx),
            Some(Ok(ev)) => match self.frame(ep, &ev) {
                Ok(frame) => Some(frame),
                Err(err) => return self.failure(&err, ep, &ctx),
            },
        };
        let state = SseState { handler: self.clone(), index, ctx, stream, first, done: false };
        let body = unfold(state, |mut st: SseState<E>| {
            Box::pin(async move {
                if let Some(frame) = st.first.take() {
                    return Some((frame, st));
                }
                if st.done {
                    return None;
                }
                let ep = &st.handler.inner.endpoints.endpoints()[st.index];
                let frame = match next(&mut st.stream).await? {
                    Ok(ev) => st.handler.frame(ep, &ev),
                    Err(err) => Err(err),
                };
                match frame {
                    Ok(frame) => Some((frame, st)),
                    Err(err) => {
                        st.done = true;
                        let frame = st.handler.error_frame(&err, ep, &st.ctx);
                        Some((frame, st))
                    }
                }
            })
        });
        Response {
            status: ep.status,
            headers: vec![
                ("content-type".into(), "text/event-stream".into()),
                ("cache-control".into(), "no-cache".into()),
                ("x-accel-buffering".into(), "no".into()),
            ],
            body: Body::Sse(Box::pin(body)),
        }
    }
    // @end
}

// @if sse
struct SseState<E: Endpoints> {
    handler: Handler<E>,
    index: usize,
    ctx: RequestContext,
    stream: EventStream<Value>,
    first: Option<Vec<u8>>,
    done: bool,
}
// @end

// @if ws_server
/// A frame the server sends on a WebSocket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsFrame {
    Text(String),
    /// Closes the socket with this code: 1000 normal end, 1008 declared errors, validation and
    /// auth, 1011 `Internal`.
    Close(u16),
}

/// The untyped sides of one conversation, handed to the endpoint.
#[derive(Debug, Clone)]
pub struct WsIo {
    incoming: Chan<String>,
    outgoing: Chan<WsFrame>,
}

/// Feeds a conversation with the text frames that arrive.
#[derive(Debug, Clone)]
pub struct WsInput(Chan<String>);

impl WsInput {
    /// A text frame from the client.
    pub fn push(&self, text: String) {
        self.0.push(text);
    }

    /// The client is gone: the endpoint's `recv` ends.
    pub fn end(&self) {
        self.0.close();
    }
}

/// What an adapter drives: write `frames` to the socket, push what arrives into `input`.
pub struct WsConversation {
    pub frames: BoxStream<WsFrame>,
    pub input: WsInput,
}

impl fmt::Debug for WsConversation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WsConversation")
    }
}

/// The frames of a conversation. Polling it also drives the conversation itself.
struct WsFrames {
    out: Chan<WsFrame>,
    conversation: Option<Pin<Box<dyn Future<Output = ()> + Send>>>,
}

impl Stream for WsFrames {
    type Item = WsFrame;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<WsFrame>> {
        loop {
            if let Poll::Ready(item) = self.out.poll_next(cx) {
                return Poll::Ready(item);
            }
            let Some(conversation) = self.conversation.as_mut() else {
                return Poll::Pending;
            };
            match conversation.as_mut().poll(cx) {
                Poll::Ready(()) => self.conversation = None,
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

/// One WebSocket conversation as an endpoint implementation sees it: `In` messages arrive, `Out`
/// messages are sent. `()` marks a side that has no messages.
pub struct WsSession<In, Out> {
    io: WsIo,
    incoming: Option<&'static Schema>,
    outgoing: Option<&'static Schema>,
    validate_in: bool,
    validate_out: bool,
    types: PhantomData<fn(Out) -> In>,
}

impl<In, Out> fmt::Debug for WsSession<In, Out> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WsSession")
    }
}

fn invalid(code: &str, message: &str) -> Error {
    Error::Invalid(vec![Issue { path: "message".into(), code: code.into(), message: message.into() }])
}

impl<In: DeserializeOwned, Out: Serialize> WsSession<In, Out> {
    /// A session over `io` for `ep` (generated code calls this).
    #[must_use]
    pub fn new(io: WsIo, ep: &'static EndpointSpec, validate_in: bool, validate_out: bool) -> Self {
        WsSession {
            io,
            incoming: ep.params.iter().find(|p| p.stream).map(|p| p.schema),
            outgoing: ep.success,
            validate_in,
            validate_out,
            types: PhantomData,
        }
    }

    /// The next message: validated and decoded. `None` when the socket closed. A message that
    /// does not fit is an `Err` (`Error::Invalid`: returning it ends the conversation with a
    /// `ValidationError` frame whose issue paths start with `message`).
    pub async fn recv(&mut self) -> Option<Result<In, Error>> {
        let text = self.io.incoming.recv().await?;
        let Some(schema) = self.incoming else {
            return Some(Err(invalid("unexpected", "this endpoint does not accept messages")));
        };
        let Ok(wire) = serde_json::from_str::<Value>(&text) else {
            return Some(Err(invalid("json", "message is not valid JSON")));
        };
        if self.validate_in {
            let issues = schema.validate(&wire, "message", true);
            if !issues.is_empty() {
                return Some(Err(Error::Invalid(issues)));
            }
        }
        Some(serde_json::from_value(schema.from_wire(&wire, true)).map_err(|e| invalid("type", &e.to_string())))
    }

    /// Sends a message to the client: `{"data": <canonical JSON>}`.
    ///
    /// # Errors
    /// [`Error::Other`] when the message cannot be encoded, or fails validation with
    /// `validate_responses` on.
    pub fn send(&self, message: Out) -> Result<(), Error> {
        let typed = serde_json::to_value(&message).map_err(Error::other)?;
        let data = match self.outgoing {
            Some(schema) => {
                if self.validate_out {
                    let issues = schema.validate(&schema.to_wire(&typed, false, false), "$", false);
                    if !issues.is_empty() {
                        return Err(Error::other(format!("message failed validation: {issues:?}")));
                    }
                }
                String::from_utf8_lossy(&schema.canonical_typed(&typed)).into_owned()
            }
            None => "null".to_string(),
        };
        self.io.outgoing.push(WsFrame::Text(format!("{{\"data\":{data}}}")));
        Ok(())
    }

    /// Ends the conversation normally (close code 1000).
    pub fn close(&self) {
        self.io.outgoing.push(WsFrame::Close(1000));
        self.io.outgoing.close();
    }
}
// @end
