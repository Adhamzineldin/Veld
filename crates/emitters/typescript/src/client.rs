//! `<package>/client.ts`: request types, endpoint descriptions and `<Service>Client` classes.
//!
//! The endpoint description writer and the request-type writer are shared with
//! the Node server target.

use std::collections::BTreeMap;

use veld_codegen::naming;
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_ir::{AuthKind, Binding, Endpoint, Ir, Method, Param, Service, ServiceId};

use crate::can;
use crate::derived;
use crate::policy::Plan;
use crate::tree::{FileCtx, Tree};
use crate::types::ts_type;
use crate::util::{add_file, doc_comment, method_key, prop_key, ts_str};
use crate::validate::schema_expr;
use crate::version;

/// Names of request interfaces. `<Endpoint>Request`, prefixed by the service name
/// when two services of the same set would otherwise collide.
#[derive(Debug, Clone)]
pub struct RequestNames {
    names: BTreeMap<(ServiceId, String), String>,
}

impl RequestNames {
    #[must_use]
    pub fn new(ir: &Ir, services: &[ServiceId]) -> Self {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for sid in services {
            for ep in &ir.service(*sid).endpoints {
                *counts.entry(base_name(&ep.name)).or_default() += 1;
            }
        }
        let mut names = BTreeMap::new();
        for sid in services {
            let svc = ir.service(*sid);
            for ep in &svc.endpoints {
                let base = base_name(&ep.name);
                let name = if counts.get(&base).copied().unwrap_or(0) > 1 {
                    format!("{}{base}", naming::pascal(&svc.name))
                } else {
                    base
                };
                names.insert((*sid, ep.name.clone()), name);
            }
        }
        Self { names }
    }

    #[must_use]
    pub fn get(&self, service: ServiceId, endpoint: &str) -> String {
        self.names
            .get(&(service, endpoint.to_string()))
            .cloned()
            .unwrap_or_else(|| base_name(endpoint))
    }
}

fn base_name(endpoint: &str) -> String {
    format!("{}Request", naming::pascal(endpoint))
}

/// Name of the module-level endpoint description constant.
#[must_use]
pub fn op_const(svc: &Service, ep: &Endpoint) -> String {
    format!("{}{}Op", naming::camel(&svc.name), naming::pascal(&ep.name))
}

/// `export interface GetOrderRequest { ... }`: path params, `body`, `query`, single
/// query params and headers, all by parameter name.
pub fn write_request_type(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, name: &str, ep: &Endpoint) {
    write_request_type_for(w, fc, name, (ep, false));
}

/// The type of one parameter of a request. A client sends a body and a query object without
/// their derived fields (`spec/rules.md` section 17.2), so there they have the `Input` type.
fn param_type(fc: &mut FileCtx<'_, '_>, p: &Param, client: bool) -> String {
    if client && matches!(p.binding, Binding::Body | Binding::QueryObject) {
        derived::shaped_type(fc, &p.ty, derived::Shape::Input)
    } else {
        ts_type(fc, &p.ty)
    }
}

fn write_request_type_for(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    name: &str,
    (ep, client): (&Endpoint, bool),
) {
    // `body: stream T` on a `ws` endpoint is the client's message type, not part of the request.
    let params: Vec<&Param> = ep.params.iter().filter(|p| !p.stream).collect();
    if params.is_empty() {
        w.line(format!("export type {name} = {{}};"));
        return;
    }
    w.open(format!("export interface {name}"));
    for p in params {
        let opt = if p.optional { "?" } else { "" };
        let ty = param_type(fc, p, client);
        w.line(format!("{}{opt}: {ty};", prop_key(&p.name)));
    }
    w.close();
}

/// The request type of a client method: the one of the server, and `ifMatch` and `idempotencyKey`
/// after the declared parameters when the endpoint has a guard or is `@idempotent`.
fn write_client_request_type(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    name: &str,
    ep: &Endpoint,
) {
    let extras = version::client_members(fc, ep);
    if extras.is_empty() {
        write_request_type_for(w, fc, name, (ep, true));
        return;
    }
    w.open(format!("export interface {name}"));
    for p in ep.params.iter().filter(|p| !p.stream) {
        let opt = if p.optional { "?" } else { "" };
        let ty = param_type(fc, p, true);
        w.line(format!("{}{opt}: {ty};", prop_key(&p.name)));
    }
    for line in extras {
        w.line(line);
    }
    w.close();
}

fn binding_name(b: Binding) -> &'static str {
    match b {
        Binding::Path => "path",
        Binding::Query => "query",
        Binding::QueryObject => "query_object",
        Binding::Header => "header",
        Binding::Body => "body",
    }
}

fn param_expr(fc: &mut FileCtx<'_, '_>, p: &Param) -> String {
    let mut parts = vec![
        format!("name: {}", ts_str(&p.name)),
        format!("binding: '{}'", binding_name(p.binding)),
    ];
    if let Some(h) = &p.header {
        parts.push(format!("header: {}", ts_str(h)));
    }
    parts.push(format!("t: {}", schema_expr(fc, &p.ty, &p.constraints)));
    if p.optional {
        parts.push("o: true".to_string());
    }
    if p.stream {
        parts.push("s: true".to_string());
    }
    format!("{{ {} }},", parts.join(", "))
}

fn auth_expr(ep: &Endpoint) -> Option<String> {
    let kind = match ep.auth.kind {
        AuthKind::None => return None,
        AuthKind::Bearer => "bearer",
        AuthKind::Basic => "basic",
        AuthKind::ApiKey => "api_key",
    };
    Some(match &ep.auth.header {
        Some(h) => format!("{{ kind: '{kind}', header: {} }}", ts_str(h)),
        None => format!("{{ kind: '{kind}' }}"),
    })
}

/// Writes the `EndpointSpec` fields of `ep` (name ... errors) at the writer's indentation.
pub fn write_spec_fields(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, ep: &Endpoint) {
    w.line(format!("name: {},", ts_str(&ep.name)));
    w.line(format!("method: {},", ts_str(ep.method.as_str())));
    w.line(format!("path: {},", ts_str(&ep.path)));
    w.line(format!("sig: {},", ts_str(&ep.sig)));
    w.line(format!("status: {},", ep.success.status));
    if ep.success.stream {
        w.line("stream: true,");
    }
    if let Some(auth) = auth_expr(ep) {
        w.line(format!("auth: {auth},"));
    }
    write_list(
        w,
        "params",
        ep.params.iter().map(|p| param_expr(fc, p)).collect(),
    );
    let success = match &ep.success.ty {
        Some(t) => schema_expr(fc, t, &veld_ir::Constraints::default()),
        None => "null".to_string(),
    };
    w.line(format!("success: {success},"));
    let errors = ep
        .errors
        .iter()
        .map(|e| {
            let (class, data) = fc.error_refs(e.decl);
            format!(
                "{{ status: {}, sig: {class}.sig, schema: {class}.schema, create: (data, init) => new {class}(data as {data}, init) }},",
                e.status
            )
        })
        .collect();
    write_list(w, "errors", errors);
    version::write_spec_members(w, fc.tree.ir, ep);
}

fn write_list(w: &mut CodeWriter, key: &str, items: Vec<String>) {
    if items.is_empty() {
        w.line(format!("{key}: [],"));
        return;
    }
    w.open_with(format!("{key}:"), " [");
    for item in items {
        w.line(item);
    }
    w.close_with("],");
}

fn write_op(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service, ep: &Endpoint) {
    let spec = fc.rt("EndpointSpec");
    w.open_with(format!("const {}: {spec} =", op_const(svc, ep)), " {");
    write_spec_fields(w, fc, ep);
    w.close_with("};");
}

fn write_service(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    (svc, plan): (&Service, &Plan),
    diags: &mut Vec<GenDiagnostic>,
) {
    for ep in &svc.endpoints {
        let name = names.get(svc.id, &ep.name);
        version::reserved_names(svc, ep, diags);
        write_client_request_type(w, fc, &name, ep);
        w.blank();
        write_op(w, fc, svc, ep);
        w.blank();
    }
    let class = format!("{}Client", naming::pascal(&svc.name));
    doc_comment(w, svc.docs.as_deref(), None);
    w.open(format!("export class {class}"));
    let core = fc.rt("ClientCore");
    w.line(format!("readonly #core: {core};"));
    w.blank();
    write_constructor(w, fc, svc);
    for ep in &svc.endpoints {
        w.blank();
        write_method(w, fc, names, svc, ep);
    }
    for ep in svc.endpoints.iter().filter(|ep| can::wanted(ep)) {
        w.blank();
        can::write_can(w, fc, plan, (svc, ep), diags);
    }
    w.close();
}

fn write_constructor(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service) {
    let opts = fc.rt("ClientOptions");
    let core = fc.rt("ClientCore");
    let validation = validation_expr(fc);
    let config = fc.tree.uses_runtime_config();
    // With a runtime config the address is `baseUrl`, always given (`spec/runtime-config.md` 5).
    let param = if config {
        format!("opts: {opts}")
    } else {
        format!("opts: {opts} = {{}}")
    };
    w.open(format!("constructor({param})"));
    let passed = if fc.tree.validate {
        "opts"
    } else {
        "{ validate: false, ...opts }"
    };
    w.open_with(format!("this.#core = new {core}({passed},"), " {");
    if !config {
        w.line(format!(
            "envVar: {},",
            ts_str(&format!("VELD_{}_URL", naming::screaming_snake(&svc.name)))
        ));
        match &svc.base_url {
            Some(u) => w.line(format!("baseUrl: {},", ts_str(u))),
            None => w.line("baseUrl: null,"),
        };
    }
    w.line(format!("validation: {validation},"));
    if let Some(digest) = svc.rules_digest.as_deref() {
        w.line(format!("service: {},", ts_str(&svc.name)));
        w.line(format!("digest: {},", ts_str(digest)));
    }
    w.close_with("});");
    w.close();
}

/// The arrow function that builds the local `ValidationError`. The compiler only includes prelude
/// declarations that are referenced, so a contract without parameters has none.
fn validation_expr(fc: &mut FileCtx<'_, '_>) -> String {
    match fc.tree.prelude_decl("ValidationError") {
        Some(id) => format!("(issues) => new {}({{ issues }})", fc.value_ref(id)),
        None => {
            let base = fc.rt("VeldError");
            format!(
                "(issues) => new {base}('about:blank', 'ValidationError', {{ issues }}, {{ status: 422 }}, 422)"
            )
        }
    }
}

fn write_method(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    ep: &Endpoint,
) {
    doc_comment(w, ep.docs.as_deref(), ep.deprecated.as_deref());
    let req = names.get(svc.id, &ep.name);
    let ret = match &ep.success.ty {
        Some(t) => ts_type(fc, t),
        None => "void".to_string(),
    };
    let default = if ep.params.iter().all(|p| p.stream) && !version::needs_request(ep) {
        " = {}"
    } else {
        ""
    };
    if ep.method == Method::Ws {
        write_ws_method(w, fc, svc, ep, &req, default);
        return;
    }
    if ep.success.stream {
        let init = fc.rt("StreamInit");
        w.open(format!(
            "{}(req: {req}{default}, init?: {init}): AsyncIterable<{ret}>",
            method_key(&ep.name)
        ));
        w.line(format!(
            "return this.#core.stream({}, req, init) as AsyncIterable<{ret}>;",
            op_const(svc, ep)
        ));
        w.close();
        return;
    }
    let init = fc.rt("CallInit");
    w.open(format!(
        "{}(req: {req}{default}, init?: {init}): Promise<{ret}>",
        method_key(&ep.name)
    ));
    w.line(format!(
        "return this.#core.call({}, req, init) as Promise<{ret}>;",
        op_const(svc, ep)
    ));
    w.close();
}

/// `chat(req, init?): WsChannel<ServerMessage, ClientMessage>`; a side that has no messages is `never`.
fn write_ws_method(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    svc: &Service,
    ep: &Endpoint,
    req: &str,
    default: &str,
) {
    let incoming = match &ep.success.ty {
        Some(t) => ts_type(fc, t),
        None => "never".to_string(),
    };
    let outgoing = match ep.params.iter().find(|p| p.stream) {
        Some(p) => derived::shaped_type(fc, &p.ty, derived::Shape::Input),
        None => "never".to_string(),
    };
    let init = fc.rt("WsInit");
    let channel = fc.rt("WsChannel");
    let ret = format!("{channel}<{incoming}, {outgoing}>");
    w.open(format!(
        "{}(req: {req}{default}, init?: {init}): {ret}",
        method_key(&ep.name)
    ));
    w.line(format!(
        "return this.#core.connect({}, req, init) as {ret};",
        op_const(svc, ep)
    ));
    w.close();
}

/// Emits `client.ts` for each package of the tree's services; returns the module paths written.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<Vec<String>, Vec<GenDiagnostic>> {
    let names = RequestNames::new(tree.ir, &tree.services);
    let plan = Plan::new(tree);
    let mut diags = Vec::new();
    let mut modules = Vec::new();
    for pkg in tree.packages() {
        let module = tree.client_module(pkg);
        let mut fc = FileCtx::new(tree, &module);
        let mut w = CodeWriter::new();
        for sid in tree
            .services
            .iter()
            .filter(|s| tree.ir.service(**s).package == pkg)
        {
            let svc = tree.ir.service(*sid);
            write_service(&mut w, &mut fc, &names, (svc, &plan), &mut diags);
            w.blank();
        }
        add_file(files, &format!("{module}.ts"), fc.finish(&w.finish()))?;
        modules.push(module);
    }
    if diags.is_empty() {
        Ok(modules)
    } else {
        Err(diags)
    }
}
