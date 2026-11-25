//! `can<Endpoint>(...)` in the client (`spec/rules.md` section 11.4): whether the server would let
//! a call past the policies and the transition guard, for showing or hiding a button.
//!
//! It takes the facts, the principal and the parameters that the endpoint's `allow` arguments and
//! its `transition` read, in that order, and never sends or rejects anything.

use veld_codegen::{CodeWriter, GenDiagnostic, naming};
use veld_ir::{Endpoint, EndpointFact, Param, Service, Type};

use crate::endpoint::{Site, unsupported};
use crate::machine;
use crate::policy::{Plan, Roots, verdict_call};
use crate::rule_expr::{EndpointScope, JavaExpr, ScopeVar};
use crate::spec::method_name;
use crate::tree::FileCtx;
use crate::types::java_type;
use crate::util::{doc_comment, member_ident, unique};

/// Whether the client has a `can` method for the endpoint.
#[must_use]
pub fn wanted(ep: &Endpoint) -> bool {
    !ep.rules.allows.is_empty() || ep.rules.transition.is_some()
}

/// What the method takes: the facts, the principal and the parameters, with their Java names.
struct Inputs<'a> {
    facts: Vec<(&'a EndpointFact, String)>,
    principal: Option<(String, Type)>,
    params: Vec<(&'a Param, String)>,
}

fn inputs<'a>(ep: &'a Endpoint, svc: &Service, roots: &Roots, taken: &[&str]) -> Inputs<'a> {
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
        .map(|p| (member_ident(&p.name), Type::Decl(p.decl)));
    let params: Vec<&Param> = ep
        .params
        .iter()
        .filter(|p| !p.stream && roots.params.contains(&p.name))
        .collect();
    let mut names: Vec<String> = taken.iter().map(|t| (*t).to_string()).collect();
    names.extend(facts.iter().map(|f| member_ident(&f.name)));
    names.extend(principal.iter().map(|(n, _)| n.clone()));
    names.extend(params.iter().map(|p| member_ident(&p.name)));
    let mut names = unique(names).split_off(taken.len()).into_iter();
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

/// The name of `can<Endpoint>` among the methods of the client.
fn can_name(svc: &Service, ep: &Endpoint) -> String {
    let mut name = format!("can{}", naming::pascal(&ep.name));
    let methods: Vec<String> = svc.endpoints.iter().map(method_name).collect();
    while methods.contains(&name) {
        name.push('_');
    }
    name
}

/// Writes `can<Endpoint>` of one endpoint.
pub fn write_can(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    svc: &Service,
    ep: &Endpoint,
    problems: &mut Vec<GenDiagnostic>,
) {
    let mut roots = Roots::default();
    for a in &ep.rules.allows {
        a.args.iter().for_each(|e| roots.add(e));
    }
    if let Some(t) = &ep.rules.transition {
        roots.facts.insert(t.fact.clone());
    }
    // `now` is a local of the body: no input may take its name.
    let given = inputs(ep, svc, &roots, &["now"]);
    let site = Site {
        svc,
        ep,
        request: "",
    };
    let signature = signature(fc, &given);
    let scope = scope_of(&given);
    let docs = docs(ep, &given);
    let rule_error = fc.rt("RuleError");
    let plan = Plan::new(fc.tree);
    let mut printer = JavaExpr::for_endpoint(fc, scope);
    let mut checks = Vec::new();
    for a in &ep.rules.allows {
        if let Some(call) = verdict_call(&mut printer, &site, &plan, a, problems) {
            checks.push(format!("{call}.permitted()"));
        }
    }
    if let Some(cond) = machine::can_condition(&mut printer, &site, problems) {
        checks.push(cond);
    }
    problems.extend(
        printer
            .unsupported()
            .iter()
            .map(|what| unsupported(&site, what)),
    );
    doc_comment(w, Some(&docs.0), &docs.1, None);
    w.open(format!(
        "public static boolean {}({signature})",
        can_name(svc, ep)
    ));
    w.line("long now = System.currentTimeMillis();");
    w.open("try");
    w.line(format!("return {};", checks.join(" && ")));
    w.close_with(format!("}} catch ({rule_error} e) {{"));
    w.indent();
    w.line("return false;");
    w.close();
    w.close();
}

fn signature(fc: &mut FileCtx<'_, '_>, given: &Inputs<'_>) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (f, name) in &given.facts {
        parts.push(format!("{} {name}", java_type(fc, &f.ty, f.optional)));
    }
    if let Some((name, ty)) = &given.principal {
        parts.push(format!("{} {name}", java_type(fc, ty, false)));
    }
    for (p, name) in &given.params {
        parts.push(format!("{} {name}", java_type(fc, &p.ty, p.optional)));
    }
    parts.join(", ")
}

fn scope_of(given: &Inputs<'_>) -> EndpointScope {
    EndpointScope {
        facts: given
            .facts
            .iter()
            .map(|(f, name)| ScopeVar {
                name: f.name.clone(),
                code: name.clone(),
                ty: f.ty.clone(),
                optional: f.optional,
            })
            .collect(),
        principal: given.principal.as_ref().map(|(name, ty)| ScopeVar {
            name: "principal".to_string(),
            code: name.clone(),
            ty: ty.clone(),
            optional: false,
        }),
        params: given
            .params
            .iter()
            .map(|(p, name)| ScopeVar {
                name: p.name.clone(),
                code: name.clone(),
                ty: p.ty.clone(),
                optional: p.optional,
            })
            .collect(),
        result: None,
    }
}

fn docs(ep: &Endpoint, given: &Inputs<'_>) -> (String, Vec<(String, String)>) {
    let text = format!(
        "Whether the server would let a call of `{}` past the policies and the transition guard, for these facts, this caller and these parameters. It never sends or rejects anything: use it to show or hide a button. The server decides.",
        ep.name
    );
    let mut tags: Vec<(String, String)> = Vec::new();
    for (f, name) in &given.facts {
        tags.push((name.clone(), format!("the fact `{}`", f.name)));
    }
    if let Some((name, _)) = &given.principal {
        tags.push((name.clone(), "the caller".to_string()));
    }
    for (p, name) in &given.params {
        tags.push((name.clone(), format!("the parameter `{}`", p.name)));
    }
    (text, tags)
}
