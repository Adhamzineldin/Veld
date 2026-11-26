//! What the server and the client modules of a service share: request structs and the static
//! endpoint descriptions.

use veld_codegen::CodeWriter;
use veld_ir::{AuthKind, Binding, Endpoint, Method, Service};

use crate::tree::Tree;
use crate::types::{Ctx, rust_type, schema_ref};
use crate::util::{bare, doc, lit, snake};

/// What an endpoint speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Http,
    Sse,
    Ws,
}

#[must_use]
pub fn kind(ep: &Endpoint) -> Kind {
    if ep.method == Method::Ws {
        Kind::Ws
    } else if ep.success.stream {
        Kind::Sse
    } else {
        Kind::Http
    }
}

/// The request struct of an endpoint: `GetOrderRequest`.
#[must_use]
pub fn request_name(ep: &Endpoint) -> String {
    let mut n = veld_codegen::naming::pascal(&ep.name);
    n.push_str("Request");
    crate::util::type_name(&n)
}

/// The parameters that belong in the request struct (a WebSocket's `body: stream` does not).
pub fn request_params(ep: &Endpoint) -> impl Iterator<Item = &veld_ir::Param> {
    ep.params.iter().filter(|p| !p.stream)
}

/// Emits the request struct of one endpoint (the server's: it has the declared parameters only).
pub fn request_struct(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, ep: &Endpoint) {
    write_request(w, ctx, ep, false);
}

/// Emits the request struct of one endpoint for a client: after the declared parameters it has
/// `if_match` and `idempotency_key` where the endpoint takes them (`spec/rules.md` section 11.12).
pub fn client_request_struct(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, ep: &Endpoint) {
    write_request(w, ctx, ep, true);
}

fn write_request(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, ep: &Endpoint, client: bool) {
    let params: Vec<_> = request_params(ep).collect();
    let key = client
        .then_some(())
        .and_then(|()| crate::conditional::key_of(ep));
    let all_optional =
        params.iter().all(|p| p.optional) && key.is_none_or(|(_, required)| !required);
    w.line(format!("/// The parameters of `{}`.", ep.name));
    let default = if all_optional { "Default, " } else { "" };
    w.line(format!(
        "#[derive(Debug, Clone, PartialEq, {default}::serde::Serialize, ::serde::Deserialize)]"
    ));
    w.open(format!("pub struct {}", request_name(ep)));
    for p in params {
        let ident = snake(&p.name);
        let mut attrs = Vec::new();
        if bare(&ident) != p.name {
            attrs.push(format!("rename = {}", lit(&p.name)));
        }
        if p.optional {
            attrs.push("default".to_string());
            attrs.push("skip_serializing_if = \"Option::is_none\"".to_string());
        }
        if !attrs.is_empty() {
            w.line(format!("#[serde({})]", attrs.join(", ")));
        }
        let t = rust_type(ctx, &p.ty, false);
        w.line(format!(
            "pub {ident}: {},",
            if p.optional {
                format!("Option<{t}>")
            } else {
                t
            }
        ));
    }
    if client {
        crate::conditional::client_fields(w, ctx, ep);
    }
    w.close().blank();
}

fn binding(b: Binding) -> &'static str {
    match b {
        Binding::Path => "Path",
        Binding::Query => "Query",
        Binding::QueryObject => "QueryObject",
        Binding::Header => "Header",
        Binding::Body => "Body",
    }
}

fn auth_kind(k: AuthKind) -> Option<&'static str> {
    match k {
        AuthKind::None => None,
        AuthKind::Bearer => Some("Bearer"),
        AuthKind::Basic => Some("Basic"),
        AuthKind::ApiKey => Some("ApiKey"),
    }
}

fn opt_str(s: Option<&str>) -> String {
    s.map_or("None".to_string(), |s| format!("Some({})", lit(s)))
}

/// Emits `pub static ENDPOINTS: [sp::EndpointSpec; N]` for the service (the server's).
pub fn endpoint_statics(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service) {
    write_statics(w, ctx, svc, false);
}

/// Emits `ENDPOINTS` for a client: the guard and the key are header parameters of its requests.
pub fn client_endpoint_statics(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service) {
    write_statics(w, ctx, svc, true);
}

fn write_statics(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service, client: bool) {
    w.line(format!(
        "/// The endpoints of `{}`, in declaration order.",
        svc.name
    ));
    w.open_with(
        format!(
            "pub static ENDPOINTS: [sp::EndpointSpec; {}] =",
            svc.endpoints.len()
        ),
        " [",
    );
    for ep in &svc.endpoints {
        endpoint_spec(w, ctx, ep, client);
    }
    w.close_with("];");
    w.blank();
}

fn endpoint_spec(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, ep: &Endpoint, client: bool) {
    w.open("sp::EndpointSpec");
    w.line(format!("name: {},", lit(&ep.name)));
    w.line(format!("method: {},", lit(ep.method.as_str())));
    w.line(format!("path: {},", lit(&ep.path)));
    w.line(format!("sig: {},", lit(&ep.sig)));
    w.line(format!("status: {},", ep.success.status));
    match auth_kind(ep.auth.kind) {
        Some(k) => w.line(format!(
            "auth: Some(sp::AuthSpec {{ kind: sp::AuthKind::{k}, header: {} }}),",
            opt_str(ep.auth.header.as_deref())
        )),
        None => w.line("auth: None,"),
    };
    let mw: Vec<String> = ep.middleware.iter().map(|m| lit(m)).collect();
    w.line(format!("middleware: &[{}],", mw.join(", ")));
    w.open_with("params:", " &[");
    for p in &ep.params {
        w.open("sp::ParamSpec");
        w.line(format!("name: {},", lit(&p.name)));
        w.line(format!("binding: sp::Binding::{},", binding(p.binding)));
        w.line(format!("header: {},", opt_str(p.header.as_deref())));
        w.line(format!(
            "schema: {},",
            schema_ref(ctx, &p.ty, &p.constraints)
        ));
        w.line(format!("optional: {},", p.optional));
        w.line(format!("stream: {},", p.stream));
        w.close_with("},");
    }
    if client {
        crate::conditional::client_params(w, ctx, ep);
    }
    w.close_with("],");
    match &ep.success.ty {
        Some(t) => w.line(format!(
            "success: Some({}),",
            schema_ref(ctx, t, &veld_ir::Constraints::default())
        )),
        None => w.line("success: None,"),
    };
    w.open_with("errors:", " &[");
    for e in &ep.errors {
        let d = ctx.tree.ir.decl(e.decl);
        w.open("sp::ErrorSpec");
        w.line(format!("status: {},", e.status));
        w.line(format!("name: {},", lit(&d.name)));
        w.line(format!("sig: {},", lit(&d.sig)));
        w.line(format!(
            "schema: &{},",
            ctx.tree.schema_path(e.decl, ctx.here)
        ));
        w.close_with("},");
    }
    w.close_with("],");
    let kind = match kind(ep) {
        Kind::Http => "Http",
        Kind::Sse => "Sse",
        Kind::Ws => "Ws",
    };
    w.line(format!("kind: sp::Kind::{kind},"));
    w.close_with("},");
}

/// Emits `const STD: sp::StdSigs`, the signatures of the prelude errors the runtime raises.
pub fn std_sigs(w: &mut CodeWriter, tree: &Tree<'_>) {
    let sig = |name: &str| {
        tree.prelude_decl(name)
            .map_or("about:blank".to_string(), |id| tree.ir.decl(id).sig.clone())
    };
    w.line("/// The signatures of the prelude errors the runtime raises itself.");
    w.open_with("const STD: sp::StdSigs =", " sp::StdSigs {");
    w.line(format!("unauthorized: {},", lit(&sig("Unauthorized"))));
    w.line(format!("validation: {},", lit(&sig("ValidationError"))));
    w.line(format!("internal: {},", lit(&sig("Internal"))));
    w.close_with("};");
    w.blank();
}

/// Emits the doc comment of an endpoint method, including its deprecation.
pub fn method_docs(w: &mut CodeWriter, ep: &Endpoint) {
    doc(w, ep.docs.as_deref());
    if let Some(msg) = &ep.deprecated {
        w.line("///").line(format!("/// **Deprecated:** {msg}"));
    }
}

/// The message types of a WebSocket endpoint as `(incoming, outgoing)` from the server's point of
/// view: the stream body the client sends and the success stream the server sends. `()` marks a
/// side with no messages.
#[must_use]
pub fn ws_types(ctx: &Ctx<'_, '_>, ep: &Endpoint) -> (String, String) {
    let incoming = ep
        .params
        .iter()
        .find(|p| p.stream)
        .map_or("()".to_string(), |p| rust_type(ctx, &p.ty, false));
    let outgoing = ep
        .success
        .ty
        .as_ref()
        .map_or("()".to_string(), |t| rust_type(ctx, t, false));
    (incoming, outgoing)
}
