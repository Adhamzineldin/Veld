//! `@version` and `@idempotent` in `veld diff` (`spec/rules.md` sections 11.7 and 11.8).
//!
//! Each change is judged by what it does to a caller built against the old contract:
//!
//! | Change | Verdict | Why |
//! |---|---|---|
//! | `@version` added to a field | non-breaking | responses gain an `ETag`; a request without `If-Match` is not checked |
//! | `@version` removed from a field | breaking | a caller that sends `If-Match` loses the protection of its write, and the `ETag` is gone |
//! | another field is the version | breaking | the tags a caller holds count something else |
//! | an endpoint gets a version guard | non-breaking | only a request that sends `If-Match` is checked |
//! | an endpoint loses its guard | breaking | `If-Match` is ignored: the write the caller thought protected is not |
//! | the guard checks another fact or field | breaking | the tags a caller holds are compared with something else |
//! | `@idempotent` added | non-breaking | a request without the key is as before; with `required: true` it is breaking, because that request is now `422` |
//! | `@idempotent` removed | breaking | a retry with the key is no longer sent the first answer: it runs again |
//! | `required` becomes true | breaking | a request without the key is now `422` |
//! | `required` becomes false | non-breaking | |
//! | the header name changes | breaking | callers send the old header: it is ignored, and `422` when required |
//!
//! The errors the two features add to an endpoint (`PreconditionFailed`, `Conflict` and the
//! `ValidationError` of a malformed header) belong to the change that adds them and are not
//! reported a second time as an error added to or removed from the endpoint.

use std::collections::BTreeMap;

use veld_ir::{Decl, DeclKind, Endpoint, Idempotency};

use super::{Change, ChangeKind, Ctx, decls};

/// The version field of a model by its wire name: `(source name, wire name)`.
fn version_field(d: &Decl) -> Option<(&str, &str)> {
    match &d.kind {
        DeclKind::Model(m) => m
            .fields
            .iter()
            .find(|f| f.version)
            .map(|f| (f.name.as_str(), f.wire.as_str())),
        DeclKind::Error(_) | DeclKind::Enum(_) | DeclKind::Union(_) | DeclKind::Const(_) => None,
        DeclKind::Event(_) => None,
    }
}

/// Which field of each model is its version, compared by qualified name.
pub(super) fn diff_version_fields(ctx: &Ctx<'_>, out: &mut Vec<Change>) {
    let (old, new): (BTreeMap<_, _>, BTreeMap<_, _>) =
        (decls::by_name(ctx.old), decls::by_name(ctx.new));
    for (q, o) in &old {
        let Some(n) = new.get(q) else { continue };
        match (version_field(o), version_field(n)) {
            (None, Some((name, _))) => out.push(Change::non_breaking(
                ChangeKind::VersionAdded,
                q,
                format!("field `{name}` is now the `@version` of `{}`: responses carry an ETag", n.name),
            )),
            (Some((name, _)), None) => out.push(Change::breaking(
                ChangeKind::VersionRemoved,
                q,
                format!(
                    "field `{name}` is no longer the `@version` of `{}`: a caller that sends `If-Match` loses its protection and the ETag is gone",
                    n.name
                ),
            )),
            (Some((_, a)), Some((_, b))) if a != b => out.push(Change::breaking(
                ChangeKind::VersionFieldChanged,
                q,
                format!("the `@version` of `{}` moved from `{a}` to `{b}`: the tags callers hold count something else", n.name),
            )),
            (None, None) | (Some(_), Some(_)) => {}
        }
    }
}

/// The guard and the idempotency of one endpoint.
pub(super) fn diff_endpoint_conditions(s: &str, o: &Endpoint, n: &Endpoint, out: &mut Vec<Change>) {
    diff_guard(s, o, n, out);
    diff_idempotency(s, o.idempotency.as_ref(), n.idempotency.as_ref(), out);
}

fn diff_guard(s: &str, o: &Endpoint, n: &Endpoint, out: &mut Vec<Change>) {
    match (&o.rules.version, &n.rules.version) {
        (None, Some(g)) => out.push(Change::non_breaking(
            ChangeKind::GuardAdded,
            s,
            format!(
                "the endpoint now compares `If-Match` with the version of `{}`; a request without the header is not checked",
                g.fact
            ),
        )),
        (Some(g), None) => out.push(Change::breaking(
            ChangeKind::GuardRemoved,
            s,
            format!(
                "the endpoint no longer compares `If-Match` with the version of `{}`: a write the caller thought protected is not",
                g.fact
            ),
        )),
        (Some(a), Some(b)) if a.fact != b.fact || a.wire != b.wire => out.push(Change::breaking(
            ChangeKind::GuardChanged,
            s,
            format!(
                "the version guard now compares `{}.{}` instead of `{}.{}`",
                b.fact, b.wire, a.fact, a.wire
            ),
        )),
        (None, None) | (Some(_), Some(_)) => {}
    }
}

fn diff_idempotency(
    s: &str,
    o: Option<&Idempotency>,
    n: Option<&Idempotency>,
    out: &mut Vec<Change>,
) {
    match (o, n) {
        (None, Some(i)) => out.push(Change::when(
            i.required,
            ChangeKind::IdempotencyAdded,
            s,
            if i.required {
                format!("the endpoint now requires the header `{}`: a request without it is 422", i.header)
            } else {
                format!("the endpoint now replays an answer for a key in the header `{}`; a request without it is as before", i.header)
            },
        )),
        (Some(i), None) => out.push(Change::breaking(
            ChangeKind::IdempotencyRemoved,
            s,
            format!(
                "the endpoint no longer replays by `{}`: a retry with the same key runs again",
                i.header
            ),
        )),
        (Some(a), Some(b)) => {
            if a.header != b.header {
                out.push(Change::breaking(
                    ChangeKind::IdempotencyHeaderChanged,
                    s,
                    format!("the idempotency header changed from `{}` to `{}`", a.header, b.header),
                ));
            }
            if a.required != b.required {
                out.push(Change::when(
                    b.required,
                    ChangeKind::IdempotencyRequiredChanged,
                    s,
                    if b.required {
                        "the idempotency key became required: a request without it is 422"
                    } else {
                        "the idempotency key is no longer required"
                    },
                ));
            }
        }
        (None, None) => {}
    }
}

/// True when the error `q` (a qualified name) of `n` (or of `o`) comes with a feature that the
/// endpoint gained (or lost) in this very change, so that the feature's own change already says it.
pub(super) fn implied_error(q: &str, o: &Endpoint, n: &Endpoint) -> bool {
    let name = q.strip_prefix("veld:prelude.").unwrap_or_default();
    if name.is_empty() {
        return false;
    }
    let guard = o.rules.version.is_some() != n.rules.version.is_some();
    let key = o.idempotency.is_some() != n.idempotency.is_some();
    match name {
        "PreconditionFailed" => guard,
        "Conflict" => key,
        "ValidationError" => guard || key,
        _ => false,
    }
}
