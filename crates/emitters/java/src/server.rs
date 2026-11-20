//! `<package>.server`: request records, the `<Service>Service` interface, `<Service>Hooks` and
//! `<Service>Handler`.

use veld_codegen::naming;
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_ir::{AuthKind, Endpoint, Service};

use crate::frameworks::{EndpointModel, ServiceModel};
use crate::spec::{
    RequestNames, emit_requests, endpoint_docs, method_name, spec_const, write_spec_const,
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
            emit_requests(files, tree, &package, &names, svc)?;
            let base = naming::pascal(&svc.name);
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
                write_handler(w, fc, &names, svc, class, &base)
            })?;
        }
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
        endpoint_docs(w, ep);
        let ret = match &ep.success.ty {
            Some(t) => java_type(fc, t, true),
            None => "void".to_string(),
        };
        let request = names.get(svc.id, &ep.name);
        w.line(format!(
            "{ret} {}({request} request, {ctx} ctx) throws Exception;",
            method_name(ep)
        ));
    }
    w.close();
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
    let mut out = Vec::new();
    for (i, (setter, ty, doc)) in auth.iter().enumerate() {
        out.push(Hook {
            param: params[i].clone(),
            ty: format!("{hooks}.{ty}"),
            register: format!("builder.{setter}({});", params[i]),
            doc: (*doc).to_string(),
        });
    }
    for (j, m) in middleware.iter().enumerate() {
        let param = params[auth.len() + j].clone();
        out.push(Hook {
            ty: format!("{hooks}.Middleware"),
            register: format!("builder.middleware({}, {param});", java_str(m)),
            doc: format!("the `{m}` middleware"),
            param,
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
    w.line(format!(
        "private final {hooks}.Builder builder = {hooks}.builder();"
    ));
    w.blank();
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
    w.open(format!("{hooks} build()"));
    w.line("return builder.build();");
    w.close();
    w.close();
}

fn write_handler(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    class: &str,
    base: &str,
) {
    let handler = fc.rt("VeldHandler");
    let router = fc.rt("Router");
    let endpoint = fc.rt("ServerEndpoint");
    let list = fc.name("java.util.List");
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
    if !needs_hooks {
        w.line("/** Builds the handler with default hooks. */");
        w.open(format!("public static {handler} create({service} impl)"));
        w.line(format!("return create(impl, new {hooks_class}());"));
        w.close();
        w.blank();
    }
    w.line("/** Builds the handler. */");
    w.open(format!(
        "public static {handler} create({service} impl, {hooks_class} hooks)"
    ));
    open_call(w, format!("{list}<{endpoint}> endpoints = {list}.of"));
    for (i, ep) in svc.endpoints.iter().enumerate() {
        write_endpoint(w, fc, names, svc, ep, i + 1 == svc.endpoints.len());
    }
    end_call(w);
    let std = std_sigs(fc);
    w.line(format!(
        "return {router}.create(endpoints, {std}, hooks.build());"
    ));
    w.close();
    w.close();
}

fn write_endpoint(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    ep: &Endpoint,
    last: bool,
) {
    let endpoint = fc.rt("ServerEndpoint");
    let list = fc.name("java.util.List");
    let request = names.get(svc.id, &ep.name);
    let middleware: Vec<String> = ep.middleware.iter().map(|m| java_str(m)).collect();
    let end = if last { ");" } else { "," };
    let method = method_name(ep);
    let call = format!("impl.{method}({request}.fromWire(params), ctx)");
    open_call(w, format!("new {endpoint}"));
    w.line(format!("{},", spec_const(ep)));
    w.line(format!("{list}.of({}),", middleware.join(", ")));
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
    end_call(w);
}

fn std_sigs(fc: &mut FileCtx<'_, '_>) -> String {
    let router = fc.rt("Router");
    let mut sig = |name: &str| match fc.tree.prelude_decl(name) {
        Some(id) => format!("{}.VELD_SIG", fc.decl(id)),
        None => "\"about:blank\"".to_string(),
    };
    let (u, v, i) = (sig("Unauthorized"), sig("ValidationError"), sig("Internal"));
    format!("new {router}.StdSigs({u}, {v}, {i})")
}

/// Whether any endpoint of the service needs an auth or middleware hook.
fn needs_hooks(svc: &Service) -> bool {
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
            endpoints: svc
                .endpoints
                .iter()
                .map(|ep| EndpointModel {
                    name: ep.name.clone(),
                    method_name: method_name(ep),
                    http_method: ep.method.as_str(),
                    path: ep.path.clone(),
                })
                .collect(),
        });
    }
    out
}
