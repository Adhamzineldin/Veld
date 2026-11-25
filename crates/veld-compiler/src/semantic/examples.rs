//! Examples (`spec/rules.md` section 12): which endpoint a call names (`E0950`), which facts
//! are given (`E0951`) and whether every value fits the type it is used at (`E0952`).
//!
//! An example states only what it cares about. The values it leaves out are filled in when it
//! runs (section 12.2), so nothing here requires a field, a parameter or a given.

mod values;

use std::collections::BTreeMap;

use super::{DKind, Program, REndpoint, ServiceInfo};
use crate::diagnostics::{Diagnostic, Span};
use crate::syntax::ast::{self, doc_text};
use values::Values;
use veld_ir::Type;
use veld_ir::endpoint_rules::ExampleValue;

/// A checked example.
#[derive(Debug, Clone)]
pub struct RExample {
    pub name: String,
    pub pkg: usize,
    pub module: usize,
    /// Index into `Program::services`.
    pub service: usize,
    pub endpoint: String,
    /// `None` is `absent`.
    pub givens: Vec<(String, Option<ExampleValue>)>,
    pub now: Option<String>,
    pub args: Vec<(String, ExampleValue)>,
    pub returns: Option<ExampleValue>,
    pub outcome: ROutcome,
    pub docs: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ROutcome {
    Ok(Option<ExampleValue>),
    /// The error declaration (an index into `Program::decls`) and its pattern.
    Error(usize, Option<ExampleValue>),
}

/// The errors every endpoint can answer with although it does not list them: the implicit ones
/// of `spec/rules.md` section 12.1, plus `Unauthorized`.
const IMPLICIT_ERRORS: [&str; 6] = [
    "ValidationError",
    "NotFound",
    "Forbidden",
    "InvalidTransition",
    "Internal",
    "Unauthorized",
];

pub fn check(prog: &mut Program<'_>, diags: &mut Vec<Diagnostic>) {
    let mut out = Vec::new();
    let mut names: BTreeMap<(usize, String), Span> = BTreeMap::new();
    for mi in 0..prog.modules.len() {
        let pkg = prog.modules[mi].pkg;
        let ast = prog.modules[mi].ast;
        for ex in &ast.examples {
            let key = (pkg, ex.name.value.clone());
            if let Some(first) = names.get(&key) {
                diags.push(
                    Diagnostic::error(
                        "E0950",
                        format!(
                            "example \"{}\" is already declared in this package",
                            ex.name.value
                        ),
                        ex.name.span,
                    )
                    .with_label(*first, "first declared here"),
                );
                continue;
            }
            names.insert(key, ex.name.span);
            out.extend(check_example(prog, mi, ex, diags));
        }
    }
    prog.examples = out;
}

/// The endpoint of `call` among the services of the package (`E0950`).
fn resolve_endpoint<'p>(
    prog: &'p Program<'_>,
    pkg: usize,
    call: &ast::ExampleCall,
    diags: &mut Vec<Diagnostic>,
) -> Option<(usize, &'p ServiceInfo, &'p REndpoint)> {
    let name = &call.endpoint.name;
    let services: Vec<(usize, &ServiceInfo)> = prog
        .services
        .iter()
        .enumerate()
        .filter(|(_, s)| s.pkg == pkg)
        .filter(|(_, s)| call.service.as_ref().is_none_or(|n| n.name == s.name))
        .collect();
    let hits: Vec<(usize, &ServiceInfo, &REndpoint)> = services
        .iter()
        .flat_map(|(i, s)| {
            s.endpoints
                .iter()
                .filter(|e| &e.name == name)
                .map(move |e| (*i, *s, e))
        })
        .collect();
    match hits.as_slice() {
        [one] => Some(*one),
        [] => {
            let all: Vec<&str> = services
                .iter()
                .flat_map(|(_, s)| s.endpoints.iter().map(|e| e.name.as_str()))
                .collect();
            let diag = Diagnostic::error(
                "E0950",
                format!("no endpoint `{name}` in this package"),
                call.endpoint.span,
            )
            .did_you_mean(name, all.iter().copied(), None);
            diags.push(diag);
            None
        }
        _ => {
            let qualified: Vec<String> = hits
                .iter()
                .map(|(_, s, e)| format!("{}.{}", s.name, e.name))
                .collect();
            diags.push(
                Diagnostic::error(
                    "E0950",
                    format!("endpoint `{name}` is ambiguous: {}", qualified.join(", ")),
                    call.endpoint.span,
                )
                .with_help("write `Service.endpoint`"),
            );
            None
        }
    }
}

fn check_example(
    prog: &Program<'_>,
    module: usize,
    ex: &ast::Example,
    diags: &mut Vec<Diagnostic>,
) -> Option<RExample> {
    let pkg = prog.modules[module].pkg;
    let (service, _, ep) = resolve_endpoint(prog, pkg, &ex.call, diags)?;
    let mut v = Values {
        prog,
        module,
        diags,
        givens: Vec::new(),
    };
    let mut ok = true;
    let mut givens = Vec::new();
    for g in &ex.givens {
        match v.given(g, ep) {
            Some(x) => givens.push(x),
            None => ok = false,
        }
    }
    ok &= v.principal(ex.principal.as_ref());
    let now = v.clock(ex.at.as_ref());
    let args = v.args(&ex.call, ep);
    let returns = v.returns(&ex.call, ep);
    let outcome = v.outcome(&ex.call, ep);
    // Every part is checked before the first failure is acted on, so one run reports them all.
    let (now, args, returns, outcome) = (now?, args?, returns?, outcome?);
    ok.then(|| RExample {
        name: ex.name.value.clone(),
        pkg,
        module,
        service,
        endpoint: ex.call.endpoint.name.clone(),
        givens,
        now,
        args,
        returns,
        outcome,
        docs: doc_text(&ex.docs),
        span: ex.span,
    })
}

/// True for a model or error declaration.
fn is_record(prog: &Program<'_>, ty: &Type) -> Option<usize> {
    match ty {
        Type::Decl(d) if matches!(prog.decls[d.index()].kind, DKind::Model | DKind::Error) => {
            Some(d.index())
        }
        Type::Decl(_) | Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Page(_) => None,
    }
}

/// Whether `name` is one of the errors every endpoint can answer with.
fn is_implicit_error(prog: &Program<'_>, decl: usize) -> bool {
    prog.is_prelude(decl) && IMPLICIT_ERRORS.contains(&prog.decls[decl].name.as_str())
}

#[cfg(test)]
mod tests;
