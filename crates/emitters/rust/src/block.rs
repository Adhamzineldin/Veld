//! Endpoint rules (`spec/rules.md` section 9, `spec/targets/rust.md` section 10): the facts of an
//! endpoint, the loader trait of a service, and the generated functions of step 6 (facts,
//! `require`, the state `ensure` needs) and step 9 (`ensure`).
//!
//! The functions are plain Rust over the typed request, the typed facts and the typed result;
//! the server core calls them through `Endpoints::prepare` and `Endpoints::ensure`.

use veld_codegen::{CodeWriter, GenDiagnostic, Printed, ValueKind, print_expr};
use veld_ir::{
    Endpoint, EndpointEnsure, EndpointFact, EndpointRequire, Expr, ExprKind, Ir, Service,
    ServiceFact, ServiceId, Type,
};

use crate::endpoints::{self, Kind};
use crate::guards;
use crate::policies::Plan;
use crate::rules::{RustPrinter, Scope, fact_ident, mark_old};
use crate::types::{Ctx, rust_type};
use crate::util::{bare, doc, lit, method, snake, type_name, unsupported};

/// Whether an endpoint has a block.
#[must_use]
pub fn ep_has_block(ep: &Endpoint) -> bool {
    !ep.rules.is_empty()
}

/// Whether a service has an endpoint with a block.
#[must_use]
pub fn has_block(svc: &Service) -> bool {
    svc.endpoints.iter().any(ep_has_block)
}

/// Whether any of the services has an endpoint with a block.
#[must_use]
pub fn served_has_blocks(ir: &Ir, services: &[ServiceId]) -> bool {
    services.iter().any(|s| has_block(ir.service(*s)))
}

/// The name of the loader trait of a service: `BookingsFactLoaders`.
#[must_use]
pub fn loaders_name(svc: &Service) -> String {
    format!("{}FactLoaders", type_name(&svc.name))
}

/// The struct of the facts an endpoint's handler receives: `CreateBookingFacts`.
#[must_use]
pub fn facts_name(ep: &Endpoint) -> String {
    let mut n = veld_codegen::naming::pascal(&ep.name);
    n.push_str("Facts");
    type_name(&n)
}

fn state_name(ep: &Endpoint) -> String {
    let mut n = veld_codegen::naming::pascal(&ep.name);
    n.push_str("State");
    type_name(&n)
}

/// The function of step 6 of an endpoint.
#[must_use]
pub fn step6_name(ep: &Endpoint) -> String {
    format!("step6_{}", bare(&snake(&ep.name)))
}

/// The function of step 9 of an endpoint.
#[must_use]
pub fn step9_name(ep: &Endpoint) -> String {
    format!("step9_{}", bare(&snake(&ep.name)))
}

/// Whether the endpoint's handler receives facts.
#[must_use]
pub fn has_facts(ep: &Endpoint) -> bool {
    !ep.rules.facts.is_empty()
}

fn has_old(ep: &Endpoint) -> bool {
    fn walk(e: &Expr) -> bool {
        match &e.e {
            ExprKind::Old(_) => true,
            ExprKind::Lit(_) | ExprKind::Path(_) | ExprKind::Has(_) | ExprKind::Now(_) => false,
            ExprKind::Not(x) | ExprKind::Neg(x) | ExprKind::Widen(x) => walk(x),
            ExprKind::Bin(b) => walk(&b.l) || walk(&b.r),
            ExprKind::Call(c) => c.args.iter().any(walk),
            ExprKind::List(items) => items.iter().any(walk),
            ExprKind::Quant(q) => walk(&q.list) || walk(&q.body),
        }
    }
    ep.rules.ensures.iter().any(|e| walk(&e.expr))
}

/// Whether the saved state holds the facts: an `ensure` reads one inside `old(...)`.
fn state_has_facts(ep: &Endpoint) -> bool {
    has_facts(ep) && has_old(ep)
}

/// Whether step 9 runs for the endpoint: it has an `ensure`, or the success value must hold the
/// target state of a transition.
#[must_use]
pub fn has_step9(ep: &Endpoint) -> bool {
    !ep.rules.ensures.is_empty() || guards::has_target(ep)
}

/// The local that holds a fact in step 6: `fact_room`.
#[must_use]
pub fn fact_local(name: &str) -> String {
    format!("fact_{}", fact_ident(name))
}

fn scope(svc: &Service, ep: &Endpoint, result: bool) -> Scope {
    Scope {
        principal: svc.principal.as_ref().map(|_| "principal".to_string()),
        locals: false,
        params: ep
            .params
            .iter()
            .filter(|p| !p.stream)
            .map(|p| (p.name.clone(), p.optional))
            .collect(),
        facts: ep
            .rules
            .facts
            .iter()
            .map(|f| (f.name.clone(), f.optional))
            .collect(),
        result,
    }
}

fn has_result(ep: &Endpoint) -> bool {
    ep.success.ty.is_some() && !ep.success.stream
}

pub(crate) fn first_problem(p: &RustPrinter<'_, '_, '_>) -> Result<(), String> {
    match p.unsupported.first() {
        Some(what) => Err(what.clone()),
        None => Ok(()),
    }
}

/// The printed Rust of a rule expression of a block (`bool`, or any type).
fn print(ctx: &Ctx<'_, '_>, scope: &Scope, expr: &Expr) -> Result<Printed, String> {
    let mut p = RustPrinter::for_block(ctx, scope.clone());
    let printed = print_expr(&mut p, &mark_old(expr));
    first_problem(&p)?;
    Ok(printed)
}

/// The value of an expression in the form the developer sees (a field's Rust type), for a fact's
/// key. A bare path is the field itself; anything computed is turned back from its rule form.
pub(crate) fn dev_value(
    ctx: &Ctx<'_, '_>,
    scope: &Scope,
    expr: &Expr,
    absent_is_none: bool,
) -> Result<String, String> {
    let mut p = RustPrinter::for_block(ctx, scope.clone());
    if absent_is_none {
        p = p.reading_absent();
    }
    if let Some(place) = p.raw_place(expr) {
        first_problem(&p)?;
        return Ok(format!("{place}.clone()"));
    }
    let printed = print_expr(&mut p, expr);
    first_problem(&p)?;
    let code = printed.at(Printed::ATOM);
    match ValueKind::of(&expr.t, ctx.tree.ir) {
        ValueKind::Bool
        | ValueKind::Int
        | ValueKind::Long
        | ValueKind::Float
        | ValueKind::Enum => Ok(code),
        ValueKind::String | ValueKind::Uuid | ValueKind::Decimal => {
            Ok(format!("{code}.to_string()"))
        }
        ValueKind::Date
        | ValueKind::Datetime
        | ValueKind::Time
        | ValueKind::Duration
        | ValueKind::Bytes
        | ValueKind::Json
        | ValueKind::Model
        | ValueKind::List
        | ValueKind::Map
        | ValueKind::Page => Err(
            "a computed date, time, duration or collection cannot be turned back into the value a loader or an error takes; read the field itself"
                .to_string(),
        ),
    }
}

/// The payload value of an `else` field as typed JSON.
fn json_value(ctx: &Ctx<'_, '_>, scope: &Scope, expr: &Expr) -> Result<String, String> {
    let mut p = RustPrinter::for_block(ctx, scope.clone());
    if let Some(place) = p.raw_place(expr) {
        first_problem(&p)?;
        return Ok(format!("vr::json(&{place})?"));
    }
    let printed = print_expr(&mut p, expr);
    first_problem(&p)?;
    let code = printed.at(Printed::ATOM);
    match ValueKind::of(&expr.t, ctx.tree.ir) {
        ValueKind::Bool => Ok(format!("serde_json::Value::Bool({code})")),
        ValueKind::Int | ValueKind::Long => Ok(format!("serde_json::Value::from({code})")),
        ValueKind::Float | ValueKind::Enum => Ok(format!("vr::json(&{code})?")),
        ValueKind::String | ValueKind::Uuid | ValueKind::Decimal => {
            Ok(format!("serde_json::Value::String({code}.to_string())"))
        }
        ValueKind::Date
        | ValueKind::Datetime
        | ValueKind::Time
        | ValueKind::Duration
        | ValueKind::Bytes
        | ValueKind::Json
        | ValueKind::Model
        | ValueKind::List
        | ValueKind::Map
        | ValueKind::Page => Err(
            "a computed date, time, duration or collection cannot be turned back into the value a loader or an error takes; read the field itself"
                .to_string(),
        ),
    }
}

fn fail(service: &Service, ep: &Endpoint, what: &str) -> Vec<GenDiagnostic> {
    unsupported(format!(
        "generator `rust`: endpoint `{}.{}`: {what}",
        service.name, ep.name
    ))
}

// ---------------------------------------------------------------------------
// Facts and loaders
// ---------------------------------------------------------------------------

/// The facts of an endpoint, as one struct the handler receives next to the request.
pub fn facts_struct(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, ep: &Endpoint) {
    w.line(format!(
        "/// The facts of `{}`, loaded before the handler runs (`spec/rules.md` section 9.2).",
        ep.name
    ));
    w.line("#[derive(Debug, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize)]");
    w.open(format!("pub struct {}", facts_name(ep)));
    for f in &ep.rules.facts {
        let ident = snake(&f.name);
        let mut attrs = Vec::new();
        if bare(&ident) != f.name {
            attrs.push(format!("rename = {}", lit(&f.name)));
        }
        if f.optional {
            attrs.push("default".to_string());
            attrs.push("skip_serializing_if = \"Option::is_none\"".to_string());
        }
        if !attrs.is_empty() {
            w.line(format!("#[serde({})]", attrs.join(", ")));
        }
        let t = rust_type(ctx, &f.ty, false);
        w.line(format!(
            "pub {ident}: {},",
            if f.optional {
                format!("Option<{t}>")
            } else {
                t
            }
        ));
    }
    w.close().blank();
}

/// The loader trait of a service: one method for each fact name.
pub fn loader_trait(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service) {
    let name = loaders_name(svc);
    w.line(format!(
        "/// Loads the facts of the `{}` service (`spec/rules.md` section 9.2): one method for each",
        svc.name
    ));
    w.line(
        "/// fact name. `Ok(None)` means \"not found\": a required fact answers `404 NotFound`, an",
    );
    w.line("/// optional one is absent. An `Err` is answered like an error of the handler.");
    w.open(format!("pub trait {name}: Send + Sync + 'static"));
    for f in &svc.facts {
        loader_method(w, ctx, f);
    }
    w.close().blank();
}

fn loader_method(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, f: &ServiceFact) {
    w.line(format!("/// Loads the fact `{}`.", f.name));
    let key = f.key.as_ref().map_or(String::new(), |k| {
        format!("key: {}, ", rust_type(ctx, k, false))
    });
    w.line(format!(
        "fn {}(&self, {key}ctx: &veld::server::RequestContext) -> impl std::future::Future<Output = Result<Option<{}>, veld::Error>> + Send;",
        method(&f.name),
        rust_type(ctx, &f.ty, false)
    ));
    w.blank();
}

// ---------------------------------------------------------------------------
// Step 6
// ---------------------------------------------------------------------------

/// The functions of steps 6 and 9 of every endpoint of the service that has a block.
///
/// # Errors
/// `E0801` when a statement uses something the generator cannot express.
pub fn steps(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    svc: &Service,
) -> Result<(), Vec<GenDiagnostic>> {
    for (i, ep) in svc.endpoints.iter().enumerate() {
        if !ep_has_block(ep) {
            continue;
        }
        if endpoints::kind(ep) == Kind::Ws {
            return Err(fail(
                svc,
                ep,
                "endpoint rules on a `ws` endpoint are not supported",
            ));
        }
        if has_step9(ep) {
            state_struct(w, ctx, svc, ep);
        }
        step6(w, ctx, svc, i, ep).map_err(|e| fail(svc, ep, &e))?;
        if has_step9(ep) {
            step9(w, ctx, svc, ep).map_err(|e| fail(svc, ep, &e))?;
        }
    }
    Ok(())
}

fn state_struct(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service, ep: &Endpoint) {
    w.line(format!(
        "/// What step 6 of `{}` keeps for step 9: the typed request, the facts when an `ensure`",
        ep.name
    ));
    w.line("/// reads one inside `old(...)`, and the clock reading.");
    w.open(format!("struct {}", state_name(ep)));
    w.line(format!("request: {},", endpoints::request_name(ep)));
    if state_has_facts(ep) {
        w.line(format!("facts: {},", facts_name(ep)));
    }
    if let Some(ty) = state_principal(ctx, svc, ep) {
        w.line(format!("principal: {ty},"));
    }
    w.line("now: i64,");
    w.close().blank();
}

/// The type of the principal that step 9 keeps: an `ensure` reads the caller.
fn state_principal(ctx: &Ctx<'_, '_>, svc: &Service, ep: &Endpoint) -> Option<String> {
    if guards::reads_principal_in_ensure(ep) {
        guards::principal_type(ctx, svc)
    } else {
        None
    }
}

const ALLOW: &str = "#[allow(clippy::all, unused_imports, unused_variables, unused_mut, unused_parens, unused_braces, unused_comparisons)]";

fn step6(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    svc: &Service,
    index: usize,
    ep: &Endpoint,
) -> Result<(), String> {
    let loaders = !svc.facts.is_empty();
    let generic = if loaders {
        format!("<L: {} + ?Sized>", loaders_name(svc))
    } else {
        String::new()
    };
    let loaders_arg = if loaders { "loaders: &L, " } else { "" };
    w.line(format!(
        "/// Step 6 of `{}` (`spec/rules.md` section 9.5): the facts in order, then every `require`;",
        ep.name
    ));
    w.line("/// the loaded facts are added to `args` for the handler.");
    w.line("#[doc(hidden)]").line(ALLOW);
    w.open(format!(
        "pub async fn {}{generic}({loaders_arg}now: i64, enforce: bool, args: &mut serde_json::Map<String, serde_json::Value>, ctx: &veld::server::RequestContext) -> Result<veld::server::Prepared, veld::server::Halt>",
        step6_name(ep)
    ));
    w.line("use veld::rules as vr;");
    w.line("use veld::server::Halt;");
    w.line(format!(
        "let request: {} = veld::server::decode_args(args.clone()).map_err(Halt::Error)?;",
        endpoints::request_name(ep)
    ));
    w.line("let p = &request;");
    let sc = scope(svc, ep, false);
    if guards::reads_principal(ep) {
        let ty = guards::principal_type(ctx, svc)
            .ok_or("the endpoint reads the principal, which the package does not declare")?;
        guards::bind_principal(w, &ty);
    }
    for f in &ep.rules.facts {
        load_fact(w, ctx, svc, f, &sc)?;
    }
    // The policies and the transition guard run whether or not requests are validated: who may
    // call an endpoint and which move is legal must not depend on a validation option.
    let plan = Plan::new(ctx.tree);
    for a in &ep.rules.allows {
        guards::allow(w, ctx, &plan, index, a, &sc)?;
    }
    guards::transition_guard(w, ctx, index, ep)?;
    w.open("if enforce");
    for r in &ep.rules.requires {
        require(w, ctx, index, ep, r, &sc)?;
    }
    w.close();
    finish6(w, ctx, svc, index, ep);
    w.close().blank();
    Ok(())
}

/// What the code that loads one fact is made of.
struct Load {
    name: String,
    ident: String,
    call: String,
    type_of: String,
    not_found: String,
}

impl Load {
    fn failed_key(&self) -> String {
        format!(
            "veld::Error::other(format!(\"the key of fact `{}` failed: {{}}\", f.as_str()))",
            self.name
        )
    }

    fn required_keyed(&self, w: &mut CodeWriter, key: &str, kt: &str) {
        w.open_with(format!("let loaded_{} =", self.ident), " {");
        w.line(format!(
            "let key: {kt} = (|| -> vr::Res<{kt}> {{ Ok({key}) }})().map_err(|f| Halt::Error({}))?;",
            self.failed_key()
        ));
        w.open(format!(
            "match loaders.{}(key, ctx).await.map_err(Halt::Error)?",
            self.call
        ));
        w.line("Some(v) => v,");
        w.line(format!(
            "None => return Err(Halt::Error({})),",
            self.not_found
        ));
        w.close().close_with("};");
    }

    fn optional_keyed(&self, w: &mut CodeWriter, key: &str, kt: &str) {
        w.open_with(
            format!("let loaded_{}: Option<{}> =", self.ident, self.type_of),
            &format!(" match (|| -> vr::Res<{kt}> {{ Ok({key}) }})() {{"),
        );
        w.line(format!(
            "Ok(key) => loaders.{}(key, ctx).await.map_err(Halt::Error)?,",
            self.call
        ));
        w.line("Err(vr::Fail::Absent) => None,");
        w.line(format!(
            "Err(f) => return Err(Halt::Error({})),",
            self.failed_key()
        ));
        w.close_with("};");
    }

    fn required_keyless(&self, w: &mut CodeWriter) {
        w.open_with(
            format!("let loaded_{} =", self.ident),
            &format!(
                " match loaders.{}(ctx).await.map_err(Halt::Error)? {{",
                self.call
            ),
        );
        w.line("Some(v) => v,");
        w.line(format!(
            "None => return Err(Halt::Error({})),",
            self.not_found
        ));
        w.close_with("};");
    }

    fn optional_keyless(&self, w: &mut CodeWriter) {
        w.line(format!(
            "let loaded_{}: Option<{}> = loaders.{}(ctx).await.map_err(Halt::Error)?;",
            self.ident, self.type_of, self.call
        ));
    }
}

fn load_fact(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    svc: &Service,
    f: &EndpointFact,
    sc: &Scope,
) -> Result<(), String> {
    let name = &f.name;
    let key = match &f.key {
        Some(k) => Some(dev_value(ctx, sc, k, f.optional)?),
        None => None,
    };
    let key_type = svc
        .facts
        .iter()
        .find(|s| &s.name == name)
        .and_then(|s| s.key.as_ref())
        .map(|t| rust_type(ctx, t, false));
    let load = Load {
        name: name.clone(),
        ident: fact_ident(name),
        call: method(name),
        type_of: rust_type(ctx, &f.ty, false),
        not_found: not_found(ctx, name),
    };
    w.line(format!("// fact {name}"));
    match (key.zip(key_type), f.optional) {
        (Some((key, kt)), false) => load.required_keyed(w, &key, &kt),
        (Some((key, kt)), true) => load.optional_keyed(w, &key, &kt),
        (None, false) => load.required_keyless(w),
        (None, true) => load.optional_keyless(w),
    }
    w.line(format!("let fact_{} = &loaded_{};", load.ident, load.ident));
    Ok(())
}

/// The `NotFound` of a missing required fact.
fn not_found(ctx: &Ctx<'_, '_>, fact: &str) -> String {
    match ctx.tree.prelude_decl("NotFound") {
        Some(id) => format!(
            "veld::Error::from({}{{ resource: Some({}.to_string()) }})",
            ctx.tree.type_path(id, ctx.here),
            lit(fact)
        ),
        None => format!(
            "veld::Error::other({})",
            lit(&format!("fact `{fact}` was not found"))
        ),
    }
}

fn message_of(name: Option<&str>, message: Option<&str>, unnamed: &str) -> String {
    match (message, name) {
        (Some(m), _) => m.to_string(),
        (None, Some(n)) => format!("violates rule {n}"),
        (None, None) => unnamed.to_string(),
    }
}

fn violated_call(
    id: &str,
    name: Option<&str>,
    kind: &str,
    reads: &[String],
    message: &str,
    body: &str,
) -> String {
    let fields: Vec<String> = reads.iter().map(|f| lit(f)).collect();
    format!(
        "vr::violated(\"request\", {}, {}, &[{}], {}, (|| -> vr::Res<bool> {{ Ok({body}) }})())",
        lit(id),
        lit(name.unwrap_or(kind)),
        fields.join(", "),
        lit(message)
    )
}

fn require(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    index: usize,
    ep: &Endpoint,
    r: &EndpointRequire,
    sc: &Scope,
) -> Result<(), String> {
    let body = print(ctx, sc, &r.expr)?.code;
    let message = message_of(
        r.name.as_deref(),
        r.message.as_deref(),
        "precondition failed",
    );
    w.line(format!(
        "// require {}",
        r.name.as_deref().unwrap_or("(unnamed)")
    ));
    w.open(format!(
        "if let Some(violation) = {}",
        violated_call(
            &r.id,
            r.name.as_deref(),
            "require",
            &r.reads,
            &message,
            &body
        )
    ));
    match &r.else_error {
        None => {
            w.line(format!(
                "return Err(Halt::Error(vr::request_rejection(STD.validation, ENDPOINTS[{index}].sig, violation)));"
            ));
        }
        Some(e) => else_answer(w, ctx, index, ep, r, e, sc, &message)?,
    }
    w.close();
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn else_answer(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    index: usize,
    ep: &Endpoint,
    r: &EndpointRequire,
    e: &veld_ir::ElseError,
    sc: &Scope,
    message: &str,
) -> Result<(), String> {
    let _ = ep;
    w.open_with(
        "let payload = (||",
        " -> vr::Res<serde_json::Map<String, serde_json::Value>> {",
    );
    w.line("let mut m = serde_json::Map::new();");
    for f in &e.fields {
        w.line(format!(
            "m.insert({}.to_string(), {});",
            lit(&f.wire),
            json_value(ctx, sc, &f.value)?
        ));
    }
    w.line("Ok(m)");
    w.close_with("})();");
    let sig = &ctx.tree.ir.decl(e.decl).sig;
    let failed = {
        let fields: Vec<String> = r.reads.iter().map(|f| lit(f)).collect();
        format!(
            "vr::violated(\"request\", {}, {}, &[{}], {}, Err(f)).into_iter().collect()",
            lit(&r.id),
            lit(r.name.as_deref().unwrap_or("require")),
            fields.join(", "),
            lit(message)
        )
    };
    w.open("return Err(match payload");
    w.open("Ok(payload) =>");
    w.open(format!(
        "match vr::else_rejection(&ENDPOINTS[{index}], {}, payload, violation)",
        lit(sig)
    ));
    w.line("Ok(err) => Halt::Error(err),");
    w.line("Err(broken) => Halt::Broken(broken),");
    w.close().close();
    w.line(format!("Err(f) => Halt::Broken({failed}),"));
    w.close_with("});");
    Ok(())
}

fn finish6(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service, index: usize, ep: &Endpoint) {
    let principal = state_principal(ctx, svc, ep).map_or(String::new(), |_| {
        ", principal: principal.clone()".to_string()
    });
    if has_facts(ep) {
        let names: Vec<String> = ep
            .rules
            .facts
            .iter()
            .map(|f| {
                let ident = snake(&f.name);
                format!("{ident}: loaded_{}", fact_ident(&f.name))
            })
            .collect();
        w.line(format!(
            "let facts = {} {{ {} }};",
            facts_name(ep),
            names.join(", ")
        ));
        let saved = state_has_facts(ep) && has_step9(ep);
        if saved {
            w.line("let saved = facts.clone();");
        }
        w.open("if let Ok(serde_json::Value::Object(m)) = serde_json::to_value(&facts)");
        w.line("args.extend(m);");
        w.close();
        guards::hand_over_transition(w, ep);
        if saved {
            w.line("if !enforce { return Ok(veld::server::Prepared::none()); }");
            w.line(format!(
                "Ok(veld::server::Prepared::new({index}, {} {{ request, facts: saved{principal}, now }}))",
                state_name(ep)
            ));
            return;
        }
    }
    if !has_step9(ep) {
        w.line("Ok(veld::server::Prepared::none())");
    } else {
        w.line("if !enforce { return Ok(veld::server::Prepared::none()); }");
        w.line(format!(
            "Ok(veld::server::Prepared::new({index}, {} {{ request{principal}, now }}))",
            state_name(ep)
        ));
    }
}

// ---------------------------------------------------------------------------
// Step 9
// ---------------------------------------------------------------------------

fn step9(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    svc: &Service,
    ep: &Endpoint,
) -> Result<(), String> {
    let result = has_result(ep);
    let sc = scope(svc, ep, result);
    w.line(format!(
        "/// Step 9 of `{}`: every `ensure`, in source order, on the success value.",
        ep.name
    ));
    w.line("#[doc(hidden)]").line(ALLOW);
    w.open(format!(
        "pub fn {}(prepared: &veld::server::Prepared, result: &serde_json::Value) -> Vec<veld::rules::Violation>",
        step9_name(ep)
    ));
    w.line("use veld::rules as vr;");
    w.line("let mut out = Vec::new();");
    w.line(format!(
        "let Some(state) = prepared.state::<{}>() else {{ return out; }};",
        state_name(ep)
    ));
    if let Some(t) = ep.success.ty.as_ref().filter(|_| result) {
        w.line(format!(
            "let Ok(result) = serde_json::from_value::<{}>(result.clone()) else {{ return out; }};",
            rust_type(ctx, t, false)
        ));
        w.line("let r = &result;");
    }
    w.line("let p = &state.request;");
    w.line("let old_p = &state.request;");
    w.line("let now = state.now;");
    if state_principal(ctx, svc, ep).is_some() {
        w.line("let principal = &state.principal;");
    }
    if state_has_facts(ep) {
        for f in &ep.rules.facts {
            w.line(format!(
                "let old_fact_{} = &state.facts.{};",
                fact_ident(&f.name),
                snake(&f.name)
            ));
        }
    }
    for e in &ep.rules.ensures {
        ensure_line(w, ctx, e, &sc)?;
    }
    guards::target_check(w, ctx, ep)?;
    w.line("out");
    w.close().blank();
    Ok(())
}

fn ensure_line(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    e: &EndpointEnsure,
    sc: &Scope,
) -> Result<(), String> {
    let body = print(ctx, sc, &e.expr)?.code;
    let message = message_of(
        e.name.as_deref(),
        e.message.as_deref(),
        "postcondition failed",
    );
    let fields: Vec<String> = e.reads.iter().map(|f| lit(f)).collect();
    w.line(format!(
        "// ensure {}",
        e.name.as_deref().unwrap_or("(unnamed)")
    ));
    w.line(format!(
        "vr::record(&mut out, \"request\", {}, {}, &[{}], {}, (|| -> vr::Res<bool> {{ Ok({body}) }})());",
        lit(&e.id),
        lit(e.name.as_deref().unwrap_or("ensure")),
        fields.join(", "),
        lit(&message)
    ));
    Ok(())
}

/// A doc line for the service trait method of an endpoint with facts.
pub fn facts_param(ep: &Endpoint) -> String {
    if has_facts(ep) {
        format!(", facts: {}", facts_name(ep))
    } else {
        String::new()
    }
}

/// `, transition: veld::rules::Transition<Status>` for the service method of an endpoint with a
/// transition: it follows the facts (`spec/rules.md` section 10.2).
pub fn transition_param(ctx: &Ctx<'_, '_>, ep: &Endpoint) -> String {
    ep.rules
        .transition
        .as_ref()
        .and_then(|t| guards::transition_type(ctx, t).ok())
        .map_or(String::new(), |ty| format!(", transition: {ty}"))
}

/// The doc lines of the service method of an endpoint with a transition: it is a guard, not an
/// atomic guarantee.
pub fn transition_docs(w: &mut CodeWriter, ep: &Endpoint) {
    let Some(t) = &ep.rules.transition else {
        return;
    };
    if ep.docs.is_some() || ep.deprecated.is_some() {
        w.line("///");
    }
    w.line(format!(
        "/// `transition` is the guard of `{}` (`spec/rules.md` section 10.2): the field `{}` had the state",
        t.fact,
        t.steps.iter().map(|s| s.wire.as_str()).collect::<Vec<_>>().join(".")
    ));
    w.line("/// `transition.from` when it was checked, and this endpoint moves it to `transition.to`. **This is");
    w.line("/// a guard, not an atomic guarantee:** another request can change the row after the check. Apply");
    w.line("/// the change with a conditional write (compare-and-swap, or a row lock) in your own");
    w.line("/// transaction, and answer `InvalidTransition` yourself when that write finds another value.");
}

/// The type of a fact for documentation and the test handler.
#[must_use]
pub fn fact_type(ctx: &Ctx<'_, '_>, ty: &Type, optional: bool) -> String {
    let t = rust_type(ctx, ty, false);
    if optional { format!("Option<{t}>") } else { t }
}

/// Documentation of an endpoint's statements for the method docs.
pub fn block_docs(w: &mut CodeWriter, ep: &Endpoint) {
    if ep_has_block(ep) {
        doc(
            w,
            Some("Its statements run around the handler (`spec/rules.md` section 9)."),
        );
    }
}
