//! The server pipeline of one request, as a pure function (`spec/rules.md` sections 5.3, 9.5).
//!
//! Step 5 (invariants on the bound parameters), step 6 (facts, `require`, the saved `old(...)`
//! values) and, once the handler has answered, steps 8 and 9 (invariants on the result, then
//! every `ensure`) in `strict` mode. Policies (section 11) and the transition guard
//! (section 10) belong between the facts and the `require` statements; see [`Pipeline::before`].
//!
//! Nothing here reads a clock, a loader or a handler: the caller hands in everything by value,
//! so the same input always gives the same [`Outcome`].

use std::collections::BTreeMap;
use std::ops::ControlFlow;

use serde_json::{Map, Value as Json, json};
use veld_ir::{DeclId, DeclKind, ElseError, Endpoint, EndpointFact, Expr, Ir, Service};

use crate::check::{Options, Violation, check_decl_full, check_type};
use crate::error::{Failure, InputError};
use crate::eval::{Env, SavedOld, eval};
use crate::helpers::num::EvalError;
use crate::machine;
use crate::old::save_old;
use crate::value::Value;

/// Everything the pipeline needs to know about one request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Request {
    /// The bound parameters by name, in wire form. An optional parameter that was not sent is
    /// left out.
    pub params: BTreeMap<String, Json>,
    /// The facts by name, in wire form. `None`, or a missing key, is a fact the loader did not
    /// find.
    pub facts: BTreeMap<String, Option<Json>>,
    /// The principal, in wire form.
    pub principal: Option<Json>,
    /// The clock: milliseconds since the epoch, one reading per request.
    pub now: i64,
    /// What the handler returned, in wire form. Without it [`Pipeline::run`] stops before the
    /// handler.
    pub result: Option<Json>,
}

/// The error the server answers with.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorAnswer {
    pub decl: DeclId,
    /// The declared error's name, for example `RoomFull` or `ValidationError`.
    pub name: String,
    pub status: u16,
    /// The error's fields, in wire form.
    pub data: Json,
    /// The `violations` member of the problem document (section 7).
    pub violations: Vec<Violation>,
    /// Violations of output rules. They go to the `onRuleViolation` hook; the caller sees a bare
    /// `500 Internal` (section 5.3, `strict`).
    pub hook: Vec<Violation>,
}

/// What one request ends in.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Every step that ran passed.
    Passes,
    /// The server answers with this error.
    Error(ErrorAnswer),
}

/// Why the pipeline could not give an outcome: the input is not what the IR describes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunError {
    #[error("service `{service}` has no endpoint `{endpoint}`")]
    UnknownEndpoint { service: String, endpoint: String },
    #[error("the IR has no declaration of the error `{0}`")]
    MissingError(String),
    #[error(transparent)]
    Input(#[from] InputError),
}

/// The result of steps 5 and 6.
#[derive(Debug, Clone)]
pub enum Before<'a> {
    /// A step failed: this is the answer.
    Rejected(ErrorAnswer),
    /// Everything passed; the handler may run.
    Ready(Prepared<'a>),
}

/// The state saved at the end of step 6: the parameters with their defaults applied, the
/// clock and every `old(...)` value.
#[derive(Debug, Clone)]
pub struct Prepared<'a> {
    pipeline: Pipeline<'a>,
    params: BTreeMap<String, Json>,
    now: i64,
    old: Vec<SavedOld>,
}

/// One endpoint, ready to process requests.
#[derive(Debug, Clone, Copy)]
pub struct Pipeline<'a> {
    ir: &'a Ir,
    endpoint: &'a Endpoint,
}

/// The verdict of one rule expression.
enum Verdict {
    Holds,
    Fails,
    Errors(EvalError),
}

impl<'a> Pipeline<'a> {
    /// The pipeline of `endpoint` of `service`.
    ///
    /// # Errors
    /// [`RunError::UnknownEndpoint`] when the service has no such endpoint.
    pub fn new(ir: &'a Ir, service: &'a Service, endpoint: &str) -> Result<Self, RunError> {
        let found = service.endpoints.iter().find(|e| e.name == endpoint);
        let endpoint = found.ok_or_else(|| RunError::UnknownEndpoint {
            service: service.name.clone(),
            endpoint: endpoint.to_owned(),
        })?;
        Ok(Pipeline { ir, endpoint })
    }

    /// Runs the whole pipeline: steps 5 and 6, and, when `request.result` is given, steps 8
    /// and 9.
    ///
    /// # Errors
    /// [`RunError`] when the parameters, facts or result do not have the shape the IR says.
    pub fn run(&self, request: &Request) -> Result<Outcome, RunError> {
        match self.before(request)? {
            Before::Rejected(answer) => Ok(Outcome::Error(answer)),
            Before::Ready(prepared) => match &request.result {
                Some(result) => prepared.after(result),
                None => Ok(Outcome::Passes),
            },
        }
    }

    /// Steps 5 and 6: invariants on the parameters, the facts, the `require` statements, and
    /// the saved `old(...)` values.
    ///
    /// # Errors
    /// [`RunError`] when the input is malformed.
    pub fn before(&self, request: &Request) -> Result<Before<'a>, RunError> {
        let params = match self.bind(request)? {
            ControlFlow::Continue(params) => params,
            ControlFlow::Break(answer) => return Ok(Before::Rejected(answer)),
        };
        let mut env = Env::new(self.ir);
        env.params = params;
        env.principal.clone_from(&request.principal);
        env.now = Some(request.now);
        if let Some(answer) = self.load_facts(&mut env, request)? {
            return Ok(Before::Rejected(answer));
        }
        // Policies (section 11, `403`) go here, between the facts and the guard.
        if let Some(answer) = self.guard(&env)? {
            return Ok(Before::Rejected(answer));
        }
        if let Some(answer) = self.requires(&env)? {
            return Ok(Before::Rejected(answer));
        }
        let ensures: Vec<&Expr> = self
            .endpoint
            .rules
            .ensures
            .iter()
            .map(|e| &e.expr)
            .collect();
        let old = save_old(&env, &ensures)?;
        Ok(Before::Ready(Prepared {
            pipeline: *self,
            params: env.params,
            now: request.now,
            old,
        }))
    }

    // ---- step 5 ----------------------------------------------------------------------

    fn bind(
        &self,
        request: &Request,
    ) -> Result<ControlFlow<ErrorAnswer, BTreeMap<String, Json>>, RunError> {
        let opts = Options {
            request: true,
            partial: false,
        };
        let mut params = BTreeMap::new();
        let mut found = Vec::new();
        for p in self.endpoint.params.iter().filter(|p| !p.stream) {
            let Some(value) = request.params.get(&p.name) else {
                if p.optional {
                    continue;
                }
                let missing = InputError::MissingField {
                    path: String::new(),
                    field: p.name.clone(),
                };
                return Err(missing.into());
            };
            let (full, violations) =
                check_type(self.ir, &p.ty, value, opts).map_err(|e| e.at_path(&p.name))?;
            found.extend(violations.into_iter().map(|v| reroot(v, &p.name)));
            params.insert(p.name.clone(), full);
        }
        if found.is_empty() {
            Ok(ControlFlow::Continue(params))
        } else {
            Ok(ControlFlow::Break(self.validation_error(found)?))
        }
    }

    /// `422 ValidationError` with one issue per violation (section 7).
    fn validation_error(&self, violations: Vec<Violation>) -> Result<ErrorAnswer, RunError> {
        let issues: Vec<Json> = violations
            .iter()
            .map(|v| {
                let code = if v.error.is_some() {
                    "rule_error"
                } else {
                    "rule"
                };
                json!({ "path": v.path, "code": code, "message": v.message })
            })
            .collect();
        let data = json!({ "issues": issues });
        self.answer("ValidationError", 422, data, violations, Vec::new())
    }

    // ---- step 6 ----------------------------------------------------------------------

    fn load_facts(
        &self,
        env: &mut Env<'a>,
        request: &Request,
    ) -> Result<Option<ErrorAnswer>, RunError> {
        for fact in &self.endpoint.rules.facts {
            let value = if fact.optional && key_is_absent(env, fact)? {
                None
            } else {
                request.facts.get(&fact.name).and_then(Option::as_ref)
            };
            match value {
                Some(v) => {
                    env.facts.insert(fact.name.clone(), v.clone());
                }
                None if fact.optional => {}
                None => {
                    let data = json!({ "resource": fact.name });
                    return self
                        .answer("NotFound", 404, data, Vec::new(), Vec::new())
                        .map(Some);
                }
            }
        }
        Ok(None)
    }

    /// The transition guard (section 10.2): `409 InvalidTransition` when the current state of the
    /// fact is not one of the sources.
    fn guard(&self, env: &Env<'_>) -> Result<Option<ErrorAnswer>, RunError> {
        let Some(t) = &self.endpoint.rules.transition else {
            return Ok(None);
        };
        let Some(fact) = env.facts.get(&t.fact) else {
            return Err(InputError::NotAvailable("the fact of a transition").into());
        };
        match machine::guard(self.ir, t, fact)? {
            None => Ok(None),
            Some(refusal) => {
                let violations = vec![refusal.violation];
                let answer = self.answer(
                    "InvalidTransition",
                    409,
                    refusal.data,
                    violations,
                    Vec::new(),
                );
                answer.map(Some)
            }
        }
    }

    fn requires(&self, env: &Env<'_>) -> Result<Option<ErrorAnswer>, RunError> {
        for r in &self.endpoint.rules.requires {
            let verdict = verdict(env, &r.expr)?;
            if matches!(verdict, Verdict::Holds) {
                continue;
            }
            let rule = RuleInfo {
                name: r.name.as_deref(),
                kind: "require",
                id: &r.id,
                reads: &r.reads,
                message: r.message.as_deref(),
                unnamed_default: "precondition failed",
            };
            let violation = rule.violation(&verdict);
            let answer = match &r.else_error {
                None => self.validation_error(vec![violation])?,
                Some(e) => self.else_answer(env, e, violation)?,
            };
            return Ok(Some(answer));
        }
        Ok(None)
    }

    /// The answer of `require .. else E { .. }`: the payload is evaluated now, in the same
    /// scope as the condition.
    fn else_answer(
        &self,
        env: &Env<'_>,
        e: &ElseError,
        violation: Violation,
    ) -> Result<ErrorAnswer, RunError> {
        let decl = self
            .ir
            .decls
            .get(e.decl.index())
            .ok_or_else(|| RunError::MissingError(format!("#{}", e.decl.index())))?;
        let mut data = Map::new();
        for f in &e.fields {
            match eval(env, &f.value) {
                Ok(v) => {
                    data.insert(f.wire.clone(), v.to_json()?);
                }
                Err(Failure::Eval(err)) => {
                    // The payload itself failed: the server cannot build the error it promised.
                    let mut v = violation;
                    v.error = Some(err);
                    v.message = format!("rule {} could not be evaluated", v.rule_name);
                    return self.answer("Internal", 500, json!({}), Vec::new(), vec![v]);
                }
                Err(Failure::Input(err)) => return Err(err.into()),
            }
        }
        let (data, invariants) =
            check_decl_full(self.ir, decl, &Json::Object(data), Options::default())?;
        if !invariants.is_empty() {
            let hook = invariants.into_iter().map(|v| reroot(v, "error")).collect();
            return self.answer("Internal", 500, json!({}), Vec::new(), hook);
        }
        Ok(ErrorAnswer {
            decl: e.decl,
            name: decl.name.clone(),
            status: e.status,
            data,
            violations: vec![violation],
            hook: Vec::new(),
        })
    }

    // ---- answers ---------------------------------------------------------------------

    /// The error `name` as the endpoint declares it (its own status), or the prelude error.
    fn answer(
        &self,
        name: &str,
        fallback: u16,
        data: Json,
        violations: Vec<Violation>,
        hook: Vec<Violation>,
    ) -> Result<ErrorAnswer, RunError> {
        let (decl, status) = self.error_decl(name, fallback)?;
        Ok(ErrorAnswer {
            decl,
            name: name.to_owned(),
            status,
            data,
            violations,
            hook,
        })
    }

    fn error_decl(&self, name: &str, fallback: u16) -> Result<(DeclId, u16), RunError> {
        let declared = self.endpoint.errors.iter().find(|e| {
            self.ir
                .decls
                .get(e.decl.index())
                .is_some_and(|d| d.name == name)
        });
        if let Some(e) = declared {
            return Ok((e.decl, e.status));
        }
        let prelude = self.ir.decls.iter().find(|d| {
            d.name == name
                && self
                    .ir
                    .packages
                    .get(d.package.index())
                    .is_some_and(|p| p.name == "veld")
        });
        match prelude {
            Some(d) => {
                let status = match &d.kind {
                    DeclKind::Error(e) => e.default_status.unwrap_or(fallback),
                    DeclKind::Model(_)
                    | DeclKind::Enum(_)
                    | DeclKind::Union(_)
                    | DeclKind::Const(_) => fallback,
                };
                Ok((d.id, status))
            }
            None => Err(RunError::MissingError(name.to_owned())),
        }
    }
}

impl Prepared<'_> {
    /// Steps 8 and 9 in `strict` mode: the invariants of the handler's result, then every
    /// `ensure`. A violation is `500 Internal`; the violations are in `hook`, not on the wire.
    /// An endpoint that has already failed with a declared error is not checked.
    ///
    /// # Errors
    /// [`RunError`] when the result does not have the shape of the success type.
    pub fn after(&self, result: &Json) -> Result<Outcome, RunError> {
        let p = &self.pipeline;
        let mut env = Env::new(p.ir);
        env.params.clone_from(&self.params);
        env.now = Some(self.now);
        env.old.clone_from(&self.old);
        // Facts are deliberately not in scope here: outside `old(...)` they are an error.
        if let Some(ty) = p
            .endpoint
            .success
            .ty
            .as_ref()
            .filter(|_| !p.endpoint.success.stream)
        {
            let (full, violations) = check_type(p.ir, ty, result, Options::default())
                .map_err(|e| e.at_path("result"))?;
            if !violations.is_empty() {
                let hook = violations
                    .into_iter()
                    .map(|v| reroot(v, "result"))
                    .collect();
                return internal(p, hook);
            }
            env.result = Some(full);
        }
        let mut hook = Vec::new();
        for e in &p.endpoint.rules.ensures {
            let verdict = verdict(&env, &e.expr)?;
            if matches!(verdict, Verdict::Holds) {
                continue;
            }
            let rule = RuleInfo {
                name: e.name.as_deref(),
                kind: "ensure",
                id: &e.id,
                reads: &e.reads,
                message: e.message.as_deref(),
                unnamed_default: "postcondition failed",
            };
            hook.push(rule.violation(&verdict));
        }
        hook.extend(machine::target(p.ir, p.endpoint, result)?);
        if hook.is_empty() {
            Ok(Outcome::Passes)
        } else {
            internal(p, hook)
        }
    }
}

fn internal(p: &Pipeline<'_>, hook: Vec<Violation>) -> Result<Outcome, RunError> {
    p.answer("Internal", 500, json!({}), Vec::new(), hook)
        .map(Outcome::Error)
}

/// `$.a[1]` becomes `body.a[1]` for the root `body`.
fn reroot(mut v: Violation, root: &str) -> Violation {
    let rest = v.path.strip_prefix('$').unwrap_or(&v.path);
    v.path = format!("{root}{rest}");
    v
}

/// True when the key of an optional fact reads a value that is absent: the loader is not
/// called and the fact is absent (section 9.2).
fn key_is_absent(env: &Env<'_>, fact: &EndpointFact) -> Result<bool, RunError> {
    let Some(key) = &fact.key else {
        return Ok(false);
    };
    match eval(env, key) {
        Ok(_) => Ok(false),
        Err(Failure::Input(InputError::MissingField { .. } | InputError::NotAvailable(_))) => {
            Ok(true)
        }
        Err(Failure::Input(e)) => Err(e.into()),
        Err(Failure::Eval(e)) => {
            let what = format!("the key of fact `{}` failed: {}", fact.name, e.as_str());
            Err(InputError::Ir(what).into())
        }
    }
}

fn verdict(env: &Env<'_>, expr: &Expr) -> Result<Verdict, RunError> {
    match eval(env, expr) {
        Ok(Value::Bool(true)) => Ok(Verdict::Holds),
        Ok(Value::Bool(false)) => Ok(Verdict::Fails),
        Ok(other) => Err(InputError::Ir(format!("a rule is {}, not bool", other.kind())).into()),
        Err(Failure::Eval(e)) => Ok(Verdict::Errors(e)),
        Err(Failure::Input(e)) => Err(e.into()),
    }
}

/// What a violation of an endpoint rule needs from the statement.
struct RuleInfo<'r> {
    name: Option<&'r str>,
    kind: &'static str,
    id: &'r str,
    reads: &'r [String],
    message: Option<&'r str>,
    unnamed_default: &'static str,
}

impl RuleInfo<'_> {
    /// The violation of section 7 for an endpoint rule: `path` is `request` and `fields` are
    /// the parameters the expression reads.
    fn violation(&self, verdict: &Verdict) -> Violation {
        let name = self.name.unwrap_or(self.kind);
        let (message, error) = match verdict {
            Verdict::Errors(e) => (format!("rule {name} could not be evaluated"), Some(*e)),
            Verdict::Holds | Verdict::Fails => {
                let text = match (self.message, self.name) {
                    (Some(m), _) => m.to_owned(),
                    (None, Some(n)) => format!("violates rule {n}"),
                    (None, None) => self.unnamed_default.to_owned(),
                };
                (text, None)
            }
        };
        Violation {
            rule_name: name.to_owned(),
            rule_id: self.id.to_owned(),
            path: "request".to_owned(),
            fields: self.reads.to_vec(),
            message,
            error,
        }
    }
}
