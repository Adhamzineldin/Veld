//! Endpoint rules in the server classes (`spec/rules.md` section 9, `spec/targets/java.md`
//! section 9): the facts record of an endpoint, the loader interface of a service, and the body of
//! the endpoint's call, which runs step 6 (clock, facts, `require`, `old`) before the handler and
//! registers step 9 (`ensure`) after it. The runtime (`EndpointRules`) does the checking and the
//! reporting.

use veld_codegen::{CodeWriter, GenDiagnostic, print_expr};
use veld_ir::{AuthKind, Endpoint, EndpointFact, EndpointRequire, Ir, Service, ServiceId, Type};

use crate::conditional;
use crate::machine;
use crate::policy;
use crate::rule_expr::{EndpointScope, JavaExpr, ScopeVar, rule_value_type};
use crate::spec::{param_idents, spec_const};
use crate::tree::FileCtx;
use crate::types::{codec_expr, java_type};
use crate::util::{doc_comment, java_str, member_ident, unique};

/// Whether an endpoint has statements.
#[must_use]
pub fn endpoint_has_rules(ep: &Endpoint) -> bool {
    !ep.rules.is_empty()
}

/// Whether any endpoint of the service has statements.
#[must_use]
pub fn service_has_rules(svc: &Service) -> bool {
    svc.endpoints.iter().any(endpoint_has_rules)
}

/// Whether any endpoint of the given services has a `transition`.
#[must_use]
pub fn services_have_transition(ir: &Ir, services: &[ServiceId]) -> bool {
    services.iter().any(|id| {
        ir.service(*id)
            .endpoints
            .iter()
            .any(|e| e.rules.transition.is_some())
    })
}

/// The record of the facts an endpoint's handler gets: `CreateBookingFacts` for `CreateBookingRequest`.
#[must_use]
pub fn facts_name(request: &str) -> String {
    format!(
        "{}Facts",
        request.strip_suffix("Request").unwrap_or(request)
    )
}

/// The loader interface of a service.
#[must_use]
pub fn loaders_name(service: &str) -> String {
    format!("{service}FactLoaders")
}

/// The fact record components and loader methods are named like the facts.
pub(crate) fn fact_idents(facts: &[EndpointFact]) -> Vec<String> {
    unique(facts.iter().map(|f| member_ident(&f.name)).collect())
}

/// `public record CreateBookingFacts(Room room, Coupon coupon)`: the facts the loaders found,
/// handed to the handler with the parameters. An absent optional fact is `null`.
pub fn write_facts_record(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    request: &str,
    ep: &Endpoint,
) {
    let idents = fact_idents(&ep.rules.facts);
    let docs: Vec<(String, String)> = ep
        .rules
        .facts
        .iter()
        .zip(&idents)
        .map(|(f, id)| {
            let note = if f.optional {
                "The loaded fact, or `null` when it is absent."
            } else {
                "The loaded fact."
            };
            (id.clone(), note.to_string())
        })
        .collect();
    doc_comment(
        w,
        Some(&format!(
            "The facts of `{}`, loaded before the handler runs and handed to it with the parameters.",
            ep.name
        )),
        &docs,
        None,
    );
    w.line("@SuppressWarnings(\"deprecation\")");
    let comps: Vec<String> = ep
        .rules
        .facts
        .iter()
        .zip(&idents)
        .map(|(f, id)| format!("{} {id}", java_type(fc, &f.ty, f.optional)))
        .collect();
    w.line(format!(
        "public record {}({}) {{}}",
        facts_name(request),
        comps.join(", ")
    ));
}

/// The loader interface: one method per entry of `Service.facts`.
pub fn write_loaders(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service, class: &str) {
    let ctx = fc.rt("RequestContext");
    let optional = fc.name("java.util.Optional");
    doc_comment(
        w,
        Some(&format!(
            "One loader per fact of the `{}` service. A loader returns an empty `Optional` when it finds nothing.",
            svc.name
        )),
        &[],
        None,
    );
    w.line("@SuppressWarnings(\"deprecation\")");
    w.open(format!("public interface {class}"));
    let names = unique(svc.facts.iter().map(|f| member_ident(&f.name)).collect());
    for (fact, name) in svc.facts.iter().zip(&names) {
        let ty = java_type(fc, &fact.ty, true);
        let key = match &fact.key {
            Some(k) => format!("{} key, ", java_type(fc, k, false)),
            None => String::new(),
        };
        w.line(format!(
            "{optional}<{ty}> {name}({key}{ctx} ctx) throws Exception;"
        ));
    }
    w.close();
}

/// What the body of one endpoint's call needs.
#[derive(Debug)]
pub struct Site<'a> {
    pub svc: &'a Service,
    pub ep: &'a Endpoint,
    /// The request record (`CreateBookingRequest`).
    pub request: &'a str,
}

pub(crate) fn unsupported(site: &Site<'_>, what: &str) -> GenDiagnostic {
    let mut d = GenDiagnostic::new(
        "E0801",
        format!(
            "{}.{}: generator `java` cannot represent {what}",
            site.svc.name, site.ep.name
        ),
    );
    d.origin = site.ep.rules.first_origin().cloned();
    d
}

/// The caller as a name in the scope of the endpoint, and the Java type of the principal: only
/// where the contract uses policies, the service has a principal and the endpoint has auth.
fn principal_of(fc: &mut FileCtx<'_, '_>, site: &Site<'_>) -> Option<(ScopeVar, String)> {
    if !fc.tree.policies || site.ep.auth.kind == AuthKind::None {
        return None;
    }
    let principal = site.svc.principal.as_ref()?;
    let class = fc.decl(principal.decl);
    let var = ScopeVar {
        name: principal.name.clone(),
        code: "principal".to_string(),
        ty: Type::Decl(principal.decl),
        optional: false,
    };
    Some((var, class))
}

/// `Name.fromWire(data, status, null)` for a prelude error the server raises itself, with the
/// status the endpoint declares for it (`fallback` when it declares none); the untyped error when
/// the prelude has no such declaration.
pub(crate) fn prelude_error(
    fc: &mut FileCtx<'_, '_>,
    site: &Site<'_>,
    (name, fallback): (&str, u16),
    data: &str,
) -> String {
    let status = site
        .ep
        .errors
        .iter()
        .find(|e| fc.tree.ir.decl(e.decl).name == name)
        .map_or(fallback, |e| e.status);
    match fc.tree.prelude_decl(name) {
        Some(id) => format!("{}.fromWire({data}, {status}, null)", fc.decl(id)),
        None => {
            let untyped = fc.rt("UntypedError");
            format!("new {untyped}(\"about:blank\", \"{name}\", {data}, {status}, null)")
        }
    }
}

fn scope(ep: &Endpoint, facts: &[String], principal: Option<ScopeVar>) -> EndpointScope {
    let params = param_idents(ep)
        .into_iter()
        .map(|(p, ident)| ScopeVar {
            name: p.name.clone(),
            code: format!("request.{ident}()"),
            ty: p.ty.clone(),
            optional: p.optional,
        })
        .collect();
    let facts = ep
        .rules
        .facts
        .iter()
        .zip(facts)
        .map(|(f, ident)| ScopeVar {
            name: f.name.clone(),
            code: format!("fact_{ident}"),
            ty: f.ty.clone(),
            optional: f.optional,
        })
        .collect();
    EndpointScope {
        params,
        facts,
        principal,
        result: ep.success.ty.clone().filter(|_| !ep.success.stream),
    }
}

pub(crate) fn list_of(fc: &mut FileCtx<'_, '_>, items: &[String]) -> String {
    let list = fc.name("java.util.List");
    let items: Vec<String> = items.iter().map(|i| java_str(i)).collect();
    format!("{list}.of({})", items.join(", "))
}

fn message(name: Option<&str>, custom: Option<&str>, unnamed: &str) -> String {
    match (custom, name) {
        (Some(m), _) => java_str(m),
        (None, Some(n)) => java_str(&format!("violates rule {n}")),
        (None, None) => java_str(unnamed),
    }
}

/// The factories of the two prelude errors step 6 raises itself.
fn factories(fc: &mut FileCtx<'_, '_>) -> (String, String) {
    let router = fc.rt("Router");
    let json = fc.rt("Json");
    let validation = match fc.tree.prelude_decl("ValidationError") {
        Some(id) => format!(
            "{}.fromWire({router}.issuesToWire(issues), 422, null)",
            fc.decl(id)
        ),
        None => {
            let untyped = fc.rt("UntypedError");
            format!(
                "new {untyped}(\"about:blank\", \"ValidationError\", {router}.issuesToWire(issues), 422, null)"
            )
        }
    };
    let data = format!("{json}.object().put(\"resource\", {json}.of(resource)).build()");
    let not_found = match fc.tree.prelude_decl("NotFound") {
        Some(id) => format!("{}.fromWire({data}, 404, null)", fc.decl(id)),
        None => {
            let untyped = fc.rt("UntypedError");
            format!("new {untyped}(\"about:blank\", \"NotFound\", {data}, 404, null)")
        }
    };
    (validation, not_found)
}

fn write_fact(
    w: &mut CodeWriter,
    printer: &mut JavaExpr<'_, '_, '_>,
    site: &Site<'_>,
    (fact, ident, method): (&EndpointFact, &str, &str),
    diags: &mut Vec<GenDiagnostic>,
) {
    let ty = java_type(printer.file(), &fact.ty, fact.optional);
    let ctx = "ctx";
    let (args, presence) = match &fact.key {
        Some(key) => {
            let presence = if fact.optional {
                printer.presence(key)
            } else {
                None
            };
            match printer.app_value(key) {
                Some(code) => (format!("{code}, {ctx}"), presence),
                None => {
                    diags.push(unsupported(site, "the key of a fact as a loader argument"));
                    (format!("null, {ctx}"), None)
                }
            }
        }
        None => (ctx.to_string(), None),
    };
    let load = format!("loaders.{method}({args})");
    let name = java_str(&fact.name);
    let line = match (fact.optional, presence) {
        (false, _) => format!("{ty} fact_{ident} = run.required({name}, {load});"),
        (true, Some(cond)) => {
            format!("{ty} fact_{ident} = {cond} ? {load}.orElse(null) : null;")
        }
        (true, None) => format!("{ty} fact_{ident} = {load}.orElse(null);"),
    };
    w.line(line);
}

fn write_otherwise(
    printer: &mut JavaExpr<'_, '_, '_>,
    site: &Site<'_>,
    r: &EndpointRequire,
    diags: &mut Vec<GenDiagnostic>,
) -> String {
    let Some(e) = &r.else_error else {
        return "null".to_string();
    };
    let class = printer.file().decl(e.decl);
    let json = printer.file().rt("Json");
    let rules = printer.file().rt("EndpointRules");
    let ir = printer.file().tree.ir;
    let decl = ir.decl(e.decl);
    let mut puts = String::new();
    for field in &e.fields {
        let Some(value) = printer.app_value(&field.value) else {
            diags.push(unsupported(site, "a value of the `else` payload"));
            continue;
        };
        let ty = decl
            .fields()
            .iter()
            .find(|f| f.name == field.name)
            .map(|f| f.ty.clone());
        let Some(ty) = ty else {
            diags.push(unsupported(site, "a payload field the error does not have"));
            continue;
        };
        let codec = codec_expr(printer.file(), &ty);
        puts.push_str(&format!(
            ".put({}, {value}, {codec})",
            java_str(&field.wire)
        ));
    }
    format!(
        "() -> {class}.fromWire({rules}.payload({class}.SCHEMA, {json}.object(){puts}.build()), {}, null)",
        e.status
    )
}

/// Writes the statements of the call of an endpoint with rules. `call` is the text that calls the
/// handler (`impl.createBooking(request, facts, ctx)`) and `encode` turns `result` into wire form
/// (`None` for an endpoint without a response body).
pub fn write_call_body(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    site: &Site<'_>,
    call: &str,
    encode: Option<&str>,
) -> Vec<GenDiagnostic> {
    let ep = site.ep;
    let mut diags = Vec::new();
    let idents = fact_idents(&ep.rules.facts);
    let principal = principal_of(fc, site);
    let held = principal.as_ref().map(|(_, class)| class.clone());
    let mut printer = JavaExpr::for_endpoint(fc, scope(ep, &idents, principal.map(|(v, _)| v)));
    // The `ensure` expressions come first: they decide the slots of `old(...)`.
    let ensures: Vec<String> = ep
        .rules
        .ensures
        .iter()
        .map(|e| print_expr(&mut printer, &e.expr).code)
        .collect();
    let request = site.request;
    write_start(w, &mut printer, site);
    if let Some(class) = &held {
        w.line(format!("{class} principal = ({class}) ctx.principal();"));
    }
    let loaders: Vec<String> = service_methods(site);
    for (fact, ident) in ep.rules.facts.iter().zip(&idents) {
        let method = loaders_method(site, &fact.name, &loaders);
        write_fact(w, &mut printer, site, (fact, ident, &method), &mut diags);
    }
    policy::write_allows(w, &mut printer, site, &mut diags);
    conditional::write_guard(w, &mut printer, site, &mut diags);
    machine::write_guard(w, &mut printer, site, &mut diags);
    for r in &ep.rules.requires {
        write_require(w, &mut printer, site, r, &mut diags);
    }
    write_saved(w, &mut printer);
    if !ep.rules.facts.is_empty() {
        let args: Vec<String> = idents.iter().map(|i| format!("fact_{i}")).collect();
        let facts = facts_name(request);
        w.line(format!("{facts} facts = new {facts}({});", args.join(", ")));
    }
    match encode {
        Some(_) => w.line(format!("var result = {call};")),
        None => w.line(format!("{call};")),
    };
    write_finish(w, &mut printer, site, &ensures, encode, &mut diags);
    diags.extend(printer_problems(&printer, site));
    diags
}

/// The request, the run (the one clock reading) and `now`.
fn write_start(w: &mut CodeWriter, printer: &mut JavaExpr<'_, '_, '_>, site: &Site<'_>) {
    let (rules, spec) = (printer_rt(printer, "EndpointRules"), spec_const(site.ep));
    let (validation, not_found) = factories(printer.file());
    let request = site.request;
    w.line(format!("{request} request = {request}.fromWire(params);"));
    w.line(format!(
        "{rules}.Run run = {rules}.start(hooks, {spec}, issues -> {validation}, resource -> {not_found}, clock.applyAsLong(ctx));"
    ));
    w.line("long now = run.now();");
}

/// The `ensure` checks, registered after the handler, and the encoded result.
fn write_finish(
    w: &mut CodeWriter,
    printer: &mut JavaExpr<'_, '_, '_>,
    site: &Site<'_>,
    ensures: &[String],
    encode: Option<&str>,
    diags: &mut Vec<GenDiagnostic>,
) {
    for (e, cond) in site.ep.rules.ensures.iter().zip(ensures) {
        let fields = list_of(printer.file(), &e.reads);
        // An audited `ensure` is registered like an enforced one and never rejects (section 17.1).
        let register = if e.rollout.is_some() {
            "ensureAudited"
        } else {
            "ensure"
        };
        w.line(format!(
            "run.{register}({}, {}, {fields}, {}, () -> {cond});",
            java_str(&e.id),
            java_str(e.name.as_deref().unwrap_or("ensure")),
            message(
                e.name.as_deref(),
                e.message.as_deref(),
                "postcondition failed"
            )
        ));
    }
    if encode.is_some() {
        machine::write_target(w, printer, site, diags);
    }
    match encode {
        Some(enc) => {
            let json = printer_rt(printer, "Json");
            w.line(format!("{json} encoded = {enc};"));
            w.line("run.arm();");
            w.line("return encoded;");
        }
        None => {
            w.line("run.arm();");
            w.line("return null;");
        }
    }
}

fn write_require(
    w: &mut CodeWriter,
    printer: &mut JavaExpr<'_, '_, '_>,
    site: &Site<'_>,
    r: &EndpointRequire,
    diags: &mut Vec<GenDiagnostic>,
) {
    let cond = print_expr(printer, &r.expr).code;
    let fields = list_of(printer.file(), &r.reads);
    if r.rollout.is_some() {
        // An audited `require` never answers, so its `else` payload is never built: it is not
        // generated at all (`spec/rules.md` section 17.1).
        w.line(format!(
            "run.requireAudited({}, {}, {fields}, {}, () -> {cond});",
            java_str(&r.id),
            java_str(r.name.as_deref().unwrap_or("require")),
            message(
                r.name.as_deref(),
                r.message.as_deref(),
                "precondition failed"
            )
        ));
        return;
    }
    let otherwise = write_otherwise(printer, site, r, diags);
    w.line(format!(
        "run.require({}, {}, {fields}, {},",
        java_str(&r.id),
        java_str(r.name.as_deref().unwrap_or("require")),
        message(
            r.name.as_deref(),
            r.message.as_deref(),
            "precondition failed"
        )
    ));
    w.indent().indent();
    w.line(format!("() -> {cond},"));
    w.line(format!("{otherwise});"));
    w.dedent().dedent();
}

/// `Saved<Long> o0 = ...` for every `old(...)` operand, evaluated before the handler runs.
fn write_saved(w: &mut CodeWriter, printer: &mut JavaExpr<'_, '_, '_>) {
    let slots: Vec<(String, veld_ir::ExprType)> = printer.olds().to_vec();
    let rules = printer_rt(printer, "EndpointRules");
    for (i, (code, ty)) in slots.iter().enumerate() {
        let ty = rule_value_type(printer.file(), ty);
        w.line(format!(
            "{rules}.Saved<{ty}> o{i} = run.active() ? {rules}.Saved.of(() -> {code}) : {rules}.Saved.skipped();"
        ));
    }
}

fn printer_rt(p: &mut JavaExpr<'_, '_, '_>, simple: &str) -> String {
    p.file().rt(simple)
}

fn printer_problems(p: &JavaExpr<'_, '_, '_>, site: &Site<'_>) -> Vec<GenDiagnostic> {
    p.unsupported()
        .iter()
        .map(|what| unsupported(site, what))
        .collect()
}

/// The loader method names of the service, by fact name, in `Service.facts` order.
fn service_methods(site: &Site<'_>) -> Vec<String> {
    unique(
        site.svc
            .facts
            .iter()
            .map(|f| member_ident(&f.name))
            .collect(),
    )
}

fn loaders_method(site: &Site<'_>, fact: &str, methods: &[String]) -> String {
    site.svc
        .facts
        .iter()
        .position(|f| f.name == fact)
        .and_then(|i| methods.get(i).cloned())
        .unwrap_or_else(|| member_ident(fact))
}

/// The refusals that exist before anything is printed.
#[must_use]
pub fn refusals(svc: &Service) -> Vec<GenDiagnostic> {
    let mut out = Vec::new();
    for ep in svc.endpoints.iter().filter(|e| endpoint_has_rules(e)) {
        let streams = ep.method == veld_ir::Method::Ws || ep.success.stream;
        if streams {
            let mut d = GenDiagnostic::new(
                "E0801",
                format!(
                    "{}.{}: generator `java` does not run endpoint rules on a streaming endpoint",
                    svc.name, ep.name
                ),
            );
            d.origin = ep.rules.first_origin().cloned();
            out.push(d);
        }
    }
    out
}
