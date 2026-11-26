//! Platform phase P1 (`spec/platform.md` sections 2 to 5, `spec/targets/rust.md` section 18):
//! field visibility, tenancy, the audit log and rate limits. Which features a crate uses, and the
//! code a declaration or a service gets for them.

use veld_codegen::{CodeWriter, GenDiagnostic, GenRequest, Role, print_expr};
use veld_ir::{Decl, DeclId, Endpoint, Ir, Model, Service, ServiceId, Type};

use crate::rules::{RustPrinter, Scope};
use crate::tree::Tree;
use crate::types::{Ctx, rust_type};
use crate::util::{lit, unsupported};

/// What a crate uses of the four features. A crate that uses none of them is generated exactly
/// as it was before the features existed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags {
    /// A served answer can reach a `@visible` field: every declaration with one has `show`.
    pub visibility: bool,
    /// A served answer or fact can be tenant-scoped.
    pub tenancy: bool,
    /// A declaration of the crate has a `@tenant` field (either role).
    pub tenant_fields: bool,
    /// An event of a served service has a `@tenant` field: its messages carry `veldtenant`.
    pub tenant_events: bool,
    /// A served endpoint is `@audited`.
    pub audit: bool,
    /// A served endpoint has a `@limit`.
    pub limits: bool,
    /// A client of the crate has an endpoint with a `@limit`.
    pub limits_client: bool,
    /// The schemas carry the `@pii` and `@secret` marks (the crate audits).
    pub pii: bool,
}

impl Flags {
    /// Whether a served endpoint uses one of the four features.
    #[must_use]
    pub fn server(&self) -> bool {
        self.visibility || self.tenancy || self.audit || self.limits
    }
}

/// The types an endpoint sends to its caller: the success, a stream item, and every declared
/// error.
fn sent_types(ep: &Endpoint) -> Vec<Type> {
    let mut out: Vec<Type> = ep.success.ty.iter().cloned().collect();
    out.extend(ep.errors.iter().map(|e| Type::Decl(e.decl)));
    out
}

fn served_endpoints<'a>(ir: &'a Ir, served: &[ServiceId]) -> impl Iterator<Item = &'a Endpoint> {
    let ids: Vec<ServiceId> = served.to_vec();
    ids.into_iter()
        .flat_map(move |s| ir.service(s).endpoints.iter())
}

/// Whether an answer of the service can reach a `@visible` field.
#[must_use]
pub fn svc_visibility(ir: &Ir, svc: &Service) -> bool {
    svc.endpoints.iter().any(|ep| ep_visibility(ir, ep))
}

fn ep_visibility(ir: &Ir, ep: &Endpoint) -> bool {
    sent_types(ep).iter().any(|t| ir.type_reaches_visible(t))
}

/// Whether an answer or a fact of the service can be tenant-scoped.
#[must_use]
pub fn svc_tenancy(ir: &Ir, svc: &Service) -> bool {
    svc.endpoints.iter().any(|ep| ep_tenancy(ir, ep))
}

/// Whether the answers or the facts of the endpoint can be tenant-scoped.
#[must_use]
pub fn ep_tenancy(ir: &Ir, ep: &Endpoint) -> bool {
    sent_types(ep).iter().any(|t| ir.type_reaches_tenant(t))
        || ep.rules.facts.iter().any(|f| ir.type_reaches_tenant(&f.ty))
}

/// Whether the service has an audited endpoint.
#[must_use]
pub fn svc_audited(svc: &Service) -> bool {
    svc.endpoints.iter().any(|e| e.audited)
}

/// Whether the service has an endpoint with a limit.
#[must_use]
pub fn svc_limited(svc: &Service) -> bool {
    svc.endpoints.iter().any(|e| e.limit.is_some())
}

/// Whether the service uses one of the four features of P1.
#[must_use]
pub fn svc_uses(ir: &Ir, svc: &Service) -> bool {
    svc_visibility(ir, svc) || svc_tenancy(ir, svc) || svc_audited(svc) || svc_limited(svc)
}

/// Whether a service has a principal that its P1 features read: the hooks are then typed (the
/// authentication hook returns the model), so the generated code can read the caller.
#[must_use]
pub fn svc_reads_caller(ir: &Ir, svc: &Service) -> bool {
    svc.principal.is_some() && svc_uses(ir, svc)
}

/// What the crate uses of the four features. `server` is the role; `own` the services the crate
/// serves (or, for a client crate, generates) and `consumed` the ones a server calls.
#[must_use]
pub fn uses(req: &GenRequest<'_>) -> Flags {
    let ir = req.ir;
    let server = req.role == Role::Server;
    let served: Vec<ServiceId> = if server {
        req.services
            .iter()
            .copied()
            .filter(|s| crate::events::serves_http(ir.service(*s)))
            .collect()
    } else {
        Vec::new()
    };
    let clients: &[ServiceId] = if server { &req.consumes } else { &req.services };
    let audit = served_endpoints(ir, &served).any(|e| e.audited);
    Flags {
        visibility: served_endpoints(ir, &served).any(|e| ep_visibility(ir, e)),
        tenancy: served_endpoints(ir, &served).any(|e| ep_tenancy(ir, e)),
        tenant_fields: crate_has_field(req, |f| f.tenant),
        tenant_events: server
            && ir
                .events_of(&req.services)
                .iter()
                .any(|id| ir.decl(*id).tenant_field().is_some()),
        audit,
        limits: served_endpoints(ir, &served).any(|e| e.limit.is_some()),
        limits_client: served_endpoints(ir, clients).any(|e| e.limit.is_some()),
        pii: audit && crate_has_field(req, |f| f.pii || f.secret),
    }
}

/// Whether a declaration of any tree of the crate has a field for which `pred` holds.
fn crate_has_field(req: &GenRequest<'_>, pred: fn(&veld_ir::Field) -> bool) -> bool {
    let has = |tree: &Tree<'_>| {
        tree.decls()
            .iter()
            .any(|id| req.ir.decl(*id).fields().iter().any(&pred))
    };
    let main = Tree::new(req.ir, "crate", "src", req.services.clone(), true);
    has(&main)
        || req.consumes.iter().any(|sid| {
            has(&Tree::new(
                req.ir,
                "crate::clients::x",
                "src/clients/x",
                vec![*sid],
                false,
            ))
        })
}

// ---------------------------------------------------------------------------------------------
// Visibility
// ---------------------------------------------------------------------------------------------

/// Whether the model has a `@visible` field.
#[must_use]
pub fn has_visible(m: &Model) -> bool {
    m.fields.iter().any(|f| f.visible.is_some())
}

/// The Rust of one `@visible` expression: over `self`, the caller as `user` and `now`.
fn print_visible(
    ctx: &Ctx<'_, '_>,
    d: &Decl,
    f: &veld_ir::Field,
) -> Result<String, Vec<GenDiagnostic>> {
    let Some(expr) = &f.visible else {
        return Ok(String::new());
    };
    let scope = Scope {
        principal: Some("user".to_string()),
        ..Scope::default()
    };
    let mut printer = RustPrinter::for_block(ctx, scope);
    let printed = print_expr(&mut printer, expr);
    match printer.unsupported.first() {
        Some(what) => Err(unsupported(format!(
            "generator `rust`: `@visible` of `{}.{}`: {what}",
            d.name, f.name
        ))),
        None => Ok(printed.code),
    }
}

/// `impl veld::platform::Visible for <Decl>` and the function that the schema calls.
///
/// # Errors
/// `E0801` when an expression uses something the generator cannot express.
pub fn emit_visible(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    d: &Decl,
    m: &Model,
) -> Result<(), Vec<GenDiagnostic>> {
    let name = crate::util::type_name(&d.name);
    let fields: Vec<&veld_ir::Field> = m.fields.iter().filter(|f| f.visible.is_some()).collect();
    let names: Vec<String> = fields.iter().map(|f| lit(&f.wire)).collect();
    let mut bodies = Vec::new();
    for f in &fields {
        bodies.push(print_visible(ctx, d, f)?);
    }
    w.open(format!("impl veld::platform::Visible for {name}"));
    w.line(format!(
        "const FIELDS: &'static [&'static str] = &[{}];",
        names.join(", ")
    ));
    w.blank();
    w.line(
        "#[allow(clippy::all, unused_variables, unused_parens, unused_braces, unused_comparisons)]",
    );
    w.open(
        "fn hidden(&self, caller: Option<&veld::server::Principal>, now: i64) -> Vec<&'static str>",
    );
    w.line("use veld::rules as vr;");
    w.line("let mut out = Vec::new();");
    for (f, body) in fields.iter().zip(&bodies) {
        let model: Option<DeclId> = f.visible_principal;
        let ty = model.map_or_else(
            || "()".to_string(),
            |id| rust_type(ctx, &Type::Decl(id), false),
        );
        w.line(format!("// {}", f.name));
        w.open_with(
            "let shown =",
            &format!(" match caller.and_then(|c| c.get::<{ty}>()) {{"),
        );
        w.line(format!(
            "Some(user) => (|| -> vr::Res<bool> {{ Ok({body}) }})().unwrap_or(false),"
        ));
        w.line("None => false,");
        w.close_with("};");
        w.line(format!("if !shown {{ out.push({}); }}", lit(&f.wire)));
    }
    w.line("out");
    w.close().close().blank();
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// What the generator does not do
// ---------------------------------------------------------------------------------------------

fn refused(ep: &Endpoint, svc: &Service, what: &str) -> Vec<GenDiagnostic> {
    let mut d = GenDiagnostic::new(
        "E0801",
        format!(
            "generator `rust` does not support {what} on `{}.{}`",
            svc.name, ep.name
        ),
    );
    d.origin = Some(ep.origin.clone());
    vec![d]
}

/// `E0801` at the endpoint for what a server of platform phase P1 cannot do: an audited endpoint
/// that streams or is a `ws` endpoint (`spec/platform.md` section 9.1), and a `ws` endpoint whose
/// messages can hold a `@visible` field or a tenant-scoped value (a message is not filtered).
///
/// # Errors
/// The first such endpoint.
pub fn refusals(req: &GenRequest<'_>) -> Result<(), Vec<GenDiagnostic>> {
    if req.role != Role::Server {
        return Ok(());
    }
    for sid in &req.services {
        let svc = req.ir.service(*sid);
        for ep in &svc.endpoints {
            let kind = crate::endpoints::kind(ep);
            if ep.audited && kind != crate::endpoints::Kind::Http {
                return Err(refused(
                    ep,
                    svc,
                    "`@audited` on an endpoint that streams or is a `ws` endpoint",
                ));
            }
            if kind == crate::endpoints::Kind::Ws
                && (ep_visibility(req.ir, ep) || ep_tenancy(req.ir, ep))
            {
                return Err(refused(
                    ep,
                    svc,
                    "messages that can hold a `@visible` field or a tenant-scoped value on a `ws` endpoint",
                ));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Tenancy
// ---------------------------------------------------------------------------------------------

/// The `TenantKind` of a field's type.
fn tenant_kind(f: &veld_ir::Field) -> &'static str {
    match &f.ty {
        Type::Scalar(veld_ir::Scalar::Int) => "Int",
        Type::Scalar(veld_ir::Scalar::Long) => "Long",
        Type::Scalar(veld_ir::Scalar::String) => "String",
        Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Page(_) | Type::Decl(_) => "Uuid",
    }
}

/// The `tenant` member of a schema: the `@tenant` field of the model, if it has one.
#[must_use]
pub fn tenant_mark(m: &Model) -> String {
    match m.fields.iter().find(|f| f.tenant) {
        Some(f) => format!(
            "Some(veld::platform::TenantMark {{ wire: {}, id: {}, kind: veld::platform::TenantKind::{} }})",
            lit(&f.wire),
            lit(f.tenant_id.as_deref().unwrap_or_default()),
            tenant_kind(f)
        ),
        None => "None".to_string(),
    }
}

/// `caller_tenant` of a dispatch whose service has a principal with a `@tenant` field: the tenant
/// of the caller, read from the typed principal (`ty` is its Rust type).
pub fn emit_caller_tenant(w: &mut CodeWriter, tree: &Tree<'_>, svc: &Service, ty: &str) {
    let Some(p) = &svc.principal else {
        return;
    };
    let Some(field) = tree.ir.decl(p.decl).tenant_field() else {
        return;
    };
    w.open("fn caller_tenant(&self, ctx: &veld::server::RequestContext) -> Option<veld::platform::Tenant>");
    w.line(format!("let principal = ctx.principal::<{ty}>()?;"));
    w.line(format!(
        "veld::platform::Tenant::of(veld::platform::TenantKind::{}, &principal.{})",
        tenant_kind(field),
        crate::util::snake(&field.name)
    ));
    w.close().blank();
}
