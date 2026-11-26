//! The test handler (`spec/rules.md` section 12.4, `spec/targets/java.md` section 9): a second
//! entry point, `<Service>TestHandler`, in a file of its own. It builds the same endpoints as the
//! production handler with loaders and a clock that read the `Veld-Test-*` request headers, a
//! service that answers a dry run or the supplied result before the application's handler, and no
//! authentication or middleware. Nothing in the production classes reads a test header.

use veld_codegen::{CodeWriter, GenDiagnostic};
use veld_ir::{Endpoint, Ir, Service};

use crate::conditional;
use crate::endpoint::{loaders_name, service_has_rules};
use crate::server::{Entry, extra_args, needs_hooks, signature, std_sigs};
use crate::spec::{RequestNames, is_sse, is_ws, method_name};
use crate::tree::FileCtx;
use crate::types::{codec_expr, java_type, schema_expr};
use crate::util::{doc_comment, java_str, member_ident, unique};

/// Whether a service gets a test handler: it has endpoint rules, or an endpoint with a guard, an
/// idempotency key or an `ETag` (`spec/rules.md` section 11.12: `veld test --live` must reach it),
/// or it subscribes to a channel (`spec/events.md` section 10.9).
#[must_use]
pub fn wanted(ir: &Ir, testing: bool, svc: &Service) -> bool {
    testing && (http_wanted(ir, svc) || !svc.subscriptions.is_empty())
}

/// Whether the test handler of a service answers its HTTP endpoints too: it has endpoint rules, a
/// guard, a key or an `ETag`. A service that only subscribes has a test handler with no application.
fn http_wanted(ir: &Ir, svc: &Service) -> bool {
    service_has_rules(svc) || conditional::service_uses(ir, svc)
}

/// What the event endpoint of a test handler does, for its Javadoc.
const EVENTS_DOC: &str = " The probe also lists the topics the service subscribes to, and `POST /__veld/test/event` delivers a message to the service's own subscriber over a fresh memory transport with handlers that only count: the application's handlers are not called.";

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
    let http = http_wanted(fc.tree.ir, svc);
    let (service, hooks) = (format!("{base}Service"), format!("{base}Hooks"));
    let loaders = (!svc.facts.is_empty()).then(|| loaders_name(base));
    let docs = if http {
        let mut text = format!(
            "THE TEST HANDLER OF `{}`. It must never be mounted in production.\n\nIt runs the same pipeline as `{base}Handler`, but the facts come from the request header `Veld-Test-Facts`, the principal from `Veld-Test-Principal`, the clock from `Veld-Test-Now` and, when present, the result from `Veld-Test-Result` in place of the application's handler. `Veld-Test-Dry-Run: 1` stops after step 6 and answers `204`. `GET /__veld/test` answers `200` with the rules digests. The application's loaders and its authentication and middleware hooks are not called.",
            svc.name
        );
        if !svc.subscriptions.is_empty() {
            text.push_str(EVENTS_DOC);
        }
        text
    } else {
        format!(
            "THE TEST HANDLER OF `{}`. It must never be mounted in production.\n\nIt takes no application: `GET /__veld/test` answers `200` with the rules digests.{EVENTS_DOC}",
            svc.name
        )
    };
    doc_comment(w, Some(&docs), &[], None);
    w.line("@SuppressWarnings(\"deprecation\")");
    w.open(format!("public final class {class}"));
    w.line(format!("private {class}() {{}}"));
    w.blank();
    if http {
        let entry = Entry {
            handler: &handler,
            given: &format!("{service} impl"),
            pass: "impl",
            hooks_class: &hooks,
            needs_hooks: needs_hooks(fc.tree, svc),
            docs: (
                "Builds the test handler with default hooks.",
                "Builds the test handler. The auth hooks of `hooks` are never called and may be `null`.",
            ),
        };
        // The test handler takes no runtime config (`spec/runtime-config.md` section 4): the output
        // mode of the rules is an argument of its own entry.
        if fc.tree.rules {
            write_mode_head(w, fc, &entry);
        } else {
            entry.write_plain(w);
        }
        write_create_body(w, fc, svc, base, loaders.is_some());
        w.close();
        if let Some(l) = &loaders {
            w.blank();
            write_loaders(w, fc, svc, l);
        }
        w.blank();
        write_stub(w, fc, names, svc, &service);
    } else {
        write_events_create(w, fc, svc, &handler, class);
    }
    if !svc.subscriptions.is_empty() {
        w.blank();
        write_event_service(w, fc, svc, base);
    }
    w.close();
    Vec::new()
}

/// `create()` of a service that only subscribes: the probe and the event endpoint.
fn write_events_create(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    svc: &Service,
    handler: &str,
    class: &str,
) {
    let support = fc.rt("TestSupport");
    let (collections, optional) = (
        fc.name("java.util.Collections"),
        fc.name("java.util.Optional"),
    );
    let digest = svc
        .rules_digest
        .as_deref()
        .map_or_else(|| "null".to_string(), java_str);
    w.line("/** Builds the test handler. */");
    w.open(format!("public static {handler} create()"));
    w.line(format!("{support}.notice({});", java_str(&svc.name)));
    w.line(format!(
        "return {support}.events({support}.handler(request -> {optional}.empty(), {collections}.singletonMap({}, {digest})), SUBSCRIBES, HANDLERS, {class}::subscribeEvents);",
        java_str(&svc.name)
    ));
    w.close();
}

/// The constants and the counting subscriber the event endpoint of a service needs.
fn write_event_service(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service, base: &str) {
    let (list, map) = (fc.name("java.util.List"), fc.name("java.util.Map"));
    let atomic = fc.name("java.util.concurrent.atomic.AtomicInteger");
    let (events, transport) = (fc.rt("Events"), fc.rt("Transport"));
    let mut topics: Vec<&str> = svc
        .subscriptions
        .iter()
        .map(|s| fc.tree.ir.channel(s.channel).topic.as_str())
        .collect();
    topics.sort_unstable();
    topics.dedup();
    let topics: Vec<String> = topics.into_iter().map(java_str).collect();
    w.line(format!(
        "private static final {map}<String, {list}<String>> SUBSCRIBES = {map}.of({}, {list}.of({}));",
        java_str(&svc.name),
        topics.join(", ")
    ));
    let names: Vec<String> = svc
        .subscriptions
        .iter()
        .flat_map(|s| {
            s.handlers
                .iter()
                .map(|h| java_str(&format!("{}.{}", svc.name, h.name)))
        })
        .collect();
    w.line(format!(
        "private static final {list}<String> HANDLERS = {list}.of({});",
        names.join(", ")
    ));
    w.blank();
    w.line(
        "/** Connects the subscriber of the service with handlers that only count their runs. */",
    );
    w.open(format!(
        "private static {transport}.Subscription subscribeEvents({transport} transport, {events}.ConsumerOptions options, {map}<String, {atomic}> called)"
    ));
    w.line(format!(
        "return {base}Subscriber.subscribe(transport, new Counting(called), options);"
    ));
    w.close();
    w.blank();
    w.line("/** The handlers of the service: each only counts. */");
    w.open(format!(
        "private static final class Counting implements {base}EventHandlers"
    ));
    w.line(format!("private final {map}<String, {atomic}> called;"));
    w.blank();
    w.open(format!("Counting({map}<String, {atomic}> called)"));
    w.line("this.called = called;");
    w.close();
    let methods = crate::events::handler_names(svc);
    let mut i = 0;
    for sub in &svc.subscriptions {
        for h in &sub.handlers {
            w.blank();
            let ty = fc.decl(h.event);
            w.line("@Override");
            w.open(format!(
                "public void {}({ty} event, {events}.EventMeta meta)",
                methods[i]
            ));
            w.line(format!(
                "called.get({}).incrementAndGet();",
                java_str(&format!("{}.{}", svc.name, h.name))
            ));
            w.close();
            i += 1;
        }
    }
    w.close();
}

/// The entries of a test handler whose target has rules: the output mode is the last argument of
/// the main one (`null` is `STRICT`), and the shorter ones leave it out.
fn write_mode_head(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, entry: &Entry<'_>) {
    let walk = fc.rt("RuleWalk");
    let (handler, given, pass, hooks) = (entry.handler, entry.given, entry.pass, entry.hooks_class);
    if !entry.needs_hooks {
        w.line(format!("/** {} */", entry.docs.0));
        w.open(format!("public static {handler} create({given})"));
        w.line(format!("return create({pass}, new {hooks}());"));
        w.close();
        w.blank();
    }
    w.line(format!("/** {} */", entry.docs.1));
    w.open(format!(
        "public static {handler} create({given}, {hooks} hooks)"
    ));
    w.line(format!("return create({pass}, hooks, null);"));
    w.close();
    w.blank();
    w.line(format!(
        "/** {} `rulesOutput` is what happens when a rule is violated by this handler's own output: `STRICT` answers `500 Internal`, `AUDIT` sends the response and reports, `OFF` skips the check; `null` is `STRICT`. */",
        entry.docs.1
    ));
    w.open(format!(
        "public static {handler} create({given}, {hooks} hooks, {walk}.OutputMode rulesOutput)"
    ));
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
    let built = if fc.tree.rules {
        "hooks.build().withRulesOutput(rulesOutput)"
    } else {
        "hooks.build()"
    };
    w.line(format!("{} built = {built};", fc.rt("HandlerHooks")));
    let uses = conditional::service_uses(fc.tree.ir, svc);
    if conditional::hooks_needed(svc).0 {
        w.line(format!(
            "built = built.withIdempotency({support}.store({base}Handler.conditions()), {support}.caller());"
        ));
    }
    if crate::flags::service_uses(fc.tree, svc) {
        w.line(format!("{support}.flags(built);"));
    }
    let given = if has_loaders { "new Loaders(), " } else { "" };
    let clock = if service_has_rules(svc) {
        format!(", {support}::now")
    } else {
        String::new()
    };
    let emissions = if crate::outbox::service_emits(svc) {
        // Every request gets a memory outbox of its own (`Veld-Test-Emitted`).
        format!(", {support}.emissions(hooks.events())")
    } else {
        String::new()
    };
    w.line(format!(
        "{list}<{endpoint}> endpoints = {base}Handler.endpoints(new Stub(impl), {given}built{clock}{emissions});"
    ));
    w.line(format!("{list}<{endpoint}> open = new {array_list}<>();"));
    let principal = match (&svc.principal, fc.tree.policies) {
        (Some(p), true) => {
            let ty = veld_ir::Type::Decl(p.decl);
            let (codec, wire) = (codec_expr(fc, &ty), fc.rt("Wire"));
            let schema = schema_expr(fc, &ty, &veld_ir::Constraints::default(), false);
            let decode =
                format!("{codec}.decode({wire}.prepare({schema}, json, {wire}.Mode.RESPONSE))");
            if fc.tree.ir.type_reaches_derived(&ty) {
                // The caller is completed once, as the hooks complete the one authentication returns.
                let (derive, class) = (fc.rt("Derive"), fc.decl(p.decl));
                format!(", json -> {derive}.principal({schema}, {codec}, {class}.class, {decode})")
            } else {
                format!(", json -> {decode}")
            }
        }
        (Some(_) | None, _) => String::new(),
    };
    w.line(format!(
        "for ({endpoint} e : endpoints) {{ open.add({support}.endpoint(e{principal})); }}"
    ));
    let std = std_sigs(fc);
    let conditions = if uses {
        format!("{base}Handler.conditions(), ")
    } else {
        String::new()
    };
    let created = format!("{router}.create({list}.copyOf(open), {conditions}{std}, built)");
    let wrapped = match (&svc.rules_digest, fc.tree.rules) {
        (Some(digest), true) => format!(
            "{}.wrap({created}, {})",
            fc.rt("DigestHandler"),
            java_str(digest)
        ),
        (Some(_) | None, _) => created,
    };
    let collections = fc.name("java.util.Collections");
    // A contract with an audited rule answers with `Veld-Test-Audited` (`spec/rules.md` section 12.4).
    let handler_method = if fc.tree.rollout {
        "auditedHandler"
    } else {
        "handler"
    };
    let digest = svc
        .rules_digest
        .as_deref()
        .map_or_else(|| "null".to_string(), java_str);
    let handler = format!(
        "{support}.{handler_method}({wrapped}, {collections}.singletonMap({}, {digest}))",
        java_str(&svc.name)
    );
    // A service whose moves emit answers with `Veld-Test-Emitted` (`spec/events.md` section 11.2).
    let handler = if crate::outbox::service_emits(svc) {
        format!("{support}.emitted({handler})")
    } else {
        handler
    };
    if svc.subscriptions.is_empty() {
        w.line(format!("return {handler};"));
    } else {
        let class = format!("{base}TestHandler");
        w.line(format!(
            "return {support}.events({handler}, SUBSCRIBES, HANDLERS, {class}::subscribeEvents);"
        ));
    }
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
        // The application's handler does not run, so nothing is checked and the header names the
        // events the edge taken emits (`spec/events.md` section 11.2).
        let skip = if crate::outbox::emits(ep) {
            "emit.emission().skip(); "
        } else {
            ""
        };
        match &ep.success.ty {
            Some(t) => {
                let codec = codec_expr(fc, t);
                let schema = schema_expr(fc, t, &veld_ir::Constraints::default(), false);
                w.open("if (supplied != null)");
                w.line(format!(
                    "{skip}return {codec}.decode({wire}.prepare({schema}, supplied, {wire}.Mode.RESPONSE));"
                ));
                w.close();
                w.line(format!("return delegate.{method}(request, {facts}ctx);"));
            }
            None => {
                w.line(format!("if (supplied != null) {{ {skip}return; }}"));
                w.line(format!("delegate.{method}(request, {facts}ctx);"));
            }
        }
    }
    w.close();
}
