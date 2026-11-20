//! Pieces shared by the client and server classes: request records and endpoint descriptions.

use std::collections::BTreeMap;

use veld_codegen::naming;
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_ir::{AuthKind, Binding, Endpoint, Ir, Param, Service, ServiceId};

use crate::tree::{FileCtx, Tree, java_path};
use crate::types::{codec_expr, java_type, schema_expr};
use crate::util::{
    add_file, deprecated_annotation, doc_comment, end_call, java_str, member_ident, open_call,
    unique,
};

/// Names of request records: `<Endpoint>Request`, prefixed by the service name when two services
/// of the same set would otherwise collide.
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

/// The Java method name of an endpoint.
#[must_use]
pub fn method_name(ep: &Endpoint) -> String {
    member_ident(&ep.name)
}

/// The constant holding an endpoint's description.
#[must_use]
pub fn spec_const(ep: &Endpoint) -> String {
    naming::escape(naming::Lang::Java, &naming::screaming_snake(&ep.name))
}

/// Emits one request record per endpoint of `svc` into `package`.
pub fn emit_requests(
    files: &mut FileSet,
    tree: &Tree<'_>,
    package: &str,
    names: &RequestNames,
    svc: &Service,
) -> Result<(), Vec<GenDiagnostic>> {
    for ep in &svc.endpoints {
        let name = names.get(svc.id, &ep.name);
        let mut fc = FileCtx::new(tree, package, &name);
        let mut w = CodeWriter::new();
        write_request(&mut w, &mut fc, &name, ep);
        add_file(files, &java_path(package, &name), fc.finish(&w.finish()))?;
    }
    Ok(())
}

/// A record with one component per parameter: path params, `body`, `query`, single query params
/// and headers, all by parameter name. `fromWire` and `toWire` use the parameter name as the key.
fn write_request(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, name: &str, ep: &Endpoint) {
    let idents = unique(ep.params.iter().map(|p| member_ident(&p.name)).collect());
    let json = fc.rt("Json");
    let params: Vec<(String, String)> = ep
        .params
        .iter()
        .zip(&idents)
        .map(|(p, id)| (id.clone(), param_doc(p)))
        .collect();
    doc_comment(
        w,
        Some(&format!(
            "The parameters of `{}`. Absent optional parameters are `null`.",
            ep.name
        )),
        &params,
        None,
    );
    w.line("@SuppressWarnings(\"deprecation\")");
    let comps: Vec<String> = ep
        .params
        .iter()
        .zip(&idents)
        .map(|(p, id)| format!("{} {id}", java_type(fc, &p.ty, p.optional)))
        .collect();
    if comps.is_empty() {
        w.open(format!("public record {name}()"));
    } else {
        w.line(format!("public record {name}("));
        w.indent().indent();
        for (i, c) in comps.iter().enumerate() {
            w.line(if i + 1 == comps.len() {
                format!("{c}) {{")
            } else {
                format!("{c},")
            });
        }
        w.dedent();
    }
    w.line("/** Reads the parameters bound by the router, keyed by parameter name. */");
    w.open(format!("public static {name} fromWire({json}.Obj params)"));
    if ep.params.is_empty() {
        w.line(format!("return new {name}();"));
    } else {
        let codec = fc.rt("Codec");
        open_call(w, format!("return new {name}"));
        for (i, p) in ep.params.iter().enumerate() {
            let get = if p.optional { "opt" } else { "req" };
            let sep = if i + 1 == ep.params.len() { ");" } else { "," };
            let c = codec_expr(fc, &p.ty);
            w.line(format!(
                "{codec}.{get}(params, {}, {c}){sep}",
                java_str(&p.name)
            ));
        }
        end_call(w);
    }
    w.close();
    w.blank();
    w.line("/** The parameters in wire form, keyed by parameter name. */");
    w.open(format!("public {json}.Obj toWire()"));
    w.line(format!("return {json}.object()"));
    w.indent().indent();
    for (p, id) in ep.params.iter().zip(&idents) {
        let c = codec_expr(fc, &p.ty);
        w.line(format!(".put({}, {id}, {c})", java_str(&p.name)));
    }
    w.line(".build();");
    w.dedent().dedent();
    w.close();
    w.close();
}

fn param_doc(p: &Param) -> String {
    let place = match p.binding {
        Binding::Path => "path parameter".to_string(),
        Binding::Query => "query parameter".to_string(),
        Binding::QueryObject => "query object".to_string(),
        Binding::Header => format!("header `{}`", p.header.as_deref().unwrap_or(&p.name)),
        Binding::Body => "request body".to_string(),
    };
    if p.optional {
        format!("The {place}; optional.")
    } else {
        format!("The {place}.")
    }
}

fn binding_name(b: Binding) -> &'static str {
    match b {
        Binding::Path => "PATH",
        Binding::Query => "QUERY",
        Binding::QueryObject => "QUERY_OBJECT",
        Binding::Header => "HEADER",
        Binding::Body => "BODY",
    }
}

fn auth_expr(fc: &mut FileCtx<'_, '_>, ep: &Endpoint) -> String {
    let kind = match ep.auth.kind {
        AuthKind::None => return "null".to_string(),
        AuthKind::Bearer => "BEARER",
        AuthKind::Basic => "BASIC",
        AuthKind::ApiKey => "API_KEY",
    };
    let spec = fc.rt("EndpointSpec");
    let header = ep
        .auth
        .header
        .as_deref()
        .map_or_else(|| "null".to_string(), java_str);
    format!("new {spec}.Auth({spec}.AuthKind.{kind}, {header})")
}

fn param_expr(fc: &mut FileCtx<'_, '_>, p: &Param) -> String {
    let spec = fc.rt("EndpointSpec");
    let header = p
        .header
        .as_deref()
        .map_or_else(|| "null".to_string(), java_str);
    let ty = schema_expr(fc, &p.ty, &p.constraints, false);
    format!(
        "new {spec}.Param({}, {spec}.Binding.{}, {header}, {ty}, {})",
        java_str(&p.name),
        binding_name(p.binding),
        p.optional
    )
}

/// `List.of(...)` over `items`, one per line, followed by `trailing`.
fn write_list(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, items: &[String], trailing: &str) {
    let list = fc.name("java.util.List");
    if items.is_empty() {
        w.line(format!("{list}.of(){trailing}"));
        return;
    }
    open_call(w, format!("{list}.of"));
    for (i, item) in items.iter().enumerate() {
        if i + 1 == items.len() {
            w.line(format!("{item}){trailing}"));
        } else {
            w.line(format!("{item},"));
        }
    }
    end_call(w);
}

/// `private static final EndpointSpec NAME = new EndpointSpec(...);`
pub fn write_spec_const(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, ep: &Endpoint) {
    let spec = fc.rt("EndpointSpec");
    open_call(
        w,
        format!(
            "private static final {spec} {} = new {spec}",
            spec_const(ep)
        ),
    );
    w.line(format!("{},", java_str(&ep.name)));
    w.line(format!("{},", java_str(ep.method.as_str())));
    w.line(format!("{},", java_str(&ep.path)));
    w.line(format!("{},", java_str(&ep.sig)));
    w.line(format!("{},", ep.success.status));
    let auth = auth_expr(fc, ep);
    w.line(format!("{auth},"));
    let params: Vec<String> = ep.params.iter().map(|p| param_expr(fc, p)).collect();
    write_list(w, fc, &params, ",");
    let success = match &ep.success.ty {
        Some(t) => schema_expr(fc, t, &veld_ir::Constraints::default(), false),
        None => "null".to_string(),
    };
    w.line(format!("{success},"));
    let errors: Vec<String> = ep
        .errors
        .iter()
        .map(|e| {
            let class = fc.decl(e.decl);
            format!(
                "new {spec}.Declared({}, {class}.VELD_SIG, {class}.SCHEMA, {class}::fromWire)",
                e.status
            )
        })
        .collect();
    write_list(w, fc, &errors, ");");
    end_call(w);
}

/// Javadoc and `@Deprecated` of an endpoint method.
pub fn endpoint_docs(w: &mut CodeWriter, ep: &Endpoint) {
    doc_comment(w, ep.docs.as_deref(), &[], ep.deprecated.as_deref());
    deprecated_annotation(w, ep.deprecated.as_deref());
}
