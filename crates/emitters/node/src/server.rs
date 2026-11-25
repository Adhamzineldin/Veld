//! `server/<service>.ts`: request types, the `<Service>Service` interface, hooks and `create<Service>Handler`.

use veld_codegen::naming;
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_emit_typescript::client::{RequestNames, write_request_type, write_spec_fields};
use veld_emit_typescript::tree::{FileCtx, Tree};
use veld_emit_typescript::types::ts_type;
use veld_emit_typescript::util::{add_file, doc_comment, method_key, prop_key, ts_str};
use veld_ir::{AuthKind, Endpoint, Method, Service};

use crate::frameworks::{EndpointModel, ServiceModel};

/// Root-relative module path of a service's server file.
fn module_of(tree: &Tree<'_>, svc: &Service) -> String {
    format!("{}server/{}", tree.root, naming::kebab(&svc.name))
}

/// Emits one server module per service and returns their module paths.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<Vec<String>, Vec<GenDiagnostic>> {
    let names = RequestNames::new(tree.ir, &tree.services);
    let mut modules = Vec::new();
    for sid in &tree.services {
        let svc = tree.ir.service(*sid);
        let module = module_of(tree, svc);
        let mut fc = FileCtx::new(tree, &module);
        let mut w = CodeWriter::new();
        write_service(&mut w, &mut fc, &names, svc);
        add_file(files, &format!("{module}.ts"), fc.finish(&w.finish()))?;
        modules.push(module);
    }
    Ok(modules)
}

fn write_service(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
) {
    for ep in &svc.endpoints {
        write_request_type(w, fc, &names.get(svc.id, &ep.name), ep);
        w.blank();
    }
    write_interface(w, fc, names, svc);
    w.blank();
    write_hooks(w, fc, svc);
    w.blank();
    write_factory(w, fc, names, svc);
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
        let (key, req) = (method_key(&ep.name), names.get(svc.id, &ep.name));
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

fn auth_hook(kind: AuthKind, ctx: &str) -> Option<(&'static str, String)> {
    match kind {
        AuthKind::None => None,
        AuthKind::Bearer => Some((
            "bearer",
            format!("(token: string, ctx: {ctx}) => Promise<unknown | null>"),
        )),
        AuthKind::Basic => Some((
            "basic",
            format!("(username: string, password: string, ctx: {ctx}) => Promise<unknown | null>"),
        )),
        AuthKind::ApiKey => Some((
            "apiKey",
            format!("(key: string, ctx: {ctx}) => Promise<unknown | null>"),
        )),
    }
}

/// Auth hooks and middleware names the service's endpoints need, in a stable order.
fn required_hooks(svc: &Service, ctx: &str) -> (Vec<(&'static str, String)>, Vec<String>) {
    let mut auth: Vec<(&'static str, String)> = Vec::new();
    let mut middleware: Vec<String> = Vec::new();
    for ep in &svc.endpoints {
        if let Some(h) = auth_hook(ep.auth.kind, ctx)
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
    let (auth, middleware) = required_hooks(svc, &ctx);
    w.open(format!(
        "export interface {}Hooks",
        naming::pascal(&svc.name)
    ));
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

fn write_endpoint(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    ep: &Endpoint,
) {
    w.open_with("", "{");
    write_spec_fields(w, fc, ep);
    let list: Vec<String> = ep.middleware.iter().map(|m| ts_str(m)).collect();
    w.line(format!("middleware: [{}],", list.join(", ")));
    let req = names.get(svc.id, &ep.name);
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
    w.close_with("},");
}

fn write_factory(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
) {
    let name = naming::pascal(&svc.name);
    let ctx = fc.rt("RequestContext");
    let (auth, middleware) = required_hooks(svc, &ctx);
    let default = if auth.is_empty() && middleware.is_empty() {
        " = {}"
    } else {
        ""
    };
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
    let endpoint = fc.rt("ServerEndpoint");
    w.open_with(format!("const endpoints: {endpoint}[] ="), " [");
    for ep in &svc.endpoints {
        write_endpoint(w, fc, names, svc, ep);
    }
    w.close_with("];");
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

fn std_sigs(fc: &mut FileCtx<'_, '_>) -> String {
    let mut sig = |name: &str| match fc.tree.prelude_decl(name) {
        Some(id) => format!("{}.sig", fc.value_ref(id)),
        None => "'about:blank'".to_string(),
    };
    let (u, v, i) = (sig("Unauthorized"), sig("ValidationError"), sig("Internal"));
    format!("{{ unauthorized: {u}, validation: {v}, internal: {i} }}")
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
        .map(|sid| {
            let svc = tree.ir.service(*sid);
            let (auth, middleware) = required_hooks(svc, "");
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
                hooks_required: !auth.is_empty() || !middleware.is_empty(),
                endpoints: endpoints.into_iter().map(|(_, e)| e).collect(),
            }
        })
        .collect()
}
