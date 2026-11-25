//! The guards of step 6 and the target check of step 9: `allow` (`spec/rules.md` section 11.3) and
//! `transition` (section 10.2), with the typed principal they read.
//!
//! Both answer through `veld::rules::refusal`: the declared error of the guard built from its
//! string members, carrying the one violation of the document, never an evaluation error.

use veld_codegen::CodeWriter;
use veld_ir::{DeclKind, Endpoint, EndpointAllow, EndpointTransition, Expr, Service, Type};

use crate::block::{dev_value, fact_local, first_problem};
use crate::policies::Plan;
use crate::rules::{RustPrinter, Scope, local_ident};
use crate::types::{Ctx, rust_type};
use crate::util::{lit, snake, variant};

/// Whether the endpoint has a guard: an `allow` or a `transition`.
#[must_use]
pub fn has_guards(ep: &Endpoint) -> bool {
    !ep.rules.allows.is_empty() || ep.rules.transition.is_some()
}

/// Whether a service has an endpoint with a guard.
#[must_use]
pub fn svc_has_guards(svc: &Service) -> bool {
    svc.endpoints.iter().any(has_guards)
}

/// Whether the endpoint reads the principal anywhere: an `allow` argument, a fact key, a
/// `require` or its `else` payload, or an `ensure`.
#[must_use]
pub fn reads_principal(ep: &Endpoint) -> bool {
    let rules = &ep.rules;
    rules
        .allows
        .iter()
        .any(|a| a.args.iter().any(Expr::reads_principal))
        || rules
            .facts
            .iter()
            .any(|f| f.key.as_ref().is_some_and(Expr::reads_principal))
        || rules.requires.iter().any(|r| {
            r.expr.reads_principal()
                || r.else_error
                    .iter()
                    .flat_map(|e| &e.fields)
                    .any(|f| f.value.reads_principal())
        })
        || reads_principal_in_ensure(ep)
}

/// Whether an `ensure` reads the principal: step 9 keeps it.
#[must_use]
pub fn reads_principal_in_ensure(ep: &Endpoint) -> bool {
    ep.rules.ensures.iter().any(|e| e.expr.reads_principal())
}

/// Whether the success value of the endpoint is the fact of its transition: the same field of the
/// result must then hold the target state (`spec/rules.md` section 10.2).
#[must_use]
pub fn has_target(ep: &Endpoint) -> bool {
    let Some(t) = &ep.rules.transition else {
        return false;
    };
    let fact = ep.rules.facts.iter().find(|f| f.name == t.fact);
    !ep.success.stream
        && matches!(
            (fact.map(|f| &f.ty), ep.success.ty.as_ref()),
            (Some(Type::Decl(a)), Some(Type::Decl(b))) if a == b
        )
}

/// The signature of a prelude error, for the answer of a guard. A prelude without it gives an
/// error the endpoint cannot declare: the answer is then `500 Internal`.
fn prelude_sig(ctx: &Ctx<'_, '_>, name: &str) -> String {
    ctx.tree.prelude_decl(name).map_or_else(
        || "about:blank".to_string(),
        |id| ctx.tree.ir.decl(id).sig.clone(),
    )
}

/// The generated enum of a transition's state machine, and its Rust path.
fn machine<'a>(
    ctx: &'a Ctx<'_, '_>,
    t: &EndpointTransition,
) -> Result<(&'a veld_ir::Enum, String), String> {
    match &ctx.tree.ir.decl(t.decl).kind {
        DeclKind::Enum(e) => Ok((e, ctx.tree.type_path(t.decl, ctx.here))),
        DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Union(_) | DeclKind::Const(_) => {
            Err("a `transition` on something that is not an enum".to_string())
        }
    }
}

/// The type the handler receives for the transition: `veld::rules::Transition<Status>`.
///
/// # Errors
/// A transition that names something other than an enum.
pub fn transition_type(ctx: &Ctx<'_, '_>, t: &EndpointTransition) -> Result<String, String> {
    let (_, ty) = machine(ctx, t)?;
    Ok(format!("veld::rules::Transition<{ty}>"))
}

/// The wire path of the state field inside the fact: `status`, or `stay.status`.
fn wire_path(t: &EndpointTransition) -> String {
    let wires: Vec<&str> = t.steps.iter().map(|s| s.wire.as_str()).collect();
    wires.join(".")
}

/// The place of the state field below `base`.
fn field_place(base: &str, t: &EndpointTransition) -> String {
    t.steps
        .iter()
        .fold(base.to_string(), |acc, s| format!("{acc}.{}", snake(&s.name)))
}

fn variant_wire(e: &veld_ir::Enum, name: &str) -> Result<String, String> {
    e.variants
        .iter()
        .find(|v| v.name == name)
        .map(|v| v.wire.clone())
        .ok_or_else(|| format!("`{name}` is not a state of the machine"))
}

/// The fact the transition reads, which must be required and reached through required steps.
fn transition_fact<'a>(
    ep: &'a Endpoint,
    t: &EndpointTransition,
) -> Result<&'a veld_ir::EndpointFact, String> {
    let fact = ep
        .rules
        .facts
        .iter()
        .find(|f| f.name == t.fact)
        .ok_or_else(|| format!("the fact `{}` of the `transition` is not declared", t.fact))?;
    if fact.optional || t.steps.iter().any(|s| s.optional) {
        return Err(
            "a `transition` through an optional fact or field cannot be read".to_string(),
        );
    }
    Ok(fact)
}

/// The transition guard of step 6 (after the policies, before the first `require`): the state the
/// field has, the answer `409 InvalidTransition` when it is not one of the sources, and the
/// `transition` the handler receives.
///
/// # Errors
/// What the generator cannot express: an optional fact or step, or a machine that is no enum.
pub fn transition_guard(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    index: usize,
    ep: &Endpoint,
) -> Result<(), String> {
    let Some(t) = &ep.rules.transition else {
        return Ok(());
    };
    let fact = transition_fact(ep, t)?;
    let (machine, ty) = machine(ctx, t)?;
    let sources: Vec<String> = t
        .from
        .iter()
        .map(|s| format!("{ty}::{}", variant(s)))
        .collect();
    let field = lit(&wire_path(t));
    w.line(format!(
        "// transition {}.{}: {} -> {}",
        t.fact,
        wire_path(t),
        t.from.join(", "),
        t.to
    ));
    w.line(format!(
        "let transition_from = {};",
        field_place(&fact_local(&fact.name), t)
    ));
    w.open(format!(
        "if !matches!(transition_from, {})",
        sources.join(" | ")
    ));
    w.line(format!(
        "return Err(vr::refusal(&ENDPOINTS[{index}], {}, &[(\"field\", {field}), (\"from\", transition_from.wire()), (\"to\", {})], vr::guard_violation({}, &[], \"violates rule transition\")));",
        lit(&prelude_sig(ctx, "InvalidTransition")),
        lit(&variant_wire(machine, &t.to)?),
        lit(&t.id)
    ));
    w.close();
    w.line(format!(
        "let transition = vr::Transition {{ field: {field}.to_string(), from: transition_from, to: {ty}::{} }};",
        variant(&t.to)
    ));
    Ok(())
}

/// Whether the state of the fact, a local of `can_<endpoint>`, is one of the sources of the
/// transition: the condition of the guard as a boolean expression.
///
/// # Errors
/// What the generator cannot express.
pub fn transition_holds(
    ctx: &Ctx<'_, '_>,
    ep: &Endpoint,
    t: &EndpointTransition,
) -> Result<String, String> {
    transition_fact(ep, t)?;
    let (_, ty) = machine(ctx, t)?;
    let sources: Vec<String> = t
        .from
        .iter()
        .map(|s| format!("{ty}::{}", variant(s)))
        .collect();
    let place = field_place(&format!("(*{})", local_ident(&t.fact)), t);
    Ok(format!("matches!({place}, {})", sources.join(" | ")))
}

/// What step 6 hands to the handler next to the facts: the `transition`, in the bound arguments.
pub fn hand_over_transition(w: &mut CodeWriter, ep: &Endpoint) {
    if ep.rules.transition.is_some() {
        w.open("if let Ok(value) = serde_json::to_value(&transition)");
        w.line("args.insert(\"transition\".to_string(), value);");
        w.close();
    }
}

/// The target check of step 9, after the written `ensure` statements: the same field of the
/// result must hold the target state. It reports a violation like any `ensure`.
///
/// # Errors
/// What the generator cannot express.
pub fn target_check(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    ep: &Endpoint,
) -> Result<(), String> {
    let (true, Some(t)) = (has_target(ep), &ep.rules.transition) else {
        return Ok(());
    };
    let (e, ty) = machine(ctx, t)?;
    let place = field_place("r", t);
    w.line(format!("// transition {}: the result holds `{}`", t.fact, t.to));
    w.open(format!("if {place} != {ty}::{}", variant(&t.to)));
    w.line(format!(
        "out.push(vr::guard_violation({}, &[], &format!(\"the result must hold the state `{{}}` in `{{}}`, found `{{}}`\", {}, {}, {place}.wire())));",
        lit(&t.id),
        lit(&variant_wire(e, &t.to)?),
        lit(&wire_path(t))
    ));
    w.close();
    Ok(())
}

/// `&place`, or `&value` for a computed argument: the argument of a policy as a reference to the
/// value the developer sees.
pub(crate) fn arg_ref(ctx: &Ctx<'_, '_>, sc: &Scope, expr: &Expr) -> Result<String, String> {
    let mut p = RustPrinter::for_block(ctx, sc.clone());
    if let Some(place) = p.raw_place(expr) {
        first_problem(&p)?;
        return Ok(format!("&{place}"));
    }
    Ok(format!("&{}", dev_value(ctx, sc, expr, false)?))
}

/// One `allow` of step 6: the first denial is `403 Forbidden`, carrying the message of the policy.
/// An argument that cannot be evaluated denies with `not permitted`.
///
/// # Errors
/// What the generator cannot express: an unknown policy, or an argument it cannot print.
pub fn allow(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    plan: &Plan,
    index: usize,
    a: &EndpointAllow,
    sc: &Scope,
) -> Result<(), String> {
    let policy = ctx
        .tree
        .ir
        .policies
        .get(a.policy.index())
        .ok_or("an `allow` names a policy that does not exist")?;
    let verdict = plan
        .verdict_path(ctx.tree, a.policy.index())
        .ok_or("an `allow` names a policy that is not generated")?;
    let mut args = Vec::new();
    for e in &a.args {
        args.push(arg_ref(ctx, sc, e)?);
    }
    args.push("now".to_string());
    let reads: Vec<String> = a.reads.iter().map(|r| lit(r)).collect();
    w.line(format!("// allow {}", policy.name));
    w.open(format!(
        "if let Some(text) = vr::denial((|| -> vr::Res<Result<(), String>> {{ Ok({verdict}({})) }})())",
        args.join(", ")
    ));
    w.line(format!(
        "return Err(vr::refusal(&ENDPOINTS[{index}], {}, &[(\"message\", &text)], vr::guard_violation({}, &[{}], &text)));",
        lit(&prelude_sig(ctx, "Forbidden")),
        lit(&policy.id),
        reads.join(", ")
    ));
    w.close();
    Ok(())
}

/// The Rust type of the principal of a service, as written in `here`.
#[must_use]
pub fn principal_type(ctx: &Ctx<'_, '_>, svc: &Service) -> Option<String> {
    let p = svc.principal.as_ref()?;
    Some(rust_type(ctx, &Type::Decl(p.decl), false))
}

/// The local `principal` of step 6: the value the auth hook returned, as the generated type of the
/// principal's model.
pub fn bind_principal(w: &mut CodeWriter, ty: &str) {
    w.line(format!(
        "let Some(principal) = ctx.principal::<{ty}>() else {{ return Err(Halt::Error(veld::Error::other(\"the caller is not known: the endpoint reads the principal\"))); }};"
    ));
}
