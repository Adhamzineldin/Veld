//! The reference of the audit event (`spec/platform.md` section 4).
//!
//! [`event`] builds the `AuditEvent` of one request from what the request came to, for every way
//! a request can end: the pipeline knows `200`s, declared errors, `404`, `403`, `409`, `412`,
//! `422` and `500`, and [`crate::Pipeline::audit_event`] builds the event of those; a server also
//! ends requests the pipeline never sees (`401`, `429`), and [`event`] takes what they came to
//! by hand. Every member of the event is the same in every target after canonicalisation:
//!
//! ```jsonc
//! { "operation": "veld:bookings/Bookings.payBooking@…",   // Endpoint.sig
//!   "time": "2026-06-10T12:00:00Z",                         // the clock of the request
//!   "principal": { … },                                     // absent without a caller
//!   "outcome": { "status": 409, "error": "AmountMismatch" },
//!   "parameters": { … },                                    // always an object
//!   "facts": { … },                                         // always an object
//!   "result": { … },                                        // absent without a body
//!   "hidden": [ "guestPhone" ],                             // always an array
//!   "redacted": [ "parameters.body.card" ] }                // always an array
//! ```
//!
//! * `time` is the one reading of the request's clock, written like every `datetime`.
//! * `principal` is the caller as the application's hook returned it, with `@pii` and `@secret`
//!   fields left out. It is absent when authentication failed (`401`) and for an endpoint with no
//!   auth.
//! * `parameters` are the parameters bound in step 4 with their defaults, by name; `{}` for a
//!   request that ended before step 4 had passed (`401`, `429`, a refused shape).
//! * `facts` are the facts loaded when the request ended, by name; `{}` before the first.
//! * `result` is the body of the answer as it was sent, after visibility: the success value, or
//!   the data of an error the contract declares. The errors of the prelude (`NotFound`,
//!   `ValidationError`, `Internal`, `TooManyRequests`, …) have none: their detail is not
//!   something an audit log should hold.
//! * `hidden` lists what `@visible` dropped from `result`, without a `$`; `redacted` lists what
//!   was left out of the other members because of `@pii` or `@secret`, written from the member:
//!   `principal.email`, `parameters.body.card`, `facts.booking.guest.phone`, `result.phone`.
//!   Both are sorted by their bytes, without duplicates.

use std::collections::BTreeMap;

use serde_json::{Map, Value as Json, json};
use veld_ir::{DeclId, Endpoint, Ir, PackageKind, Type};

use crate::error::InputError;
use crate::redact::redact_type_at;
use crate::wire::format_datetime;

/// What a request came to.
#[derive(Debug, Clone)]
pub struct Ended<'a> {
    /// The one reading of the request's clock.
    pub time_ms: i64,
    /// The caller; `None` when authentication failed or the endpoint has no auth.
    pub principal: Option<&'a Json>,
    pub status: u16,
    /// The name of the error answered with, `None` for a success.
    pub error: Option<&'a str>,
    /// The bound parameters, `None` when the request ended before they were bound.
    pub parameters: Option<&'a BTreeMap<String, Json>>,
    /// The facts loaded.
    pub facts: &'a BTreeMap<String, Json>,
    /// The body of the answer with its type, as sent.
    pub result: Option<(&'a Type, &'a Json)>,
    /// What `@visible` dropped from `result`, without the `$`.
    pub hidden: &'a [String],
}

/// The audit event of one request of `endpoint`, whose service has the principal model
/// `principal` (if it declares one).
///
/// # Errors
/// [`InputError`] when a value does not have the shape of its type: it cannot be known to be
/// clean, so no event is built from it.
pub fn event(
    ir: &Ir,
    endpoint: &Endpoint,
    principal: Option<DeclId>,
    ended: &Ended<'_>,
) -> Result<Json, InputError> {
    let mut redacted: Vec<String> = Vec::new();
    let mut out = Map::new();
    out.insert("operation".into(), json!(endpoint.sig));
    out.insert("time".into(), json!(format_datetime(ended.time_ms)));
    if let Some(p) = ended.principal {
        let ty = principal.map(Type::Decl);
        out.insert(
            "principal".into(),
            clean(ir, ty.as_ref(), p, "principal", &mut redacted)?,
        );
    }
    let mut outcome = Map::new();
    outcome.insert("status".into(), json!(ended.status));
    if let Some(name) = ended.error {
        outcome.insert("error".into(), json!(name));
    }
    out.insert("outcome".into(), Json::Object(outcome));
    out.insert(
        "parameters".into(),
        parameters(ir, endpoint, ended, &mut redacted)?,
    );
    out.insert("facts".into(), facts(ir, endpoint, ended, &mut redacted)?);
    if let Some((ty, value)) = ended.result {
        out.insert(
            "result".into(),
            clean(ir, Some(ty), value, "result", &mut redacted)?,
        );
    }
    out.insert("hidden".into(), json!(sorted(ended.hidden.to_vec())));
    out.insert("redacted".into(), json!(sorted(redacted)));
    Ok(Json::Object(out))
}

fn sorted(mut paths: Vec<String>) -> Vec<String> {
    paths.sort();
    paths.dedup();
    paths
}

/// `value` of `ty` with the marked fields left out; their paths go to `redacted`. A value whose
/// type is not known (a principal without a declaration) is kept as it is.
fn clean(
    ir: &Ir,
    ty: Option<&Type>,
    value: &Json,
    root: &str,
    redacted: &mut Vec<String>,
) -> Result<Json, InputError> {
    let Some(ty) = ty else {
        return Ok(value.clone());
    };
    let got = redact_type_at(ir, ty, value, root)?;
    redacted.extend(got.paths);
    Ok(got.value)
}

fn parameters(
    ir: &Ir,
    endpoint: &Endpoint,
    ended: &Ended<'_>,
    redacted: &mut Vec<String>,
) -> Result<Json, InputError> {
    let mut out = Map::new();
    for (name, value) in ended.parameters.into_iter().flatten() {
        let ty = endpoint
            .params
            .iter()
            .find(|p| &p.name == name)
            .map(|p| &p.ty);
        let root = format!("parameters.{name}");
        out.insert(name.clone(), clean(ir, ty, value, &root, redacted)?);
    }
    Ok(Json::Object(out))
}

fn facts(
    ir: &Ir,
    endpoint: &Endpoint,
    ended: &Ended<'_>,
    redacted: &mut Vec<String>,
) -> Result<Json, InputError> {
    let mut out = Map::new();
    for (name, value) in ended.facts {
        let ty = endpoint
            .rules
            .facts
            .iter()
            .find(|f| &f.name == name)
            .map(|f| &f.ty);
        let root = format!("facts.{name}");
        out.insert(name.clone(), clean(ir, ty, value, &root, redacted)?);
    }
    Ok(Json::Object(out))
}

/// True for a declaration of the prelude: an error the contract does not declare itself.
#[must_use]
pub fn is_prelude(ir: &Ir, decl: DeclId) -> bool {
    ir.decls
        .get(decl.index())
        .is_some_and(|d| ir.package(d.package).kind == PackageKind::Prelude)
}
