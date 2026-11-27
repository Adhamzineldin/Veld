//! Sagas (`spec/platform.md` sections 17 and 20): `E1201`, `E1202`, `E1203` and `W1201`.
//!
//! A saga endpoint has no block of rule statements: its body is a list of steps. Each step calls
//! an endpoint of a service of this package or of a package it consumes, and every argument is a
//! rule expression over the parameters of the saga and the steps before it. The checker is the
//! one of endpoint blocks: a parameter is `Root::Param` and a step is read as `Root::Fact`, typed
//! as the success type of the endpoint it calls.

use std::collections::{BTreeMap, BTreeSet};

use super::checker::{Checker, EpScope, Phase};
use super::endpoint::scope_field;
use super::saga_const::{self, root_names};
use crate::diagnostics::{Diagnostic, Span};
use crate::semantic::{Program, REndpoint, RField, ServiceInfo};
use crate::syntax::ast::{self, DeclKind};
use veld_ir::rules::Expr;
use veld_ir::{Constraints, Method, Type};

/// A checked saga.
#[derive(Debug, Clone)]
pub struct RSaga {
    pub steps: Vec<RStep>,
    /// The `timeout`, in milliseconds.
    pub timeout_ms: i64,
    /// The errors the saga writes in its own responses: those a step may fail with and the saga
    /// then answers as the step did (`spec/platform.md` section 18, "The answer").
    pub passes: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct RStep {
    pub name: String,
    pub call: RCall,
    pub compensate: Option<RCall>,
    pub span: Span,
}

/// A checked call.
#[derive(Debug, Clone)]
pub struct RCall {
    /// Index into `Program::services`.
    pub service: usize,
    pub endpoint: String,
    /// In the order of the parameters of the endpoint called.
    pub args: Vec<RArg>,
}

#[derive(Debug, Clone)]
pub struct RArg {
    pub name: String,
    pub value: RValue,
}

#[derive(Debug, Clone)]
pub enum RValue {
    Expr(Expr),
    Object(Vec<RObjectField>),
}

#[derive(Debug, Clone)]
pub struct RObjectField {
    pub name: String,
    pub wire: String,
    pub value: RValue,
}

/// Checks every saga of the program and stores the result on its endpoint.
pub(crate) fn check(
    prog: &mut Program<'_>,
    consumes: &BTreeMap<String, Vec<String>>,
    diags: &mut Vec<Diagnostic>,
) {
    let mut found: Vec<(usize, usize, RSaga)> = Vec::new();
    for (si, s) in prog.services.iter().enumerate() {
        for (ei, ep) in s.endpoints.iter().enumerate() {
            let Some(ast_ep) = ast_saga(prog, s, &ep.name) else {
                continue;
            };
            if let Some(saga) = check_saga(prog, consumes, s, ep, ast_ep, diags) {
                found.push((si, ei, saga));
            }
        }
    }
    for (si, ei, saga) in found {
        prog.services[si].endpoints[ei].saga = Some(saga);
    }
}

/// The syntax of the saga endpoint `name` of service `s`.
fn ast_saga<'a>(prog: &Program<'a>, s: &ServiceInfo, name: &str) -> Option<&'a ast::Endpoint> {
    let file = prog.modules[s.module].ast;
    file.decls.iter().find_map(|d| match &d.kind {
        DeclKind::Service(sd) if sd.name.name == s.name => sd
            .endpoints
            .iter()
            .find(|e| e.name.name == name)
            .filter(|e| e.saga.is_some()),
        DeclKind::Model(_)
        | DeclKind::Enum(_)
        | DeclKind::Error(_)
        | DeclKind::Union(_)
        | DeclKind::Service(_)
        | DeclKind::Const(_)
        | DeclKind::Event(_)
        | DeclKind::Channel(_) => None,
    })
}

/// What the check of one call needs to know about the saga that makes it.
pub(super) struct Cx<'p, 'a, 'd> {
    pub prog: &'p Program<'a>,
    pub consumes: &'p BTreeMap<String, Vec<String>>,
    pub module: usize,
    pub pkg: usize,
    pub ep: &'p REndpoint,
    pub diags: &'d mut Vec<Diagnostic>,
    pub params: Vec<RField>,
    /// The steps that can be read here, as facts.
    pub facts: Vec<RField>,
    pub poisoned: BTreeSet<String>,
}

fn check_saga(
    prog: &Program<'_>,
    consumes: &BTreeMap<String, Vec<String>>,
    s: &ServiceInfo,
    ep: &REndpoint,
    ast_ep: &ast::Endpoint,
    diags: &mut Vec<Diagnostic>,
) -> Option<RSaga> {
    let decl = ast_ep.saga.as_ref()?;
    let before = diags.iter().filter(|d| d.is_error()).count();
    let params: Vec<RField> = ep
        .params
        .iter()
        .filter(|p| !p.stream)
        .map(|p| scope_field(&p.name, &p.ty, p.optional, p.constraints.clone()))
        .collect();
    let poisoned = ast_ep
        .params
        .iter()
        .map(|p| p.name.name.clone())
        .filter(|n| !ep.params.iter().any(|p| &p.name == n))
        .collect();
    let mut cx = Cx {
        prog,
        consumes,
        module: s.module,
        pkg: s.pkg,
        ep,
        diags,
        params,
        facts: Vec::new(),
        poisoned,
    };
    cx.shape(ast_ep, decl);
    let steps = cx.steps(decl);
    let timeout_ms = cx.timeout(decl);
    cx.last_step_type(decl, &steps);
    let errors = cx.diags.iter().filter(|d| d.is_error()).count();
    if errors > before {
        return None;
    }
    Some(RSaga {
        steps,
        timeout_ms: timeout_ms?,
        passes: written_errors(prog, ep, ast_ep),
    })
}

/// The errors of the responses the saga writes: the declarations of its `|` items after the
/// success, found among the errors of the endpoint.
fn written_errors(prog: &Program<'_>, ep: &REndpoint, e: &ast::Endpoint) -> Vec<usize> {
    let Some(r) = &e.responses else {
        return Vec::new();
    };
    let mut out: Vec<usize> = Vec::new();
    for (_, d) in &ep.errors {
        let name = &prog.decls[*d].name;
        let written = r.items.iter().skip(1).any(|i| {
            matches!(
                i.ty.as_ref().map(|t| &t.kind),
                Some(ast::TypeExprKind::Named { name: n, .. }) if &n.name == name
            )
        });
        if written && !out.contains(d) {
            out.push(*d);
        }
    }
    out
}

impl Cx<'_, '_, '_> {
    pub fn err(&mut self, code: &'static str, msg: impl Into<String>, span: Span) {
        self.diags.push(Diagnostic::error(code, msg, span));
    }

    /// Runs `f` with an expression checker whose scope is the parameters and `facts`.
    pub fn with_checker<T>(&mut self, f: impl FnOnce(&mut Checker<'_, '_, '_>) -> T) -> T {
        let mut c = Checker {
            prog: self.prog,
            module: self.module,
            fields: &[],
            poisoned: self.poisoned.clone(),
            diags: &mut *self.diags,
            lambdas: Vec::new(),
            known: BTreeSet::new(),
            bounded: Vec::new(),
            reads: Vec::new(),
            reads_readonly: false,
            ep: Some(EpScope {
                params: self.params.clone(),
                facts: self.facts.clone(),
                result: None,
                phase: Phase::Require,
                in_old: false,
                lenient: false,
                principal: None,
                fields_in_scope: false,
            }),
        };
        f(&mut c)
    }

    /// `E1203`: what the head of the saga must not be.
    fn shape(&mut self, e: &ast::Endpoint, decl: &ast::SagaDecl) {
        if decl.steps.is_empty() {
            self.err("E1203", "a saga needs at least one `step`", decl.word);
        }
        if let Some(p) = e.params.iter().find(|p| p.stream) {
            self.err("E1203", "a saga cannot take a stream", p.name.span);
        }
        if self.ep.success_stream || self.ep.method != Method::Post {
            self.err(
                "E1203",
                "a saga is a `post` endpoint with no stream",
                decl.word,
            );
        }
    }

    /// The steps, in order.
    fn steps(&mut self, decl: &ast::SagaDecl) -> Vec<RStep> {
        let mut out = Vec::new();
        let mut seen: BTreeMap<String, Span> = BTreeMap::new();
        for (i, st) in decl.steps.iter().enumerate() {
            let later: Vec<&str> = decl.steps[i + 1..]
                .iter()
                .map(|s| s.name.name.as_str())
                .collect();
            let name_ok = self.step_name(st, &mut seen);
            let call = self.call(&st.call, &later);
            let own = if name_ok {
                call.as_ref().and_then(|c| self.value_type(c))
            } else {
                None
            };
            let compensate = st.compensate.as_ref().map(|c| {
                self.push_fact(&st.name.name, own.as_ref());
                let checked = self.call(c, &later);
                self.pop_fact(own.is_some());
                checked
            });
            if name_ok {
                self.push_fact(&st.name.name, own.as_ref());
            } else {
                self.poisoned.insert(st.name.name.clone());
            }
            self.warn_no_compensate(st, i + 1 == decl.steps.len());
            if let (true, Some(call)) = (name_ok, call) {
                out.push(RStep {
                    name: st.name.name.clone(),
                    call,
                    compensate: compensate.flatten(),
                    span: st.name.span,
                });
            }
        }
        out
    }

    /// Makes the value of a step readable.
    fn push_fact(&mut self, name: &str, ty: Option<&Type>) {
        if let Some(ty) = ty {
            self.facts
                .push(scope_field(name, ty, false, Constraints::default()));
        }
    }

    fn pop_fact(&mut self, pushed: bool) {
        if pushed {
            self.facts.pop();
        }
    }

    /// The value a step has: the success type of its endpoint, when it is not a stream.
    fn value_type(&self, call: &RCall) -> Option<Type> {
        let ep = self
            .prog
            .services
            .get(call.service)?
            .endpoints
            .iter()
            .find(|e| e.name == call.endpoint)?;
        if ep.success_stream {
            None
        } else {
            ep.success_ty.clone()
        }
    }

    /// `E1203` for a name that is taken; `true` when the name is free.
    fn step_name(&mut self, st: &ast::SagaStepDecl, seen: &mut BTreeMap<String, Span>) -> bool {
        let name = st.name.name.as_str();
        if let Some(first) = seen.get(name).copied() {
            self.diags.push(
                Diagnostic::error(
                    "E1203",
                    format!("step `{name}` is already declared in this saga"),
                    st.name.span,
                )
                .with_label(first, "first declared here"),
            );
            return false;
        }
        let taken = if self.params.iter().any(|p| p.name == name) {
            Some("a parameter")
        } else if name == "now" {
            Some("the clock")
        } else {
            None
        };
        if let Some(what) = taken {
            self.diags.push(
                Diagnostic::error(
                    "E1203",
                    format!("step `{name}` has the name of {what}"),
                    st.name.span,
                )
                .with_help("every name in a saga must be different"),
            );
            return false;
        }
        seen.insert(name.to_string(), st.name.span);
        true
    }

    /// `W1201`: a step before the last one that nothing can undo.
    fn warn_no_compensate(&mut self, st: &ast::SagaStepDecl, last: bool) {
        if last || st.compensate.is_some() {
            return;
        }
        self.diags.push(
            Diagnostic::warning(
                "W1201",
                format!("step `{}` has no `compensate`", st.name.name),
                st.name.span,
            )
            .with_help(
                "once it has run, the saga cannot be undone past it: a failure of a later step leaves its effect",
            ),
        );
    }

    /// `E1203`: the last step answers the type the saga answers.
    fn last_step_type(&mut self, decl: &ast::SagaDecl, steps: &[RStep]) {
        let (Some(want), Some(last), Some(ast_last)) =
            (self.ep.success_ty.as_ref(), steps.last(), decl.steps.last())
        else {
            return;
        };
        if steps.len() != decl.steps.len() {
            return;
        }
        let found = self.value_type(&last.call);
        if found.as_ref() != Some(want) {
            let shown = found.map_or_else(|| "nothing".to_string(), |t| self.prog.type_name(&t));
            self.err(
                "E1203",
                format!(
                    "the last step `{}` answers `{shown}`, the saga answers `{}`",
                    last.name,
                    self.prog.type_name(want)
                ),
                ast_last.name.span,
            );
        }
    }

    /// `timeout`: a positive constant duration, in milliseconds.
    fn timeout(&mut self, decl: &ast::SagaDecl) -> Option<i64> {
        let Some(t) = &decl.timeout else {
            self.err(
                "E1203",
                "a saga needs a `timeout`, such as `timeout minutes(5)`",
                decl.word,
            );
            return None;
        };
        let typed = self.with_checker(|c| c.expr(&t.expr, None))?;
        let Some(ms) = saga_const::duration_ms(&typed) else {
            self.err(
                "E1203",
                "the `timeout` must be a constant duration such as `minutes(5)`",
                t.expr.span,
            );
            return None;
        };
        if ms <= 0 {
            self.err(
                "E1203",
                "the `timeout` must be longer than zero",
                t.expr.span,
            );
            return None;
        }
        Some(ms)
    }

    /// The names an expression reads that are not in scope yet, for `E1203`.
    pub fn reads_later(&mut self, e: &ast::Expr, later: &[&str]) -> bool {
        let mut bad = false;
        for id in root_names(e) {
            let known = self
                .params
                .iter()
                .chain(&self.facts)
                .any(|f| f.name == id.name);
            let reason = if known {
                None
            } else if later.contains(&id.name.as_str()) {
                Some(format!("step `{}` runs later than this call", id.name))
            } else if id.name == "now" {
                Some(
                    "a saga cannot read the clock: a call that is made again must be the same call"
                        .to_string(),
                )
            } else {
                None
            };
            if let Some(why) = reason {
                self.err("E1203", why, id.span);
                bad = true;
            }
        }
        bad
    }
}
