//! `can<Endpoint>(...)` in the client (`spec/rules.md` section 11.4): whether the server would let
//! a call past the policies and the transition guard, for showing or hiding a button.
//!
//! It takes the facts, the principal and the parameters that the endpoint's `allow` arguments and
//! its `transition` read, in that order, reads the system clock, and never sends or rejects
//! anything. An evaluation error answers `false`.

use std::collections::BTreeSet;

use veld_codegen::{CodeWriter, GenDiagnostic, naming};
use veld_ir::{Endpoint, EndpointFact, Param, Root, Service, Type};

use crate::expr::{EndpointPrinter, EndpointScope, Principal};
use crate::machine::Move;
use crate::policy::{LOCALS, Plan, Roots, idents, verdict_call};
use crate::tree::FileCtx;
use crate::types::ts_type;
use crate::util::{doc_comment, method_key, prop_key, ts_str};

/// Whether the client has a `can` method for the endpoint.
#[must_use]
pub fn wanted(ep: &Endpoint) -> bool {
    !ep.rules.allows.is_empty() || ep.rules.transition.is_some()
}

fn unsupported(generator: &str, svc: &Service, ep: &Endpoint, what: &str) -> GenDiagnostic {
    let mut d = GenDiagnostic::new(
        "E0801",
        format!(
            "{}.{}: the `{generator}` generator cannot represent {what}",
            svc.name, ep.name
        ),
    );
    d.origin = ep.rules.first_origin().cloned();
    d
}

/// What the method takes, with the names it gives them.
struct Inputs<'a> {
    facts: Vec<(&'a EndpointFact, String)>,
    principal: Option<(String, Type)>,
    params: Vec<(&'a Param, String)>,
}

fn inputs<'a>(ep: &'a Endpoint, svc: &Service, roots: &Roots) -> Inputs<'a> {
    let facts: Vec<&EndpointFact> = ep
        .rules
        .facts
        .iter()
        .filter(|f| roots.facts.contains(&f.name))
        .collect();
    let principal = svc
        .principal
        .as_ref()
        .filter(|_| roots.principal)
        .map(|p| (p.name.clone(), Type::Decl(p.decl)));
    let params: Vec<&Param> = ep
        .params
        .iter()
        .filter(|p| !p.stream && roots.params.contains(&p.name))
        .collect();
    let mut names: Vec<&str> = facts.iter().map(|f| f.name.as_str()).collect();
    names.extend(principal.iter().map(|(n, _)| n.as_str()));
    names.extend(params.iter().map(|p| p.name.as_str()));
    let mut names = idents(names, &LOCALS).into_iter();
    Inputs {
        facts: facts
            .into_iter()
            .filter_map(|f| names.next().map(|n| (f, n)))
            .collect(),
        principal: principal.and_then(|(_, ty)| names.next().map(|n| (n, ty))),
        params: params
            .into_iter()
            .filter_map(|p| names.next().map(|n| (p, n)))
            .collect(),
    }
}

/// The name of `can<Endpoint>` among the members of the client class.
fn can_name(svc: &Service, ep: &Endpoint) -> String {
    let mut name = format!("can{}", naming::pascal(&ep.name));
    let methods: Vec<String> = svc.endpoints.iter().map(|e| method_key(&e.name)).collect();
    while methods.contains(&name) {
        name.push('_');
    }
    prop_key(&name)
}

fn scope_of(svc: &Service, ep: &Endpoint, given: &Inputs<'_>) -> EndpointScope {
    let mut locals: Vec<(Root, String)> = Vec::new();
    locals.extend(
        given
            .facts
            .iter()
            .map(|(f, n)| (Root::Fact(f.name.clone()), n.clone())),
    );
    if let Some((name, _)) = &given.principal {
        locals.push((Root::Principal, name.clone()));
    }
    locals.extend(
        given
            .params
            .iter()
            .map(|(p, n)| (Root::Param(p.name.clone()), n.clone())),
    );
    EndpointScope {
        optional_params: ep
            .params
            .iter()
            .filter(|p| p.optional)
            .map(|p| p.name.clone())
            .collect(),
        optional_facts: ep
            .rules
            .facts
            .iter()
            .filter(|f| f.optional)
            .map(|f| f.name.clone())
            .collect::<BTreeSet<_>>(),
        principal: svc.principal.as_ref().map(|p| Principal {
            name: p.name.clone(),
            ty: Type::Decl(p.decl),
        }),
        locals,
        ensure: false,
    }
}

/// Writes `can<Endpoint>` of one endpoint, a static method of the client class.
pub fn write_can(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    plan: &Plan,
    (svc, ep): (&Service, &Endpoint),
    diags: &mut Vec<GenDiagnostic>,
) {
    let mut roots = Roots::default();
    for a in &ep.rules.allows {
        a.args.iter().for_each(|e| roots.add(e));
    }
    if let Some(t) = &ep.rules.transition {
        roots.facts.insert(t.fact.clone());
    }
    let given = inputs(ep, svc, &roots);
    let ns = fc.rules_ns();
    let mut printer = EndpointPrinter::new(fc.tree.ir, &ns, scope_of(svc, ep, &given));
    let mut notes: Vec<&'static str> = Vec::new();
    let mut checks = Vec::new();
    for a in &ep.rules.allows {
        if let Some(call) = verdict_call(fc, &mut printer, plan, a, &mut notes) {
            checks.push(format!("{call}.permitted"));
        }
    }
    match Move::of(fc.tree.ir, ep) {
        Ok(Some(mv)) => match mv.current(&printer) {
            Some(current) => {
                let sources = live_sources(fc, &mv);
                checks.push(format!("{sources}.includes({current})"));
            }
            None => notes.push("a transition path it cannot read"),
        },
        Ok(None) => {}
        Err(why) => notes.push(why),
    }
    diags.extend(notes.iter().map(|n| unsupported("typescript", svc, ep, n)));
    let flagged = takes_flags(fc.tree.ir, ep);
    if flagged {
        let names = given
            .facts
            .iter()
            .map(|(_, n)| n)
            .chain(given.principal.iter().map(|(n, _)| n))
            .chain(given.params.iter().map(|(_, n)| n));
        if names.into_iter().any(|n| n == "flags" || n == "veldFlags") {
            diags.push(unsupported(
                "typescript",
                svc,
                ep,
                "a parameter or a fact called `flags` or `veldFlags` next to the flags argument of `can<Endpoint>`",
            ));
        }
    }
    let mut signature = signature(fc, &given);
    if flagged {
        if !signature.is_empty() {
            signature.push_str(", ");
        }
        signature.push_str("flags?: (name: string) => boolean");
    }
    doc_comment(w, Some(&docs(ep, &given)), None);
    w.open(format!(
        "static {}({signature}): boolean",
        can_name(svc, ep)
    ));
    if !ep.rules.allows.is_empty() {
        w.line("const now = BigInt(Date.now());");
    }
    let guarded = fc.rt("guarded");
    let body = if checks.is_empty() {
        "true".to_string()
    } else {
        checks.join(" && ")
    };
    if flagged {
        let ns = fc.flags_ns();
        w.line(format!(
            "return {ns}.withAnswers(flags, () => {guarded}(() => {body}));"
        ));
    } else {
        w.line(format!("return {guarded}(() => {body});"));
    }
    w.close();
}

/// Whether `can<Endpoint>` takes the caller's answers about flags: a policy it asks has a flagged
/// clause, or an edge its transition takes is flagged.
fn takes_flags(ir: &veld_ir::Ir, ep: &Endpoint) -> bool {
    let policies = ep.rules.allows.iter().any(|a| {
        ir.policies
            .get(a.policy.index())
            .is_some_and(|p| p.clauses.iter().any(|c| c.flag.is_some()))
    });
    policies
        || ep
            .rules
            .transition
            .as_ref()
            .is_some_and(|t| flagged_edge(ir, t))
}

fn flagged_edge(ir: &veld_ir::Ir, t: &veld_ir::EndpointTransition) -> bool {
    let veld_ir::DeclKind::Enum(machine) = &ir.decl(t.decl).kind else {
        return false;
    };
    machine
        .flows
        .iter()
        .any(|f| f.to == t.to && t.from.contains(&f.from) && f.flag.is_some())
}

/// The source states of the transition, with the flagged edges there only while their flag is on
/// (a reader with no provider counts a `flow` as off).
fn live_sources(fc: &mut FileCtx<'_, '_>, mv: &Move<'_>) -> String {
    if !flagged_edge(fc.tree.ir, mv.transition) {
        return mv.sources_array();
    }
    let ns = fc.flags_ns();
    let items: Vec<String> = mv
        .transition
        .from
        .iter()
        .zip(&mv.sources)
        .map(|(name, wire)| {
            let flag = mv
                .machine
                .flows
                .iter()
                .find(|f| f.from == *name && f.to == mv.transition.to)
                .and_then(|f| f.flag.as_deref());
            match flag {
                Some(flag) => format!(
                    "...({ns}.flagOn({}, false) ? [{}] : [])",
                    ts_str(flag),
                    ts_str(wire)
                ),
                None => ts_str(wire),
            }
        })
        .collect();
    format!("[{}]", items.join(", "))
}

fn signature(fc: &mut FileCtx<'_, '_>, given: &Inputs<'_>) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (f, name) in &given.facts {
        parts.push(format!("{name}: {}", typed(fc, &f.ty, f.optional)));
    }
    if let Some((name, ty)) = &given.principal {
        parts.push(format!("{name}: {}", ts_type(fc, ty)));
    }
    for (p, name) in &given.params {
        parts.push(format!("{name}: {}", typed(fc, &p.ty, p.optional)));
    }
    parts.join(", ")
}

/// A type as an argument: an optional value is `T | undefined`, because the arguments come in a
/// fixed order and an optional one may not be followed by a required one.
fn typed(fc: &mut FileCtx<'_, '_>, ty: &Type, optional: bool) -> String {
    let t = ts_type(fc, ty);
    if optional {
        format!("{t} | undefined")
    } else {
        t
    }
}

fn docs(ep: &Endpoint, given: &Inputs<'_>) -> String {
    let mut text = format!(
        "Whether the server would let a call of `{}` past the policies and the transition guard, for these facts, this caller and these parameters. It never sends or rejects anything: use it to show or hide a button. The server decides.",
        ep.name
    );
    let mut args: Vec<String> = Vec::new();
    for (f, name) in &given.facts {
        args.push(format!("@param {name} the fact `{}`", f.name));
    }
    if let Some((name, _)) = &given.principal {
        args.push(format!("@param {name} the caller"));
    }
    for (p, name) in &given.params {
        args.push(format!("@param {name} the parameter `{}`", p.name));
    }
    if !args.is_empty() {
        text.push_str("\n\n");
        text.push_str(&args.join("\n"));
    }
    text
}
