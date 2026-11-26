//! Platform phase P1 in the generated Java (`spec/platform.md` sections 2 to 9,
//! `spec/targets/java.md` section 17): which of field visibility, tenancy, the audit log and rate
//! limits a server uses, what a declaration says about itself (the marks on its schema and the
//! `showWire` function of its `@visible` fields) and what `<Service>Hooks` registers. The behaviour
//! is the runtime's (`runtime/Platform.java` and the `// #if platform` regions of the router).

use veld_codegen::{CodeWriter, GenDiagnostic, print_expr};
use veld_ir::{Decl, Endpoint, EndpointFact, Ir, Scalar, Service, ServiceId, Type};

use crate::rule_expr::JavaExpr;
use crate::tree::FileCtx;
use crate::util::java_str;

/// What a server uses of platform phase P1.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Uses {
    /// A response, a declared error or a stream item can reach a `@visible` field.
    pub visibility: bool,
    /// A response, a declared error, a stream item or a fact can be tenant-scoped, or an event
    /// the server publishes has a `@tenant` field.
    pub tenancy: bool,
    /// An event the server publishes has a `@tenant` field (`veldtenant`).
    pub tenant_events: bool,
    /// An endpoint is `@audited`.
    pub audit: bool,
    /// An endpoint has a `@limit`.
    pub limits: bool,
}

impl Uses {
    /// Whether the server uses any of the four.
    #[must_use]
    pub fn any(self) -> bool {
        self.visibility || self.tenancy || self.audit || self.limits
    }
}

/// What the services of a server use of platform phase P1.
#[must_use]
pub fn uses(ir: &Ir, services: &[ServiceId]) -> Uses {
    let mut out = Uses::default();
    for sid in services {
        let one = service_uses_of(ir, ir.service(*sid));
        out.visibility |= one.visibility;
        out.tenancy |= one.tenancy;
        out.audit |= one.audit;
        out.limits |= one.limits;
    }
    let events = ir
        .events_of(services)
        .into_iter()
        .any(|id| ir.decl(id).tenant_field().is_some());
    out.tenant_events = events;
    out.tenancy |= events;
    out
}

/// What one service uses.
#[must_use]
pub fn service_uses_of(ir: &Ir, svc: &Service) -> Uses {
    let mut out = Uses::default();
    for ep in &svc.endpoints {
        out.visibility |= answers_reach(ir, ep, |ir, ty| ir.type_reaches_visible(ty));
        out.tenancy |= answers_reach(ir, ep, |ir, ty| ir.type_reaches_tenant(ty));
        out.audit |= ep.audited;
        out.limits |= ep.limit.is_some();
    }
    out.tenancy |= svc.facts.iter().any(|f| ir.type_reaches_tenant(&f.ty));
    out
}

/// Whether a service of a server uses any of the four.
#[must_use]
pub fn service_uses(ir: &Ir, svc: &Service) -> bool {
    service_uses_of(ir, svc).any()
}

/// Whether the success type (a stream's item type too) or a declared error of the endpoint reaches
/// what `reaches` looks for.
fn answers_reach(ir: &Ir, ep: &Endpoint, reaches: impl Fn(&Ir, &Type) -> bool) -> bool {
    ep.success.ty.as_ref().is_some_and(|t| reaches(ir, t))
        || ep.errors.iter().any(|e| reaches(ir, &Type::Decl(e.decl)))
}

/// Whether a fact is walked when it is loaded: the tenant check, or the audit trail of an audited
/// endpoint.
#[must_use]
pub fn walks_fact(fc: &FileCtx<'_, '_>, ep: &Endpoint, fact: &EndpointFact) -> bool {
    (fc.tree.platform.tenancy && fc.tree.ir.type_reaches_tenant(&fact.ty))
        || (fc.tree.platform.audit && ep.audited)
}

/// Whether the service has an endpoint that is `@audited`: its hooks require `audit`.
#[must_use]
pub fn service_audited(svc: &Service) -> bool {
    svc.endpoints.iter().any(|e| e.audited)
}

/// The refusals of platform phase P1: an audited endpoint that streams (`sse`) or is a `ws`
/// endpoint is `E0801` at the endpoint (`spec/platform.md` section 9.1): a stream has no single
/// answer to record.
#[must_use]
pub fn refusals(ir: &Ir, services: &[ServiceId]) -> Vec<GenDiagnostic> {
    let mut out = Vec::new();
    for sid in services {
        let svc = ir.service(*sid);
        for ep in svc
            .endpoints
            .iter()
            .filter(|e| e.audited && (e.success.stream || e.method == veld_ir::Method::Ws))
        {
            out.push(GenDiagnostic {
                code: "E0801",
                message: format!(
                    "{}.{}: generator `java` cannot audit a streaming or `ws` endpoint (`@audited`)",
                    svc.name, ep.name
                ),
                origin: Some(ep.origin.clone()),
            });
        }
    }
    out
}

/// The wire names of the fields marked `@pii` or `@secret`.
fn pii_fields(d: &Decl) -> Vec<&str> {
    d.fields()
        .iter()
        .filter(|f| f.pii || f.secret)
        .map(|f| f.wire.as_str())
        .collect()
}

/// Whether a declaration has a `@visible` field.
fn has_visible(d: &Decl) -> bool {
    d.fields().iter().any(|f| f.visible.is_some())
}

/// The `.marked(...)` suffix of the schema of a declaration: what a server's runtime needs to know
/// about its fields. Empty for a client, for a declaration with nothing to say and for a contract
/// that does not use the feature.
#[must_use]
pub fn schema_suffix(fc: &mut FileCtx<'_, '_>, d: &Decl, class: &str) -> String {
    let mut marks = String::new();
    if fc.tree.platform.visibility && has_visible(d) {
        marks.push_str(&format!(".show({class}::showWire)"));
    }
    if fc.tree.platform.tenancy
        && let Some(field) = d.tenant_field()
    {
        let platform = fc.rt("Platform");
        let kind = match &field.ty {
            Type::Scalar(Scalar::Uuid) => "UUID",
            Type::Scalar(Scalar::String) => "STRING",
            Type::Scalar(Scalar::Int) => "INT",
            Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Page(_) | Type::Decl(_) => {
                "LONG"
            }
        };
        marks.push_str(&format!(
            ".tenant({}, {}, {platform}.TenantKind.{kind})",
            java_str(&field.wire),
            java_str(field.tenant_id.as_deref().unwrap_or_default())
        ));
    }
    let pii = pii_fields(d);
    if fc.tree.platform.audit && !pii.is_empty() {
        let names: Vec<String> = pii.iter().map(|w| java_str(w)).collect();
        marks.push_str(&format!(".pii({})", names.join(", ")));
    }
    if marks.is_empty() {
        return String::new();
    }
    let platform = fc.rt("Platform");
    format!(".marked({platform}.Marks.of(){marks})")
}

/// Whether the declaration gets members of its own for the platform features.
#[must_use]
pub fn has_members(fc: &FileCtx<'_, '_>, d: &Decl) -> bool {
    fc.tree.platform.visibility && has_visible(d)
}

/// Writes the platform members of `d` into the body of its class. `record` is the Java type of the
/// value: the model itself, or `Data` for an error.
///
/// # Errors
/// `E0801` for an expression the printer cannot express.
pub fn write_members(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    d: &Decl,
    record: &str,
) -> Result<(), Vec<GenDiagnostic>> {
    if fc.tree.platform.visibility && has_visible(d) {
        write_show(w, fc, d, record)?;
    }
    Ok(())
}

/// `showWire`: the wire names of the `@visible` fields of one record that the caller may not see.
/// Every expression is judged against the whole record as received, so one verdict never changes
/// another; a field the record lacks is not judged.
fn write_show(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    d: &Decl,
    record: &str,
) -> Result<(), Vec<GenDiagnostic>> {
    let (list, array_list) = (fc.name("java.util.List"), fc.name("java.util.ArrayList"));
    let (json, platform) = (fc.rt("Json"), fc.rt("Platform"));
    let mut lines = Vec::new();
    let mut problems = Vec::new();
    for f in d.fields().iter().filter(|f| f.visible.is_some()) {
        let (Some(expr), Some(model)) = (&f.visible, f.visible_principal) else {
            continue;
        };
        let caller = fc.decl(model);
        let mut printer = JavaExpr::for_visibility(fc, d.id, model);
        let body = print_expr(&mut printer, expr);
        for what in printer.unsupported() {
            problems.push(GenDiagnostic {
                code: "E0801",
                message: format!(
                    "`@visible` of `{}.{}` uses {what}, which generator `java` cannot enforce",
                    d.name, f.name
                ),
                origin: Some(d.origin.clone()),
            });
        }
        let wire = java_str(&f.wire);
        lines.push(format!(
            "if (json.has({wire}) && !{platform}.visibleTo(caller, {caller}.class, principal -> {})) {{ hide.add({wire}); }}",
            body.code
        ));
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    w.line("/** The wire names of the `@visible` fields of a record that the caller may not see (`spec/platform.md` section 2). */");
    w.open(format!(
        "static {list}<String> showWire({json}.Obj json, Object caller, long now)"
    ));
    w.line(format!(
        "{record} v = {record}.fromWire({platform}.decodable(json));"
    ));
    w.line(format!("{list}<String> hide = new {array_list}<>();"));
    for line in lines {
        w.line(line);
    }
    w.line("return hide;");
    w.close();
    Ok(())
}

/// The registrations of `<Service>Hooks` for the platform features, as statements of its
/// constructor: the clock of a request and the principal's model.
#[must_use]
pub fn registrations(fc: &mut FileCtx<'_, '_>, svc: &Service) -> Vec<String> {
    let ir = fc.tree.ir;
    if !service_uses(ir, svc) {
        return Vec::new();
    }
    let mut out = vec!["builder.clock(ctx -> this.clock.millis());".to_string()];
    if let Some(p) = &svc.principal {
        let class = fc.decl(p.decl);
        let platform = fc.rt("Platform");
        let tenant = ir
            .decl(p.decl)
            .tenant_field()
            .map_or_else(|| "null".to_string(), |f| java_str(&f.wire));
        out.push(format!(
            "builder.principal({platform}.principal({class}.class, {class}::toWire, {tenant}, {class}.SCHEMA));"
        ));
    }
    let entries: Vec<String> = svc
        .endpoints
        .iter()
        .filter(|e| e.audited)
        .map(|ep| audited_entry(fc, ep))
        .collect();
    if !entries.is_empty() {
        let map = fc.name("java.util.Map");
        out.push(format!(
            "builder.audited({map}.ofEntries({}));",
            entries.join(", ")
        ));
    }
    out
}

/// `Map.entry("<operation>", Platform.audited(<has auth>, <facts>, <errors>))`: what an audit event
/// needs to know of one endpoint.
fn audited_entry(fc: &mut FileCtx<'_, '_>, ep: &Endpoint) -> String {
    let ir = fc.tree.ir;
    let (map, platform) = (fc.name("java.util.Map"), fc.rt("Platform"));
    let facts: Vec<String> = ep
        .rules
        .facts
        .iter()
        .map(|f| {
            let schema =
                crate::types::schema_expr(fc, &f.ty, &veld_ir::Constraints::default(), false);
            format!("{map}.entry({}, {schema})", java_str(&f.name))
        })
        .collect();
    let facts = if facts.is_empty() {
        format!("{map}.of()")
    } else {
        format!("{map}.ofEntries({})", facts.join(", "))
    };
    let mut args = vec![(ep.auth.kind != veld_ir::AuthKind::None).to_string(), facts];
    for error in &ep.errors {
        // The data of an error the contract declares is the result of the event; the prelude's is not.
        if ir.package(ir.decl(error.decl).package).kind != veld_ir::PackageKind::Prelude {
            args.push(format!("{}.VELD_SIG", fc.decl(error.decl)));
        }
    }
    format!(
        "{map}.entry({}, {platform}.audited({}))",
        java_str(&ep.sig),
        args.join(", ")
    )
}
