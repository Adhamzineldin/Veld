//! `@version` and `@idempotent` in the shared TypeScript building blocks (`spec/rules.md` sections
//! 11.7 to 11.12, `spec/targets/typescript.md` section 5g): the members of an endpoint
//! description, the extra members of a client request type and the names a client reserves.
//!
//! A contract without the two annotations gets none of it.

use veld_codegen::{CodeWriter, GenDiagnostic};
use veld_ir::{DeclKind, Endpoint, Field, Ir, Service, Type};

use crate::tree::FileCtx;
use crate::types::ts_type;
use crate::util::ts_str;

/// The client's name for the `If-Match` of an endpoint with a guard.
pub const IF_MATCH: &str = "ifMatch";

/// The client's name for the key of an `@idempotent` endpoint.
pub const KEY: &str = "idempotencyKey";

/// The version field of the model of the fact an endpoint's guard compares.
#[must_use]
pub fn guard_field<'a>(ir: &'a Ir, ep: &Endpoint) -> Option<&'a Field> {
    let guard = ep.rules.version.as_ref()?;
    let fact = ep.rules.facts.iter().find(|f| f.name == guard.fact)?;
    let Type::Decl(id) = &fact.ty else {
        return None;
    };
    match &ir.decls.get(id.index())?.kind {
        DeclKind::Model(m) => m.fields.iter().find(|f| f.name == guard.field),
        DeclKind::Error(_)
        | DeclKind::Event(_)
        | DeclKind::Enum(_)
        | DeclKind::Union(_)
        | DeclKind::Const(_) => None,
    }
}

/// The members of an endpoint description that belong to the two features: `ifMatch`, `etag` and
/// `idempotency`. Nothing for an endpoint that has neither.
pub fn write_spec_members(w: &mut CodeWriter, ir: &Ir, ep: &Endpoint) {
    if ep.rules.version.is_some() {
        w.line("ifMatch: true,");
    }
    if let Some(field) = ir.etag_field(ep) {
        w.line(format!("etag: {},", ts_str(&field.name)));
    }
    if let Some(i) = &ep.idempotency {
        w.line(format!(
            "idempotency: {{ header: {}, required: {}, caller: {} }},",
            ts_str(&i.header),
            i.required,
            i.caller
        ));
    }
}

/// The members a client request type gains: `ifMatch` (a version typed like the version field, or
/// `'*'`) and `idempotencyKey` (required when the annotation says so). They come after the
/// declared parameters.
pub fn client_members(fc: &mut FileCtx<'_, '_>, ep: &Endpoint) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(field) = guard_field(fc.tree.ir, ep) {
        let ty = ts_type(fc, &field.ty);
        out.push("/** The version you read, or `'*'` for any: sent as `If-Match`. The server answers `PreconditionFailed` when it is not the current one. */".to_string());
        out.push(format!("{IF_MATCH}?: {ty} | '*';"));
    }
    if let Some(i) = &ep.idempotency {
        out.push(format!(
            "/** The idempotency key, sent in the `{}` header. A retry with the same key and the same parameters gets the first answer. */",
            i.header
        ));
        let opt = if i.required { "" } else { "?" };
        out.push(format!("{KEY}{opt}: string;"));
    }
    out
}

/// True when a request type of `ep` must be given: it has a required idempotency key.
#[must_use]
pub fn needs_request(ep: &Endpoint) -> bool {
    ep.idempotency.as_ref().is_some_and(|i| i.required)
}

/// `E0801` for a declared parameter that has the name of a member a client adds: the client could
/// not tell the two apart (`spec/rules.md` section 11.12).
pub fn reserved_names(svc: &Service, ep: &Endpoint, diags: &mut Vec<GenDiagnostic>) {
    for (name, on) in [
        (IF_MATCH, ep.rules.version.is_some()),
        (KEY, ep.idempotency.is_some()),
    ] {
        if on && ep.params.iter().any(|p| p.name == name) {
            let mut d = GenDiagnostic::new(
                "E0801",
                format!(
                    "{}.{}: a parameter called `{name}` clashes with the member the client adds for `{}`",
                    svc.name,
                    ep.name,
                    if name == KEY {
                        "@idempotent"
                    } else {
                        "@version"
                    }
                ),
            );
            d.origin = Some(ep.origin.clone());
            diags.push(d);
        }
    }
}
