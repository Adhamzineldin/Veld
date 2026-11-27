//! Platform phase P1 in the models of a server (`spec/platform.md` sections 2 and 3,
//! `spec/targets/typescript.md` section 5k): the `show` function of a declaration with `@visible`
//! fields and the members of its schema (`show`, `tn`, the `pii` and `secret` marks of a field).
//!
//! A client has none of it: the trees of a client never switch the gates on.

use veld_codegen::CodeWriter;
use veld_ir::{AuthKind, Decl, Endpoint, Expr, Root, Service, Type};

use crate::expr::{EndpointPrinter, EndpointScope, Principal};
use crate::tree::FileCtx;
use crate::types::ts_type;
use crate::util::{ts_str, type_name};
use crate::validate::schema_expr;

/// Whether `d` has a `@visible` field of its own or an inherited one.
#[must_use]
pub fn has_visible(d: &Decl) -> bool {
    d.fields().iter().any(|f| f.visible.is_some())
}

/// The name of the function of `d` that says which fields the caller may not see.
#[must_use]
pub fn show_name(d: &Decl) -> String {
    format!("show{}", type_name(d))
}

/// Whether the schema of `d` has members of the platform features in this tree.
#[must_use]
pub fn has_members(fc: &FileCtx<'_, '_>, d: &Decl) -> bool {
    (fc.tree.uses_visibility() && has_visible(d))
        || (fc.tree.uses_tenancy() && d.tenant_field().is_some())
}

/// The function of `d` that answers the source names of the `@visible` fields a caller may not
/// see (`spec/platform.md` section 2). Every field is judged against the whole value; a field
/// whose expression is false, cannot be evaluated, or has no caller of its principal's model is
/// hidden.
pub fn write_show_fn(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, d: &Decl, self_ty: &str) {
    let caller = fc.rt("Caller");
    let visible_to = fc.rt("visibleTo");
    let ns = fc.rules_ns();
    w.line(format!(
        "/** The `@visible` fields of `{}` that the caller may not see (`spec/platform.md` section 2). */",
        d.name
    ));
    w.open(format!(
        "function {}(self: {self_ty}, caller: {caller} | undefined, now: bigint): string[]",
        show_name(d)
    ));
    w.line("const hidden: string[] = [];");
    for f in d.fields() {
        let (Some(expr), Some(model)) = (&f.visible, f.visible_principal) else {
            continue;
        };
        let principal = Type::Decl(model);
        let who = ts_type(fc, &principal);
        let sig = ts_str(&fc.tree.ir.decl(model).sig);
        let code = print_visible(fc, &ns, expr, &principal);
        w.line(format!(
            "if (!{visible_to}(caller, {sig}, (user: {who}) => {code})) hidden.push({});",
            ts_str(&f.name)
        ));
    }
    w.line("void now;");
    w.line("return hidden;");
    w.close();
    w.blank();
}

/// The expression of a `@visible` field: the fields of the value are `self`, the caller is the
/// local `user`, and `now` is the clock of the request (`bigint`).
fn print_visible(fc: &FileCtx<'_, '_>, ns: &str, expr: &Expr, principal: &Type) -> String {
    let scope = EndpointScope {
        principal: Some(Principal {
            name: "user".to_string(),
            ty: principal.clone(),
        }),
        locals: vec![(Root::Principal, "user".to_string())],
        ..EndpointScope::default()
    };
    EndpointPrinter::new(fc.tree.ir, ns, scope).print(expr)
}

/// The members of the object schema of `d` that the platform features add: `show` for a
/// declaration with `@visible` fields and `tn` for a tenant-scoped one. Call it where the
/// declaration writes `rules:` and `derive:`.
pub fn write_schema_members(w: &mut CodeWriter, fc: &FileCtx<'_, '_>, d: &Decl) {
    if fc.tree.uses_visibility() && has_visible(d) {
        w.line(format!("show: {},", show_name(d)));
    }
    if fc.tree.uses_tenancy()
        && let Some(f) = d.tenant_field()
    {
        w.line(format!("tn: {},", tenant_member(f)));
    }
}

/// `{ n: 'tenantId', w?: 'tenant_id', id: 'veld:...', k: 'uuid' }`: the tenant field of a model.
fn tenant_member(f: &veld_ir::Field) -> String {
    let wire = if f.wire == f.name {
        String::new()
    } else {
        format!(", w: {}", ts_str(&f.wire))
    };
    format!(
        "{{ n: {}{wire}, id: {}, k: {} }}",
        ts_str(&f.name),
        ts_str(f.tenant_id.as_deref().unwrap_or_default()),
        ts_str(tenant_kind(&f.ty))
    )
}

/// The type of a tenant field as the runtime compares it.
#[must_use]
pub fn tenant_kind(ty: &Type) -> &'static str {
    use veld_ir::Scalar;
    match ty {
        Type::Scalar(Scalar::Uuid) => "uuid",
        Type::Scalar(Scalar::Int) => "int",
        Type::Scalar(Scalar::Long) => "long",
        Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Page(_) | Type::Decl(_) => "string",
    }
}

/// The types of what an endpoint answers with: its success and its declared errors.
fn answers(ep: &Endpoint) -> Vec<Type> {
    ep.success
        .ty
        .iter()
        .cloned()
        .chain(ep.errors.iter().map(|e| Type::Decl(e.decl)))
        .collect()
}

/// The members of the endpoint description the platform features add (`spec/platform.md`
/// sections 2 to 5), one line each, in the order of `ServerEndpoint`.
pub fn endpoint_members(fc: &mut FileCtx<'_, '_>, svc: &Service, ep: &Endpoint) -> Vec<String> {
    let mut out = Vec::new();
    if !fc.tree.uses_platform() {
        return out;
    }
    if ep.auth.kind != AuthKind::None
        && let Some(p) = &svc.principal
    {
        out.push(format!("caller: {},", ts_str(&fc.tree.ir.decl(p.decl).sig)));
    }
    let ir = fc.tree.ir;
    if fc.tree.uses_visibility() && answers(ep).iter().any(|t| ir.type_reaches_visible(t)) {
        out.push("visible: true,".to_string());
    }
    if fc.tree.uses_tenancy()
        && endpoint_reaches_tenant(ir, ep)
        && let Some(p) = &svc.principal
        && let Some(field) = ir.decl(p.decl).tenant_field()
    {
        out.push(format!(
            "tenant: {{ n: {}, k: {} }},",
            ts_str(&field.name),
            ts_str(tenant_kind(&field.ty))
        ));
    }
    if fc.tree.uses_audit() && ep.audited && !ep.success.stream {
        out.push(audit_member(fc, svc, ep));
    }
    if fc.tree.uses_limits_server()
        && let Some(limit) = &ep.limit
    {
        out.push(limit_member(ir, svc, limit));
    }
    if fc.tree.uses_capture() {
        out.push(capture_member(fc, svc, ep));
    }
    out
}

/// `capture: { service, facts, principal? }`: what the capture of a rule violation needs to know
/// of an endpoint (`spec/rules.md` section 17.3). The principal's schema is there for an endpoint
/// with auth in a service that has a principal, like `caller`.
fn capture_member(fc: &mut FileCtx<'_, '_>, svc: &Service, ep: &Endpoint) -> String {
    let facts: Vec<String> = ep.rules.facts.iter().map(|f| ts_str(&f.name)).collect();
    let mut parts = vec![
        format!("service: {}", ts_str(&svc.name)),
        format!("facts: [{}]", facts.join(", ")),
    ];
    if ep.auth.kind != AuthKind::None
        && let Some(p) = &svc.principal
    {
        let schema = schema_expr(fc, &Type::Decl(p.decl), &veld_ir::Constraints::default());
        parts.push(format!("principal: {schema}"));
    }
    format!("capture: {{ {} }},", parts.join(", "))
}

/// `limit: { count, seconds, by, tenant? }`: the limit of an endpoint (`spec/platform.md` section 5).
fn limit_member(ir: &veld_ir::Ir, svc: &Service, limit: &veld_ir::Limit) -> String {
    let mut parts = vec![
        format!("count: {}", limit.count),
        format!("seconds: {}", limit.per.seconds()),
        format!("by: {}", ts_str(limit.by.word())),
    ];
    if limit.by == veld_ir::LimitSubject::Tenant
        && let Some(p) = &svc.principal
        && let Some(field) = ir.decl(p.decl).tenant_field()
    {
        parts.push(format!(
            "tenant: {{ n: {}, k: {} }}",
            ts_str(&field.name),
            ts_str(tenant_kind(&field.ty))
        ));
    }
    format!("limit: {{ {} }},", parts.join(", "))
}

/// `audit: { authenticated, principal?, errors }`: what the audit event of an endpoint needs.
fn audit_member(fc: &mut FileCtx<'_, '_>, svc: &Service, ep: &Endpoint) -> String {
    let ir = fc.tree.ir;
    let mut parts = vec![format!("authenticated: {}", ep.auth.kind != AuthKind::None)];
    if let Some(p) = &svc.principal {
        let schema = schema_expr(fc, &Type::Decl(p.decl), &veld_ir::Constraints::default());
        parts.push(format!("principal: {schema}"));
    }
    let errors: Vec<String> = ep
        .errors
        .iter()
        .filter(|e| ir.package(ir.decl(e.decl).package).kind != veld_ir::PackageKind::Prelude)
        .map(|e| ts_str(&ir.decl(e.decl).sig))
        .collect();
    parts.push(format!("errors: [{}]", errors.join(", ")));
    format!("audit: {{ {} }},", parts.join(", "))
}

/// Whether a fact or an answer of `ep` can hold a tenant-scoped value (`spec/platform.md` section 3).
#[must_use]
pub fn endpoint_reaches_tenant(ir: &veld_ir::Ir, ep: &Endpoint) -> bool {
    answers(ep).iter().any(|t| ir.type_reaches_tenant(t))
        || ep.rules.facts.iter().any(|f| ir.type_reaches_tenant(&f.ty))
}

/// Whether the server enforces any of the four features of phase P1 for `ep`.
#[must_use]
pub fn endpoint_uses_platform(tree: &crate::tree::Tree<'_>, ep: &Endpoint) -> bool {
    let ir = tree.ir;
    (tree.uses_visibility() && answers(ep).iter().any(|t| ir.type_reaches_visible(t)))
        || (tree.uses_tenancy() && endpoint_reaches_tenant(ir, ep))
        || (tree.uses_audit() && ep.audited && !ep.success.stream)
        || (tree.uses_limits_server() && ep.limit.is_some())
        || tree.uses_capture()
}
