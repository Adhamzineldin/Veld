//! `<package>.server`: request records, the `<Service>Service` interface, `<Service>Hooks` and
//! `<Service>Handler`.

use std::cell::RefCell;

use veld_codegen::naming;
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_ir::{AuthKind, Endpoint, Service};

use crate::endpoint;
use crate::frameworks::{EndpointKind, EndpointModel, ServiceModel};
use crate::machine;
use crate::spec::{
    RequestNames, emit_requests, endpoint_docs, endpoint_docs_note, is_sse, is_ws, method_name,
    spec_const, write_spec_const, ws_incoming,
};
use crate::tree::{FileCtx, Tree, java_path};
use crate::types::{codec_expr, java_type};
use crate::util::{add_file, doc_comment, end_call, java_str, member_ident, open_call, unique};

/// Emits the server packages of every package of the tree's services.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<(), Vec<GenDiagnostic>> {
    let names = RequestNames::new(tree.ir, &tree.services);
    for pkg in tree.packages() {
        let package = tree.api_package(pkg, "server");
        for sid in tree
            .services
            .iter()
            .filter(|s| tree.ir.service(**s).package == pkg)
        {
            let svc = tree.ir.service(*sid);
            let refused = endpoint::refusals(svc);
            if !refused.is_empty() {
                return Err(refused);
            }
            emit_requests(files, tree, &package, &names, svc)?;
            emit_facts(files, tree, &package, &names, svc)?;
            let base = naming::pascal(&svc.name);
            let problems = RefCell::new(Vec::new());
            let mut put =
                |suffix: &str, write: &dyn Fn(&mut CodeWriter, &mut FileCtx<'_, '_>, &str)| {
                    let class = format!("{base}{suffix}");
                    let mut fc = FileCtx::new(tree, &package, &class);
                    let mut w = CodeWriter::new();
                    write(&mut w, &mut fc, &class);
                    add_file(files, &java_path(&package, &class), fc.finish(&w.finish()))
                };
            put("Service", &|w, fc, class| {
                write_interface(w, fc, &names, svc, class)
            })?;
            put("Hooks", &|w, fc, class| write_hooks(w, fc, svc, class))?;
            put("Handler", &|w, fc, class| {
                let found = write_handler(w, fc, &names, svc, class, &base);
                problems.borrow_mut().extend(found);
            })?;
            if crate::testing::wanted(tree.testing, svc) {
                put("TestHandler", &|w, fc, class| {
                    let found =
                        crate::testing::write_test_handler(w, fc, &names, svc, class, &base);
                    problems.borrow_mut().extend(found);
                })?;
            }
            if !svc.facts.is_empty() {
                put("FactLoaders", &|w, fc, class| {
                    endpoint::write_loaders(w, fc, svc, class);
                })?;
            }
            let problems = problems.into_inner();
            if !problems.is_empty() {
                return Err(problems);
            }
        }
    }
    Ok(())
}

/// One record of loaded facts per endpoint that has facts.
fn emit_facts(
    files: &mut FileSet,
    tree: &Tree<'_>,
    package: &str,
    names: &RequestNames,
    svc: &Service,
) -> Result<(), Vec<GenDiagnostic>> {
    for ep in svc.endpoints.iter().filter(|e| !e.rules.facts.is_empty()) {
        let request = names.get(svc.id, &ep.name);
        let class = endpoint::facts_name(&request);
        let mut fc = FileCtx::new(tree, package, &class);
        let mut w = CodeWriter::new();
        endpoint::write_facts_record(&mut w, &mut fc, &request, ep);
        add_file(files, &java_path(package, &class), fc.finish(&w.finish()))?;
    }
    Ok(())
}

fn write_interface(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    class: &str,
) {
    let ctx = fc.rt("RequestContext");
    let docs = svc
        .docs
        .clone()
        .unwrap_or_else(|| format!("The implementation of the `{}` service.", svc.name));
    doc_comment(
        w,
        Some(&format!(
            "{docs}\n\nMethods may throw any exception: a declared error is answered at its status, anything else becomes `Internal`."
        )),
        &[],
        None,
    );
    w.line("@SuppressWarnings(\"deprecation\")");
    w.open(format!("public interface {class}"));
    for (i, ep) in svc.endpoints.iter().enumerate() {
        if i > 0 {
            w.blank();
        }
        if ep.rules.transition.is_some() {
            endpoint_docs_note(w, ep, machine::TRANSITION_NOTE);
        } else {
            endpoint_docs(w, ep);
        }
        let request = names.get(svc.id, &ep.name);
        w.line(signature(fc, ep, &request, &ctx));
    }
    w.close();
}

/// The request parameter list of a handler method: the request record, then the facts record
/// when the endpoint has facts, then the `transition` when it has one, then the context.
fn request_params(fc: &mut FileCtx<'_, '_>, ep: &Endpoint, request: &str, ctx: &str) -> String {
    let mut params = vec![format!("{request} request")];
    if !ep.rules.facts.is_empty() {
        params.push(format!("{} facts", endpoint::facts_name(request)));
    }
    if let Some(t) = &ep.rules.transition {
        params.push(format!("{} transition", machine::transition_type(fc, t)));
    }
    params.push(format!("{ctx} ctx"));
    params.join(", ")
}

/// What follows `request` in a call of a handler method: `facts, transition, `, each when the
/// endpoint has it.
pub(crate) fn extra_args(ep: &Endpoint) -> String {
    let mut out = String::new();
    if !ep.rules.facts.is_empty() {
        out.push_str("facts, ");
    }
    if ep.rules.transition.is_some() {
        out.push_str("transition, ");
    }
    out
}

/// One required hook of a service.
struct Hook {
    /// Constructor parameter name.
    param: String,
    /// Parameter type text (an inner type of `HandlerHooks`).
    ty: String,
    /// Statement that registers it on the builder.
    register: String,
    doc: String,
    /// The typed interface this hook has in the hooks class (an auth hook of a service with a
    /// principal), as lines relative to the class body.
    iface: Vec<String>,
}

/// The typed auth hook of a service with a principal: it returns the principal's model.
fn typed_auth(ctx: &str, principal: &str, (setter, name): (&str, &str)) -> Vec<String> {
    let (what, params) = match setter {
        "basic" => (
            "The basic-credentials hook",
            format!("String username, String password, {ctx} ctx"),
        ),
        "apiKey" => ("The API-key hook", format!("String key, {ctx} ctx")),
        _ => ("The bearer-token hook", format!("String token, {ctx} ctx")),
    };
    vec![
        format!("/** {what}: returns the principal (a `{principal}`), or `null` to reject. */"),
        "@FunctionalInterface".to_string(),
        format!("public interface {name} {{"),
        "  /** Authenticates. */".to_string(),
        format!("  {principal} authenticate({params}) throws Exception;"),
        "}".to_string(),
    ]
}

fn auth_kind_name(kind: AuthKind) -> Option<(&'static str, &'static str, &'static str)> {
    match kind {
        AuthKind::None => None,
        AuthKind::Bearer => Some(("bearer", "BearerAuth", "the bearer-token hook")),
        AuthKind::Basic => Some(("basic", "BasicAuth", "the basic-credentials hook")),
        AuthKind::ApiKey => Some(("apiKey", "ApiKeyAuth", "the API-key hook")),
    }
}

/// The hooks the service's endpoints need, in a stable order: auth hooks by name, then
/// middleware in order of first use.
fn required_hooks(fc: &mut FileCtx<'_, '_>, svc: &Service) -> Vec<Hook> {
    let hooks = fc.rt("HandlerHooks");
    let mut auth: Vec<(&'static str, &'static str, &'static str)> = Vec::new();
    let mut middleware: Vec<String> = Vec::new();
    for ep in &svc.endpoints {
        if let Some(a) = auth_kind_name(ep.auth.kind)
            && !auth.contains(&a)
        {
            auth.push(a);
        }
        for m in &ep.middleware {
            if !middleware.contains(m) {
                middleware.push(m.clone());
            }
        }
    }
    auth.sort_unstable();
    let params = unique(
        auth.iter()
            .map(|a| a.0.to_string())
            .chain(middleware.iter().map(|m| member_ident(m)))
            .collect(),
    );
    let principal = match (&svc.principal, fc.tree.policies, auth.is_empty()) {
        (Some(p), true, false) => Some(fc.decl(p.decl)),
        (Some(_) | None, _, _) => None,
    };
    let ctx = match &principal {
        Some(_) => fc.rt("RequestContext"),
        None => String::new(),
    };
    let mut out = Vec::new();
    for (i, (setter, ty, doc)) in auth.iter().enumerate() {
        let hook = match &principal {
            Some(class) => Hook {
                param: params[i].clone(),
                ty: (*ty).to_string(),
                register: format!("builder.{setter}({}::authenticate);", params[i]),
                doc: (*doc).to_string(),
                iface: typed_auth(&ctx, class, (setter, ty)),
            },
            None => Hook {
                param: params[i].clone(),
                ty: format!("{hooks}.{ty}"),
                register: format!("builder.{setter}({});", params[i]),
                doc: (*doc).to_string(),
                iface: Vec::new(),
            },
        };
        out.push(hook);
    }
    for (j, m) in middleware.iter().enumerate() {
        let param = params[auth.len() + j].clone();
        out.push(Hook {
            ty: format!("{hooks}.Middleware"),
            register: format!("builder.middleware({}, {param});", java_str(m)),
            doc: format!("the `{m}` middleware"),
            param,
            iface: Vec::new(),
        });
    }
    out
}

fn write_hooks(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service, class: &str) {
    let hooks = fc.rt("HandlerHooks");
    let required = required_hooks(fc, svc);
    doc_comment(
        w,
        Some(&format!(
            "The hooks of the `{}` service. The constructor takes the hooks its endpoints need.",
            svc.name
        )),
        &[],
        None,
    );
    w.open(format!("public final class {class}"));
    let off = if fc.tree.validate {
        ""
    } else {
        ".validateRequests(false)"
    };
    w.line(format!(
        "private final {hooks}.Builder builder = {hooks}.builder(){off};"
    ));
    let clock = if endpoint::service_has_rules(svc) {
        Some(fc.name("java.time.Clock"))
    } else {
        None
    };
    if let Some(clock) = &clock {
        w.line(format!("private {clock} clock = {clock}.systemUTC();"));
    }
    w.blank();
    for h in required.iter().filter(|h| !h.iface.is_empty()) {
        for line in &h.iface {
            w.line(line.clone());
        }
        w.blank();
    }
    let params: Vec<(String, String)> = required
        .iter()
        .map(|h| (h.param.clone(), h.doc.clone()))
        .collect();
    doc_comment(w, Some("Creates the hooks."), &params, None);
    let sig: Vec<String> = required
        .iter()
        .map(|h| format!("{} {}", h.ty, h.param))
        .collect();
    w.open(format!("public {class}({})", sig.join(", ")));
    for h in &required {
        w.line(h.register.clone());
    }
    w.close();
    w.blank();
    w.line(
        "/** Observes errors that become `Internal`; a failing hook never changes the response. */",
    );
    w.open(format!("public {class} onError({hooks}.ErrorHook hook)"));
    w.line("builder.onError(hook);");
    w.line("return this;");
    w.close();
    w.blank();
    w.line("/** When true, a response that fails validation becomes a 500 (default false). */");
    w.open(format!("public {class} validateResponses(boolean value)"));
    w.line("builder.validateResponses(value);");
    w.line("return this;");
    w.close();
    w.blank();
    if fc.tree.rules {
        write_rule_hooks(w, fc, class);
    }
    if let Some(clock) = &clock {
        w.line("/** The clock `now` reads in endpoint rules, once per request (default: the system clock). */");
        w.open(format!("public {class} clock({clock} value)"));
        w.line("this.clock = value;");
        w.line("return this;");
        w.close();
        w.blank();
        w.open(format!("{clock} clock()"));
        w.line("return clock;");
        w.close();
        w.blank();
    }
    w.open(format!("{hooks} build()"));
    w.line("return builder.build();");
    w.close();
    w.close();
}

/// The rule options of the hooks: the output mode and the violation hook (`spec/rules.md` 5.3).
fn write_rule_hooks(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, class: &str) {
    let walk = fc.rt("RuleWalk");
    w.line("/** What happens when a rule is violated by this server's own output: `STRICT` (default) answers `500 Internal`, `AUDIT` sends the response and reports, `OFF` skips the check. */");
    w.open(format!(
        "public {class} rulesOutput({walk}.OutputMode mode)"
    ));
    w.line("builder.rulesOutput(mode);");
    w.line("return this;");
    w.close();
    w.blank();
    w.line("/** Observes every violated rule; a failing hook never changes the response. */");
    w.open(format!("public {class} onRuleViolation({walk}.Hook hook)"));
    w.line("builder.onRuleViolation(hook);");
    w.line("return this;");
    w.close();
    w.blank();
}

fn write_handler(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    class: &str,
    base: &str,
) -> Vec<GenDiagnostic> {
    let handler = fc.rt("VeldHandler");
    let endpoint = fc.rt("ServerEndpoint");
    let list = fc.name("java.util.List");
    let mut problems = Vec::new();
    doc_comment(
        w,
        Some(&format!(
            "Builds the HTTP handler for `{}`. Mount it with an adapter.",
            svc.name
        )),
        &[],
        None,
    );
    w.line("@SuppressWarnings(\"deprecation\")");
    w.open(format!("public final class {class}"));
    w.line(format!("private {class}() {{}}"));
    for ep in &svc.endpoints {
        w.blank();
        write_spec_const(w, fc, ep);
    }
    w.blank();
    let needs_hooks = needs_hooks(svc);
    let hooks_class = format!("{base}Hooks");
    let service = format!("{base}Service");
    let rules = endpoint::service_has_rules(svc);
    let loaders = (!svc.facts.is_empty()).then(|| endpoint::loaders_name(base));
    let (given, pass) = match &loaders {
        Some(l) => (format!("{service} impl, {l} loaders"), "impl, loaders"),
        None => (format!("{service} impl"), "impl"),
    };
    if !needs_hooks {
        w.line("/** Builds the handler with default hooks. */");
        w.open(format!("public static {handler} create({given})"));
        w.line(format!("return create({pass}, new {hooks_class}());"));
        w.close();
        w.blank();
    }
    w.line("/** Builds the handler. */");
    w.open(format!(
        "public static {handler} create({given}, {hooks_class} hooks)"
    ));
    let site = EndpointsSite {
        names,
        svc,
        endpoint: &endpoint,
        list: &list,
    };
    if rules {
        write_create_with_rules(w, fc, &site, pass);
        w.blank();
        write_endpoints_method(w, fc, &site, &given, &mut problems);
    } else {
        problems.extend(write_create_plain(w, fc, &site));
    }
    w.close();
    problems
}

/// The body of `create` for a service whose endpoints have rules: the clock and the hooks are
/// read once and handed to `endpoints(...)`, which the test handler calls too.
fn write_create_with_rules(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    site: &EndpointsSite<'_>,
    pass: &str,
) {
    let (built, clock) = (fc.rt("HandlerHooks"), fc.name("java.time.Clock"));
    let (router, list) = (fc.rt("Router"), site.list);
    w.line(format!("{built} built = hooks.build();"));
    w.line(format!("{clock} clock = hooks.clock();"));
    w.line(format!(
        "{list}<{}> endpoints = endpoints({pass}, built, ctx -> clock.millis());",
        site.endpoint
    ));
    let std = std_sigs(fc);
    let create = format!("{router}.create(endpoints, {std}, built)");
    w.line(format!("return {};", wrap_digest(fc, site.svc, &create)));
    w.close();
}

/// The body of `create` for a service without endpoint rules.
fn write_create_plain(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    site: &EndpointsSite<'_>,
) -> Vec<GenDiagnostic> {
    let (router, list, svc) = (fc.rt("Router"), site.list, site.svc);
    let mut problems = Vec::new();
    open_call(
        w,
        format!("{list}<{}> endpoints = {list}.of", site.endpoint),
    );
    for (i, ep) in svc.endpoints.iter().enumerate() {
        let last = i + 1 == svc.endpoints.len();
        problems.extend(write_endpoint(w, fc, site.names, svc, ep, last));
    }
    end_call(w);
    let std = std_sigs(fc);
    let create = format!("{router}.create(endpoints, {std}, hooks.build())");
    w.line(format!("return {};", wrap_digest(fc, svc, &create)));
    w.close();
    problems
}

/// What the method that lists the endpoints of a service needs.
struct EndpointsSite<'a> {
    names: &'a RequestNames,
    svc: &'a Service,
    endpoint: &'a str,
    list: &'a str,
}

/// `return DigestHandler.wrap(create, "digest")` for a service that has a rules digest.
fn wrap_digest(fc: &mut FileCtx<'_, '_>, svc: &Service, create: &str) -> String {
    match (&svc.rules_digest, fc.tree.rules) {
        (Some(digest), true) => {
            let wrapper = fc.rt("DigestHandler");
            format!("{wrapper}.wrap({create}, {})", java_str(digest))
        }
        (Some(_) | None, _) => create.to_string(),
    }
}

/// `static List<ServerEndpoint> endpoints(impl, loaders, hooks, clock)`: the endpoints of a
/// service with rules. Package-private: the test handler builds the same endpoints with loaders
/// and a clock that read the request.
fn write_endpoints_method(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    site: &EndpointsSite<'_>,
    given: &str,
    problems: &mut Vec<GenDiagnostic>,
) {
    let (hooks, ctx, to_long) = (
        fc.rt("HandlerHooks"),
        fc.rt("RequestContext"),
        fc.name("java.util.function.ToLongFunction"),
    );
    w.line("/** The endpoints of the service; the test handler reuses them. */");
    w.open(format!(
        "static {}<{}> endpoints({given}, {hooks} hooks, {to_long}<{ctx}> clock)",
        site.list, site.endpoint
    ));
    open_call(w, format!("return {}.of", site.list));
    let svc = site.svc;
    for (i, ep) in svc.endpoints.iter().enumerate() {
        problems.extend(write_endpoint(
            w,
            fc,
            site.names,
            svc,
            ep,
            i + 1 == svc.endpoints.len(),
        ));
    }
    end_call(w);
    w.close();
}

fn write_endpoint(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    ep: &Endpoint,
    last: bool,
) -> Vec<GenDiagnostic> {
    let mut problems = Vec::new();
    let endpoint = fc.rt("ServerEndpoint");
    let list = fc.name("java.util.List");
    let request = names.get(svc.id, &ep.name);
    let middleware: Vec<String> = ep.middleware.iter().map(|m| java_str(m)).collect();
    let end = if last { ");" } else { "," };
    let method = method_name(ep);
    let call = format!("impl.{method}({request}.fromWire(params), ctx)");
    let head = if is_ws(ep) {
        format!("{endpoint}.ws")
    } else if is_sse(ep) {
        format!("{endpoint}.sse")
    } else {
        format!("new {endpoint}")
    };
    open_call(w, head);
    w.line(format!("{},", spec_const(ep)));
    w.line(format!("{list}.of({}),", middleware.join(", ")));
    if is_ws(ep) {
        write_ws_call(
            w,
            fc,
            ep,
            &format!("impl.{method}({request}.fromWire(params), ctx, "),
            end,
        );
    } else if is_sse(ep) {
        let (encode, codec) = (
            fc.rt("SseEvent"),
            ep.success.ty.as_ref().map(|t| codec_expr(fc, t)),
        );
        let codec = codec.unwrap_or_else(|| "null".to_string());
        w.line(format!(
            "(params, ctx) -> {encode}.encode({call}, {codec})){end}"
        ));
    } else if endpoint::endpoint_has_rules(ep) {
        let site = endpoint::Site {
            svc,
            ep,
            request: &request,
        };
        let call = format!("impl.{method}(request, {}ctx)", extra_args(ep));
        let encode = ep
            .success
            .ty
            .as_ref()
            .map(|t| format!("{}.encode(result)", codec_expr(fc, t)));
        w.open_with("(params, ctx) ->", " {");
        problems.extend(endpoint::write_call_body(
            w,
            fc,
            &site,
            &call,
            encode.as_deref(),
        ));
        w.close_with(format!("}}){end}"));
    } else {
        match &ep.success.ty {
            Some(t) => {
                let c = codec_expr(fc, t);
                w.line(format!("(params, ctx) -> {c}.encode({call})){end}"));
            }
            None => {
                w.open_with("(params, ctx) ->", " {");
                w.line(format!("{call};"));
                w.line("return null;");
                w.close_with(format!("}}){end}"));
            }
        }
    }
    end_call(w);
    problems
}

/// The lambda of a `ws` endpoint: the router's raw session is wrapped with the message codecs.
fn write_ws_call(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    ep: &Endpoint,
    call: &str,
    end: &str,
) {
    let session = fc.rt("WsSession");
    let incoming = ws_incoming(ep)
        .map(|p| codec_expr(fc, &p.ty))
        .unwrap_or_else(|| "null".to_string());
    let outgoing = ep
        .success
        .ty
        .as_ref()
        .map(|t| codec_expr(fc, t))
        .unwrap_or_else(|| "null".to_string());
    w.line(format!(
        "(params, ctx, ws) -> {call}{session}.adapt(ws, {incoming}, {outgoing}))){end}"
    ));
}

pub(crate) fn std_sigs(fc: &mut FileCtx<'_, '_>) -> String {
    let router = fc.rt("Router");
    let mut sig = |name: &str| match fc.tree.prelude_decl(name) {
        Some(id) => format!("{}.VELD_SIG", fc.decl(id)),
        None => "\"about:blank\"".to_string(),
    };
    let (u, v, i) = (sig("Unauthorized"), sig("ValidationError"), sig("Internal"));
    format!("new {router}.StdSigs({u}, {v}, {i})")
}

/// Whether any endpoint of the service needs an auth or middleware hook.
pub(crate) fn needs_hooks(svc: &Service) -> bool {
    svc.endpoints
        .iter()
        .any(|e| auth_kind_name(e.auth.kind).is_some() || !e.middleware.is_empty())
}

/// The services of the tree as the framework adapters see them, in tree order.
#[must_use]
pub fn service_models(tree: &Tree<'_>) -> Vec<ServiceModel> {
    let mut out = Vec::new();
    for sid in &tree.services {
        let svc = tree.ir.service(*sid);
        let pkg = svc.package;
        let dir = tree
            .pkg_package(pkg)
            .strip_prefix(&format!("{}.", tree.base))
            .unwrap_or_default()
            .to_string();
        out.push(ServiceModel {
            name: naming::pascal(&svc.name),
            server_package: tree.api_package(pkg, "server"),
            adapter_package: format!("{}.adapters.{dir}", tree.base),
            needs_hooks: needs_hooks(svc),
            loaders: (!svc.facts.is_empty())
                .then(|| endpoint::loaders_name(&naming::pascal(&svc.name))),
            endpoints: svc
                .endpoints
                .iter()
                .map(|ep| EndpointModel {
                    name: ep.name.clone(),
                    method_name: method_name(ep),
                    http_method: ep.method.as_str(),
                    path: ep.path.clone(),
                    kind: if is_ws(ep) {
                        EndpointKind::Ws
                    } else if is_sse(ep) {
                        EndpointKind::Sse
                    } else {
                        EndpointKind::Http
                    },
                })
                .collect(),
        });
    }
    out
}

/// The message types of a `ws` endpoint as Java text: (from the client, to the client); `Void`
/// for a side that has none.
fn ws_types(fc: &mut FileCtx<'_, '_>, ep: &Endpoint) -> (String, String) {
    let incoming = match ws_incoming(ep) {
        Some(p) => java_type(fc, &p.ty, true),
        None => "Void".to_string(),
    };
    let outgoing = match &ep.success.ty {
        Some(t) => java_type(fc, t, true),
        None => "Void".to_string(),
    };
    (incoming, outgoing)
}

/// The declaration of an endpoint's method in the `<Service>Service` interface.
pub(crate) fn signature(
    fc: &mut FileCtx<'_, '_>,
    ep: &Endpoint,
    request: &str,
    ctx: &str,
) -> String {
    let method = method_name(ep);
    if is_ws(ep) {
        let (incoming, outgoing) = ws_types(fc, ep);
        let session = fc.rt("WsSession");
        return format!(
            "void {method}({request} request, {ctx} ctx, {session}<{incoming}, {outgoing}> ws) throws Exception;"
        );
    }
    let ret = match &ep.success.ty {
        Some(t) if is_sse(ep) => {
            let (stream, event) = (fc.name("java.util.stream.Stream"), fc.rt("SseEvent"));
            format!("{stream}<{event}<{}>>", java_type(fc, t, true))
        }
        Some(t) => java_type(fc, t, true),
        None => "void".to_string(),
    };
    let params = request_params(fc, ep, request, ctx);
    format!("{ret} {method}({params}) throws Exception;")
}
