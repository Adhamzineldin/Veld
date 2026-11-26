//! The server of a service: the `Service` trait, the hooks, and the dispatch into the core.

use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_ir::{AuthKind, Binding, Endpoint, Service, ServiceId};

use crate::block;
use crate::endpoints::{self, Kind, endpoint_statics, request_name, request_struct, std_sigs};
use crate::models::with_imports;
use crate::tree::Tree;
use crate::types::{Ctx, rust_type};
use crate::util::{add_file, doc, method, module, snake, type_name};

/// The file of a service's server module.
#[must_use]
pub fn service_mod(svc: &Service) -> String {
    format!("{}_server", module(&svc.name))
}

pub(crate) fn service_type(svc: &Service) -> String {
    type_name(&svc.name)
}

/// The authentication kinds and middleware names the endpoints of a service need.
pub(crate) struct Needs {
    bearer: bool,
    basic: bool,
    api_key: bool,
    middleware: Vec<String>,
    /// An `@idempotent` endpoint: the hooks give the store.
    idempotency: bool,
    /// An `@idempotent` endpoint whose keys are scoped to the caller: the hooks give the caller text.
    caller: bool,
    /// An endpoint can reach a flag: the hooks give the provider.
    flags: bool,
    /// An endpoint whose move emits an event: the hooks give the outbox.
    outbox: bool,
}

pub(crate) fn needs(tree: &Tree<'_>, svc: &Service) -> Needs {
    let mut n = Needs {
        bearer: false,
        basic: false,
        api_key: false,
        middleware: Vec::new(),
        idempotency: crate::conditional::svc_idempotent(svc),
        caller: crate::conditional::svc_needs_caller(svc),
        flags: crate::flags::svc_reads_flags(tree, svc),
        outbox: crate::outbox::svc_emits(svc),
    };
    for ep in &svc.endpoints {
        match ep.auth.kind {
            AuthKind::None => {}
            AuthKind::Bearer => n.bearer = true,
            AuthKind::Basic => n.basic = true,
            AuthKind::ApiKey => n.api_key = true,
        }
        for m in &ep.middleware {
            if !n.middleware.contains(m) {
                n.middleware.push(m.clone());
            }
        }
    }
    n
}

impl Needs {
    pub(crate) fn any(&self) -> bool {
        self.bearer
            || self.basic
            || self.api_key
            || !self.middleware.is_empty()
            || self.idempotency
            || self.flags
            || self.outbox
    }
}

/// Emits `<pkg>/<service>_server.rs` for every service of the tree.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<(), Vec<GenDiagnostic>> {
    for sid in &tree.services {
        let svc = tree.ir.service(*sid);
        // A service that only publishes or subscribes has no HTTP surface: its events are in
        // `<service>_events.rs`.
        if !crate::events::serves_http(svc) {
            continue;
        }
        let here = format!(
            "{}::{}::{}",
            tree.root,
            tree.pkg_mod(svc.package),
            service_mod(svc)
        );
        let ctx = Ctx { tree, here: &here };
        let mut w = CodeWriter::with_unit("    ");
        service_file(&mut w, &ctx, svc, *sid)?;
        let path = format!(
            "{}/{}/{}.rs",
            tree.dir,
            tree.pkg_mod(svc.package),
            service_mod(svc)
        );
        add_file(files, &path, with_imports(&w.finish()))?;
    }
    Ok(())
}

fn service_file(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    svc: &Service,
    _sid: ServiceId,
) -> Result<(), Vec<GenDiagnostic>> {
    w.line(format!(
        "//! The `{}` service: requests, the service trait, hooks and the handler.",
        svc.name
    ))
    .blank();
    for ep in &svc.endpoints {
        request_struct(w, ctx, ep);
        if block::has_facts(ep) {
            block::facts_struct(w, ctx, ep);
        }
    }
    endpoint_statics(w, ctx, svc);
    std_sigs(w, ctx.tree);
    service_trait(w, ctx, svc);
    let n = needs(ctx.tree, svc);
    let typed = typed_principal(ctx, svc);
    hooks_trait(
        w,
        svc,
        &n,
        ctx.tree.validate,
        (
            ctx.tree.rules,
            ctx.tree.timed,
            crate::flags::svc_reads_flags(ctx.tree, svc),
        ),
        typed.as_deref(),
    );
    if !svc.facts.is_empty() {
        block::loader_trait(w, ctx, svc);
    }
    block::steps(w, ctx, svc)?;
    if crate::flags::svc_reads_flags(ctx.tree, svc) {
        crate::flags::names_fn(w, ctx.tree, svc);
    }
    dispatch(w, ctx.tree, svc, &n, typed.as_deref());
    handler(w, ctx.tree, svc, &n);
    if ctx.tree.timed {
        crate::timed::emit(w, ctx.tree.ir, svc);
    }
    Ok(())
}

/// Whether the rule output mode comes from the runtime config (`rules.output`) and not from the
/// hooks.
pub(crate) fn mode_in_runtime(tree: &Tree<'_>) -> bool {
    tree.rules && tree.runtime.as_ref().is_some_and(|r| r.output)
}

fn context_type() -> &'static str {
    "&veld::server::RequestContext"
}

fn success_type(ctx: &Ctx<'_, '_>, ep: &Endpoint) -> String {
    let item = ep
        .success
        .ty
        .as_ref()
        .map_or("()".to_string(), |t| rust_type(ctx, t, false));
    match endpoints::kind(ep) {
        Kind::Sse => format!("veld::stream::EventStream<{item}>"),
        Kind::Http | Kind::Ws => item,
    }
}

fn service_trait(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service) {
    let name = service_type(svc);
    doc(w, svc.docs.as_deref());
    w.line("///").line("/// Return a declared error (`Err(NotFound { .. }.into())`) to answer with its status. Any other");
    w.line("/// error becomes `Internal` without its message; the original goes to `on_error`.");
    w.open(format!("pub trait {name}Service: Send + Sync + 'static"));
    for ep in &svc.endpoints {
        endpoints::method_docs(w, ep);
        block::transition_docs(w, ep);
        crate::conditional::version_docs(w, ep);
        crate::outbox::emit_docs(w, ep);
        if endpoints::kind(ep) == Kind::Ws {
            let (incoming, outgoing) = endpoints::ws_types(ctx, ep);
            w.line(format!("fn {}(&self, request: {}, ctx: {}, ws: veld::server::WsSession<{incoming}, {outgoing}>) -> impl std::future::Future<Output = Result<(), veld::Error>> + Send;", method(&ep.name), request_name(ep), context_type()));
        } else {
            w.line(format!("fn {}(&self, request: {}{}{}{}, ctx: {}) -> impl std::future::Future<Output = Result<{}, veld::Error>> + Send;", method(&ep.name), request_name(ep), block::facts_param(ep), crate::conditional::version_param(ctx, ep), block::transition_param(ctx, ep) + &crate::outbox::emit_param(ctx.tree, svc, ep), context_type(), success_type(ctx, ep)));
        }
        w.blank();
    }
    w.close().blank();
}

fn hooks_trait(
    w: &mut CodeWriter,
    svc: &Service,
    n: &Needs,
    validate: bool,
    (rules, ctx_timed, reads_flags): (bool, bool, bool),
    typed: Option<&str>,
) {
    let name = service_type(svc);
    w.line(format!(
        "/// The hooks of the `{}` service: the authentication and middleware its endpoints need,",
        svc.name
    ));
    w.line("/// which have no default, and optional observers.");
    w.open(format!("pub trait {name}Hooks: Send + Sync + 'static"));
    let future = |out: &str| format!("impl std::future::Future<Output = {out}> + Send + 'a");
    // A service with a principal that the rules read: the hooks return the model itself.
    let principal = future(&typed.map_or_else(
        || "Option<veld::server::Principal>".to_string(),
        |ty| format!("Option<{ty}>"),
    ));
    if n.bearer {
        w.line("/// Authenticates a bearer token; `None` answers `401 Unauthorized`.");
        w.line(format!("fn auth_bearer<'a>(&'a self, token: &'a str, ctx: &'a veld::server::RequestContext) -> {principal};"));
        w.blank();
    }
    if n.basic {
        w.line("/// Authenticates basic credentials; `None` answers `401 Unauthorized`.");
        w.line(format!("fn auth_basic<'a>(&'a self, user: &'a str, password: &'a str, ctx: &'a veld::server::RequestContext) -> {principal};"));
        w.blank();
    }
    if n.api_key {
        w.line("/// Authenticates an API key; `None` answers `401 Unauthorized`.");
        w.line(format!("fn auth_api_key<'a>(&'a self, key: &'a str, ctx: &'a veld::server::RequestContext) -> {principal};"));
        w.blank();
    }
    for m in &n.middleware {
        w.line(format!(
            "/// The `{m}` middleware. An error stops the request."
        ));
        w.line(format!(
            "fn middleware_{}<'a>(&'a self, ctx: &'a veld::server::RequestContext) -> {};",
            snake(m),
            future("Result<(), veld::Error>")
        ));
        w.blank();
    }
    w.line("/// Observes every error that becomes `Internal`. It cannot change the response.");
    w.line("fn on_error(&self, _err: &veld::Error, _ctx: &veld::server::RequestContext) {}")
        .blank();
    w.line(format!("/// Validate parameters and bodies (default `{validate}`). Off, the types stay and values are only decoded."));
    w.open("fn validate_requests(&self) -> bool");
    w.line(validate.to_string());
    w.close().blank();
    w.line("/// Validate every response before it is sent (default `false`).");
    w.open("fn validate_responses(&self) -> bool");
    w.line("false");
    w.close();
    if rules {
        w.blank();
        w.line("/// Observes every violated invariant of an input (rejected) or an output (rejected or");
        w.line("/// audited). It cannot change the response.");
        w.line("fn on_rule_violation(&self, _event: &veld::rules::RuleEvent) {}");
    }
    if n.idempotency {
        idempotency_hooks(w, n, typed);
    }
    if reads_flags {
        crate::flags::hooks(w);
    }
    if n.outbox {
        crate::outbox::hooks(w);
    }
    if svc.endpoints.iter().any(|e| e.slo.is_some()) {
        w.blank();
        w.line("/// Receives the metric of every request that matched an endpoint of this server, once its");
        w.line("/// answer is ready (`spec/platform.md` section 13). It cannot change the response; a hook that");
        w.line("/// panics is ignored. Optional.");
        w.line("fn metrics(&self, _metric: &veld::server::Metric) {}");
    }
    if ctx_timed && crate::timed::svc_timed(svc) {
        w.blank();
        w.line("/// The principal of the due-moves runner (`spec/platform.md` section 10.1): the caller of a");
        w.line("/// timed move. The default is none; an endpoint with authentication needs one, or the run");
        w.line("/// does not start.");
        w.open(format!("fn timer_principal<'a>(&'a self) -> impl std::future::Future<Output = Option<{}>> + Send + 'a", typed.unwrap_or("veld::server::Principal")));
        w.line("async { None }");
        w.close();
    }
    if block::has_block(svc) {
        w.blank();
        w.line("/// The clock of the endpoint rules (`now`): read once per request, before the facts are");
        w.line("/// loaded. The default is the system clock.");
        w.open("fn clock(&self, _ctx: &veld::server::RequestContext) -> std::time::SystemTime");
        w.line("std::time::SystemTime::now()");
        w.close();
    }
    w.close().blank();
    if !n.any() {
        w.line("/// The hooks of a service that needs none.");
        w.line("#[derive(Debug, Clone, Copy, Default)]");
        w.line("pub struct NoHooks;").blank();
        w.line(format!("impl {name}Hooks for NoHooks {{}}")).blank();
    }
}

/// Whether a body, query object or WebSocket message of the service can reach a rule.
fn input_reaches_rules(tree: &Tree<'_>, svc: &Service) -> bool {
    svc.endpoints.iter().any(|ep| {
        !ep.rules.is_empty()
            || ep.params.iter().any(|p| {
                (p.stream || matches!(p.binding, Binding::Body | Binding::QueryObject))
                    && tree.type_reaches_rules(&p.ty)
            })
    })
}

fn dispatch(w: &mut CodeWriter, tree: &Tree<'_>, svc: &Service, n: &Needs, typed: Option<&str>) {
    let rules = tree.rules;
    let name = service_type(svc);
    w.line(format!(
        "/// Connects `{name}Service` and `{name}Hooks` to the server core."
    ));
    let loaders = !svc.facts.is_empty();
    let (decl, bounds) = if loaders {
        (
            "S, H, L".to_string(),
            format!(
                "S: {name}Service, H: {name}Hooks, L: {}",
                block::loaders_name(svc)
            ),
        )
    } else {
        (
            "S, H".to_string(),
            format!("S: {name}Service, H: {name}Hooks"),
        )
    };
    w.open(format!("pub struct {name}Dispatch<{decl}>"));
    // The test handler of a service with endpoint rules reaches into its dispatch.
    let vis = if block::has_block(svc) || n.idempotency || crate::flags::svc_reads_flags(tree, svc)
    {
        "pub(crate) "
    } else {
        ""
    };
    w.line(format!("{vis}service: S,"));
    w.line(format!("{vis}hooks: H,"));
    if loaders {
        w.line(format!("{vis}loaders: L,"));
    }
    if mode_in_runtime(tree) {
        w.line(format!("{vis}output: veld::rules::RuleOutput,"));
    }
    w.close().blank();
    w.open(format!(
        "impl<{bounds}> veld::server::Endpoints for {name}Dispatch<{decl}>"
    ));
    w.open("fn endpoints(&self) -> &'static [sp::EndpointSpec]");
    w.line("&ENDPOINTS");
    w.close().blank();
    w.open("fn std_sigs(&self) -> sp::StdSigs");
    w.line("STD");
    w.close().blank();
    w.open("fn validate_requests(&self) -> bool");
    w.line("self.hooks.validate_requests()");
    w.close().blank();
    w.open("fn validate_responses(&self) -> bool");
    w.line("self.hooks.validate_responses()");
    w.close().blank();
    w.open("fn on_error(&self, err: &veld::Error, ctx: &veld::server::RequestContext)");
    w.line("self.hooks.on_error(err, ctx);");
    w.close().blank();
    if rules {
        rules_forwards(w, tree, svc);
    }
    if block::has_block(svc) {
        block_methods(w, svc, tree.timed);
    }
    conditional_method(w, tree, svc);
    if crate::flags::svc_reads_flags(tree, svc) {
        crate::flags::forwards(w);
    }
    if svc.endpoints.iter().any(|e| e.slo.is_some()) {
        w.open("fn metrics(&self, metric: &veld::server::Metric)");
        w.line("self.hooks.metrics(metric);");
        w.close().blank();
    }
    if tree.timed && crate::timed::svc_timed(svc) {
        timer_forwards(w, typed);
    }
    complete_principal_method(w, tree, svc, typed);
    if n.idempotency {
        idempotency_forwards(w, n, typed);
    }
    auth_forwards(w, n, typed);
    call_methods(w, tree, svc);
    w.close().blank();
}

/// The rule hooks of a dispatch, the input reach and the digest.
fn rules_forwards(w: &mut CodeWriter, tree: &Tree<'_>, svc: &Service) {
    // A server whose rules are all in consumed services has no output mode (the default, `Strict`,
    // of the runtime): `spec/runtime-config.md` section 5.
    if mode_in_runtime(tree) {
        w.open("fn rules_output(&self) -> veld::rules::RuleOutput");
        w.line("self.output");
        w.close().blank();
    }
    w.open("fn on_rule_violation(&self, event: &veld::rules::RuleEvent)");
    w.line("self.hooks.on_rule_violation(event);");
    w.close().blank();
    w.open("fn reaches_rules(&self) -> bool");
    w.line(input_reaches_rules(tree, svc).to_string());
    w.close().blank();
    if let Some(digest) = &svc.rules_digest {
        w.open("fn digest(&self) -> Option<&'static str>");
        w.line(format!("Some({})", crate::util::lit(digest)));
        w.close().blank();
    }
}

/// The forwards of a dispatch to the timer hooks of the hooks.
fn timer_forwards(w: &mut CodeWriter, typed: Option<&str>) {
    allow_async(w, typed);
    w.open("fn timer_principal<'a>(&'a self) -> impl std::future::Future<Output = Option<veld::server::Principal>> + Send + 'a");
    w.line(forward("timer_principal()", typed));
    w.close().blank();
    w.open("fn timer_clock(&self, ctx: &veld::server::RequestContext) -> std::time::SystemTime");
    w.line("self.hooks.clock(ctx)");
    w.close().blank();
}

/// `prepare` and `ensure` of a dispatch whose service has endpoint rules.
fn block_methods(w: &mut CodeWriter, svc: &Service, tree_timed: bool) {
    let loaders = !svc.facts.is_empty();
    let pass = if loaders { "&self.loaders, " } else { "" };
    w.open("async fn prepare(&self, endpoint: usize, args: &mut serde_json::Map<String, serde_json::Value>, ctx: &veld::server::RequestContext) -> Result<veld::server::Prepared, veld::server::Halt>");
    w.line("let enforce = self.hooks.validate_requests();");
    if tree_timed {
        w.line("let now = veld::rules::clock_ms(ctx.timer_now().unwrap_or_else(|| self.hooks.clock(ctx)));");
    } else {
        w.line("let now = veld::rules::clock_ms(self.hooks.clock(ctx));");
    }
    w.open("match endpoint");
    for (i, ep) in svc.endpoints.iter().enumerate() {
        if block::ep_has_block(ep) {
            w.line(format!(
                "{i} => {}({pass}now, enforce, args, ctx).await,",
                block::step6_name(ep)
            ));
        }
    }
    w.line("_ => Ok(veld::server::Prepared::none()),");
    w.close().close().blank();
    if !svc.endpoints.iter().any(block::has_step9) {
        return;
    }
    w.open("fn ensure(&self, prepared: &veld::server::Prepared, result: &serde_json::Value) -> Vec<veld::rules::Violation>");
    w.open("match prepared.index()");
    for (i, ep) in svc.endpoints.iter().enumerate() {
        if block::has_step9(ep) {
            w.line(format!(
                "{i} => {}(prepared, result),",
                block::step9_name(ep)
            ));
        }
    }
    w.line("_ => Vec::new(),");
    w.close().close().blank();
    audited_ensure(w, svc);
}

/// `ensure_audited` of a dispatch whose service has an audited `ensure` (`@rollout(audit)`).
fn audited_ensure(w: &mut CodeWriter, svc: &Service) {
    if !svc.endpoints.iter().any(block::has_audited_ensure) {
        return;
    }
    w.open("fn ensure_audited(&self, prepared: &veld::server::Prepared, result: &serde_json::Value) -> Vec<veld::rules::Violation>");
    w.open("match prepared.index()");
    for (i, ep) in svc.endpoints.iter().enumerate() {
        if block::has_audited_ensure(ep) {
            w.line(format!(
                "{i} => {}(prepared, result),",
                block::step9_audited_name(ep)
            ));
        }
    }
    w.line("_ => Vec::new(),");
    w.close().close().blank();
}

/// The principal type of a service whose hooks are typed: it has a principal and some endpoint
/// reads it or calls a policy (`spec/rules.md` section 11.1). A principal that nothing reads
/// changes no bytes.
pub(crate) fn typed_principal(ctx: &Ctx<'_, '_>, svc: &Service) -> Option<String> {
    if principal_has_derived(ctx.tree, svc)
        || svc
            .endpoints
            .iter()
            .any(|e| e.rules.first_policy_origin().is_some())
    {
        crate::guards::principal_type(ctx, svc)
    } else {
        None
    }
}

/// Whether the principal's model of the service has derived fields: the server completes the
/// principal for every endpoint, so the hooks return the model itself (`spec/rules.md` 17.2).
pub(crate) fn principal_has_derived(tree: &Tree<'_>, svc: &Service) -> bool {
    tree.derived
        && svc
            .principal
            .as_ref()
            .is_some_and(|p| tree.ir.decl(p.decl).has_derived())
}

/// `complete_principal` of a dispatch whose principal model has derived fields.
fn complete_principal_method(
    w: &mut CodeWriter,
    tree: &Tree<'_>,
    svc: &Service,
    typed: Option<&str>,
) {
    let Some(ty) = typed.filter(|_| principal_has_derived(tree, svc)) else {
        return;
    };
    w.open("fn complete_principal(&self, ctx: &mut veld::server::RequestContext) -> Result<(), Vec<veld::rules::Violation>>");
    w.line(format!(
        "let Some(mut principal) = ctx.principal::<{ty}>().cloned() else {{ return Ok(()) }};"
    ));
    w.line(format!(
        "veld::rules::complete_with(<{ty} as veld::VeldDecl>::schema(), &mut principal)?;"
    ));
    w.line("ctx.principal = Some(veld::server::Principal::new(principal));");
    w.line("Ok(())");
    w.close().blank();
}

/// The call of a hook of the service: as it is, or wrapped into the untyped principal of the core.
fn forward(call: &str, typed: Option<&str>) -> String {
    if typed.is_some() {
        format!("async move {{ self.hooks.{call}.await.map(veld::server::Principal::new) }}")
    } else {
        format!("self.hooks.{call}")
    }
}

/// The forward of a typed hook is an `async` block in a function that returns `impl Future`.
fn allow_async(w: &mut CodeWriter, typed: Option<&str>) {
    if typed.is_some() {
        w.line("#[allow(clippy::manual_async_fn)]");
    }
}

fn auth_forwards(w: &mut CodeWriter, n: &Needs, typed: Option<&str>) {
    let future = |out: &str| format!("impl std::future::Future<Output = {out}> + Send + 'a");
    let principal = future("Option<veld::server::Principal>");
    if n.bearer {
        allow_async(w, typed);
        w.open(format!("fn auth_bearer<'a>(&'a self, token: &'a str, ctx: &'a veld::server::RequestContext) -> {principal}"));
        w.line(forward("auth_bearer(token, ctx)", typed));
        w.close().blank();
    }
    if n.basic {
        allow_async(w, typed);
        w.open(format!("fn auth_basic<'a>(&'a self, user: &'a str, password: &'a str, ctx: &'a veld::server::RequestContext) -> {principal}"));
        w.line(forward("auth_basic(user, password, ctx)", typed));
        w.close().blank();
    }
    if n.api_key {
        allow_async(w, typed);
        w.open(format!("fn auth_api_key<'a>(&'a self, key: &'a str, ctx: &'a veld::server::RequestContext) -> {principal}"));
        w.line(forward("auth_api_key(key, ctx)", typed));
        w.close().blank();
    }
    if !n.middleware.is_empty() {
        w.open("async fn middleware(&self, name: &str, ctx: &veld::server::RequestContext) -> Result<(), veld::Error>");
        w.open("match name");
        for m in &n.middleware {
            w.line(format!(
                "{} => self.hooks.middleware_{}(ctx).await,",
                crate::util::lit(m),
                snake(m)
            ));
        }
        w.line("_ => Ok(()),");
        w.close().close().blank();
    }
}

/// Decodes the request, and the facts that step 6 added to `args`; returns the arguments the
/// service method takes before `ctx`.
fn decode_request(w: &mut CodeWriter, ep: &Endpoint) -> &'static str {
    if ep.rules.version.is_some() {
        w.line("let request = veld::server::decode_args(args.clone())?;");
        w.line("let facts = veld::server::decode_args(args.clone())?;");
        if ep.rules.transition.is_some() {
            w.line("let version = veld::server::decode_key(&args, \"version\")?;");
            w.line("let transition = veld::server::decode_key(&args, \"transition\")?;");
            "request, facts, version, transition"
        } else {
            w.line("let version = veld::server::decode_key(&args, \"version\")?;");
            "request, facts, version"
        }
    } else if ep.rules.transition.is_some() {
        w.line("let request = veld::server::decode_args(args.clone())?;");
        w.line("let facts = veld::server::decode_args(args.clone())?;");
        w.line("let transition = veld::server::decode_key(&args, \"transition\")?;");
        "request, facts, transition"
    } else if block::has_facts(ep) {
        w.line("let request = veld::server::decode_args(args.clone())?;");
        w.line("let facts = veld::server::decode_args(args)?;");
        "request, facts"
    } else {
        w.line("let request = veld::server::decode_args(args)?;");
        "request"
    }
}

fn call_methods(w: &mut CodeWriter, tree: &Tree<'_>, svc: &Service) {
    let args = "args: serde_json::Map<String, serde_json::Value>";
    let http: Vec<(usize, &Endpoint)> = svc
        .endpoints
        .iter()
        .enumerate()
        .filter(|(_, e)| endpoints::kind(e) == Kind::Http)
        .collect();
    w.open(format!("async fn call(&self, endpoint: usize, {args}, ctx: &veld::server::RequestContext) -> Result<serde_json::Value, veld::Error>"));
    if http.is_empty() {
        w.line("let _ = (endpoint, &args, ctx);");
        w.line("Err(veld::Error::other(\"not an HTTP endpoint\"))");
        w.close();
        call_sse_method(w, tree, svc, args);
        call_ws_method(w, svc, args);
        return;
    }
    w.open("match endpoint");
    for (i, ep) in &http {
        w.open(format!("{i} =>"));
        let pass = decode_request(w, ep);
        let emits = crate::outbox::emits(ep);
        let pass = if emits {
            format!("{pass}, emit")
        } else {
            pass.to_string()
        };
        if emits {
            crate::outbox::write_begin(w, tree, svc, ep, true);
        }
        if ep.success.ty.is_some() {
            w.line(format!(
                "let out = self.service.{}({pass}, ctx).await?;",
                method(&ep.name)
            ));
            if emits {
                crate::outbox::write_check(w, ep);
            }
            w.line("veld::server::encode_value(&out)");
        } else {
            w.line(format!(
                "self.service.{}({pass}, ctx).await?;",
                method(&ep.name)
            ));
            if emits {
                crate::outbox::write_check(w, ep);
            }
            w.line("Ok(serde_json::Value::Null)");
        }
        w.close();
    }
    w.line("_ => Err(veld::Error::other(\"not an HTTP endpoint\")),");
    w.close().close();
    call_sse_method(w, tree, svc, args);
    call_ws_method(w, svc, args);
}

fn call_sse_method(w: &mut CodeWriter, tree: &Tree<'_>, svc: &Service, args: &str) {
    let sse: Vec<(usize, &Endpoint)> = svc
        .endpoints
        .iter()
        .enumerate()
        .filter(|(_, e)| endpoints::kind(e) == Kind::Sse)
        .collect();
    if sse.is_empty() {
        return;
    }
    w.blank();
    w.open(format!("async fn call_sse(&self, endpoint: usize, {args}, ctx: &veld::server::RequestContext) -> Result<veld::stream::EventStream<serde_json::Value>, veld::Error>"));
    w.open("match endpoint");
    for (i, ep) in &sse {
        w.open(format!("{i} =>"));
        let pass = decode_request(w, ep);
        let pass = if crate::outbox::emits(ep) {
            // A stream is not checked after the handler: it has no single result.
            crate::outbox::write_begin(w, tree, svc, ep, false);
            format!("{pass}, emit")
        } else {
            pass.to_string()
        };
        w.line(format!(
            "let stream = self.service.{}({pass}, ctx).await?;",
            method(&ep.name)
        ));
        w.line("Ok(veld::stream::erase_events(stream))");
        w.close();
    }
    w.line("_ => Err(veld::Error::other(\"not a server-sent-events endpoint\")),");
    w.close().close();
}

fn call_ws_method(w: &mut CodeWriter, svc: &Service, args: &str) {
    let ws: Vec<(usize, &Endpoint)> = svc
        .endpoints
        .iter()
        .enumerate()
        .filter(|(_, e)| endpoints::kind(e) == Kind::Ws)
        .collect();
    if ws.is_empty() {
        return;
    }
    w.blank();
    w.open(format!("async fn call_ws(&self, endpoint: usize, {args}, ctx: &veld::server::RequestContext, io: veld::server::WsIo) -> Result<(), veld::Error>"));
    w.open("match endpoint");
    for (i, ep) in &ws {
        w.open(format!("{i} =>"));
        w.line("let request = veld::server::decode_args(args)?;");
        w.line(format!("let session = veld::server::WsSession::new(io, &ENDPOINTS[{i}], self.hooks.validate_requests(), self.hooks.validate_responses());"));
        w.line(format!(
            "self.service.{}(request, ctx, session).await",
            method(&ep.name)
        ));
        w.close();
    }
    w.line("_ => Err(veld::Error::other(\"not a WebSocket endpoint\")),");
    w.close().close();
}

/// What the entry of a handler has because of the runtime config: the parameter after the
/// loaders, the fallible result, the check and the dispatch member that carries the mode.
pub(crate) struct Entry {
    /// `, runtime: &BookingsRuntime`, or nothing.
    pub param: String,
    /// The text that passes it on (`, runtime`), or nothing.
    pub pass: String,
    /// The check, before the dispatch is built.
    pub check: Option<String>,
    /// `, output: runtime.rule_output()`, or nothing.
    pub member: String,
}

/// The dispatch member that holds the output mode: the config's `rules.output`. A server that has
/// no such setting has no mode (`spec/runtime-config.md` section 5).
fn output_member(tree: &Tree<'_>) -> String {
    if mode_in_runtime(tree) {
        ", output: runtime.rule_output()".to_string()
    } else {
        String::new()
    }
}

impl Entry {
    pub(crate) fn of(tree: &Tree<'_>) -> Entry {
        match &tree.runtime {
            None => Entry {
                param: String::new(),
                pass: String::new(),
                check: None,
                member: output_member(tree),
            },
            Some(rt) => Entry {
                param: format!(", {}", rt.param()),
                pass: ", runtime".to_string(),
                check: Some(rt.check_stmt()),
                member: output_member(tree),
            },
        }
    }

    /// The result type that wraps `ok`.
    pub(crate) fn result(&self, ok: &str) -> String {
        if self.check.is_some() {
            format!("Result<{ok}, {}>", crate::config::RuntimeCfg::error_type())
        } else {
            ok.to_string()
        }
    }

    /// The expression that returns `value`, after the check.
    pub(crate) fn finish(&self, w: &mut CodeWriter, value: &str) {
        match &self.check {
            Some(check) => {
                w.line(check);
                w.line(format!("Ok({value})"));
            }
            None => {
                w.line(value);
            }
        }
    }

    pub(crate) fn docs(&self, w: &mut CodeWriter) {
        if self.check.is_some() {
            w.line("///");
            w.line("/// # Errors");
            w.line("/// [`ConfigError`](crate::runtime_config::ConfigError) with every problem of `runtime`.");
        }
    }
}

fn handler(w: &mut CodeWriter, tree: &Tree<'_>, svc: &Service, n: &Needs) {
    let name = service_type(svc);
    let e = Entry::of(tree);
    w.line(format!(
        "/// Builds the handler of the `{}` service.",
        svc.name
    ));
    w.line("#[derive(Debug, Clone, Copy)]");
    w.line(format!("pub struct {name}Handler;")).blank();
    w.open(format!("impl {name}Handler"));
    let (loaders, loaders_decl, loaders_param, loaders_pass, dispatch_ty, member) =
        if svc.facts.is_empty() {
            (
                false,
                String::new(),
                String::new(),
                String::new(),
                format!("{name}Dispatch<S, H>"),
                String::new(),
            )
        } else {
            let l = block::loaders_name(svc);
            (
                true,
                format!(", L: {l}"),
                ", loaders: L".to_string(),
                ", loaders".to_string(),
                format!("{name}Dispatch<S, H, L>"),
                ", loaders".to_string(),
            )
        };
    if loaders {
        w.line("/// The handler for `service`, with `hooks` and the `loaders` of the facts the endpoint rules");
        w.line("/// need. Mount it with an adapter, for example `crate::adapters::axum::router(handler)`.");
    } else {
        w.line(
            "/// The handler for `service`, with `hooks`. Mount it with an adapter, for example",
        );
        w.line("/// `crate::adapters::axum::router(handler)`.");
    }
    e.docs(w);
    let ret = e.result(&format!("veld::server::Handler<{dispatch_ty}>"));
    w.open(format!(
        "pub fn create<S: {name}Service, H: {name}Hooks{loaders_decl}>(service: S, hooks: H{loaders_param}{}) -> {ret}",
        e.param
    ));
    e.finish(
        w,
        &format!(
            "veld::server::Handler::new({name}Dispatch {{ service, hooks{member}{} }})",
            e.member
        ),
    );
    w.close();
    if !n.any() {
        w.blank();
        if loaders {
            w.line("/// The handler for `service` and `loaders` with the default hooks.");
        } else {
            w.line("/// The handler for `service` with the default hooks.");
        }
        e.docs(w);
        let plain_ret = e.result(&format!(
            "veld::server::Handler<{name}Dispatch<S, NoHooks{}>>",
            if loaders { ", L" } else { "" }
        ));
        let bound = if loaders {
            format!(", L: {}", block::loaders_name(svc))
        } else {
            String::new()
        };
        w.open(format!(
            "pub fn plain<S: {name}Service{bound}>(service: S{loaders_param}{}) -> {plain_ret}",
            e.param
        ));
        w.line(format!(
            "Self::create(service, NoHooks{loaders_pass}{})",
            e.pass
        ));
        w.close();
    }
    w.close().blank();
}

/// The members of the hooks of a service with an `@idempotent` endpoint: the store, and the caller
/// text when a key is scoped to the caller. The types require them, so a server without them does
/// not build (`spec/rules.md` section 11.12).
fn idempotency_hooks(w: &mut CodeWriter, n: &Needs, typed: Option<&str>) {
    w.blank();
    w.line("/// The store of the idempotency keys of the `@idempotent` endpoints. Implement");
    w.line("/// `veld::idempotency::IdempotencyStore` over your database, or use");
    w.line("/// `veld::idempotency::MemoryIdempotencyStore` in tests.");
    w.line("type IdempotencyStore: veld::idempotency::IdempotencyStore;");
    w.blank();
    w.line("/// The store the `@idempotent` endpoints use.");
    w.line("fn idempotency_store(&self) -> &Self::IdempotencyStore;");
    if n.caller {
        let principal = typed.map_or_else(|| "veld::server::Principal".to_string(), str::to_string);
        w.blank();
        w.line("/// The text that tells callers apart: the scope of an idempotency key includes it, so that one");
        w.line("/// caller's key never replays another caller's answer. Two callers must not give the same text.");
        w.line(format!(
            "fn idempotency_caller(&self, principal: &{principal}, ctx: &veld::server::RequestContext) -> String;"
        ));
    }
}

/// `conditional` of a dispatch: what each endpoint owes to `@version` and `@idempotent`.
fn conditional_method(w: &mut CodeWriter, tree: &Tree<'_>, svc: &Service) {
    let arms: Vec<(usize, &Endpoint)> = svc
        .endpoints
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            crate::conditional::serves_version(tree.ir, e) || crate::conditional::is_idempotent(e)
        })
        .collect();
    if arms.is_empty() {
        return;
    }
    w.open("fn conditional(&self, endpoint: usize) -> veld::server::Conditional");
    w.open("match endpoint");
    for (i, ep) in arms {
        let etag = tree.ir.etag_field(ep).map_or("None".to_string(), |f| {
            format!("Some({})", crate::util::lit(&f.wire))
        });
        let mut members = format!("guard: {}, etag: {etag}", crate::conditional::has_guard(ep));
        if tree.idempotency {
            let idem = ep.idempotency.as_ref().map_or("None".to_string(), |i| {
                format!(
                    "Some(veld::server::Idempotent {{ header: {}, required: {}, caller: {} }})",
                    crate::util::lit(&i.header),
                    i.required,
                    i.caller
                )
            });
            members.push_str(&format!(", idempotency: {idem}"));
        }
        w.line(format!("{i} => veld::server::Conditional {{ {members} }},"));
    }
    w.line("_ => veld::server::Conditional::NONE,");
    w.close().close().blank();
}

/// The forwards of a dispatch to the store and the caller hook of the hooks.
fn idempotency_forwards(w: &mut CodeWriter, n: &Needs, typed: Option<&str>) {
    if n.caller {
        w.open("fn idem_caller(&self, ctx: &veld::server::RequestContext) -> String");
        match typed {
            Some(ty) => w.line(format!("ctx.principal::<{ty}>().map_or_else(String::new, |p| self.hooks.idempotency_caller(p, ctx))")),
            None => w.line("ctx.principal.as_ref().map_or_else(String::new, |p| self.hooks.idempotency_caller(p, ctx))"),
        };
        w.close().blank();
    }
    w.open("fn idem_begin<'a>(&'a self, _endpoint: usize, scope: &'a str, key: &'a str, fingerprint: &'a str, _ctx: &'a veld::server::RequestContext) -> impl std::future::Future<Output = Result<veld::idempotency::Begin, veld::Error>> + Send + 'a");
    w.line("veld::idempotency::IdempotencyStore::begin(self.hooks.idempotency_store(), scope, key, fingerprint)");
    w.close().blank();
    w.open("fn idem_complete<'a>(&'a self, scope: &'a str, key: &'a str, answer: veld::idempotency::StoredAnswer) -> impl std::future::Future<Output = Result<(), veld::Error>> + Send + 'a");
    w.line("veld::idempotency::IdempotencyStore::complete(self.hooks.idempotency_store(), scope, key, answer)");
    w.close().blank();
    w.open("fn idem_release<'a>(&'a self, scope: &'a str, key: &'a str) -> impl std::future::Future<Output = Result<(), veld::Error>> + Send + 'a");
    w.line(
        "veld::idempotency::IdempotencyStore::release(self.hooks.idempotency_store(), scope, key)",
    );
    w.close().blank();
}
