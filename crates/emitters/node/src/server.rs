//! `server/<service>.ts`: request types, the `<Service>Service` interface, hooks and `create<Service>Handler`.

use veld_codegen::naming;
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_emit_typescript::client::{RequestNames, write_request_type, write_spec_fields};
use veld_emit_typescript::tree::{FileCtx, Tree};
use veld_emit_typescript::types::ts_type;
use veld_emit_typescript::util::{add_file, doc_comment, method_key, prop_key, ts_str};
use veld_ir::{AuthKind, Endpoint, Method, Service};

use crate::events;
use crate::frameworks::{EndpointModel, ServiceModel};
use crate::guards;
use crate::rules::{self, Site};

/// Root-relative module path of a service's server file.
pub(crate) fn module_of(tree: &Tree<'_>, svc: &Service) -> String {
    format!("{}server/{}", tree.root, naming::kebab(&svc.name))
}

/// Emits one server module per service and returns their module paths.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<Vec<String>, Vec<GenDiagnostic>> {
    let names = RequestNames::new(tree.ir, &tree.services);
    let mut modules = Vec::new();
    let mut diags = Vec::new();
    for sid in &tree.services {
        let svc = tree.ir.service(*sid);
        // A service that only publishes or subscribes has no HTTP surface: its module is the
        // events module (`events.rs`).
        if !events::serves_http(svc) {
            continue;
        }
        let module = module_of(tree, svc);
        let mut fc = FileCtx::new(tree, &module);
        let mut w = CodeWriter::new();
        write_service(&mut w, &mut fc, &names, svc, &mut diags);
        add_file(files, &format!("{module}.ts"), fc.finish(&w.finish()))?;
        modules.push(module);
    }
    if diags.is_empty() {
        Ok(modules)
    } else {
        Err(diags)
    }
}

fn write_service(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    diags: &mut Vec<GenDiagnostic>,
) {
    for ep in &svc.endpoints {
        veld_emit_typescript::version::reserved_names(svc, ep, diags);
        let request = names.get(svc.id, &ep.name);
        write_request_type(w, fc, &request, ep);
        w.blank();
        rules::write_facts_type(w, fc, &request, ep);
        guards::write_version_type(w, fc, &request, ep);
        guards::write_transition_type(w, fc, &request, ep);
    }
    write_interface(w, fc, names, svc);
    w.blank();
    if !svc.facts.is_empty() {
        rules::write_loaders(w, fc, svc, &naming::pascal(&svc.name));
        w.blank();
    }
    write_hooks(w, fc, svc);
    w.blank();
    write_factory(w, fc, names, svc, diags);
}

/// The request type of an endpoint as the implementation receives it: with its loaded facts.
fn impl_request(ep: &Endpoint, request: &str) -> String {
    let mut parts = vec![request.to_string()];
    if !ep.rules.facts.is_empty() {
        parts.push(rules::facts_name(request));
    }
    if ep.rules.version.is_some() {
        parts.push(guards::version_name(request));
    }
    if ep.rules.transition.is_some() {
        parts.push(guards::transition_name(request));
    }
    parts.join(" & ")
}

/// Name of the function that builds the endpoint descriptions of a service with endpoint rules.
pub(crate) fn builder_name(svc: &Service) -> String {
    format!("{}Endpoints", naming::camel(&svc.name))
}

fn write_interface(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
) {
    let name = naming::pascal(&svc.name);
    let ctx = fc.rt("RequestContext");
    doc_comment(w, svc.docs.as_deref(), None);
    w.open(format!("export interface {name}Service<Ctx = {ctx}>"));
    for ep in &svc.endpoints {
        doc_comment(w, ep.docs.as_deref(), ep.deprecated.as_deref());
        let ret = match &ep.success.ty {
            Some(t) => ts_type(fc, t),
            None => "void".to_string(),
        };
        let key = method_key(&ep.name);
        let request = names.get(svc.id, &ep.name);
        let req = impl_request(ep, &request);
        if ep.method == Method::Ws {
            let (incoming, outgoing) = ws_types(fc, ep);
            let session = fc.rt("WsSession");
            w.line(format!(
                "{key}(req: {req}, ctx: Ctx, ws: {session}<{incoming}, {outgoing}>): Promise<void>;"
            ));
        } else if ep.success.stream {
            let event = fc.rt("SseEvent");
            let item = format!("{ret} | {event}<{ret}>");
            w.line(format!(
                "{key}(req: {req}, ctx: Ctx): AsyncIterable<{item}> | Promise<AsyncIterable<{item}>>;"
            ));
        } else {
            w.line(format!("{key}(req: {req}, ctx: Ctx): Promise<{ret}>;"));
        }
    }
    w.close();
}

/// The message types of a `ws` endpoint: (from the client, to the client); `never` for a side
/// that has none.
fn ws_types(fc: &mut FileCtx<'_, '_>, ep: &Endpoint) -> (String, String) {
    let incoming = match ep.params.iter().find(|p| p.stream) {
        Some(p) => ts_type(fc, &p.ty),
        None => "never".to_string(),
    };
    let outgoing = match &ep.success.ty {
        Some(t) => ts_type(fc, t),
        None => "never".to_string(),
    };
    (incoming, outgoing)
}

/// The auth hook of a kind. `who` is what the hook resolves to: `unknown`, or the generated
/// type of the principal of a service that has one (`spec/rules.md` section 11.1).
fn auth_hook(kind: AuthKind, ctx: &str, who: &str) -> Option<(&'static str, String)> {
    match kind {
        AuthKind::None => None,
        AuthKind::Bearer => Some((
            "bearer",
            format!("(token: string, ctx: {ctx}) => Promise<{who} | null>"),
        )),
        AuthKind::Basic => Some((
            "basic",
            format!("(username: string, password: string, ctx: {ctx}) => Promise<{who} | null>"),
        )),
        AuthKind::ApiKey => Some((
            "apiKey",
            format!("(key: string, ctx: {ctx}) => Promise<{who} | null>"),
        )),
    }
}

/// Auth hooks and middleware names the service's endpoints need, in a stable order.
fn required_hooks(
    svc: &Service,
    ctx: &str,
    who: &str,
) -> (Vec<(&'static str, String)>, Vec<String>) {
    let mut auth: Vec<(&'static str, String)> = Vec::new();
    let mut middleware: Vec<String> = Vec::new();
    for ep in &svc.endpoints {
        if let Some(h) = auth_hook(ep.auth.kind, ctx, who)
            && !auth.iter().any(|(n, _)| *n == h.0)
        {
            auth.push(h);
        }
        for m in &ep.middleware {
            if !middleware.contains(m) {
                middleware.push(m.clone());
            }
        }
    }
    auth.sort_by_key(|(n, _)| *n);
    (auth, middleware)
}

fn write_hooks(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service) {
    let ctx = fc.rt("RequestContext");
    // A service with a principal resolves its auth hooks to the principal's model, when a
    // statement reads the caller (`spec/rules.md` section 11.1).
    let who = match (&svc.principal, fc.tree.uses_policies()) {
        (Some(p), true) => ts_type(fc, &veld_ir::Type::Decl(p.decl)),
        _ => "unknown".to_string(),
    };
    let (auth, middleware) = required_hooks(svc, &ctx, &who);
    w.open(format!(
        "export interface {}Hooks",
        naming::pascal(&svc.name)
    ));
    if !svc.facts.is_empty() {
        w.line("/** The loaders of the facts the endpoint rules need. */");
        w.line(format!(
            "facts: {};",
            rules::loaders_name(&naming::pascal(&svc.name))
        ));
    }
    if rules::service_has_rules(svc) {
        w.line("/** The server's clock, read once per request before the facts load. Default: the system clock. */");
        w.line(format!("clock?: (ctx: {ctx}) => Date;"));
    }
    write_idempotency_hook(w, fc, svc, (&ctx, &who));
    if !auth.is_empty() {
        w.open("auth:");
        for (name, ty) in &auth {
            w.line(format!("{name}: {ty};"));
        }
        w.close_with("};");
    }
    if !middleware.is_empty() {
        w.open("middleware:");
        for name in &middleware {
            w.line(format!(
                "{}: (ctx: {ctx}) => Promise<void>;",
                prop_key(name)
            ));
        }
        w.close_with("};");
    }
    w.line(format!("onError?: (err: unknown, ctx: {ctx}) => void;"));
    w.line("validateResponses?: boolean;");
    if fc.tree.uses_rules() {
        let mode = fc.rt("RuleOutputMode");
        let event = fc.rt("RuleViolationEvent");
        w.line("/** How rules on this server's own output are enforced (default `strict`). */");
        w.line(format!("rules?: {{ output?: {mode} }};"));
        w.line(format!("onRuleViolation?: (event: {event}) => void;"));
    }
    if !fc.tree.validate {
        w.line("/** This target was generated with `validate = false`: requests are not validated unless this is true. */");
        w.line("validateRequests?: boolean;");
    }
    w.close();
}

/// `idempotency: { store, caller }` in the hooks of a service with an `@idempotent` endpoint
/// (`spec/rules.md` section 11.12). The type requires what the endpoints need, so a server that
/// lacks it does not compile; the runtime checks it again when the handler is created.
fn write_idempotency_hook(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    svc: &Service,
    (ctx, who): (&str, &str),
) {
    if !rules::service_is_idempotent(svc) {
        return;
    }
    let store = fc.rt("IdempotencyStore");
    w.line("/** Where the keys of the `@idempotent` endpoints live, and who the caller is for the scope of a key. */");
    let caller = svc
        .endpoints
        .iter()
        .any(|ep| ep.idempotency.as_ref().is_some_and(|i| i.caller));
    if caller {
        w.line(format!(
            "idempotency: {{ store: {store}; caller: (principal: {who}, ctx: {ctx}) => string | Promise<string> }};"
        ));
    } else {
        w.line(format!("idempotency: {{ store: {store} }};"));
    }
}

/// Where the writers of one service put their output: the body and the declarations that must
/// come before it.
struct Out<'a> {
    w: &'a mut CodeWriter,
    pre: &'a mut CodeWriter,
    diags: &'a mut Vec<GenDiagnostic>,
}

fn write_endpoint(
    out: &mut Out<'_>,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    ep: &Endpoint,
) {
    let w = &mut *out.w;
    w.open_with("", "{");
    write_spec_fields(w, fc, ep);
    let list: Vec<String> = ep.middleware.iter().map(|m| ts_str(m)).collect();
    w.line(format!("middleware: [{}],", list.join(", ")));
    let request = names.get(svc.id, &ep.name);
    let req = impl_request(ep, &request);
    if ep.method == Method::Ws {
        let (incoming, outgoing) = ws_types(fc, ep);
        let session = fc.rt("WsSession");
        w.line(format!(
            "call: (req, ctx, ws) => impl.{}(req as {req}, ctx, ws as {session}<{incoming}, {outgoing}>),",
            method_key(&ep.name)
        ));
    } else {
        w.line(format!(
            "call: (req, ctx) => impl.{}(req as {req}, ctx),",
            method_key(&ep.name)
        ));
    }
    if rules::endpoint_has_rules(ep) {
        if ep.method == Method::Ws {
            let mut d = GenDiagnostic::new(
                "E0801",
                format!(
                    "{}.{}: endpoint rules on a `ws` endpoint are not supported by the `node` generator",
                    svc.name, ep.name
                ),
            );
            d.origin = ep.rules.first_origin().cloned();
            out.diags.push(d);
        }
        let site = Site {
            svc,
            ep,
            request: &request,
        };
        rules::write_rules_member(w, out.pre, fc, &site, out.diags);
    }
    if let Some(digest) = svc.rules_digest.as_deref() {
        w.line(format!("digest: {},", ts_str(digest)));
    }
    w.close_with("},");
}

/// `export function bookingsEndpoints(impl, loaders): ServerEndpoint[]`: the endpoint
/// descriptions of a service with endpoint rules. The handler factory and the test handler build
/// on it.
fn write_builder(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    diags: &mut Vec<GenDiagnostic>,
) {
    let name = naming::pascal(&svc.name);
    let loaders = if svc.facts.is_empty() {
        String::new()
    } else {
        format!(", loaders: {}", rules::loaders_name(&name))
    };
    let endpoint = fc.rt("ServerEndpoint");
    let mut body = CodeWriter::new();
    doc_comment(
        &mut body,
        Some(&format!(
            "The endpoint descriptions of `{}`, with the facts, `require`, `old` and `ensure` of their blocks.",
            svc.name
        )),
        None,
    );
    body.open(format!(
        "export function {}(impl: {name}Service{loaders}): {endpoint}[]",
        builder_name(svc)
    ));
    body.open_with("return", " [");
    let mut pre = CodeWriter::new();
    let mut inner = Out {
        w: &mut body,
        pre: &mut pre,
        diags,
    };
    for ep in &svc.endpoints {
        write_endpoint(&mut inner, fc, names, svc, ep);
    }
    body.close_with("];");
    body.close();
    let (pre, body) = (pre.finish(), body.finish());
    for line in pre.lines() {
        w.line(line);
    }
    if !pre.is_empty() {
        w.blank();
    }
    for line in body.lines() {
        w.line(line);
    }
    w.blank();
}

fn write_factory(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    diags: &mut Vec<GenDiagnostic>,
) {
    let name = naming::pascal(&svc.name);
    let ctx = fc.rt("RequestContext");
    let (auth, middleware) = required_hooks(svc, &ctx, "unknown");
    let default = if auth.is_empty()
        && middleware.is_empty()
        && svc.facts.is_empty()
        && !rules::service_is_idempotent(svc)
    {
        " = {}"
    } else {
        ""
    };
    let ruled = rules::service_has_builder(svc);
    if ruled {
        write_builder(w, fc, names, svc, diags);
    }
    let handler = fc.rt("VeldHandler");
    doc_comment(
        w,
        Some(&format!(
            "Builds the HTTP handler for `{}`. Mount it with an adapter from `./adapters`.",
            svc.name
        )),
        None,
    );
    w.open(format!("export function create{name}Handler(impl: {name}Service, hooks: {name}Hooks{default}): {handler}"));
    if ruled {
        let loaders = if svc.facts.is_empty() {
            ""
        } else {
            ", hooks.facts"
        };
        w.line(format!(
            "const endpoints = {}(impl{loaders});",
            builder_name(svc)
        ));
    } else {
        let endpoint = fc.rt("ServerEndpoint");
        w.open_with(format!("const endpoints: {endpoint}[] ="), " [");
        let mut pre = CodeWriter::new();
        let mut out = Out {
            w: &mut *w,
            pre: &mut pre,
            diags,
        };
        for ep in &svc.endpoints {
            write_endpoint(&mut out, fc, names, svc, ep);
        }
        w.close_with("];");
    }
    let std = std_sigs(fc);
    let create = fc.rt("createHandler");
    let hooks = if fc.tree.validate {
        "hooks"
    } else {
        "{ validateRequests: false, ...hooks }"
    };
    w.line(format!("return {create}(endpoints, {std}, {hooks});"));
    w.close();
}

pub(crate) fn std_sigs(fc: &mut FileCtx<'_, '_>) -> String {
    let endpoint_rules = fc.tree.uses_endpoint_rules();
    let conflict = fc.tree.uses_idempotency();
    let mut sig = |name: &str| match fc.tree.prelude_decl(name) {
        Some(id) => format!("{}.sig", fc.value_ref(id)),
        None => "'about:blank'".to_string(),
    };
    let (u, v, i) = (sig("Unauthorized"), sig("ValidationError"), sig("Internal"));
    let mut out = format!("unauthorized: {u}, validation: {v}, internal: {i}");
    if endpoint_rules {
        out.push_str(&format!(", notFound: {}", sig("NotFound")));
    }
    if conflict {
        out.push_str(&format!(", conflict: {}", sig("Conflict")));
    }
    format!("{{ {out} }}")
}

/// Nest's route syntax: `{id}` becomes `:id`, and the ends carry no slash.
fn nest_path(path: &str) -> String {
    let mut out = String::new();
    let mut rest = path.trim_matches('/');
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(len) = rest[open..].find('}') else {
            break;
        };
        out.push(':');
        out.push_str(&rest[open + 1..open + len]);
        rest = &rest[open + len + 1..];
    }
    out.push_str(rest);
    out
}

/// The services as the framework adapters see them.
#[must_use]
pub fn models(tree: &Tree<'_>) -> Vec<ServiceModel> {
    tree.services
        .iter()
        .filter(|sid| events::serves_http(tree.ir.service(**sid)))
        .map(|sid| {
            let svc = tree.ir.service(*sid);
            let (auth, middleware) = required_hooks(svc, "", "unknown");
            let prefix = svc.prefix.trim_end_matches('/');
            let mut endpoints: Vec<(usize, EndpointModel)> = svc
                .endpoints
                .iter()
                // `ws` endpoints are not Nest routes: they attach to the HTTP server (`server/ws.ts`).
                .filter(|ep| ep.method != Method::Ws)
                .map(|ep| {
                    let below = ep.path.strip_prefix(prefix).unwrap_or(&ep.path);
                    let params = ep.path.matches('{').count();
                    let model = EndpointModel {
                        name: method_key(&ep.name),
                        method: ep.method,
                        path: nest_path(below),
                        status: ep.success.status,
                    };
                    (params, model)
                })
                .collect();
            endpoints.sort_by_key(|(params, _)| *params);
            ServiceModel {
                raw_name: svc.name.clone(),
                name: naming::pascal(&svc.name),
                file: naming::kebab(&svc.name),
                module: module_of(tree, svc),
                prefix: nest_path(prefix),
                hooks_required: !auth.is_empty()
                    || !middleware.is_empty()
                    || !svc.facts.is_empty()
                    || rules::service_is_idempotent(svc),
                endpoints: endpoints.into_iter().map(|(_, e)| e).collect(),
            }
        })
        .collect()
}
