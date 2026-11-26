//! `@version` and `@idempotent` in the server and client classes (`spec/rules.md` sections 11.7
//! to 11.12, `spec/targets/java.md` section 13): the version guard of step 6, the `version` the
//! handler receives, the conditional headers the router reads (`Conditions`), the idempotency
//! hooks of `<Service>Hooks`, and the `ifMatch` and `idempotencyKey` of the client's requests.

use veld_codegen::{CodeWriter, GenDiagnostic};
use veld_ir::{DeclKind, Endpoint, EndpointVersion, Feature, Ir, Scalar, Service, ServiceId, Type};

use crate::endpoint::{Site, prelude_error, unsupported};
use crate::models::field_idents;
use crate::rule_expr::JavaExpr;
use crate::tree::FileCtx;
use crate::types::java_type;
use crate::util::{end_call, java_str, member_ident, open_call};

/// The name of the record component and request parameter that carries `If-Match`.
pub const IF_MATCH: &str = "ifMatch";
/// The name of the record component and request parameter that carries the idempotency key.
pub const KEY: &str = "idempotencyKey";

/// What the Javadoc of an endpoint with a version guard says about the `version` argument.
pub const VERSION_NOTE: &str = "The `version` argument is the version the server checked before it called this method: the version the caller sent in `If-Match`, which the guarded fact held. It is `null` when the request sent no `If-Match`, sent `*`, or the guarded fact is absent. The check is a guard, not a guarantee: a second request can change the row after it. Write conditionally (only where the row still holds `version`) and answer `PreconditionFailed` yourself when the row has moved on.";

/// Whether the services use `@version` or `@idempotent`: an endpoint with a guard, a response that
/// owes an `ETag`, or an idempotency key.
#[must_use]
pub fn services_use(ir: &Ir, services: &[ServiceId]) -> bool {
    let features = ir.http_features_used(services);
    features.contains(&Feature::RulesVersion) || features.contains(&Feature::Idempotency)
}

/// Whether an endpoint has anything for the router: a guard, an `ETag` or a key.
#[must_use]
pub fn endpoint_uses(ir: &Ir, ep: &Endpoint) -> bool {
    ep.rules.version.is_some() || ep.idempotency.is_some() || ir.etag_field(ep).is_some()
}

/// Whether the service has an endpoint the router has to know about.
#[must_use]
pub fn service_uses(ir: &Ir, svc: &Service) -> bool {
    svc.endpoints.iter().any(|ep| endpoint_uses(ir, ep))
}

/// Whether a server of the service needs the store hook, and whether it needs the caller hook.
#[must_use]
pub fn hooks_needed(svc: &Service) -> (bool, bool) {
    let keyed = svc.endpoints.iter().filter_map(|e| e.idempotency.as_ref());
    let (mut store, mut caller) = (false, false);
    for idem in keyed {
        store = true;
        caller |= idem.caller;
    }
    (store, caller)
}

/// `E0801` for a declared parameter that would be shadowed by `ifMatch` or `idempotencyKey` of
/// the client's request (`spec/rules.md` section 11.12).
#[must_use]
pub fn refusals(svc: &Service) -> Vec<GenDiagnostic> {
    let mut out = Vec::new();
    for ep in &svc.endpoints {
        let reserved = [
            (IF_MATCH, ep.rules.version.is_some()),
            (KEY, ep.idempotency.is_some()),
        ];
        for (name, taken) in reserved {
            let clash = ep.params.iter().any(|p| member_ident(&p.name) == name);
            if taken && clash {
                out.push(GenDiagnostic::new(
                    "E0801",
                    format!(
                        "{}.{}: generator `java` cannot represent a parameter called `{name}` on an endpoint that takes it as the {}",
                        svc.name,
                        ep.name,
                        if name == IF_MATCH { "`If-Match` of `@version`" } else { "key of `@idempotent`" },
                    ),
                ));
            }
        }
    }
    out
}

/// The field of the guarded fact's model that holds the version, and its index among the fields.
fn version_field<'a>(
    ir: &'a Ir,
    ep: &Endpoint,
    guard: &EndpointVersion,
) -> Option<(&'a veld_ir::Field, String)> {
    let fact = ep.rules.facts.iter().find(|f| f.name == guard.fact)?;
    let Type::Decl(id) = &fact.ty else {
        return None;
    };
    let decl = ir.decl(*id);
    if !matches!(decl.kind, DeclKind::Model(_)) {
        return None;
    }
    let idents = field_idents(decl.fields());
    let at = decl.fields().iter().position(|f| f.name == guard.field)?;
    Some((decl.fields().get(at)?, idents.get(at)?.clone()))
}

/// The Java type of the `version` argument: the version field's type, boxed (it may be `null`).
pub fn version_type(fc: &mut FileCtx<'_, '_>, ep: &Endpoint) -> Option<String> {
    let guard = ep.rules.version.as_ref()?;
    let ir = fc.tree.ir;
    let (field, _) = version_field(ir, ep, guard)?;
    Some(java_type(fc, &field.ty, true))
}

/// The guard of a `@version` fact (step 6, after the policies and before the transition): leaves
/// the `version` the handler receives.
pub fn write_guard(
    w: &mut CodeWriter,
    printer: &mut JavaExpr<'_, '_, '_>,
    site: &Site<'_>,
    diags: &mut Vec<GenDiagnostic>,
) {
    let Some(guard) = &site.ep.rules.version else {
        return;
    };
    let ir = printer.file().tree.ir;
    let Some((field, accessor)) = version_field(ir, site.ep, guard) else {
        diags.push(unsupported(
            site,
            "a version guard on a fact it cannot read",
        ));
        return;
    };
    let Some(index) = site
        .ep
        .rules
        .facts
        .iter()
        .position(|f| f.name == guard.fact)
    else {
        return;
    };
    let idents = crate::endpoint::fact_idents(&site.ep.rules.facts);
    let fact = format!("fact_{}", idents[index]);
    let read = if site.ep.rules.facts[index].optional {
        format!("{fact} == null ? null : Long.valueOf({fact}.{accessor}())")
    } else {
        format!("Long.valueOf({fact}.{accessor}())")
    };
    let json = printer.file().rt("Json");
    let data = format!(
        "{json}.object().put(\"expected\", {json}.of(Long.toString(expected))).put(\"actual\", {json}.of(Long.toString(actual))).build()"
    );
    let error = prelude_error(printer.file(), site, ("PreconditionFailed", 412), &data);
    let checked = format!(
        "run.version(ctx, {}, {read}, (expected, actual) -> {error})",
        java_str(&guard.id)
    );
    if matches!(field.ty, Type::Scalar(Scalar::Int)) {
        w.line(format!("Long version_checked = {checked};"));
        w.line("Integer version = version_checked == null ? null : Integer.valueOf(version_checked.intValue());");
    } else {
        w.line(format!("Long version = {checked};"));
    }
}

/// `static Map<String, Conditions> conditions()`: what the router reads for the endpoints of the
/// service that have a guard, an `ETag` or a key, by operation signature. Package-private: the
/// test handler builds the same router.
pub fn write_conditions(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service) {
    let (map, conditions) = (fc.name("java.util.Map"), fc.rt("Conditions"));
    w.line("/** The conditional headers of the endpoints, by operation signature; the test handler reuses them. */");
    w.open(format!("static {map}<String, {conditions}> conditions()"));
    open_call(w, format!("return {map}.ofEntries"));
    let ir = fc.tree.ir;
    let uses: Vec<&Endpoint> = svc
        .endpoints
        .iter()
        .filter(|ep| endpoint_uses(ir, ep))
        .collect();
    for (i, ep) in uses.iter().enumerate() {
        let entry = condition_entry(fc, ep);
        let end = if i + 1 == uses.len() { ");" } else { "," };
        w.line(format!("{map}.entry({}, {entry}){end}", java_str(&ep.sig)));
    }
    end_call(w);
    w.close();
}

fn condition_entry(fc: &mut FileCtx<'_, '_>, ep: &Endpoint) -> String {
    let conditions = fc.rt("Conditions");
    let ir = fc.tree.ir;
    let etag = ir
        .etag_field(ep)
        .map_or_else(|| "null".to_string(), |f| java_str(&f.wire));
    let idem = match &ep.idempotency {
        Some(i) => {
            let status = ep
                .errors
                .iter()
                .find(|e| ir.decl(e.decl).name == "Conflict")
                .map_or(409, |e| e.status);
            let sig = match fc.tree.prelude_decl("Conflict") {
                Some(id) => format!("{}.VELD_SIG", fc.decl(id)),
                None => java_str("about:blank"),
            };
            format!(
                "new {conditions}.Idempotency({}, {}, {}, {sig}, {status})",
                java_str(&i.header),
                i.required,
                i.caller
            )
        }
        None => "null".to_string(),
    };
    format!(
        "new {conditions}({etag}, {}, {idem})",
        ep.rules.version.is_some()
    )
}

/// The `router` arguments up to the standard signatures: `endpoints`, or `endpoints, conditions()`
/// for a service that has conditional endpoints.
#[must_use]
pub fn router_args(ir: &Ir, svc: &Service) -> &'static str {
    if service_uses(ir, svc) {
        "endpoints, conditions()"
    } else {
        "endpoints"
    }
}

/// The components `ifMatch` and `idempotencyKey` of a client's request record, with their types
/// and docs: `(name, type, doc)`.
pub fn client_components(
    fc: &mut FileCtx<'_, '_>,
    ep: &Endpoint,
) -> Vec<(&'static str, String, String)> {
    let mut out = Vec::new();
    if ep.rules.version.is_some() {
        out.push((
            IF_MATCH,
            fc.rt("IfMatch"),
            "The `If-Match` header: `IfMatch.of(version)` for the version you read, `IfMatch.any()` for `*`, or `null` to send none.".to_string(),
        ));
    }
    if let Some(idem) = &ep.idempotency {
        let doc = if idem.required {
            format!("The idempotency key, sent in `{}`; required.", idem.header)
        } else {
            format!(
                "The idempotency key, sent in `{}`; optional (`null` sends none).",
                idem.header
            )
        };
        out.push((KEY, "String".to_string(), doc));
    }
    out
}

/// The headers argument of a client call: `ClientCore.conditional(ifMatch, "Idempotency-Key", key)`
/// with `null` for what the endpoint does not take; `None` for an endpoint with neither.
pub fn client_headers(fc: &mut FileCtx<'_, '_>, ep: &Endpoint) -> Option<String> {
    if ep.rules.version.is_none() && ep.idempotency.is_none() {
        return None;
    }
    let core = fc.rt("ClientCore");
    let if_match = if ep.rules.version.is_some() {
        "request.ifMatch()"
    } else {
        "null"
    };
    let (header, key) = match &ep.idempotency {
        Some(i) => (java_str(&i.header), "request.idempotencyKey()"),
        None => ("null".to_string(), "null"),
    };
    Some(format!("{core}.conditional({if_match}, {header}, {key})"))
}
