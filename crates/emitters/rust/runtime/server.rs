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

// @if conditional
/// What an endpoint owes to `@version` and `@idempotent` (`spec/rules.md` sections 11.7 to 11.12).
/// The generated dispatch says it for each endpoint; the core does the rest.
#[derive(Debug, Clone, Copy)]
pub struct Conditional {
    /// The endpoint has a version guard: `If-Match` is read, its syntax checked and the handler
    /// gets the version that was compared.
    pub guard: bool,
    /// The wire name of the version field of the success model: its value is the `ETag`.
    pub etag: Option<&'static str>,
    // @if idempotency
    /// `@idempotent`.
    pub idempotency: Option<Idempotent>,
    // @end
}

// @if idempotency
/// `@idempotent` on an endpoint.
#[derive(Debug, Clone, Copy)]
pub struct Idempotent {
    /// The header of the key.
    pub header: &'static str,
    /// A request without the header is `422`.
    pub required: bool,
    /// The scope of a key includes the caller.
    pub caller: bool,
}

// @end
impl Conditional {
    /// An endpoint with neither.
    pub const NONE: Conditional = Conditional {
        guard: false,
        etag: None,
        // @if idempotency
        idempotency: None,
        // @end
    };
}

// @end
/// What a service method and a hook can see of the request.
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub method: String,
    pub path: String,
    /// Header names are lower case.
    pub headers: Vec<(String, String)>,
    /// Set by the auth hook of an endpoint that needs auth.
    pub principal: Option<Principal>,
    // @if conditional
    /// What the endpoint says about `If-Match`, `ETag` and the idempotency key.
    conditional: Conditional,
    // @end
    // #if endpoint_rules
    /// What step 6 saved for step 9.
    prepared: Prepared,
    // #endif
    // #if rollout
    /// The audited rules this request reported (`@rollout(audit)`).
    audit: AuditLog,
    // #endif
    // @if timeouts
    /// The instant at which the request has no more time (arrival plus the endpoint's `@timeout`),
    /// or `None` for an endpoint without a deadline. A handler that writes should watch it:
    /// the future of the handler is dropped at the deadline, which cancels it at its next `.await`,
    /// and what it already wrote stays written.
    pub deadline: Option<std::time::SystemTime>,
    // @end
    // #if flags
    /// What this request read of each flag it can reach, once, after authentication.
    #[doc(hidden)]
    pub flags: super::rules::FlagView,
    // #endif
    // @if timed
    /// Set for a call of the due-moves runner (`spec/platform.md` section 10.1): the caller is the
    /// timer's, no authentication and no middleware run, and `now` is the run's.
    timer: Option<TimerCall>,
    // @end
    // @if derived
    /// The derived fields of each bound parameter that have no value (patterns, step 5): what
    /// the statements of an endpoint block that read one must skip (`spec/rules.md` 17.2).
    #[doc(hidden)]
    pub absent: BTreeMap<String, Vec<String>>,
    // @end
    // @if outbox
    /// The events this request wrote to the outbox, by name (`spec/events.md` section 11.1): the
    /// server checks them after the handler of a move that emits.
    #[doc(hidden)]
    pub emitted: super::events::EmitLog,
    // @end
    // @if platform
    /// The clock reading of the request, taken once when the request is matched
    /// (`spec/platform.md` section 9.2): the `now` of the rules and of `@visible`.
    now: std::time::SystemTime,
    /// What the platform features keep of the request; the clones of the context share it.
    #[doc(hidden)]
    pub platform: Arc<super::platform::State>,
    // @end
    // @if tenant_types
    /// The tenant of the caller (`spec/platform.md` section 3); `None` when the caller has none.
    tenant: Option<super::platform::Tenant>,
    // @end
}
// #if rollout

/// The audited violations of one request: what step 6 found and has not reported yet, and what
/// has been reported (the test handler answers with it in `Veld-Test-Audited`).
#[derive(Debug, Clone, Default)]
pub struct AuditLog {
    // #endif
    // #if rollout_endpoint
    pending: Arc<std::sync::Mutex<Vec<super::rules::Violation>>>,
    // #endif
    // #if rollout
    sent: Arc<std::sync::Mutex<Vec<super::rules::Violation>>>,
}

impl AuditLog {
    // #endif
    // #if rollout_endpoint
    fn take_pending(&self) -> Vec<super::rules::Violation> {
        self.pending.lock().map(|mut p| std::mem::take(&mut *p)).unwrap_or_default()
    }

    // #endif
    // #if rollout
    fn record(&self, found: &[super::rules::Violation]) {
        if let Ok(mut sent) = self.sent.lock() {
            sent.extend(found.iter().cloned());
        }
    }

    /// Everything reported so far, in order.
    fn sent(&self) -> Vec<super::rules::Violation> {
        self.sent.lock().map(|s| s.clone()).unwrap_or_default()
    }
}
// #endif
// #if rollout_endpoint

impl RequestContext {
    /// An audited `require` that does not hold (`@rollout(audit)`): the server reports it to the
    /// hook with the outcome `audited` and the request goes on. For generated code.
    #[doc(hidden)]
    pub fn audited(&self, violation: super::rules::Violation) {
        if let Ok(mut pending) = self.audit.pending.lock() {
            pending.push(violation);
        }
    }
}
// #endif
// #if endpoint_rules

/// What step 6 of an endpoint saved for step 9: the typed parameters, the facts and the clock
/// reading that its `ensure` statements read. Opaque to everyone but the generated code.
#[derive(Clone)]
pub struct Prepared {
    index: usize,
    state: Option<Arc<dyn Any + Send + Sync>>,
}

impl Prepared {
    /// Nothing to remember.
    #[must_use]
    pub fn none() -> Self {
        Prepared { index: 0, state: None }
    }

    /// The state of the endpoint `index`.
    #[must_use]
    pub fn new<T: Any + Send + Sync>(index: usize, state: T) -> Self {
        Prepared { index, state: Some(Arc::new(state)) }
    }

    /// The endpoint the state belongs to.
    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }

    /// The saved state, when it is a `T`.
    #[must_use]
    pub fn state<T: Any>(&self) -> Option<&T> {
        self.state.as_deref().and_then(|s| s.downcast_ref::<T>())
    }
}

impl fmt::Debug for Prepared {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Prepared({})", self.index)
    }
}

/// Why step 6 stopped the request.
#[derive(Debug)]
pub enum Halt {
    /// Answer with this error: a missing fact, a failed `require`, or whatever a loader returned.
    /// The violations it carries go to the hook.
    Error(Error),
    /// The server could not build the error a `require ... else` promised: `500 Internal`, and
    /// these violations go to the hook.
    Broken(Vec<super::rules::Violation>),
    /// Answer with this response and stop (the test handler's dry run).
    Early(Response),
}
// #endif

// @if timeouts
impl RequestContext {
    /// Whether the deadline of the request has passed (always `false` for an endpoint without one).
    #[must_use]
    pub fn expired(&self) -> bool {
        self.deadline.is_some_and(|d| std::time::SystemTime::now() >= d)
    }
}
// @end

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
// @if platform

impl RequestContext {
    // @if tenant_types
    /// The tenant of the caller, when the principal has one. For generated code.
    #[doc(hidden)]
    #[must_use]
    pub fn tenant(&self) -> Option<&super::platform::Tenant> {
        self.tenant.as_ref()
    }

    // @end
    /// The clock reading of the request in milliseconds since the epoch. For generated code.
    #[doc(hidden)]
    #[must_use]
    pub fn now_ms(&self) -> i64 {
        super::rules::clock_ms(self.now)
    }
}
// @end
// @if etag

impl RequestContext {
    /// What `If-Match` says, for an endpoint with a version guard (step 5 has refused a header that
    /// is malformed). `None` without the header and for an endpoint without a guard. For generated
    /// code.
    #[doc(hidden)]
    #[must_use]
    pub fn if_match(&self) -> Option<super::conditional::IfMatch<i64>> {
        if !self.conditional.guard {
            return None;
        }
        super::conditional::parse_if_match(self.header("if-match")?)
    }
}
// @end

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

    /// The rules digest of the service, sent as `Veld-Rules` on every response (`spec/rules.md`
    /// section 9.7).
    fn digest(&self) -> Option<&'static str> {
        None
    }
    // #endif
    // #if endpoint_rules

    /// Step 6: the facts, the policies and the `require` statements of `endpoint`, and what step 9
    /// needs. It adds the loaded facts to `args`. Called once per request, after step 5.
    fn prepare<'a>(
        &'a self,
        _endpoint: usize,
        _args: &'a mut Map<String, Value>,
        _ctx: &'a RequestContext,
    ) -> impl Future<Output = Result<Prepared, Halt>> + Send + 'a {
        async { Ok(Prepared::none()) }
    }

    /// Step 9: the violated `ensure` statements for the success value `result` (typed JSON).
    fn ensure(&self, _prepared: &Prepared, _result: &Value) -> Vec<super::rules::Violation> {
        Vec::new()
    }

    /// A principal that replaces authentication and middleware altogether. Only a test handler
    /// has one.
    fn trusted_principal(&self, _ctx: &RequestContext) -> Option<Principal> {
        None
    }
    // #endif
    // #if flags

    /// Reads, once, every flag the endpoint can reach (`spec/platform.md` section 11). A failing
    /// provider is a `Failed` reading and goes to `on_error`.
    fn read_flags<'a>(&'a self, _endpoint: usize, _ctx: &'a RequestContext) -> impl Future<Output = super::rules::FlagView> + Send + 'a {
        async { super::rules::FlagView::new() }
    }
    // #endif
    // @if slo

    /// Receives the metric of every request that matched an endpoint (`spec/platform.md` section 13).
    fn metrics(&self, _metric: &Metric) {}
    // @end
    // @if timed

    /// The principal of the due-moves runner (`hooks.timer_principal()`): `None` when the hooks
    /// give none.
    fn timer_principal<'a>(&'a self) -> impl Future<Output = Option<Principal>> + Send + 'a {
        async { None }
    }

    /// The clock of the hooks, for a run that is given no `now`.
    fn timer_clock(&self, _ctx: &RequestContext) -> std::time::SystemTime {
        std::time::SystemTime::now()
    }
    // @end
    // @if derived

    /// Gives the principal its derived fields, once per request after authentication and
    /// middleware, for every endpoint of a service whose principal model has some
    /// (`spec/rules.md` section 17.2): the completed value replaces the one the hook returned, so
    /// the rules and the handler see it. The violations of a field that could not be computed are
    /// `Err`.
    fn complete_principal(&self, _ctx: &mut RequestContext) -> Result<(), Vec<super::rules::Violation>> {
        Ok(())
    }
    // @end
    // @if outbox

    /// Whether the answers carry the events the request wrote in `Veld-Test-Emitted`, and the
    /// events go to an outbox of the request's own (`spec/events.md` section 11.2). Only a test
    /// handler says so.
    fn test_emitted(&self) -> bool {
        false
    }
    // @end
    // #if rollout

    /// Whether the answers carry the audited reports of the request in `Veld-Test-Audited`
    /// (`spec/rules.md` section 12.4). Only a test handler says so.
    fn test_audited(&self) -> bool {
        false
    }
    // #endif
    // #if rollout_endpoint

    /// Step 9 for the audited `ensure` statements: the violated ones, which never reject.
    fn ensure_audited(&self, _prepared: &Prepared, _result: &Value) -> Vec<super::rules::Violation> {
        Vec::new()
    }
    // #endif
    // @if tenant_types

    /// The tenant of the caller, read from the principal once authentication has answered
    /// (`spec/platform.md` section 3). The default is none.
    fn caller_tenant(&self, _ctx: &RequestContext) -> Option<super::platform::Tenant> {
        None
    }
    // @end
    // @if platform

    /// The clock of the request (`now` of the rules and of `@visible`, `time` of an audit event):
    /// read once, when the request is matched.
    fn request_clock(&self, _ctx: &RequestContext) -> std::time::SystemTime {
        std::time::SystemTime::now()
    }
    // @end

    /// Observes every error that becomes `Internal`. It cannot change the response.
    fn on_error(&self, _err: &Error, _ctx: &RequestContext) {}
    // @if conditional

    /// What `endpoint` owes to `@version` and `@idempotent`.
    fn conditional(&self, _endpoint: usize) -> Conditional {
        Conditional::NONE
    }
    // @if idempotency

    /// The caller text of the scope of an idempotency key: what the application's hook makes of
    /// the principal.
    fn idem_caller(&self, _ctx: &RequestContext) -> String {
        String::new()
    }

    /// Asks the store about a key (`endpoint` is the index of the endpoint, for a test handler).
    fn idem_begin<'a>(
        &'a self,
        _endpoint: usize,
        _scope: &'a str,
        _key: &'a str,
        _fingerprint: &'a str,
        _ctx: &'a RequestContext,
    ) -> impl Future<Output = Result<super::idempotency::Begin, Error>> + Send + 'a {
        async { Err(Error::other("the service has no idempotency store")) }
    }

    /// Stores the answer of a started request.
    fn idem_complete<'a>(
        &'a self,
        _scope: &'a str,
        _key: &'a str,
        _answer: super::idempotency::StoredAnswer,
    ) -> impl Future<Output = Result<(), Error>> + Send + 'a {
        async { Ok(()) }
    }

    /// Removes the entry of a started request.
    fn idem_release<'a>(&'a self, _scope: &'a str, _key: &'a str) -> impl Future<Output = Result<(), Error>> + Send + 'a {
        async { Ok(()) }
    }
    // @end
    // @end

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
    /// Calls a server-sent-events endpoint; the stream's items are typed JSON. A service without
    /// such an endpoint keeps this default, so a crate with SSE compiles for every service.
    fn call_sse<'a>(
        &'a self,
        _endpoint: usize,
        _args: Map<String, Value>,
        _ctx: &'a RequestContext,
    ) -> impl Future<Output = Result<EventStream<Value>, Error>> + Send + 'a {
        async { Err(Error::other("not a server-sent-events endpoint")) }
    }
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
// #if guards

/// Decodes one member of the bound arguments: the `transition` that step 6 added for the handler.
///
/// # Errors
/// [`Error::Invalid`] when it is missing or does not fit.
pub fn decode_key<T: DeserializeOwned>(args: &Map<String, Value>, key: &str) -> Result<T, Error> {
    let value = args.get(key).cloned().unwrap_or(Value::Null);
    serde_json::from_value(value).map_err(|e| {
        Error::Invalid(vec![Issue { path: "request".into(), code: "type".into(), message: format!("{key}: {e}") }])
    })
}
// #endif

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
// #if rules

impl<E> fmt::Debug for Inner<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Inner({} routes)", self.routes.len())
    }
}

/// What a conversation needs of the server to apply the rules to its messages. A WebSocket session
/// outlives the call that made it, so it holds the server's core behind this trait.
trait RuleSink: Send + Sync + fmt::Debug {
    /// An output (a result, a declared error, a stream item or message) under the output mode:
    /// `Err` says why it must not be sent. The violations go to the hook either way.
    fn check_output(&self, ep: &EndpointSpec, schema: &Schema, wire: &Value, what: &str) -> Result<(), String>;

    /// The error that rejects an input whose rules are violated: a `ValidationError` with the
    /// violations. The hook gets them.
    fn reject_input(&self, ep: &EndpointSpec, violations: Vec<super::rules::Violation>, micros: u64) -> Problem;
    // @if derived

    /// An output value (typed form) with its derived fields computed, before its rules and before
    /// it is written: `Err` says why it must not be sent (`strict`, and a derived field could not
    /// be computed). The violations go to the hook either way; `off` computes and reports nothing.
    fn derive_output(&self, ep: &EndpointSpec, schema: &Schema, typed: &Value, what: &str) -> Result<Value, String>;
    // @end
    // #endif
    // #if rollout

    /// The audited rules (`@rollout(audit)`) of an output value, under the output mode: reported to
    /// the hook, never rejecting. `root` is the name the value has in a path (`result`, `error`).
    fn audit_output(&self, ep: &EndpointSpec, schema: &Schema, wire: &Value, root: &str, log: Option<&AuditLog>);

    // @if ws_server
    /// The audited rules of a message a WebSocket client sent: reported, never rejecting.
    fn audit_message(&self, ep: &EndpointSpec, schema: &Schema, wire: &Value);
    // @end
    // #endif
    // #if rules
}
// #endif
// #if rollout

impl<E: Endpoints> Inner<E> {
    /// Reports audited violations to the hook with the outcome `audited`; `root` rewrites the `$`
    /// of an output path.
    fn report_audited(
        &self,
        ep: &EndpointSpec,
        found: Vec<super::rules::Violation>,
        position: super::rules::Position,
        micros: u64,
        (root, log): (Option<&str>, Option<&AuditLog>),
    ) {
        let found: Vec<super::rules::Violation> = found
            .into_iter()
            .map(|mut v| {
                if let Some(root) = root {
                    v.path = format!("{root}{}", v.path.strip_prefix('$').unwrap_or(&v.path));
                }
                v
            })
            .collect();
        if let Some(log) = log {
            log.record(&found);
        }
        for v in &found {
            self.report(ep, v, position, super::rules::Outcome::Audited, micros);
        }
    }
}
// #endif
// #if rules

impl<E: Endpoints> Inner<E> {
    fn report(&self, ep: &EndpointSpec, v: &super::rules::Violation, position: super::rules::Position, outcome: super::rules::Outcome, micros: u64) {
        self.endpoints.on_rule_violation(&super::rules::RuleEvent {
            rule: v.rule.clone(),
            operation: ep.sig.to_string(),
            position,
            outcome,
            violation: v.clone(),
            elapsed_micros: micros,
        });
    }
}

impl<E: Endpoints> RuleSink for Inner<E> {
    fn check_output(&self, ep: &EndpointSpec, schema: &Schema, wire: &Value, what: &str) -> Result<(), String> {
        use super::rules::{Mode, Outcome, Position, RuleOutput};
        let mode = self.endpoints.rules_output();
        if mode == RuleOutput::Off {
            return Ok(());
        }
        let issues = schema.validate(wire, "$", false);
        if !issues.is_empty() {
            // In `audit` a value of the wrong shape is passed on without its rules.
            return if mode == RuleOutput::Strict { Err(format!("{}: {what} failed validation: {issues:?}", ep.name)) } else { Ok(()) };
        }
        let watch = super::rules::Stopwatch::start();
        // @if !derived
        let violations = super::rules::check_wire(schema, wire, "$", Mode::Full);
        // @end
        // @if derived
        let violations = super::rules::check_computed(schema, wire, "$", Mode::Full);
        // @end
        let micros = watch.micros();
        if violations.is_empty() {
            return Ok(());
        }
        let outcome = if mode == RuleOutput::Strict { Outcome::Rejected } else { Outcome::Audited };
        for v in &violations {
            self.report(ep, v, Position::Output, outcome, micros);
        }
        if mode == RuleOutput::Strict {
            return Err(format!("{}: the {what} violates {} rule(s)", ep.name, violations.len()));
        }
        Ok(())
    }

    // @if derived
    fn derive_output(&self, ep: &EndpointSpec, schema: &Schema, typed: &Value, what: &str) -> Result<Value, String> {
        use super::rules::{Mode, Outcome, Position, RuleOutput};
        let mut value = typed.clone();
        let mut found = Vec::new();
        let watch = super::rules::Stopwatch::start();
        super::rules::derive_walk(schema, &mut value, "$", Mode::Full, &mut found);
        let mode = self.endpoints.rules_output();
        if found.is_empty() || mode == RuleOutput::Off {
            return Ok(value);
        }
        let outcome = if mode == RuleOutput::Strict { Outcome::Rejected } else { Outcome::Audited };
        for v in &found {
            self.report(ep, v, Position::Output, outcome, watch.micros());
        }
        if mode == RuleOutput::Strict {
            return Err(format!("{}: {} derived field(s) of the {what} could not be computed", ep.name, found.len()));
        }
        Ok(value)
    }
    // @end

    fn reject_input(&self, ep: &EndpointSpec, violations: Vec<super::rules::Violation>, micros: u64) -> Problem {
        for v in &violations {
            self.report(ep, v, super::rules::Position::Input, super::rules::Outcome::Rejected, micros);
        }
        Problem {
            sig: self.endpoints.std_sigs().validation.to_string(),
            title: "ValidationError".to_string(),
            status: 422,
            op: Some(ep.sig.to_string()),
            data: super::rules::rejection_data(&violations),
            schema: None,
            rulings: super::rules::Rulings::boxed(violations, false),
        }
    }
    // #endif
    // #if rollout

    fn audit_output(&self, ep: &EndpointSpec, schema: &Schema, wire: &Value, root: &str, log: Option<&AuditLog>) {
        if self.endpoints.rules_output() == super::rules::RuleOutput::Off {
            return;
        }
        if !schema.validate(wire, "$", false).is_empty() {
            return;
        }
        let watch = super::rules::Stopwatch::start();
        let found = super::rules::audit_wire(schema, wire, "$", super::rules::Mode::Full);
        self.report_audited(ep, found, super::rules::Position::Output, watch.micros(), (Some(root), log));
    }

    // @if ws_server
    fn audit_message(&self, ep: &EndpointSpec, schema: &Schema, wire: &Value) {
        let watch = super::rules::Stopwatch::start();
        let found = super::rules::audit_wire(schema, wire, "message", super::rules::Mode::Request);
        self.report_audited(ep, found, super::rules::Position::Input, watch.micros(), (None, None));
    }
    // @end
    // #endif
    // #if rules
}
// #endif

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

// #if rollout
/// `Veld-Test-Audited` (`spec/rules.md` section 12.4): the audited reports of the request, the first
/// 100, as a compact JSON array in unpadded base64url. Absent when there is none.
fn stamp_audited(mut resp: Response, log: &AuditLog) -> Response {
    let sent = log.sent();
    if sent.is_empty() {
        return resp;
    }
    let items: Vec<String> = sent.iter().take(100).map(super::rules::Violation::to_text).collect();
    let text = format!("[{}]", items.join(","));
    let encoded = super::util::base64_encode(text.as_bytes()).replace('+', "-").replace('/', "_");
    resp.headers.push(("veld-test-audited".into(), encoded.trim_end_matches('=').to_string()));
    resp
}

// #endif
// @if idempotency
/// A key that this request holds.
struct Started {
    scope: String,
    key: String,
}

/// What a request that was dropped hands to the store.
enum Settle {
    /// Nothing was answered yet: the key goes back.
    Release,
    /// The answer exists and is below `500`: it is stored (a drop must not run the request twice).
    Complete(super::idempotency::StoredAnswer),
}

/// The key a request holds while it runs. Dropping it while the key is still held (the request
/// future was dropped or cancelled, the handler panicked) hands `release`, or `complete` once the
/// answer exists, to a task of its own: an entry must not stay "running" because the server gave
/// up (`spec/rules.md` section 11.12).
struct Hold<E: Endpoints> {
    inner: Arc<Inner<E>>,
    started: Option<Started>,
    settle: Settle,
}

impl<E: Endpoints> Drop for Hold<E> {
    fn drop(&mut self) {
        let Some(Started { scope, key }) = self.started.take() else {
            return;
        };
        let inner = Arc::clone(&self.inner);
        let settle = std::mem::replace(&mut self.settle, Settle::Release);
        spawn_release(async move {
            // There is no request left to report a failing store to.
            let _ = match settle {
                Settle::Release => inner.endpoints.idem_release(&scope, &key).await,
                Settle::Complete(answer) => inner.endpoints.idem_complete(&scope, &key, answer).await,
            };
        });
    }
}

// @if axum
/// Runs `fut` on the tokio runtime this thread belongs to; with none, on a thread of its own.
fn spawn_release<F: Future<Output = ()> + Send + 'static>(fut: F) {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.spawn(fut);
        }
        Err(_) => spawn_thread(fut),
    }
}
// @end
// @if !axum
fn spawn_release<F: Future<Output = ()> + Send + 'static>(fut: F) {
    spawn_thread(fut);
}
// @end

/// Drives `fut` on a thread of its own (the executor of `veld::block_on`).
fn spawn_thread<F: Future<Output = ()> + Send + 'static>(fut: F) {
    let _ = std::thread::Builder::new().name("veld-idempotency-release".into()).spawn(move || super::util::block_on(fut));
}

/// What a response keeps in the store: the status, the bytes and the headers that matter.
fn stored_of(resp: &Response) -> super::idempotency::StoredAnswer {
    let find = |name: &str| resp.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone());
    super::idempotency::StoredAnswer {
        status: resp.status,
        body: if let Body::Bytes(bytes) = &resp.body { bytes.clone() } else { Vec::new() },
        content_type: find("content-type"),
        etag: find("etag"),
        location: find("location"),
    }
}

/// A stored answer sent again, with `Veld-Idempotent-Replay: true`.
fn replay(answer: super::idempotency::StoredAnswer) -> Response {
    let mut headers = Vec::new();
    for (name, value) in [("content-type", answer.content_type), ("etag", answer.etag), ("location", answer.location)] {
        if let Some(value) = value {
            headers.push((name.to_string(), value));
        }
    }
    headers.push(("veld-idempotent-replay".to_string(), "true".to_string()));
    let body = if answer.body.is_empty() { Body::Empty } else { Body::Bytes(answer.body) };
    Response { status: answer.status, headers, body }
}

// @end
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

// @if slo
/// The value the metrics hook receives for a request that matched an endpoint, once its answer is
/// ready (`spec/platform.md` section 13): the duration in seconds with the attributes of the
/// OpenTelemetry HTTP semantic conventions.
#[derive(Debug, Clone, PartialEq)]
pub struct Metric {
    /// `http.server.request.duration`.
    pub name: &'static str,
    /// `s`.
    pub unit: &'static str,
    pub value: f64,
    /// `http.request.method`.
    pub http_request_method: String,
    /// `http.route`: the full path template.
    pub http_route: String,
    /// `http.response.status_code`.
    pub http_response_status_code: u16,
    /// `error.type`: the status as text, only for a `5xx`.
    pub error_type: Option<String>,
    /// `veld.service`: `<package>/<Service>`.
    pub veld_service: String,
    /// `veld.endpoint`.
    pub veld_endpoint: String,
    /// `veld.operation`: the whole operation signature of the endpoint.
    pub veld_operation: String,
}

impl Metric {
    /// The value of `spec/platform.md` section 13 as JSON, with the attribute names of the
    /// conventions.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut attributes = Map::new();
        attributes.insert("http.request.method".into(), Value::from(self.http_request_method.clone()));
        attributes.insert("http.route".into(), Value::from(self.http_route.clone()));
        attributes.insert("http.response.status_code".into(), Value::from(self.http_response_status_code));
        if let Some(t) = &self.error_type {
            attributes.insert("error.type".into(), Value::from(t.clone()));
        }
        attributes.insert("veld.service".into(), Value::from(self.veld_service.clone()));
        attributes.insert("veld.endpoint".into(), Value::from(self.veld_endpoint.clone()));
        attributes.insert("veld.operation".into(), Value::from(self.veld_operation.clone()));
        serde_json::json!({ "name": self.name, "unit": self.unit, "value": self.value, "attributes": attributes })
    }
}

impl<E: Endpoints> Handler<E> {
    /// `run`, and then the metrics hook with the answer that is ready to be written. A hook that
    /// panics is ignored.
    async fn measured(&self, index: usize, caps: Vec<(String, String)>, req: Request) -> Response {
        let started = std::time::Instant::now();
        let method = req.method.to_ascii_uppercase();
        let resp = self.run(index, caps, req).await;
        let ep = &self.inner.endpoints.endpoints()[index];
        let metric = Metric {
            name: "http.server.request.duration",
            unit: "s",
            value: started.elapsed().as_secs_f64(),
            http_request_method: method,
            http_route: ep.path.to_string(),
            http_response_status_code: resp.status,
            error_type: (resp.status >= 500).then(|| resp.status.to_string()),
            veld_service: ep.service.to_string(),
            veld_endpoint: ep.name.to_string(),
            veld_operation: ep.sig.to_string(),
        };
        let endpoints = &self.inner.endpoints;
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| endpoints.metrics(&metric)));
        resp
    }
}
// @end
// @if timeouts
/// Runs `fut` until `at`: `None` when it has no answer by then (the future is dropped).
async fn within<T>(at: Option<tokio::time::Instant>, fut: impl Future<Output = T>) -> Option<T> {
    match at {
        Some(at) => tokio::time::timeout_at(at, fut).await.ok(),
        None => Some(fut.await),
    }
}
// @end
// @if timed
/// The caller of a due move: the principal the application gives its timer and the clock reading
/// of the run (`spec/platform.md` section 10.1).
#[derive(Debug, Clone)]
pub struct TimerCall {
    pub principal: Option<Principal>,
    pub now: std::time::SystemTime,
}

impl RequestContext {
    /// The clock reading of the run that makes this call, when the request is a due move.
    #[doc(hidden)]
    #[must_use]
    pub fn timer_now(&self) -> Option<std::time::SystemTime> {
        self.timer.as_ref().map(|t| t.now)
    }
}

/// What one call of the due-moves runner came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DueOutcome {
    /// The pipeline answered with a success.
    Moved,
    /// `409 InvalidTransition`: another request moved the value first, which is expected.
    Refused,
    /// Any other answer.
    Failed,
}

/// The counts of one endpoint in a [`DueReport`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DueCounts {
    /// Calls the loader gave and the runner made.
    pub ran: usize,
    pub moved: usize,
    /// Calls refused with `InvalidTransition`.
    pub refused: usize,
    /// Calls that failed, and loaders that failed.
    pub failed: usize,
}

/// The report of a run: one entry per endpoint with a timed transition, by endpoint name.
pub type DueReport = BTreeMap<String, DueCounts>;

fn due_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Array(_) | Value::Object(_) => super::canonical::canonical(v),
        other => other.to_string(),
    }
}

fn due_push(query: &mut Vec<(String, String)>, key: &str, wire: &Value) {
    match wire {
        Value::Null => {}
        Value::Array(items) => query.extend(items.iter().map(|i| (key.to_string(), due_text(i)))),
        other => query.push((key.to_string(), due_text(other))),
    }
}

/// The wire request of a call of `ep` whose parameters are `args` (the typed JSON of its request
/// struct, by name): what the due-moves runner runs through the pipeline in process.
#[doc(hidden)]
#[must_use]
pub fn due_request(ep: &EndpointSpec, args: &Map<String, Value>) -> Request {
    let mut path = ep.path.to_string();
    let mut query: Vec<(String, String)> = Vec::new();
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut body = Vec::new();
    for p in ep.params.iter().filter(|p| !p.stream) {
        let Some(typed) = args.get(p.name).filter(|v| !v.is_null()) else {
            continue;
        };
        let wire = p.schema.to_wire(typed, true, false);
        match p.binding {
            Binding::Path => path = path.replace(&format!("{{{}}}", p.name), &super::util::pct_encode(&due_text(&wire))),
            Binding::Query => due_push(&mut query, p.name, &wire),
            Binding::QueryObject => {
                for (k, v) in wire.as_object().into_iter().flatten() {
                    due_push(&mut query, k, v);
                }
            }
            Binding::Header => headers.push((p.header.unwrap_or(p.name).to_string(), due_text(&wire))),
            Binding::Body => {
                body = super::canonical::canonical(&wire).into_bytes();
                headers.push(("content-type".to_string(), "application/json".to_string()));
            }
        }
    }
    query.sort_by(|a, b| a.0.cmp(&b.0));
    let pairs: Vec<String> = query
        .iter()
        .map(|(k, v)| format!("{}={}", super::util::pct_encode(k), super::util::pct_encode(v)))
        .collect();
    Request { method: ep.method.to_string(), path, query: pairs.join("&"), headers, body }
}

impl<E: Endpoints> Handler<E> {
    /// A context for the loaders of the due-moves runner: the endpoint's own method and path, no
    /// principal and no headers.
    #[doc(hidden)]
    #[must_use]
    pub fn timer_context(&self, name: &str) -> Option<RequestContext> {
        let ep = self.inner.endpoints.endpoints().iter().find(|ep| ep.name == name)?;
        Some(self.context(ep, &Request { method: ep.method.to_string(), path: ep.path.to_string(), ..Request::default() }))
    }

    /// Runs one call of the endpoint `name` with the parameters `args` through steps 4 to 10 in
    /// process, as the caller `call` names (no step 2, no middleware: there is no request). A
    /// failure goes to `on_error`.
    #[doc(hidden)]
    pub async fn due_move(&self, name: &str, args: &Map<String, Value>, call: TimerCall) -> DueOutcome {
        let e = &self.inner.endpoints;
        let Some(route) = self.inner.routes.iter().find(|r| e.endpoints()[r.index].name == name) else {
            return DueOutcome::Failed;
        };
        let ep = &e.endpoints()[route.index];
        let req = due_request(ep, args);
        let Some(caps) = matches(route, &req.path) else {
            return DueOutcome::Failed;
        };
        let mut ctx = self.context(ep, &req);
        ctx.principal = call.principal.clone();
        ctx.timer = Some(call);
        let resp = self.run_ctx(route.index, caps, req, ctx.clone()).await;
        if resp.status < 300 {
            return DueOutcome::Moved;
        }
        let refused = resp.status == 409
            && if let Body::Bytes(bytes) = &resp.body {
                serde_json::from_slice::<Value>(bytes)
                    .ok()
                    .and_then(|doc| doc.get("type").and_then(Value::as_str).map(str::to_string))
                    .is_some_and(|ty| ep.errors.iter().any(|er| er.name == "InvalidTransition" && er.sig == ty))
            } else {
                false
            };
        if refused {
            return DueOutcome::Refused;
        }
        if resp.status < 500 {
            e.on_error(&Error::other(format!("{name}: a due move was answered {}", resp.status)), &ctx);
        }
        DueOutcome::Failed
    }
}
// @end
/// Collects what binding finds.
struct Bound {
    value: Map<String, Value>,
    issues: Vec<Issue>,
    check: bool,
    // #if rules
    violations: Vec<super::rules::Violation>,
    micros: u64,
    // #endif
    // #if rollout
    audited: Vec<super::rules::Violation>,
    // #endif
    // @if derived
    /// The patterns of the derived fields without a value of each bound parameter.
    absent: BTreeMap<String, Vec<String>>,
    // @end
}
// #if rules

impl Bound {
    /// Step 5: the invariants of a bound body or query object, once its shape is valid.
    fn rules(&mut self, p: &ParamSpec, wire: &Value) {
        let watch = super::rules::Stopwatch::start();
        let found = super::rules::check_wire(p.schema, wire, &p.location(), super::rules::Mode::Request);
        self.micros += watch.micros();
        self.violations.extend(found);
    // #endif
    // #if rollout
        self.audited.extend(super::rules::audit_wire(p.schema, wire, &p.location(), super::rules::Mode::Request));
    // #endif
    // #if rules
    }
}
// #endif

// @if derived
impl Bound {
    /// The typed value of a body or query object with its derived fields computed (step 5, after
    /// the defaults, before the invariants). With the check on, the violations of a derived field
    /// that could not be computed come from the check of `rules`; with it off they are collected
    /// here, because the value cannot be handed to the handler without them.
    fn derived(&mut self, p: &ParamSpec, wire: &Value) -> Value {
        let mut typed = p.schema.from_wire(wire, true);
        let mut found = Vec::new();
        super::rules::derive_walk(p.schema, &mut typed, &p.location(), super::rules::Mode::Request, &mut found);
        let absent = super::rules::absent_patterns(p.schema, &typed);
        if !absent.is_empty() {
            self.absent.insert(p.name.to_string(), absent);
        }
        // A derived field that reads a field the request does not have is left without a value; the
        // handler reads a stand-in for it.
        super::rules::fill_derived(p.schema, &mut typed);
        if !self.check {
            self.violations.extend(found);
        }
        typed
    }
}
// @end

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
        // @if derived
        let wire = p.schema.without_derived(&wire);
        // @end
        let issues = if self.check { p.schema.validate(&wire, &p.location(), true) } else { Vec::new() };
        // #if rules
        if self.check && issues.is_empty() {
            self.rules(p, &wire);
        }
        // #endif
        if issues.is_empty() {
            // @if !derived
            self.value.insert(p.name.to_string(), p.schema.from_wire(&wire, true));
            // @end
            // @if derived
            let typed = self.derived(p, &wire);
            self.value.insert(p.name.to_string(), typed);
            // @end
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
        // @if derived
        let raw = p.schema.without_derived(&raw);
        // @end
        let issues = if self.check { p.schema.validate(&raw, "body", true) } else { Vec::new() };
        // #if rules
        if self.check && issues.is_empty() {
            self.rules(p, &raw);
        }
        // #endif
        if issues.is_empty() {
            // @if !derived
            self.value.insert(p.name.to_string(), p.schema.from_wire(&raw, true));
            // @end
            // @if derived
            let typed = self.derived(p, &raw);
            self.value.insert(p.name.to_string(), typed);
            // @end
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
        // @if !slo
        // #if !rules
        Some(self.run(route.index, caps, req).await)
        // #endif
        // #if rules
        Some(self.stamp(self.run(route.index, caps, req).await))
        // #endif
        // @end
        // @if slo
        // #if !rules
        Some(self.measured(route.index, caps, req).await)
        // #endif
        // #if rules
        Some(self.stamp(self.measured(route.index, caps, req).await))
        // #endif
        // @end
    }

    /// Runs the endpoint named `name` through the same pipeline without choosing it from the
    /// method; `None` when it is unknown or the path does not fit it. For frameworks that route
    /// themselves.
    pub async fn invoke(&self, name: &str, req: Request) -> Option<Response> {
        let route = self.inner.routes.iter().find(|r| self.inner.endpoints.endpoints()[r.index].name == name)?;
        let caps = matches(route, &req.path)?;
        // @if !slo
        // #if !rules
        Some(self.run(route.index, caps, req).await)
        // #endif
        // #if rules
        Some(self.stamp(self.run(route.index, caps, req).await))
        // #endif
        // @end
        // @if slo
        // #if !rules
        Some(self.measured(route.index, caps, req).await)
        // #endif
        // #if rules
        Some(self.stamp(self.measured(route.index, caps, req).await))
        // #endif
        // @end
    }
    // #if rules

    /// Every response of a service with a rules digest carries it (`spec/rules.md` section 9.7).
    fn stamp(&self, mut resp: Response) -> Response {
        if let Some(digest) = self.inner.endpoints.digest() {
            resp.headers.push(("veld-rules".into(), digest.into()));
        }
        resp
    }
    // #endif

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
        // #if !rules
        let io = WsIo { incoming: incoming.clone(), outgoing: outgoing.clone() };
        // #endif
        // #if rules
        let rules: Arc<dyn RuleSink> = self.inner.clone();
        // @if !flags
        let io = WsIo { incoming: incoming.clone(), outgoing: outgoing.clone(), rules };
        // @end
        // @if flags
        let io = WsIo { incoming: incoming.clone(), outgoing: outgoing.clone(), rules, flags: super::rules::FlagView::new() };
        // @end
        // #endif
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
            // @if !flags
            Ok(args) => match e.call_ws(index, args, &ctx, io).await {
            // @end
            // @if flags
            Ok(args) => match e.call_ws(index, args, &ctx, io.with_flags(&ctx.flags)).await {
            // @end
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
            // @if conditional
            conditional: Conditional::NONE,
            // @end
            // #if endpoint_rules
            prepared: Prepared::none(),
            // #endif
            // #if rollout
            audit: AuditLog::default(),
            // #endif
            // @if timeouts
            deadline: None,
            // @end
            // #if flags
            flags: super::rules::FlagView::new(),
            // #endif
            // @if timed
            timer: None,
            // @end
            // @if derived
            absent: BTreeMap::new(),
            // @end
            // @if outbox
            emitted: super::events::EmitLog::new(self.inner.endpoints.test_emitted()),
            // @end
            // @if platform
            now: std::time::SystemTime::now(),
            platform: Arc::new(super::platform::State::default()),
            // @end
            // @if tenant_types
            tenant: None,
            // @end
        }
    }

    // @if !refactor
    // #if !rollout
    async fn run(&self, index: usize, caps: Vec<(String, String)>, req: Request) -> Response {
    // #endif
    // #if rollout
    async fn run(&self, index: usize, caps: Vec<(String, String)>, req: Request) -> Response {
        let audit = AuditLog::default();
        let resp = self.run_audited(index, caps, req, audit.clone()).await;
        if self.inner.endpoints.test_audited() {
            return stamp_audited(resp, &audit);
        }
        resp
    }

    async fn run_audited(&self, index: usize, caps: Vec<(String, String)>, req: Request, audit: AuditLog) -> Response {
    // #endif
        let e = &self.inner.endpoints;
        let ep = &e.endpoints()[index];
        let mut ctx = self.context(ep, &req);
        // #if rollout
        ctx.audit = audit;
        // #endif
        // @if conditional
        ctx.conditional = e.conditional(index);
        // @end
    // @end
    // @if refactor
    async fn run(&self, index: usize, caps: Vec<(String, String)>, req: Request) -> Response {
        // #if !rollout
        let ctx = self.context(&self.inner.endpoints.endpoints()[index], &req);
        // @if outbox
        let emitted = ctx.emitted.clone();
        let resp = self.run_ctx(index, caps, req, ctx).await;
        self.stamp_emitted(resp, &emitted)
        // @end
        // @if !outbox
        self.run_ctx(index, caps, req, ctx).await
        // @end
        // #endif
        // #if rollout
        self.run_logged(index, caps, req).await
        // #endif
    }

    // #if rollout
    /// `run` for a server that listens to audited rules: the audit log of the request is stamped on
    /// the answer of a test handler.
    async fn run_logged(&self, index: usize, caps: Vec<(String, String)>, req: Request) -> Response {
        let audit = AuditLog::default();
        let mut ctx = self.context(&self.inner.endpoints.endpoints()[index], &req);
        ctx.audit = audit.clone();
        // @if outbox
        let emitted = ctx.emitted.clone();
        // @end
        let resp = self.run_ctx(index, caps, req, ctx).await;
        // @if outbox
        let resp = self.stamp_emitted(resp, &emitted);
        // @end
        if self.inner.endpoints.test_audited() {
            return stamp_audited(resp, &audit);
        }
        resp
    }
    // #endif
    // @if outbox

    /// `Veld-Test-Emitted` (`spec/events.md` section 11.2): the declaration names of the events the
    /// request wrote, `,` without spaces, in the order written; absent when nothing was written. Only
    /// a test handler stamps it.
    fn stamp_emitted(&self, mut resp: Response, log: &super::events::EmitLog) -> Response {
        if self.inner.endpoints.test_emitted() {
            if let Some(names) = log.header() {
                resp.headers.push(("veld-test-emitted".into(), names));
            }
        }
        resp
    }
    // @end

    /// The pipeline from the guard on, for a context that is ready.
    #[allow(unused_mut)]
    async fn run_ctx(&self, index: usize, caps: Vec<(String, String)>, req: Request, mut ctx: RequestContext) -> Response {
        let e = &self.inner.endpoints;
        let ep = &e.endpoints()[index];
        // @if conditional
        ctx.conditional = e.conditional(index);
        // @end
    // @end
        // @if timeouts
        // The deadline of the request: the pipeline is dropped, which cancels it, when it has no
        // answer by then (`spec/platform.md` section 12).
        let limit = ep.timeout_ms;
        let at = limit.map(|ms| tokio::time::Instant::now() + std::time::Duration::from_millis(ms));
        if let Some(ms) = limit {
            ctx.deadline = Some(std::time::SystemTime::now() + std::time::Duration::from_millis(ms));
        }
        let args = match within(at, self.guard(ep, &caps, &req, &mut ctx, false)).await {
            Some(Ok(args)) => args,
            Some(Err(resp)) => return resp,
            None => return self.deadline_exceeded(ep, limit.unwrap_or(0)),
        };
        // @end
        // @if !timeouts
        let args = match self.guard(ep, &caps, &req, &mut ctx, false).await {
            Ok(args) => args,
            Err(resp) => return resp,
        };
        // @end
        // @if split
        // @if idempotency
        let started = match self.idem_start(index, ep, &args, &ctx).await {
            Ok(started) => started,
            Err(resp) => return resp,
        };
        // The hold gives the key back when this future is dropped or the handler panics.
        let hold = Hold { inner: Arc::clone(&self.inner), started, settle: Settle::Release };
        // @end
        // @if !timeouts
        let resp = self.run_after_guard(index, args, ctx.clone()).await;
        // @end
        // @if timeouts
        // At the deadline the future of the handler is dropped; the answer is the `504`, a `5xx`,
        // so the key is given back by `idem_finish`, once, here, and not by a late settle.
        let resp = match within(at, self.run_after_guard(index, args, ctx.clone())).await {
            Some(resp) => resp,
            None => self.deadline_exceeded(ep, limit.unwrap_or(0)),
        };
        // @end
        // @if idempotency
        self.idem_finish(hold, resp, &ctx).await
        // @end
        // @if !idempotency
        resp
        // @end
    }
    // @if timeouts

    /// The answer of a request that had no answer at its deadline: `504 DeadlineExceeded`.
    fn deadline_exceeded(&self, ep: &EndpointSpec, timeout_ms: u64) -> Response {
        let std = self.inner.endpoints.std_sigs();
        error_response(std.deadline, "DeadlineExceeded", 504, ep.sig, serde_json::json!({ "timeoutMs": timeout_ms.to_string() }))
    }
    // @end

    /// Steps 6 to 9 and the answer: what runs once an idempotency key is held.
    #[allow(unused_mut)]
    async fn run_after_guard(&self, index: usize, args: Map<String, Value>, mut ctx: RequestContext) -> Response {
        let e = &self.inner.endpoints;
        let ep = &e.endpoints()[index];
        // @end
        // #if endpoint_rules
        // Step 6: facts, `require`, and what step 9 needs.
        let mut args = args;
        let watch = super::rules::Stopwatch::start();
        match e.prepare(index, &mut args, &ctx).await {
            Ok(prepared) => ctx.prepared = prepared,
            Err(halt) => return self.halted(halt, ep, &ctx, watch.micros()),
        }
        // #endif
        // #if rollout_endpoint
        let pending = ctx.audit.take_pending();
        self.inner.report_audited(ep, pending, super::rules::Position::Input, watch.micros(), (None, Some(&ctx.audit)));
        // #endif
        // #if endpoint_rules
        // #endif
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
        // @if platform
        self.open(ctx);
        // @end
        // #if endpoint_rules
        let trusted = e.trusted_principal(ctx);
        // @if timed
        if ctx.timer.is_some() {
            // A call of the due-moves runner: the principal is the timer's, set by the runner.
        } else if trusted.is_some() {
        // @end
        // @if !timed
        if trusted.is_some() {
        // @end
            ctx.principal = trusted;
        } else {
        // #endif
        if let Some(auth) = &ep.auth {
            ctx.principal = self.authenticate(auth, ctx).await;
            if ctx.principal.is_none() {
                return Err(error_response(std.unauthorized, "Unauthorized", 401, ep.sig, serde_json::json!({})));
            }
            // @if tenant_types
            ctx.tenant = e.caller_tenant(ctx);
            // @end
        }
        for name in ep.middleware {
            if let Err(err) = e.middleware(name, ctx).await {
                return Err(self.failure(&err, ep, ctx));
            }
        }
        // #if endpoint_rules
        }
        // #endif
        // @if derived
        if ctx.principal.is_some() {
            if let Err(violations) = e.complete_principal(ctx) {
                for v in &violations {
                    self.inner.report(ep, v, super::rules::Position::Output, super::rules::Outcome::Rejected, 0);
                }
                let err = Error::other(format!("{}: a derived field of the caller could not be computed", ep.name));
                return Err(self.failure(&err, ep, ctx));
            }
        }
        // @end
        // @if tenant_types
        ctx.tenant = e.caller_tenant(ctx);
        // @end
        // #if flags
        if let Some(index) = e.endpoints().iter().position(|x| std::ptr::eq(x, ep)) {
            let read = e.read_flags(index, ctx).await;
            ctx.flags = read;
        }
        // #endif
        let bound = self.bind(ep, caps, req, ctx, ws);
        // @if derived
        ctx.absent = bound.absent.clone();
        // @end
        // @if conditional
        let headers = Self::header_issues(ctx);
        // @end
        // #if rollout
        self.inner.report_audited(
            ep,
            bound.audited.clone(),
            super::rules::Position::Input,
            bound.micros,
            (None, Some(&ctx.audit)),
        );
        // #endif
        // @if conditional
        if !headers.is_empty() {
            return Err(self.refused(ep, headers, bound));
        }
        // @end
        // #if rules
        if bound.issues.is_empty() && !bound.violations.is_empty() {
            return Err(self.rejection(ep, bound.violations, bound.micros));
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
        // #if flags
        let _window = super::rules::enter(&ctx.flags);
        // #endif
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
            // @if derived
            absent: BTreeMap::new(),
            // @end
        // #endif
        // #if rollout
            audited: Vec::new(),
        // #endif
        // #if rules
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
        // #if flags
        let _window = super::rules::enter(&ctx.flags);
        // #endif
        let Some(schema) = ep.success else {
            // #if endpoint_rules
            if let Some(resp) = self.ensure_rules(ep, value, ctx) {
                return resp;
            }
            // #endif
            return Response { status: ep.status, headers: Vec::new(), body: Body::Empty };
        };
        // @if derived
        let completed = match self.inner.derive_output(ep, schema, value, "result") {
            Ok(v) => v,
            Err(why) => return self.failure(&Error::other(why), ep, ctx),
        };
        let value = &completed;
        // @end
        let wire = schema.to_wire(value, false, false);
        // #if rules
        if let Some(resp) = self.output_rules(ep, schema, &wire, ctx) {
            return resp;
        }
        // #endif
        // #if endpoint_rules
        if let Some(resp) = self.ensure_rules(ep, value, ctx) {
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
        // @if tenancy
        // A value of another tenant is never sent.
        if self.tenant_refused(ep, schema, &wire, "result", ctx) {
            return self.failure(&Error::other(format!("{}: the result holds a value of another tenant", ep.name)), ep, ctx);
        }
        // @end
        // @if visibility
        // What the caller may see is sent; the rules above judged the whole value.
        let wire = self.shown(ep, schema, &wire, ctx);
        // @end
        // @if !etag
        Response {
            status: ep.status,
            headers: vec![("content-type".into(), "application/json".into())],
            body: Body::Bytes(serde_json::to_vec(&wire).unwrap_or_default()),
        }
        // @end
        // @if etag
        let mut resp = Response {
            status: ep.status,
            headers: vec![("content-type".into(), "application/json".into())],
            body: Body::Bytes(serde_json::to_vec(&wire).unwrap_or_default()),
        };
        // The `ETag` of a success whose type is a model with a version field (section 11.10).
        if let Some(tag) = ctx.conditional.etag.and_then(|field| super::conditional::etag_of(&wire, field)) {
            resp.headers.push(("etag".into(), tag));
        }
        resp
        // @end
    }
    // @if conditional

    /// The issues of the two headers the core reads: `If-Match` of an endpoint with a version
    /// guard and the key of an `@idempotent` one. They are not rule violations.
    fn header_issues(ctx: &RequestContext) -> Vec<Issue> {
        let mut out = Vec::new();
        // @if etag
        if let Some(text) = ctx.header("if-match").filter(|_| ctx.conditional.guard) {
            if super::conditional::parse_if_match(text).is_none() {
                out.push(Issue {
                    path: "header.If-Match".into(),
                    code: "type".into(),
                    message: super::conditional::IF_MATCH_MESSAGE.into(),
                });
            }
        }
        // @end
        // @if idempotency
        if let Some(idem) = ctx.conditional.idempotency {
            let path = format!("header.{}", idem.header);
            match ctx.header(idem.header) {
                Some(key) if !super::idempotency::is_valid_key(key) => {
                    out.push(Issue { path, code: "type".into(), message: super::idempotency::KEY_MESSAGE.into() });
                }
                None if idem.required => out.push(Issue { path, code: "required".into(), message: "is required".into() }),
                Some(_) | None => {}
            }
        }
        // @end
        out
    }

    /// Step 5 found a malformed `If-Match` or key: `422 ValidationError`, the header issues first,
    /// then what else step 5 found. The violations of a rule stay.
    fn refused(&self, ep: &EndpointSpec, headers: Vec<Issue>, bound: Bound) -> Response {
        let shape_ok = bound.issues.is_empty();
        let mut issues = headers;
        issues.extend(bound.issues);
        // @if rules
        if shape_ok && !bound.violations.is_empty() {
            let mut problem = self.inner.reject_input(ep, bound.violations, bound.micros);
            if let Value::Object(data) = &mut problem.data {
                if let Some(Value::Array(list)) = data.get_mut("issues") {
                    let mut all: Vec<Value> = issues.iter().filter_map(|i| serde_json::to_value(i).ok()).collect();
                    all.append(list);
                    *list = all;
                }
            }
            return Response {
                status: problem.status,
                headers: vec![("content-type".into(), "application/problem+json".into())],
                body: Body::Bytes(problem.to_text().into_bytes()),
            };
        }
        // @end
        let _ = shape_ok;
        error_response(self.inner.endpoints.std_sigs().validation, "ValidationError", 422, ep.sig, issues_json(&issues))
    }
    // @end
    // @if idempotency

    /// After step 5, for an `@idempotent` endpoint and a request with its key: asks the store.
    /// `Ok(Some)` when the request is to run (the entry is held), `Err` with the answer when it
    /// is not: a replay, a conflict or a failed store.
    async fn idem_start(&self, index: usize, ep: &EndpointSpec, args: &Map<String, Value>, ctx: &RequestContext) -> Result<Option<Started>, Response> {
        use super::idempotency::{Begin, OTHER_MESSAGE, RUNNING_MESSAGE, fingerprint};
        let Some(idem) = ctx.conditional.idempotency else {
            return Ok(None);
        };
        let Some(key) = ctx.header(idem.header) else {
            return Ok(None);
        };
        let e = &self.inner.endpoints;
        let mut params = Map::new();
        for p in ep.params.iter().filter(|p| !p.stream) {
            if let Some(typed) = args.get(p.name) {
                params.insert(p.name.to_string(), p.schema.to_wire_fingerprint(typed));
            }
        }
        let fingerprint = fingerprint(&params);
        let scope = if idem.caller { format!("{}\n{}", ep.sig, e.idem_caller(ctx)) } else { ep.sig.to_string() };
        match e.idem_begin(index, &scope, key, &fingerprint, ctx).await {
            Ok(Begin::Started) => Ok(Some(Started { scope, key: key.to_string() })),
            Ok(Begin::Replay(answer)) => Err(replay(answer)),
            Ok(Begin::Running) => Err(self.conflict(ep, RUNNING_MESSAGE, ctx)),
            Ok(Begin::Other) => Err(self.conflict(ep, OTHER_MESSAGE, ctx)),
            Err(err) => {
                // Running without the store could run a request twice: the request does not run.
                let failed = Error::other(format!("{}: the idempotency store failed: {err}", ep.name));
                Err(self.failure(&failed, ep, ctx))
            }
        }
    }

    /// The `409 Conflict` of a key that cannot be used now.
    fn conflict(&self, ep: &EndpointSpec, message: &str, ctx: &RequestContext) -> Response {
        match ep.errors.iter().find(|d| d.status == 409 && d.name == "Conflict") {
            Some(d) => error_response(d.sig, d.name, 409, ep.sig, serde_json::json!({ "message": message })),
            None => self.failure(&Error::other(format!("{}: the endpoint does not declare `Conflict`", ep.name)), ep, ctx),
        }
    }

    /// The answer is below `500`: it is stored; otherwise the entry is removed. A store that fails
    /// here does not change the answer: the failure goes to `on_error`.
    async fn idem_finish(&self, mut hold: Hold<E>, resp: Response, ctx: &RequestContext) -> Response {
        let Some(Started { scope, key }) = hold.started.as_ref().map(|s| Started { scope: s.scope.clone(), key: s.key.clone() }) else {
            return resp;
        };
        let e = &self.inner.endpoints;
        // Still held while the store is asked: a future dropped here settles it the same way.
        let done = if resp.status < 500 {
            let answer = stored_of(&resp);
            hold.settle = Settle::Complete(answer.clone());
            e.idem_complete(&scope, &key, answer).await
        } else {
            e.idem_release(&scope, &key).await
        };
        hold.started = None;
        if let Err(err) = done {
            e.on_error(&err, ctx);
        }
        resp
    }
    // @end

    // #if rules
    /// Step 5 failed: `422 ValidationError` with one issue and one violation per broken rule.
    fn rejection(&self, ep: &EndpointSpec, violations: Vec<super::rules::Violation>, micros: u64) -> Response {
        let problem = self.inner.reject_input(ep, violations, micros);
        Response {
            status: problem.status,
            headers: vec![("content-type".into(), "application/problem+json".into())],
            body: Body::Bytes(problem.to_text().into_bytes()),
        }
    }

    /// Step 8: the invariants of the result, by output mode. `Some` replaces the response.
    fn output_rules(&self, ep: &EndpointSpec, schema: &'static Schema, wire: &Value, ctx: &RequestContext) -> Option<Response> {
        // #endif
        // #if rollout
        self.inner.audit_output(ep, schema, wire, "result", Some(&ctx.audit));
        // #endif
        // #if rules
        match self.inner.check_output(ep, schema, wire, "result") {
            Ok(()) => None,
            Err(why) => Some(self.failure(&Error::other(why), ep, ctx)),
        }
    }
    // #endif
    // #if endpoint_rules

    /// Step 9: every `ensure`, on the success value, under the output mode. `Some` replaces the
    /// response.
    fn ensure_rules(&self, ep: &EndpointSpec, value: &Value, ctx: &RequestContext) -> Option<Response> {
        use super::rules::{Outcome, Position, RuleOutput};
        let mode = self.inner.endpoints.rules_output();
        if mode == RuleOutput::Off {
            return None;
        }
        let watch = super::rules::Stopwatch::start();
        let violations = self.inner.endpoints.ensure(&ctx.prepared, value);
        // #endif
        // #if rollout_endpoint
        let audited = self.inner.endpoints.ensure_audited(&ctx.prepared, value);
        self.inner.report_audited(ep, audited, Position::Output, watch.micros(), (None, Some(&ctx.audit)));
        // #endif
        // #if endpoint_rules
        if violations.is_empty() {
            return None;
        }
        let outcome = if mode == RuleOutput::Strict { Outcome::Rejected } else { Outcome::Audited };
        for v in &violations {
            self.inner.report(ep, v, Position::Output, outcome, watch.micros());
        }
        if mode == RuleOutput::Strict {
            return Some(self.failure(&Error::other(format!("{}: the result violates {} ensure statement(s)", ep.name, violations.len())), ep, ctx));
        }
        None
    }

    /// Step 6 stopped the request.
    fn halted(&self, halt: Halt, ep: &EndpointSpec, ctx: &RequestContext, micros: u64) -> Response {
        // #endif
        // #if rollout_endpoint
        let pending = ctx.audit.take_pending();
        self.inner.report_audited(ep, pending, super::rules::Position::Input, micros, (None, Some(&ctx.audit)));
        // #endif
        // #if endpoint_rules
        use super::rules::{Outcome, Position};
        match halt {
            Halt::Early(resp) => resp,
            Halt::Error(err) => {
                if let Error::Problem(p) = &err {
                    for v in p.violations() {
                        self.inner.report(ep, v, Position::Input, Outcome::Rejected, micros);
                    }
                }
                self.failure(&err, ep, ctx)
            }
            Halt::Broken(violations) => {
                for v in &violations {
                    self.inner.report(ep, v, Position::Output, Outcome::Rejected, micros);
                }
                let err = Error::other(format!("{}: the declared error of a failed `require` could not be built", ep.name));
                self.failure(&err, ep, ctx)
            }
        }
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
        // #if flags
        let _window = super::rules::enter(&ctx.flags);
        // #endif
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
                // #if rules
                if p.sig == std.validation && p.rulings.is_some() {
                    // A rejected input carries its violations.
                    let (status, mut out) = problem(std.validation, "ValidationError", 422, p.data.clone());
                    out.rulings = p.rulings.clone();
                    return (status, out);
                }
                // #endif
                if let Some(d) = ep.errors.iter().find(|d| d.sig == p.sig) {
                    // @if derived
                    // A declared error is an output: its derived fields are computed first.
                    let data = match self.inner.derive_output(ep, d.schema, &d.schema.from_wire(&p.data, false), "declared error") {
                        Ok(typed) => d.schema.to_wire(&typed, false, false),
                        Err(why) => {
                            e.on_error(&Error::other(why), ctx);
                            return problem(std.internal, "Internal", 500, serde_json::json!({}));
                        }
                    };
                    let p = &Problem { data, ..p.clone() };
                    // @end
                    // #if rules
                    // A declared error is an output: its own rules are checked like a result's.
                    // #endif
                    // #if rollout
                    self.inner.audit_output(ep, d.schema, &p.data, "error", Some(&ctx.audit));
                    // #endif
                    // #if rules
                    if let Err(why) = self.inner.check_output(ep, d.schema, &p.data, "declared error") {
                        e.on_error(&Error::other(why), ctx);
                        return problem(std.internal, "Internal", 500, serde_json::json!({}));
                    }
                    // @if tenancy
                    if self.tenant_refused(ep, d.schema, &p.data, "error", ctx) {
                        e.on_error(&Error::other(format!("{}: a declared error holds a value of another tenant", ep.name)), ctx);
                        return problem(std.internal, "Internal", 500, serde_json::json!({}));
                    }
                    // @end
                    // @if visibility
                    let p = &Problem { data: self.shown(ep, d.schema, &p.data, ctx), ..p.clone() };
                    // @end
                    let (status, mut out) = problem(d.sig, d.name, d.status, p.data.clone());
                    out.rulings = p.rulings.clone();
                    return (status, out);
                    // #endif
                    // #if !rules
                    return problem(d.sig, d.name, d.status, p.data.clone());
                    // #endif
                }
            }
            Error::Invalid(issues) => return problem(std.validation, "ValidationError", 422, issues_json(issues)),
            Error::Http { .. } | Error::Transport(_) | Error::Other(_) => {}
        }
        e.on_error(err, ctx);
        problem(std.internal, "Internal", 500, serde_json::json!({}))
    }

    // @if sse
    // @if !shapes
    fn frame(&self, ep: &EndpointSpec, ev: &SseEvent<Value>) -> Result<Vec<u8>, Error> {
    // @end
    // @if shapes
    fn frame(&self, ep: &EndpointSpec, ev: &SseEvent<Value>, ctx: &RequestContext) -> Result<Vec<u8>, Error> {
    // @end
        let schema = ep.success.ok_or_else(|| Error::other(format!("{}: a stream needs an item type", ep.name)))?;
        // @if derived
        let item = self.inner.derive_output(ep, schema, &ev.data, "stream item").map_err(Error::other)?;
        let ev = &SseEvent { data: item, ..ev.clone() };
        // @end
        // #if rules
        // A stream item is an output: in `strict` a violating item is not sent.
        // #endif
        // #if rollout
        self.inner.audit_output(ep, schema, &schema.to_wire(&ev.data, false, false), "result", None);
        // #endif
        // #if rules
        self.inner.check_output(ep, schema, &schema.to_wire(&ev.data, false, false), "stream item").map_err(Error::other)?;
        // #endif
        if self.inner.endpoints.validate_responses() {
            let issues = schema.validate(&schema.to_wire(&ev.data, false, false), "$", false);
            if !issues.is_empty() {
                return Err(Error::other(format!("{}: stream item failed validation: {issues:?}", ep.name)));
            }
        }
        // @if !shapes
        let data = String::from_utf8_lossy(&schema.canonical_typed(&ev.data)).into_owned();
        // @end
        // @if shapes
        let data = self.item_text(ep, schema, &ev.data, ctx)?;
        // @end
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
            // @if !flags
            // @if !shapes
            Some(Ok(ev)) => match self.frame(ep, &ev) {
            // @end
            // @if shapes
            Some(Ok(ev)) => match self.frame(ep, &ev, &ctx) {
            // @end
            // @end
            // @if flags
            // @if !shapes
            Some(Ok(ev)) => match super::rules::with_view(Some(&ctx.flags), || self.frame(ep, &ev)) {
            // @end
            // @if shapes
            Some(Ok(ev)) => match super::rules::with_view(Some(&ctx.flags), || self.frame(ep, &ev, &ctx)) {
            // @end
            // @end
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
                    // @if !flags
                    // @if !shapes
                    Ok(ev) => st.handler.frame(ep, &ev),
                    // @end
                    // @if shapes
                    Ok(ev) => st.handler.frame(ep, &ev, &st.ctx),
                    // @end
                    // @end
                    // @if flags
                    // @if !shapes
                    Ok(ev) => super::rules::with_view(Some(&st.ctx.flags), || st.handler.frame(ep, &ev)),
                    // @end
                    // @if shapes
                    Ok(ev) => super::rules::with_view(Some(&st.ctx.flags), || st.handler.frame(ep, &ev, &st.ctx)),
                    // @end
                    // @end
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
// @if platform

impl<E: Endpoints> Handler<E> {
    /// The first thing a request does once it is matched: the clock is read once, and that
    /// reading is the `now` of everything the request evaluates.
    fn open(&self, ctx: &mut RequestContext) {
        // @if timed
        let own = ctx.timer_now();
        // @end
        // @if !timed
        let own: Option<std::time::SystemTime> = None;
        // @end
        ctx.now = own.unwrap_or_else(|| self.inner.endpoints.request_clock(ctx));
    }

    // @if tenancy

    /// Whether `wire`, a value that is about to be sent for `ep`, holds a tenant-scoped value that
    /// is not the caller's; each such value is reported to the hook (position `output`, outcome
    /// `rejected`, `spec/platform.md` section 3). No output mode turns it off.
    fn tenant_refused(&self, ep: &EndpointSpec, schema: &Schema, wire: &Value, root: &str, ctx: &RequestContext) -> bool {
        let found = super::platform::foreign_values(schema, wire, ctx.tenant(), root);
        for v in &found {
            self.inner.report(ep, v, super::rules::Position::Output, super::rules::Outcome::Rejected, 0);
        }
        !found.is_empty()
    }
    // @end
    // @if visibility

    /// The caller as far as visibility is concerned: the principal of an endpoint that has
    /// authentication. A request with no caller has none.
    fn caller<'a>(ep: &EndpointSpec, ctx: &'a RequestContext) -> Option<&'a Principal> {
        if ep.auth.is_some() { ctx.principal.as_ref() } else { None }
    }

    /// `wire`, a value of `schema` that is about to be sent for `ep`, without the `@visible`
    /// fields its caller may not see (`spec/platform.md` section 2). What was dropped is noted.
    fn shown(&self, ep: &EndpointSpec, schema: &Schema, wire: &Value, ctx: &RequestContext) -> Value {
        let shown = super::platform::show_value(schema, wire, Self::caller(ep, ctx), ctx.now_ms(), "$");
        ctx.platform.note_hidden(&shown.hidden);
        shown.value
    }
    // @end
    // @if shapes
    // @if sse

    /// The canonical text of a stream item as it is sent.
    fn item_text(&self, ep: &EndpointSpec, schema: &Schema, typed: &Value, ctx: &RequestContext) -> Result<String, Error> {
        let wire = schema.to_wire(typed, false, true);
        // @if tenancy
        if self.tenant_refused(ep, schema, &wire, "item", ctx) {
            return Err(Error::other(format!("{}: a stream item holds a value of another tenant", ep.name)));
        }
        // @end
        // @if visibility
        let wire = self.shown(ep, schema, &wire, ctx);
        // @end
        Ok(super::canonical::canonical(&wire))
    }
    // @end
    // @end
}
// @end

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
    // #if rules
    rules: Arc<dyn RuleSink>,
    // #endif
    // @if flags
    /// What the upgrade request read of its flags: the checks of the messages run with it in force.
    flags: super::rules::FlagView,
    // @end
}
// @if flags

impl WsIo {
    /// The same sides with the readings of the request that opened the conversation.
    fn with_flags(mut self, view: &super::rules::FlagView) -> Self {
        self.flags = view.clone();
        self
    }

    /// Puts the readings in force until the guard is dropped: the checks of a message are
    /// synchronous, so it is never held across an `.await`.
    fn window(&self) -> super::rules::Window {
        super::rules::enter(&self.flags)
    }
}
// @end

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
    // #if rules
    ep: &'static EndpointSpec,
    // #endif
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
            // #if rules
            ep,
            // #endif
        }
    }

    /// The next message: validated and decoded. `None` when the socket closed. A message that
    /// does not fit is an `Err` (`Error::Invalid`: returning it ends the conversation with a
    /// `ValidationError` frame whose issue paths start with `message`).
    pub async fn recv(&mut self) -> Option<Result<In, Error>> {
        let text = self.io.incoming.recv().await?;
        // @if flags
        let _window = self.io.window();
        // @end
        let Some(schema) = self.incoming else {
            return Some(Err(invalid("unexpected", "this endpoint does not accept messages")));
        };
        let Ok(wire) = serde_json::from_str::<Value>(&text) else {
            return Some(Err(invalid("json", "message is not valid JSON")));
        };
        // @if derived
        let wire = schema.without_derived(&wire);
        // @end
        if self.validate_in {
            let issues = schema.validate(&wire, "message", true);
            if !issues.is_empty() {
                return Some(Err(Error::Invalid(issues)));
            }
            // #if rules
            // The rules of a message are checked like those of a request body.
            // #endif
            // #if rollout
            self.io.rules.audit_message(self.ep, schema, &wire);
            // #endif
            // #if rules
            let watch = super::rules::Stopwatch::start();
            let violations = super::rules::check_wire(schema, &wire, "message", super::rules::Mode::Request);
            if !violations.is_empty() {
                return Some(Err(Error::Problem(self.io.rules.reject_input(self.ep, violations, watch.micros()))));
            }
            // #endif
        }
        // @if !derived
        Some(serde_json::from_value(schema.from_wire(&wire, true)).map_err(|e| invalid("type", &e.to_string())))
        // @end
        // @if derived
        let mut typed = schema.from_wire(&wire, true);
        let mut found = Vec::new();
        super::rules::derive_walk(schema, &mut typed, "message", super::rules::Mode::Request, &mut found);
        if !found.is_empty() {
            return Some(Err(Error::Problem(self.io.rules.reject_input(self.ep, found, 0))));
        }
        super::rules::fill_derived(schema, &mut typed);
        Some(serde_json::from_value(typed).map_err(|e| invalid("type", &e.to_string())))
        // @end
    }

    /// Sends a message to the client: `{"data": <canonical JSON>}`.
    ///
    /// # Errors
    /// [`Error::Other`] when the message cannot be encoded, or fails validation with
    /// `validate_responses` on.
    pub fn send(&self, message: Out) -> Result<(), Error> {
        // @if flags
        let _window = self.io.window();
        // @end
        let typed = serde_json::to_value(&message).map_err(Error::other)?;
        let data = match self.outgoing {
            Some(schema) => {
                // @if derived
                let typed = self.io.rules.derive_output(self.ep, schema, &typed, "message").map_err(Error::other)?;
                // @end
                if self.validate_out {
                    let issues = schema.validate(&schema.to_wire(&typed, false, false), "$", false);
                    if !issues.is_empty() {
                        return Err(Error::other(format!("message failed validation: {issues:?}")));
                    }
                }
                // #if rules
                // A message the server sends is an output: in `strict` a violating one is not sent.
                // #endif
                // #if rollout
                self.io.rules.audit_output(self.ep, schema, &schema.to_wire(&typed, false, false), "result", None);
                // #endif
                // #if rules
                self.io.rules.check_output(self.ep, schema, &schema.to_wire(&typed, false, false), "message").map_err(Error::other)?;
                // #endif
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
