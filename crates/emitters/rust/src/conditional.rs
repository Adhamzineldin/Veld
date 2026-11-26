//! `@version` and `@idempotent` (`spec/rules.md` sections 11.7 to 11.12): what an endpoint needs
//! of the runtime, the `version` the handler receives, the hooks of the idempotency store and the
//! parameters the client takes for `If-Match` and the key.

use veld_codegen::{CodeWriter, GenDiagnostic};
use veld_ir::{DeclKind, Endpoint, Ir, Service, ServiceId, Type};

use crate::endpoints::{self, Kind};
use crate::types::{Ctx, rust_type};
use crate::util::{lit, snake, unsupported};

/// The Rust names of the two client parameters, which a declared parameter must not take.
pub const IF_MATCH_PARAM: &str = "if_match";
pub const KEY_PARAM: &str = "idempotency_key";

/// Whether the endpoint has an `If-Match` guard.
#[must_use]
pub fn has_guard(ep: &Endpoint) -> bool {
    ep.rules.version.is_some()
}

/// Whether the endpoint is `@idempotent`.
#[must_use]
pub fn is_idempotent(ep: &Endpoint) -> bool {
    ep.idempotency.is_some()
}

/// Whether a server has to read `If-Match` or write an `ETag` for the endpoint.
#[must_use]
pub fn serves_version(ir: &Ir, ep: &Endpoint) -> bool {
    has_guard(ep) || ir.etag_field(ep).is_some()
}

/// Whether any of the services has an endpoint that a server serves with a guard or an `ETag`.
#[must_use]
pub fn any_version(ir: &Ir, services: &[ServiceId]) -> bool {
    services.iter().any(|s| {
        ir.service(*s)
            .endpoints
            .iter()
            .any(|e| serves_version(ir, e))
    })
}

/// Whether any of the services has an endpoint with a guard (what a client takes `if_match` for).
#[must_use]
pub fn any_guard(ir: &Ir, services: &[ServiceId]) -> bool {
    services
        .iter()
        .any(|s| ir.service(*s).endpoints.iter().any(has_guard))
}

/// Whether any of the services has an `@idempotent` endpoint.
#[must_use]
pub fn any_idempotent(ir: &Ir, services: &[ServiceId]) -> bool {
    services.iter().any(|s| svc_idempotent(ir.service(*s)))
}

/// Whether the service has an `@idempotent` endpoint.
#[must_use]
pub fn svc_idempotent(svc: &Service) -> bool {
    svc.endpoints.iter().any(is_idempotent)
}

/// Whether the service has an endpoint whose keys are scoped to the caller.
#[must_use]
pub fn svc_needs_caller(svc: &Service) -> bool {
    svc.endpoints
        .iter()
        .any(|e| e.idempotency.as_ref().is_some_and(|i| i.caller))
}

/// What the generator cannot express: a guard on a stream, and a declared parameter that would
/// take the place of `if_match` or `idempotency_key` in the request struct of the client.
///
/// # Errors
/// `E0801` for the first such endpoint.
pub fn check(ir: &Ir, services: &[ServiceId]) -> Result<(), Vec<GenDiagnostic>> {
    for sid in services {
        let svc = ir.service(*sid);
        for ep in &svc.endpoints {
            let (guard, idem) = (has_guard(ep), is_idempotent(ep));
            if !guard && !idem {
                continue;
            }
            if guard && endpoints::kind(ep) != Kind::Http {
                return Err(unsupported(format!(
                    "generator `rust` does not support a `@version` guard on the streaming endpoint `{}.{}`",
                    svc.name, ep.name
                )));
            }
            for p in &ep.params {
                let name = snake(&p.name);
                if (guard && name == IF_MATCH_PARAM) || (idem && name == KEY_PARAM) {
                    return Err(unsupported(format!(
                        "generator `rust`: endpoint `{}.{}` has a parameter `{}` that the client needs for `{}`",
                        svc.name,
                        ep.name,
                        p.name,
                        if name == KEY_PARAM {
                            "the idempotency key"
                        } else {
                            "If-Match"
                        }
                    )));
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The version the handler receives
// ---------------------------------------------------------------------------

/// The type of the version field of the guarded fact's model.
fn version_field_type(ir: &Ir, ep: &Endpoint) -> Option<Type> {
    let v = ep.rules.version.as_ref()?;
    let fact = ep.rules.facts.iter().find(|f| f.name == v.fact)?;
    let Type::Decl(id) = &fact.ty else {
        return None;
    };
    match &ir.decl(*id).kind {
        DeclKind::Model(m) => m
            .fields
            .iter()
            .find(|f| f.name == v.field)
            .map(|f| f.ty.clone()),
        DeclKind::Enum(_)
        | DeclKind::Error(_)
        | DeclKind::Union(_)
        | DeclKind::Const(_)
        | DeclKind::Event(_) => None,
    }
}

/// The type of the version in the developer's form: `i64` for a `long`, `i32` for an `int`.
fn is_int(ir: &Ir, ep: &Endpoint) -> bool {
    matches!(
        version_field_type(ir, ep),
        Some(Type::Scalar(veld_ir::Scalar::Int))
    )
}

/// `, version: Option<i64>` for the service method of an endpoint with a guard: it follows the
/// facts and the transition (`spec/rules.md` section 11.9).
pub fn version_param(ctx: &Ctx<'_, '_>, ep: &Endpoint) -> String {
    let Some(ty) = version_field_type(ctx.tree.ir, ep) else {
        return String::new();
    };
    format!(", version: Option<{}>", rust_type(ctx, &ty, false))
}

/// The doc lines of the service method of an endpoint with a guard: like a transition it is a
/// guard, not an atomic guarantee.
pub fn version_docs(w: &mut CodeWriter, ep: &Endpoint) {
    let Some(v) = &ep.rules.version else {
        return;
    };
    if ep.docs.is_some() || ep.deprecated.is_some() || ep.rules.transition.is_some() {
        w.line("///");
    }
    w.line(format!(
        "/// `version` is the version of `{}.{}` that the caller sent in `If-Match` and that was equal to the",
        v.fact, v.wire
    ));
    w.line("/// current one when it was checked. It is `None` without `If-Match`, with `*`, and when the");
    w.line("/// fact is absent. **This is a guard, not an atomic guarantee:** another request can change the");
    w.line("/// row after the check. Write conditionally (`... where version = ?`), and answer");
    w.line("/// `PreconditionFailed` yourself when that write finds that the row has moved on.");
}

/// What step 6 hands to the handler next to the facts: the `version`, in the bound arguments.
pub fn hand_over_version(w: &mut CodeWriter, ep: &Endpoint) {
    if has_guard(ep) {
        w.line("args.insert(\"version\".to_string(), serde_json::to_value(version).unwrap_or(serde_json::Value::Null));");
    }
}

/// The version guard of step 6 (after the policies, before the transition guard): `412
/// PreconditionFailed` when the tag the caller sent is not the version of the guarded fact.
///
/// # Errors
/// What the generator cannot express: the fact or the field is not in the IR as expected.
pub fn version_guard(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    index: usize,
    ep: &Endpoint,
) -> Result<(), String> {
    let Some(v) = &ep.rules.version else {
        return Ok(());
    };
    let fact = ep
        .rules
        .facts
        .iter()
        .find(|f| f.name == v.fact)
        .ok_or("the fact of the version guard is not declared")?;
    let ty = version_field_type(ctx.tree.ir, ep)
        .ok_or("the version field of the guard is not a field of the fact")?;
    let local = crate::block::fact_local(&fact.name);
    let place = if fact.optional {
        "found".to_string()
    } else {
        local.clone()
    };
    let field = format!("{place}.{}", snake(&v.field));
    let int = is_int(ctx.tree.ir, ep);
    let prelude = ctx.tree.prelude_decl("PreconditionFailed");
    let sig = prelude.map_or_else(
        || "about:blank".to_string(),
        |id| ctx.tree.ir.decl(id).sig.clone(),
    );
    let version_ty = rust_type(ctx, &ty, false);
    w.line(format!("// version {}.{}", v.fact, v.wire));
    w.line(format!("let mut version: Option<{version_ty}> = None;"));
    w.open("if let Some(veld::conditional::IfMatch::Version(expected)) = ctx.if_match()");
    if fact.optional {
        // An absent fact has nothing to compare: the request passes and nothing is handed over.
        w.open(format!("if let Some(found) = {local}"));
    }
    w.line(format!(
        "let actual: i64 = {};",
        if int {
            format!("i64::from({field})")
        } else {
            field.clone()
        }
    ));
    w.open("if actual != expected");
    w.line(format!(
        "return Err(vr::refusal(&ENDPOINTS[{index}], {}, &[(\"expected\", &expected.to_string()), (\"actual\", &actual.to_string())], vr::guard_violation({}, &[], \"violates rule version\")));",
        lit(&sig),
        lit(&v.id)
    ));
    w.close();
    w.line(if int {
        "version = i32::try_from(expected).ok();".to_string()
    } else {
        "version = Some(expected);".to_string()
    });
    if fact.optional {
        w.close();
    }
    w.close();
    Ok(())
}

// ---------------------------------------------------------------------------
// The client
// ---------------------------------------------------------------------------

/// The header of the key and whether it is required, for an `@idempotent` endpoint.
#[must_use]
pub fn key_of(ep: &Endpoint) -> Option<(&str, bool)> {
    ep.idempotency
        .as_ref()
        .map(|i| (i.header.as_str(), i.required))
}

/// Whether the request struct of a client has `if_match` or `idempotency_key`.
#[must_use]
pub fn has_client_fields(ep: &Endpoint) -> bool {
    has_guard(ep) || is_idempotent(ep)
}

/// The doc lines of a client method that takes `if_match` or `idempotency_key`.
pub fn client_docs(w: &mut CodeWriter, ep: &Endpoint) {
    if !has_client_fields(ep) {
        return;
    }
    if ep.docs.is_some() || ep.deprecated.is_some() {
        w.line("///");
    }
    if let Some(v) = &ep.rules.version {
        w.line(format!(
            "/// `request.if_match` is sent as `If-Match`: a version of `{}.{}` the caller read (`IfMatch::Version(3)`), or",
            v.fact, v.wire
        ));
        w.line("/// `IfMatch::Any`. A stale version answers `PreconditionFailed`.");
    }
    if let Some(i) = &ep.idempotency {
        w.line(format!(
            "/// `request.idempotency_key` is sent in the header `{}`; the server answers a retry with the same key and parameters",
            i.header
        ));
        w.line("/// with the stored answer. The client never invents a key.");
    }
}

/// The fields of the request struct of a client that stand for `If-Match` and the key.
pub fn client_fields(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, ep: &Endpoint) {
    if let Some(v) = &ep.rules.version {
        let ty = version_field_type(ctx.tree.ir, ep)
            .map_or_else(|| "i64".to_string(), |t| rust_type(ctx, &t, false));
        w.line(format!(
            "/// `If-Match`: a version of `{}.{}` or `IfMatch::Any`. Not part of the body or the query.",
            v.fact, v.wire
        ));
        w.line("#[serde(skip_serializing_if = \"Option::is_none\", skip_deserializing)]");
        w.line(format!(
            "pub {IF_MATCH_PARAM}: Option<veld::conditional::IfMatch<{ty}>>,"
        ));
    }
    if let Some((header, required)) = key_of(ep) {
        w.line(format!(
            "/// The idempotency key, sent in the header `{header}`."
        ));
        if required {
            w.line(format!("pub {KEY_PARAM}: String,"));
        } else {
            w.line("#[serde(default, skip_serializing_if = \"Option::is_none\")]");
            w.line(format!("pub {KEY_PARAM}: Option<String>,"));
        }
    }
}

/// The header parameters of an endpoint that the client core sends from those fields.
pub fn client_params(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, ep: &Endpoint) {
    let string = crate::types::schema_ref(
        ctx,
        &Type::Scalar(veld_ir::Scalar::String),
        &veld_ir::Constraints::default(),
    );
    let mut param = |name: &str, header: &str, optional: bool| {
        w.open("sp::ParamSpec");
        w.line(format!("name: {},", lit(name)));
        w.line("binding: sp::Binding::Header,");
        w.line(format!("header: Some({}),", lit(header)));
        w.line(format!("schema: {string},"));
        w.line(format!("optional: {optional},"));
        w.line("stream: false,");
        w.close_with("},");
    };
    if has_guard(ep) {
        param(IF_MATCH_PARAM, "If-Match", true);
    }
    if let Some((header, required)) = key_of(ep) {
        param(KEY_PARAM, header, !required);
    }
}
