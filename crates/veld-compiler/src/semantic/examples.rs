//! Examples (`spec/rules.md` section 12): which endpoint a call names (`E0950`), which facts
//! are given (`E0951`) and whether every value fits the type it is used at (`E0952`).
//!
//! An example states only what it cares about. The values it leaves out are filled in when it
//! runs (section 12.2), so nothing here requires a field, a parameter or a given.

mod emitted;
mod events;
mod idempotency;
mod saga;
mod values;

use std::collections::BTreeMap;

use super::{DKind, Program, REndpoint, ServiceInfo, flags};
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
    /// `as value`: the principal, typed by the model of the package's principal.
    pub principal: Option<ExampleValue>,
    pub now: Option<String>,
    pub args: Vec<(String, ExampleValue)>,
    pub returns: Option<ExampleValue>,
    /// `ifMatch 3` or `ifMatch any`.
    pub if_match: Option<veld_ir::ExampleIfMatch>,
    /// The `key` line.
    pub key: Option<RKey>,
    /// The saga lines (`spec/platform.md` section 18).
    pub saga: Option<RSagaExample>,
    /// The flags that are on (`given flag "name"`), sorted.
    pub flags: Vec<String>,
    pub outcome: ROutcome,
    /// `expect emitted` (`spec/events.md` section 11.1): the events, as indices into
    /// `Program::decls`; empty for `expect not emitted`, `None` without such a line.
    pub emitted: Option<Vec<usize>>,
    pub docs: Option<String>,
    pub span: Span,
}

/// The checked saga lines of an example: what each call answers, and what must have happened.
#[derive(Debug, Clone, Default)]
pub struct RSagaExample {
    /// `given step`, in source order, by the name of the step.
    pub steps: Vec<(String, RSagaAnswer)>,
    /// `given undo`, in source order.
    pub undos: Vec<(String, RSagaAnswer)>,
    pub expect: Option<RSagaExpect>,
}

/// What a call of a saga answers.
#[derive(Debug, Clone)]
pub enum RSagaAnswer {
    Unavailable,
    Answer(ROutcome),
}

/// `expect compensated ..` or `expect stuck`.
#[derive(Debug, Clone)]
pub enum RSagaExpect {
    Compensated(Vec<String>),
    Stuck,
}

/// A checked `key` line (`spec/rules.md` section 11.9).
#[derive(Debug, Clone)]
pub struct RKey {
    pub value: String,
    pub store: RKeyStore,
}

/// What the line says the store holds.
#[derive(Debug, Clone)]
pub enum RKeyStore {
    New,
    Stored(ROutcome),
    Other,
    Running,
}

/// A checked event example (`spec/events.md` section 12).
#[derive(Debug, Clone)]
pub struct REventExample {
    pub name: String,
    pub pkg: usize,
    pub module: usize,
    /// The event, an index into `Program::decls`.
    pub event: usize,
    /// The channel, an index into `Program::channels`.
    pub channel: usize,
    pub value: ExampleValue,
    pub expect: REventExpect,
    pub docs: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum REventExpect {
    Rejected,
    /// (service index, handler name, whether it is expected to be called, whether `once`).
    Calls(Vec<(usize, String, bool, bool)>),
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
    let mut event_out = Vec::new();
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
            match &ex.body {
                ast::ExampleBody::Call(call) => {
                    out.extend(check_example(prog, mi, ex, call, diags))
                }
                ast::ExampleBody::Event(ev) => {
                    event_out.extend(events::check_event_example(prog, mi, ex, ev, diags));
                }
            }
        }
    }
    prog.examples = out;
    prog.event_examples = event_out;
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
        [] if services.is_empty() && call.service.is_some() => {
            unknown_service(prog, pkg, call, diags);
            None
        }
        [] => {
            let names = services
                .iter()
                .flat_map(|(_, s)| s.endpoints.iter().map(|e| e.name.as_str()));
            let diag = Diagnostic::error(
                "E0950",
                format!("no endpoint `{name}` in this package"),
                call.endpoint.span,
            )
            .did_you_mean(name, names, None);
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

fn unknown_service(
    prog: &Program<'_>,
    pkg: usize,
    call: &ast::ExampleCall,
    diags: &mut Vec<Diagnostic>,
) {
    let known = prog
        .services
        .iter()
        .filter(|s| s.pkg == pkg)
        .map(|s| s.name.as_str());
    let wanted = call.service.as_ref().map_or("", |s| s.name.as_str());
    let at = call.service.as_ref().map_or(call.endpoint.span, |s| s.span);
    let diag = Diagnostic::error(
        "E0950",
        format!("no service `{wanted}` in this package"),
        at,
    )
    .did_you_mean(wanted, known, None);
    diags.push(diag);
}

fn check_example(
    prog: &Program<'_>,
    module: usize,
    ex: &ast::Example,
    call: &ast::ExampleCall,
    diags: &mut Vec<Diagnostic>,
) -> Option<RExample> {
    let pkg = prog.modules[module].pkg;
    let (service, _, ep) = resolve_endpoint(prog, pkg, call, diags)?;
    let mut v = Values {
        prog,
        module,
        diags,
        givens: Vec::new(),
        pattern: false,
    };
    let mut ok = true;
    let mut givens = Vec::new();
    let flags = flags::example_flags(prog, ex, v.diags);
    ok &= flags.is_some();
    for g in ex.givens.iter().filter(|g| g.flag.is_none()) {
        match v.given(g, ep) {
            Some(x) => givens.push(x),
            None => ok = false,
        }
    }
    let principal = v.principal(ex.principal.as_ref());
    ok &= principal.is_some();
    let now = v.clock(ex.at.as_ref());
    let args = v.args(call, ep);
    let returns = v.returns(call, ep);
    let if_match = v.if_match(call.if_match.as_ref(), ep);
    let key = v.key(call.key.as_deref(), ep);
    let saga = v.saga_lines(ex.saga.as_ref(), call, ep);
    let outcome = v.outcome(call, ep);
    let emitted = emitted::check_emitted(prog, module, call, ep, call.returns.is_some(), diags);
    // Every part is checked before the first failure is acted on, so one run reports them all.
    let (now, args, returns, outcome) = (now?, args?, returns?, outcome?);
    let (if_match, key, saga, emitted) = (if_match?, key?, saga?, emitted?);
    ok.then(|| RExample {
        name: ex.name.value.clone(),
        pkg,
        module,
        service,
        endpoint: call.endpoint.name.clone(),
        givens,
        principal: principal.flatten(),
        now,
        args,
        returns,
        if_match,
        key,
        saga,
        flags: flags.unwrap_or_default(),
        outcome,
        emitted,
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
