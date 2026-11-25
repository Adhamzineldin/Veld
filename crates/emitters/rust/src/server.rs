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
}

pub(crate) fn needs(svc: &Service) -> Needs {
    let mut n = Needs {
        bearer: false,
        basic: false,
        api_key: false,
        middleware: Vec::new(),
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
        self.bearer || self.basic || self.api_key || !self.middleware.is_empty()
    }
}

/// Emits `<pkg>/<service>_server.rs` for every service of the tree.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<(), Vec<GenDiagnostic>> {
    for sid in &tree.services {
        let svc = tree.ir.service(*sid);
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
    let n = needs(svc);
    let typed = typed_principal(ctx, svc);
    hooks_trait(w, svc, &n, ctx.tree.validate, ctx.tree.rules, typed.as_deref());
    if !svc.facts.is_empty() {
        block::loader_trait(w, ctx, svc);
    }
    block::steps(w, ctx, svc)?;
    dispatch(w, ctx.tree, svc, &n, typed.as_deref());
    handler(w, svc, &n);
    Ok(())
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
        if endpoints::kind(ep) == Kind::Ws {
            let (incoming, outgoing) = endpoints::ws_types(ctx, ep);
            w.line(format!("fn {}(&self, request: {}, ctx: {}, ws: veld::server::WsSession<{incoming}, {outgoing}>) -> impl std::future::Future<Output = Result<(), veld::Error>> + Send;", method(&ep.name), request_name(ep), context_type()));
        } else {
            w.line(format!("fn {}(&self, request: {}{}{}, ctx: {}) -> impl std::future::Future<Output = Result<{}, veld::Error>> + Send;", method(&ep.name), request_name(ep), block::facts_param(ep), block::transition_param(ctx, ep), context_type(), success_type(ctx, ep)));
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
    rules: bool,
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
        w.line(
            "/// What happens when the result of an endpoint violates an invariant: `Strict` (the",
        );
        w.line("/// default) answers `500 Internal`, `Audit` sends it and reports, `Off` does not check.");
        w.open("fn rules_output(&self) -> veld::rules::RuleOutput");
        w.line("veld::rules::RuleOutput::Strict");
        w.close().blank();
        w.line("/// Observes every violated invariant of an input (rejected) or an output (rejected or");
        w.line("/// audited). It cannot change the response.");
        w.line("fn on_rule_violation(&self, _event: &veld::rules::RuleEvent) {}");
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
    let vis = if block::has_block(svc) {
        "pub(crate) "
    } else {
        ""
    };
    w.line(format!("{vis}service: S,"));
    w.line(format!("{vis}hooks: H,"));
    if loaders {
        w.line(format!("{vis}loaders: L,"));
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
        block_methods(w, svc);
    }
    auth_forwards(w, n, typed);
    call_methods(w, svc);
    w.close().blank();
}

/// The rule hooks of a dispatch, the input reach and the digest.
fn rules_forwards(w: &mut CodeWriter, tree: &Tree<'_>, svc: &Service) {
    w.open("fn rules_output(&self) -> veld::rules::RuleOutput");
    w.line("self.hooks.rules_output()");
    w.close().blank();
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

/// `prepare` and `ensure` of a dispatch whose service has endpoint rules.
fn block_methods(w: &mut CodeWriter, svc: &Service) {
    let loaders = !svc.facts.is_empty();
    let pass = if loaders { "&self.loaders, " } else { "" };
    w.open("async fn prepare(&self, endpoint: usize, args: &mut serde_json::Map<String, serde_json::Value>, ctx: &veld::server::RequestContext) -> Result<veld::server::Prepared, veld::server::Halt>");
    w.line("let enforce = self.hooks.validate_requests();");
    w.line("let now = veld::rules::clock_ms(self.hooks.clock(ctx));");
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
}

/// The principal type of a service whose hooks are typed: it has a principal and some endpoint
/// reads it or calls a policy (`spec/rules.md` section 11.1). A principal that nothing reads
/// changes no bytes.
pub(crate) fn typed_principal(ctx: &Ctx<'_, '_>, svc: &Service) -> Option<String> {
    if svc
        .endpoints
        .iter()
        .any(|e| e.rules.first_policy_origin().is_some())
    {
        crate::guards::principal_type(ctx, svc)
    } else {
        None
    }
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
    if ep.rules.transition.is_some() {
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

fn call_methods(w: &mut CodeWriter, svc: &Service) {
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
        call_sse_method(w, svc, args);
        call_ws_method(w, svc, args);
        return;
    }
    w.open("match endpoint");
    for (i, ep) in &http {
        w.open(format!("{i} =>"));
        let pass = decode_request(w, ep);
        if ep.success.ty.is_some() {
            w.line(format!(
                "let out = self.service.{}({pass}, ctx).await?;",
                method(&ep.name)
            ));
            w.line("veld::server::encode_value(&out)");
        } else {
            w.line(format!(
                "self.service.{}({pass}, ctx).await?;",
                method(&ep.name)
            ));
            w.line("Ok(serde_json::Value::Null)");
        }
        w.close();
    }
    w.line("_ => Err(veld::Error::other(\"not an HTTP endpoint\")),");
    w.close().close();
    call_sse_method(w, svc, args);
    call_ws_method(w, svc, args);
}

fn call_sse_method(w: &mut CodeWriter, svc: &Service, args: &str) {
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

fn handler(w: &mut CodeWriter, svc: &Service, n: &Needs) {
    let name = service_type(svc);
    w.line(format!(
        "/// Builds the handler of the `{}` service.",
        svc.name
    ));
    w.line("#[derive(Debug, Clone, Copy)]");
    w.line(format!("pub struct {name}Handler;")).blank();
    w.open(format!("impl {name}Handler"));
    if svc.facts.is_empty() {
        w.line(
            "/// The handler for `service`, with `hooks`. Mount it with an adapter, for example",
        );
        w.line("/// `crate::adapters::axum::router(handler)`.");
        w.open(format!(
            "pub fn create<S: {name}Service, H: {name}Hooks>(service: S, hooks: H) -> veld::server::Handler<{name}Dispatch<S, H>>"
        ));
        w.line(format!(
            "veld::server::Handler::new({name}Dispatch {{ service, hooks }})"
        ));
        w.close();
        if !n.any() {
            w.blank();
            w.line("/// The handler for `service` with the default hooks.");
            w.open(format!("pub fn plain<S: {name}Service>(service: S) -> veld::server::Handler<{name}Dispatch<S, NoHooks>>"));
            w.line("Self::create(service, NoHooks)");
            w.close();
        }
    } else {
        let loaders = block::loaders_name(svc);
        w.line("/// The handler for `service`, with `hooks` and the `loaders` of the facts the endpoint rules");
        w.line("/// need. Mount it with an adapter, for example `crate::adapters::axum::router(handler)`.");
        w.open(format!(
            "pub fn create<S: {name}Service, H: {name}Hooks, L: {loaders}>(service: S, hooks: H, loaders: L) -> veld::server::Handler<{name}Dispatch<S, H, L>>"
        ));
        w.line(format!(
            "veld::server::Handler::new({name}Dispatch {{ service, hooks, loaders }})"
        ));
        w.close();
        if !n.any() {
            w.blank();
            w.line("/// The handler for `service` and `loaders` with the default hooks.");
            w.open(format!("pub fn plain<S: {name}Service, L: {loaders}>(service: S, loaders: L) -> veld::server::Handler<{name}Dispatch<S, NoHooks, L>>"));
            w.line("Self::create(service, NoHooks, loaders)");
            w.close();
        }
    }
    w.close().blank();
}
