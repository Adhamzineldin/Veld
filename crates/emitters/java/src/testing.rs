//! The test handler (`spec/rules.md` section 12.4, `spec/targets/java.md` section 9): a second
//! entry point, `<Service>TestHandler`, in a file of its own. It builds the same endpoints as the
//! production handler with loaders and a clock that read the `Veld-Test-*` request headers, a
//! service that answers a dry run or the supplied result before the application's handler, and no
//! authentication or middleware. Nothing in the production classes reads a test header.

use veld_codegen::{CodeWriter, GenDiagnostic};
use veld_ir::{Endpoint, Service};

use crate::endpoint::{loaders_name, service_has_rules};
use crate::server::{extra_args, needs_hooks, signature, std_sigs};
use crate::spec::{RequestNames, is_sse, is_ws, method_name};
use crate::tree::FileCtx;
use crate::types::{codec_expr, java_type, schema_expr};
use crate::util::{doc_comment, java_str, member_ident, unique};

/// Whether a service gets a test handler.
#[must_use]
pub fn wanted(testing: bool, svc: &Service) -> bool {
    testing && service_has_rules(svc)
}

/// Writes `<Service>TestHandler`.
pub fn write_test_handler(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    class: &str,
    base: &str,
) -> Vec<GenDiagnostic> {
    let handler = fc.rt("VeldHandler");
    let (service, hooks) = (format!("{base}Service"), format!("{base}Hooks"));
    let loaders = (!svc.facts.is_empty()).then(|| loaders_name(base));
    doc_comment(
        w,
        Some(&format!(
            "THE TEST HANDLER OF `{}`. It must never be mounted in production.\n\nIt runs the same pipeline as `{base}Handler`, but the facts come from the request header `Veld-Test-Facts`, the principal from `Veld-Test-Principal`, the clock from `Veld-Test-Now` and, when present, the result from `Veld-Test-Result` in place of the application's handler. `Veld-Test-Dry-Run: 1` stops after step 6 and answers `204`. `GET /__veld/test` answers `200` with the rules digests. The application's loaders and its authentication and middleware hooks are not called.",
            svc.name
        )),
        &[],
        None,
    );
    w.line("@SuppressWarnings(\"deprecation\")");
    w.open(format!("public final class {class}"));
    w.line(format!("private {class}() {{}}"));
    w.blank();
    if !needs_hooks(svc) {
        w.line("/** Builds the test handler with default hooks. */");
        w.open(format!("public static {handler} create({service} impl)"));
        w.line(format!("return create(impl, new {hooks}());"));
        w.close();
        w.blank();
    }
    w.line("/** Builds the test handler. The auth hooks of `hooks` are never called and may be `null`. */");
    w.open(format!(
        "public static {handler} create({service} impl, {hooks} hooks)"
    ));
    write_create_body(w, fc, svc, base, loaders.is_some());
    w.close();
    if let Some(l) = &loaders {
        w.blank();
        write_loaders(w, fc, svc, l);
    }
    w.blank();
    write_stub(w, fc, names, svc, &service);
    w.close();
    Vec::new()
}

/// The body of `create`: the production endpoints over the stub service and the header loaders,
/// without authentication and middleware, behind the probe.
fn write_create_body(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    svc: &Service,
    base: &str,
    has_loaders: bool,
) {
    let (router, support) = (fc.rt("Router"), fc.rt("TestSupport"));
    let endpoint = fc.rt("ServerEndpoint");
    let (list, array_list) = (fc.name("java.util.List"), fc.name("java.util.ArrayList"));
    w.line(format!("{support}.notice({});", java_str(&svc.name)));
    w.line(format!("{} built = hooks.build();", fc.rt("HandlerHooks")));
    let given = if has_loaders { "new Loaders(), " } else { "" };
    w.line(format!(
        "{list}<{endpoint}> endpoints = {base}Handler.endpoints(new Stub(impl), {given}built, {support}::now);"
    ));
    w.line(format!("{list}<{endpoint}> open = new {array_list}<>();"));
    let principal = match (&svc.principal, fc.tree.policies) {
        (Some(p), true) => {
            let ty = veld_ir::Type::Decl(p.decl);
            let (codec, wire) = (codec_expr(fc, &ty), fc.rt("Wire"));
            let schema = schema_expr(fc, &ty, &veld_ir::Constraints::default(), false);
            format!(
                ", json -> {codec}.decode({wire}.prepare({schema}, json, {wire}.Mode.RESPONSE))"
            )
        }
        (Some(_) | None, _) => String::new(),
    };
    w.line(format!(
        "for ({endpoint} e : endpoints) {{ open.add({support}.endpoint(e{principal})); }}"
    ));
    let std = std_sigs(fc);
    let created = format!("{router}.create({list}.copyOf(open), {std}, built)");
    let wrapped = match (&svc.rules_digest, fc.tree.rules) {
        (Some(digest), true) => format!(
            "{}.wrap({created}, {})",
            fc.rt("DigestHandler"),
            java_str(digest)
        ),
        (Some(_) | None, _) => created,
    };
    let collections = fc.name("java.util.Collections");
    let digest = svc
        .rules_digest
        .as_deref()
        .map_or_else(|| "null".to_string(), java_str);
    w.line(format!(
        "return {support}.handler({wrapped}, {collections}.singletonMap({}, {digest}));",
        java_str(&svc.name)
    ));
}

/// The loaders that read their fact from `Veld-Test-Facts`; the key is ignored.
fn write_loaders(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service, interface: &str) {
    let (support, ctx) = (fc.rt("TestSupport"), fc.rt("RequestContext"));
    let (optional, wire) = (fc.name("java.util.Optional"), fc.rt("Wire"));
    w.line("/** Reads every fact from the request header `Veld-Test-Facts`. */");
    w.open(format!(
        "private static final class Loaders implements {interface}"
    ));
    let names = unique(svc.facts.iter().map(|f| member_ident(&f.name)).collect());
    for (fact, name) in svc.facts.iter().zip(&names) {
        let ty = java_type(fc, &fact.ty, true);
        let key = match &fact.key {
            Some(k) => format!("{} key, ", java_type(fc, k, false)),
            None => String::new(),
        };
        let codec = codec_expr(fc, &fact.ty);
        let schema = schema_expr(fc, &fact.ty, &veld_ir::Constraints::default(), false);
        w.line("@Override");
        w.open(format!("public {optional}<{ty}> {name}({key}{ctx} ctx)"));
        w.line(format!(
            "return {support}.fact(ctx, {}).map(json -> {codec}.decode({wire}.prepare({schema}, json, {wire}.Mode.RESPONSE)));",
            java_str(&fact.name)
        ));
        w.close();
    }
    w.close();
}

/// The service the pipeline calls: answers a dry run or the supplied result, else the application.
fn write_stub(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    service: &str,
) {
    let ctx = fc.rt("RequestContext");
    w.line(
        "/** Answers a dry run or the result of the request, and else calls the application. */",
    );
    w.open(format!(
        "private static final class Stub implements {service}"
    ));
    w.line(format!("private final {service} delegate;"));
    w.blank();
    w.open(format!("Stub({service} delegate)"));
    w.line("this.delegate = delegate;");
    w.close();
    for ep in &svc.endpoints {
        w.blank();
        write_stub_method(w, fc, names, svc, ep, &ctx);
    }
    w.close();
}

fn write_stub_method(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    ep: &Endpoint,
    ctx: &str,
) {
    let request = names.get(svc.id, &ep.name);
    let declaration = signature(fc, ep, &request, ctx);
    let declaration = declaration.trim_end_matches(';').to_string();
    let method = method_name(ep);
    let facts = extra_args(ep);
    w.line("@Override");
    w.open(format!("public {declaration}"));
    if is_ws(ep) {
        w.line(format!("delegate.{method}(request, ctx, ws);"));
    } else if is_sse(ep) {
        w.line(format!("return delegate.{method}(request, ctx);"));
    } else {
        let (support, wire) = (fc.rt("TestSupport"), fc.rt("Wire"));
        w.line(format!(
            "if ({support}.dryRun(ctx)) {{ throw {support}.passed(); }}"
        ));
        let json = fc.rt("Json");
        w.line(format!("{json} supplied = {support}.result(ctx);"));
        match &ep.success.ty {
            Some(t) => {
                let codec = codec_expr(fc, t);
                let schema = schema_expr(fc, t, &veld_ir::Constraints::default(), false);
                w.open("if (supplied != null)");
                w.line(format!(
                    "return {codec}.decode({wire}.prepare({schema}, supplied, {wire}.Mode.RESPONSE));"
                ));
                w.close();
                w.line(format!("return delegate.{method}(request, {facts}ctx);"));
            }
            None => {
                w.line("if (supplied != null) { return; }");
                w.line(format!("delegate.{method}(request, {facts}ctx);"));
            }
        }
    }
    w.close();
}
