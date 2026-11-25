//! Endpoint rules in the server module (`spec/rules.md` section 9, `spec/targets/typescript.md`
//! section 5c): the fact types and loader interface, and the `rules` member of an endpoint
//! description. The runtime (`runtime/veld.ts`, region `endpoint_rules`) runs them as step 6 and
//! step 9 of the pipeline.

use std::collections::BTreeSet;

use crate::guards;
use veld_codegen::{CodeWriter, GenDiagnostic};
use veld_emit_typescript::expr::{EndpointPrinter, EndpointScope, Principal};
use veld_emit_typescript::machine::Move;
use veld_emit_typescript::policy::app_value;
use veld_emit_typescript::tree::FileCtx;
use veld_emit_typescript::types::ts_type;
use veld_emit_typescript::util::{method_key, prop_key, ts_str};
use veld_ir::{
    Endpoint, EndpointEnsure, EndpointFact, EndpointRequire, ExprType, Scalar, Service, Type,
};

/// Whether an endpoint has any statement.
#[must_use]
pub fn endpoint_has_rules(ep: &Endpoint) -> bool {
    !ep.rules.is_empty()
}

/// Whether the service has an endpoint with statements: it gets an endpoints builder, and the
/// handler factory needs the fact loaders and takes a clock.
#[must_use]
pub fn service_has_rules(svc: &Service) -> bool {
    svc.endpoints.iter().any(endpoint_has_rules)
}

/// The type of the loaded facts of an endpoint: `CreateBookingFacts` for `CreateBookingRequest`.
#[must_use]
pub fn facts_name(request: &str) -> String {
    format!(
        "{}Facts",
        request.strip_suffix("Request").unwrap_or(request)
    )
}

fn old_name(request: &str) -> String {
    format!("{}Old", request.strip_suffix("Request").unwrap_or(request))
}

/// The loader interface of a service.
#[must_use]
pub fn loaders_name(service: &str) -> String {
    format!("{service}FactLoaders")
}

/// `export interface CreateBookingFacts { room: Room; coupon?: Coupon; }`: what the handler gets
/// besides the parameters.
pub fn write_facts_type(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    request: &str,
    ep: &Endpoint,
) {
    if ep.rules.facts.is_empty() {
        return;
    }
    w.line("/** The facts the loaders found, handed to the handler with the parameters. */");
    w.open(format!("export interface {}", facts_name(request)));
    for fact in &ep.rules.facts {
        let opt = if fact.optional { "?" } else { "" };
        w.line(format!(
            "{}{opt}: {};",
            prop_key(&fact.name),
            ts_type(fc, &fact.ty)
        ));
    }
    w.close();
    w.blank();
}

/// `export interface BookingsFactLoaders { room(key: string, ctx): Promise<Room | null | undefined>; }`.
pub fn write_loaders(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service, name: &str) {
    let ctx = fc.rt("RequestContext");
    w.line(format!(
        "/** One loader per fact of `{}`. A loader resolves to `null` or `undefined` when it finds nothing. */",
        svc.name
    ));
    w.open(format!("export interface {}", loaders_name(name)));
    for fact in &svc.facts {
        let ty = ts_type(fc, &fact.ty);
        let key = match &fact.key {
            Some(k) => format!("key: {}, ", ts_type(fc, k)),
            None => String::new(),
        };
        w.line(format!(
            "{}({key}ctx: {ctx}): Promise<{ty} | null | undefined>;",
            method_key(&fact.name)
        ));
    }
    w.close();
}

/// What the writer of one endpoint needs.
pub struct Site<'a> {
    pub svc: &'a Service,
    pub ep: &'a Endpoint,
    /// The request type name (`CreateBookingRequest`).
    pub request: &'a str,
}

/// Whether a statement of the endpoint reads the caller (`spec/rules.md` section 11.1): an
/// `allow` argument, a fact key, a `require` or its `else` payload, or an `ensure`.
fn reads_principal(ep: &Endpoint) -> bool {
    let r = &ep.rules;
    r.allows
        .iter()
        .any(|a| a.args.iter().any(veld_ir::Expr::reads_principal))
        || r.facts
            .iter()
            .any(|f| f.key.as_ref().is_some_and(veld_ir::Expr::reads_principal))
        || r.requires.iter().any(|q| {
            q.expr.reads_principal()
                || q.else_error
                    .iter()
                    .flat_map(|e| &e.fields)
                    .any(|f| f.value.reads_principal())
        })
        || r.ensures.iter().any(|e| e.expr.reads_principal())
}

/// The caller of the service when the rules of `ep` read it: it is part of the scope `f` the
/// rules of this endpoint see, under the name it has in the rules.
#[must_use]
pub fn caller(site: &Site<'_>) -> Option<Principal> {
    let p = site.svc.principal.as_ref()?;
    reads_principal(site.ep).then(|| Principal {
        name: p.name.clone(),
        ty: Type::Decl(p.decl),
    })
}

fn scope(ep: &Endpoint, principal: Option<Principal>) -> EndpointScope {
    let params: BTreeSet<String> = ep
        .params
        .iter()
        .filter(|p| p.optional)
        .map(|p| p.name.clone())
        .collect();
    let facts: BTreeSet<String> = ep
        .rules
        .facts
        .iter()
        .filter(|f| f.optional)
        .map(|f| f.name.clone())
        .collect();
    EndpointScope {
        optional_params: params,
        optional_facts: facts,
        principal,
        ..EndpointScope::default()
    }
}

pub(crate) fn unsupported(site: &Site<'_>, what: &str) -> GenDiagnostic {
    let mut d = GenDiagnostic::new(
        "E0801",
        format!(
            "{}.{}: the `node` generator cannot represent {what}",
            site.svc.name, site.ep.name
        ),
    );
    d.origin = site.ep.rules.first_origin().cloned();
    d
}

/// `bigint` for the 64 bit and temporal kinds, `number` for `int` and `float`, and so on: the
/// type of a value in the rule representation (`veld_emit_typescript::expr`).
fn rule_ty(fc: &mut FileCtx<'_, '_>, ty: &ExprType) -> String {
    match ty {
        ExprType::Duration => "bigint".to_string(),
        ExprType::Type(t) => rule_type(fc, t),
    }
}

fn rule_type(fc: &mut FileCtx<'_, '_>, t: &Type) -> String {
    match t {
        Type::Scalar(s) => match s {
            Scalar::Int | Scalar::Float => "number".to_string(),
            Scalar::Long | Scalar::Date | Scalar::Datetime | Scalar::Time => "bigint".to_string(),
            Scalar::Decimal => format!("{}.Decimal", fc.rules_ns()),
            Scalar::Bool => "boolean".to_string(),
            Scalar::Json => "unknown".to_string(),
            Scalar::String | Scalar::Uuid | Scalar::Bytes => "string".to_string(),
        },
        Type::List(inner) => format!("{}[]", rule_type(fc, inner)),
        Type::Map(_) | Type::Page(_) | Type::Decl(_) => ts_type(fc, t),
    }
}

fn message(name: Option<&str>, custom: Option<&str>, unnamed: &str) -> String {
    match (custom, name) {
        (Some(m), _) => ts_str(m),
        (None, Some(n)) => ts_str(&format!("violates rule {n}")),
        (None, None) => ts_str(unnamed),
    }
}

fn fields_list(reads: &[String]) -> String {
    let items: Vec<String> = reads.iter().map(|r| ts_str(r)).collect();
    format!("[{}]", items.join(", "))
}

/// The parameter list of a generated rule function.
pub(crate) struct Sig {
    pub p: String,
    pub f: String,
}

fn write_fact(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    printer: &mut EndpointPrinter<'_>,
    site: &Site<'_>,
    sig: &Sig,
    fact: &EndpointFact,
    diags: &mut Vec<GenDiagnostic>,
) {
    let (p, f) = (&sig.p, &sig.f);
    w.open_with("", "{");
    w.line(format!("name: {},", ts_str(&fact.name)));
    w.line(format!("optional: {},", fact.optional));
    let loader = format!("loaders.{}", method_key(&fact.name));
    match &fact.key {
        Some(key) => {
            match app_value(fc, printer, key) {
                Some(code) => w.line(format!("key: ({p}, {f}) => {code},")),
                None => {
                    diags.push(unsupported(
                        site,
                        "the key of this fact as a loader argument",
                    ));
                    w.line("key: () => undefined,")
                }
            };
            w.line(format!("load: (key, ctx) => {loader}(key, ctx),"));
        }
        None => {
            w.line(format!("load: (_key, ctx) => {loader}(ctx),"));
        }
    }
    w.close_with("},");
}

fn write_require(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    printer: &mut EndpointPrinter<'_>,
    site: &Site<'_>,
    sig: &Sig,
    r: &EndpointRequire,
    diags: &mut Vec<GenDiagnostic>,
) {
    let (p, f) = (&sig.p, &sig.f);
    w.open_with("", "{");
    w.line(format!("id: {},", ts_str(&r.id)));
    w.line(format!(
        "name: {},",
        ts_str(r.name.as_deref().unwrap_or("require"))
    ));
    w.line(format!("fields: {},", fields_list(&r.reads)));
    w.line(format!(
        "message: {},",
        message(
            r.name.as_deref(),
            r.message.as_deref(),
            "precondition failed"
        )
    ));
    let cond = printer.print(&r.expr);
    w.line(format!("holds: ({p}, {f}, now: bigint) => {cond},"));
    if let Some(e) = &r.else_error {
        let (class, _) = fc.error_refs(e.decl);
        let mut items = Vec::new();
        for field in &e.fields {
            let Some(value) = app_value(fc, printer, &field.value) else {
                diags.push(unsupported(site, "a value of the `else` payload"));
                continue;
            };
            items.push(format!("{}: {value}", prop_key(&field.name)));
        }
        let payload = if items.is_empty() {
            "{}".to_string()
        } else {
            format!("{{ {} }}", items.join(", "))
        };
        w.line(format!(
            "otherwise: {{ sig: {class}.sig, data: ({p}, {f}, now: bigint) => ({payload}) }},"
        ));
    }
    w.close_with("},");
}

fn write_ensure(
    w: &mut CodeWriter,
    sig: &Sig,
    result: &str,
    old: Option<&str>,
    (e, cond): (&EndpointEnsure, &str),
) {
    w.open_with("", "{");
    w.line(format!("id: {},", ts_str(&e.id)));
    w.line(format!(
        "name: {},",
        ts_str(e.name.as_deref().unwrap_or("ensure"))
    ));
    w.line(format!("fields: {},", fields_list(&e.reads)));
    w.line(format!(
        "message: {},",
        message(
            e.name.as_deref(),
            e.message.as_deref(),
            "postcondition failed"
        )
    ));
    let o = old.map_or_else(String::new, |t| format!(", o: {t}"));
    w.line(format!(
        "holds: ({}, result: {result}, now: bigint{o}) => {cond},",
        sig.p
    ));
    w.close_with("},");
}

/// The tuple type of the saved `old(...)` values, by slot.
fn old_type(fc: &mut FileCtx<'_, '_>, printer: &EndpointPrinter<'_>) -> Option<String> {
    let slots = printer.olds();
    if slots.is_empty() {
        return None;
    }
    let types: Vec<String> = slots.iter().map(|(_, ty)| rule_ty(fc, ty)).collect();
    Some(format!("readonly [{}]", types.join(", ")))
}

/// The `old` member: one function per saved operand.
fn write_old(w: &mut CodeWriter, printer: &EndpointPrinter<'_>, sig: &Sig) {
    let (p, f) = (&sig.p, &sig.f);
    if printer.olds().is_empty() {
        w.line("old: [],");
        return;
    }
    w.open_with("old:", " [");
    for (code, _) in printer.olds() {
        w.line(format!("({p}, {f}, now: bigint) => {code},"));
    }
    w.close_with("],");
}

/// Writes the `rules:` member of the endpoint description of `site`, and the type alias of its
/// saved values just before the builder that holds it (`pre`).
pub fn write_rules_member(
    w: &mut CodeWriter,
    pre: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    site: &Site<'_>,
    diags: &mut Vec<GenDiagnostic>,
) {
    let ep = site.ep;
    let caller = caller(site);
    let facts = (!ep.rules.facts.is_empty()).then(|| facts_name(site.request));
    let sig = Sig {
        p: format!("p: {}", site.request),
        f: format!("f: {}", guards::scope_type(fc, facts, caller.as_ref())),
    };
    let ns = fc.rules_ns();
    let mut printer = EndpointPrinter::new(fc.tree.ir, &ns, scope(ep, caller.clone()));
    let moved = match Move::of(fc.tree.ir, ep) {
        Ok(moved) => moved,
        Err(why) => {
            diags.push(unsupported(site, why));
            None
        }
    };
    let result = match &ep.success.ty {
        Some(t) if !ep.success.stream => ts_type(fc, t),
        Some(_) | None => "undefined".to_string(),
    };
    // The `ensure` expressions come first: they decide the slots of `old(...)`.
    printer.set_ensure(true);
    let conds: Vec<String> = ep
        .rules
        .ensures
        .iter()
        .map(|e| printer.print(&e.expr))
        .collect();
    printer.set_ensure(false);
    // The target check of a transition is the last `ensure`.
    let found = moved.as_ref().and_then(|m| m.found(&printer, ep));
    let old_ty = old_type(fc, &printer);
    let alias = old_ty.as_ref().map(|_| old_name(site.request));
    let mut ensure_code = CodeWriter::new();
    for (e, cond) in ep.rules.ensures.iter().zip(&conds) {
        write_ensure(&mut ensure_code, &sig, &result, alias.as_deref(), (e, cond));
    }
    if let (Some(mv), Some(state)) = (&moved, &found) {
        guards::write_target(&mut ensure_code, &sig, &result, (mv, state));
    }
    if let Some(ty) = &old_ty {
        pre.line(format!("type {} = {ty};", old_name(site.request)));
        pre.blank();
    }
    w.open_with("rules:", " {");
    write_list(w, "facts", |w| {
        for fact in &ep.rules.facts {
            write_fact(w, fc, &mut printer, site, &sig, fact, diags);
        }
    });
    write_list(w, "requires", |w| {
        for r in &ep.rules.requires {
            write_require(w, fc, &mut printer, site, &sig, r, diags);
        }
    });
    write_old(w, &printer, &sig);
    write_list(w, "ensures", |w| {
        for line in ensure_code.finish().lines() {
            w.line(line);
        }
    });
    let members = (caller.as_ref(), moved.as_ref(), found.as_deref());
    guards::write_members(w, fc, &mut printer, (site, &sig), members, diags);
    w.close_with("},");
}

fn write_list(w: &mut CodeWriter, key: &str, body: impl FnOnce(&mut CodeWriter)) {
    let mut inner = CodeWriter::new();
    body(&mut inner);
    let text = inner.finish();
    if text.is_empty() {
        w.line(format!("{key}: [],"));
        return;
    }
    w.open_with(format!("{key}:"), " [");
    for line in text.lines() {
        w.line(line);
    }
    w.close_with("],");
}
