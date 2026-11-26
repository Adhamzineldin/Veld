//! The server pipeline of one request, as a pure function (`spec/rules.md` sections 5.3, 9.5).
//!
//! Step 5 (invariants on the bound parameters), step 6 (facts, `require`, the saved `old(...)`
//! values) and, once the handler has answered, steps 8 and 9 (invariants on the result, then
//! every `ensure`, then the target state of a `transition`) in `strict` mode. The transition guard
//! (section 10, [`crate::machine`]) runs between the policies and the `require` statements; the
//! policies (section 11, `allow`) run right after the facts, see [`Pipeline::before`].
//!
//! Nothing here reads a clock, a loader or a handler: the caller hands in everything by value,
//! so the same input always gives the same [`Outcome`].

use std::collections::BTreeMap;
use std::ops::ControlFlow;

use serde_json::{Map, Value as Json, json};
use veld_ir::{DeclId, DeclKind, ElseError, Endpoint, EndpointFact, Expr, Ir, Policy, Service};

use crate::check::{Options, Violation, check_decl_audited, check_type_audited};
use crate::error::{Failure, InputError};
use crate::eval::{Env, SavedOld, eval};
use crate::helpers::num::EvalError;
use crate::idempotency::{
    IdempotencyKey, OTHER_MESSAGE, RUNNING_MESSAGE, StoreEffect, StoreState, StoredAnswer,
    is_valid_key,
};
use crate::machine;
use crate::old::save_old;
use crate::policy::{NOT_PERMITTED, eval_policy};
use crate::value::Value;
use crate::version::{self, IfMatch, parse_if_match};

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
    /// The `If-Match` header as sent (`*` or `"3"`), when the request has one. An endpoint with no
    /// version guard ignores it (`spec/rules.md` section 11.9).
    pub if_match: Option<String>,
    /// The idempotency header and what the store holds for it. An endpoint that is not
    /// `@idempotent` ignores it (`spec/rules.md` section 11.8).
    pub key: Option<IdempotencyKey>,
    /// The text the application gives for the caller; part of the scope of a key.
    pub caller: Option<String>,
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

/// What the server does with the checks on its own output (steps 8 and 9, `spec/rules.md`
/// section 5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputMode {
    /// A violated output rule becomes `500 Internal`.
    #[default]
    Strict,
    /// The response is sent unchanged; the violation goes to the hook.
    Audit,
    /// Output rules are not evaluated, audited ones included.
    Off,
}

/// Everything one request came to: the outcome, and what went to the hook besides it.
#[derive(Debug, Clone, PartialEq)]
pub struct Reported {
    pub outcome: Outcome,
    /// Violations of audited rules (`@rollout(audit)`, section 17.1), in the order of
    /// evaluation. They never reject and are never in a response.
    pub audited: Vec<Violation>,
    /// Violations of enforced output rules that did not reject because the mode is `audit`.
    pub hook: Vec<Violation>,
    /// True when the answer is a stored one that was sent again (`Veld-Idempotent-Replay: true`):
    /// no fact was loaded and the handler was not called.
    pub replayed: bool,
    /// What the store holds for the key afterwards (`spec/rules.md` section 11.9).
    pub store: StoreEffect,
    /// The version that was checked and that the handler receives: the one `If-Match` sent. `None`
    /// when the header was absent or `*`, and when no check was reached.
    pub checked_version: Option<i64>,
    /// The body of a success: the handler's result as checked (defaults applied), or the body of a
    /// replayed success. `None` for an error, an endpoint without a body and a run that stopped
    /// before the handler.
    pub body: Option<Json>,
    /// The `ETag` of a success whose type is a model with a `@version` field.
    pub etag: Option<String>,
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
    /// The key had a stored answer: it is sent again, and nothing else runs (`spec/rules.md`
    /// section 11.9).
    Replayed(StoredAnswer),
    /// Everything passed; the handler may run.
    Ready(Prepared<'a>),
}

/// The state saved at the end of step 6: the parameters with their defaults applied, the
/// principal, the clock and every `old(...)` value.
#[derive(Debug, Clone)]
pub struct Prepared<'a> {
    pipeline: Pipeline<'a>,
    params: BTreeMap<String, Json>,
    now: i64,
    old: Vec<SavedOld>,
    principal: Option<Json>,
    checked_version: Option<i64>,
}

/// One endpoint, ready to process requests.
#[derive(Debug, Clone, Copy)]
pub struct Pipeline<'a> {
    ir: &'a Ir,
    endpoint: &'a Endpoint,
}

/// Steps 8 and 9: the outcome, what went to the hook, and the body of a success.
#[derive(Debug, Clone, PartialEq)]
pub struct After {
    pub outcome: Outcome,
    pub hook: Vec<Violation>,
    /// The result as checked; `None` when the handler's result was refused.
    pub body: Option<Json>,
}

impl After {
    fn new(outcome: Outcome, hook: Vec<Violation>, body: Option<Json>) -> Self {
        After {
            outcome,
            hook,
            body,
        }
    }
}

/// What the store says about the key of a request (`spec/rules.md` section 11.9).
enum Lookup {
    /// The endpoint is not `@idempotent`, or the request has no key.
    NoKey,
    /// The store knows nothing: the request runs and its answer is stored.
    NewKey,
    Replay(StoredAnswer),
    Conflict(ErrorAnswer),
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
        self.run_reported(request, OutputMode::Strict)
            .map(|r| r.outcome)
    }

    /// Like [`Pipeline::run`] with the output `mode`, and with what the hook is told on top of
    /// the outcome: the audited violations (`@rollout(audit)`) of every step that ran.
    ///
    /// # Errors
    /// [`RunError`] when the parameters, facts or result do not have the shape the IR says.
    pub fn run_reported(&self, request: &Request, mode: OutputMode) -> Result<Reported, RunError> {
        let mut audited = Vec::new();
        let (before, fresh) = self.before_full(request, &mut audited)?;
        let mut reported = Reported {
            outcome: Outcome::Passes,
            audited: Vec::new(),
            hook: Vec::new(),
            replayed: false,
            store: StoreEffect::Untouched,
            checked_version: None,
            body: None,
            etag: None,
        };
        match before {
            Before::Rejected(answer) => reported.outcome = Outcome::Error(answer),
            Before::Replayed(stored) => {
                reported.outcome = self.replayed(&stored)?;
                reported.replayed = true;
                reported.body = stored.body;
            }
            Before::Ready(prepared) => {
                reported.checked_version = prepared.checked_version;
                match &request.result {
                    Some(result) => {
                        let after = prepared.after_full(result, mode, &mut audited)?;
                        reported.outcome = after.outcome;
                        reported.hook = after.hook;
                        reported.body = after.body;
                    }
                    None => reported.store = StoreEffect::Running,
                }
            }
        }
        reported.etag = reported
            .body
            .as_ref()
            .filter(|_| matches!(reported.outcome, Outcome::Passes))
            .and_then(|b| version::etag(self.ir, self.endpoint, b));
        if fresh {
            reported.store =
                self.store_effect(&reported.outcome, reported.store, reported.body.as_ref());
        } else {
            reported.store = StoreEffect::Untouched;
        }
        reported.audited = audited;
        Ok(reported)
    }

    /// What the store holds for a new key once the request has its outcome: the answer when it is
    /// below `500`, nothing when it is not, and "running" while the handler has not answered.
    fn store_effect(
        &self,
        outcome: &Outcome,
        ran: StoreEffect,
        body: Option<&Json>,
    ) -> StoreEffect {
        match outcome {
            Outcome::Error(a) if a.status >= 500 => StoreEffect::Removed,
            Outcome::Error(a) => StoreEffect::Stored(StoredAnswer {
                status: a.status,
                error: Some(a.name.clone()),
                body: Some(a.data.clone()),
            }),
            Outcome::Passes if ran == StoreEffect::Running => StoreEffect::Running,
            Outcome::Passes => StoreEffect::Stored(StoredAnswer {
                status: self.endpoint.success.status,
                error: None,
                body: body.cloned(),
            }),
        }
    }

    /// The outcome of a stored answer that is sent again.
    fn replayed(&self, stored: &StoredAnswer) -> Result<Outcome, RunError> {
        let Some(name) = &stored.error else {
            return Ok(Outcome::Passes);
        };
        let data = stored.body.clone().unwrap_or_else(|| json!({}));
        let mut answer = self.answer(name, stored.status, data, Vec::new(), Vec::new())?;
        answer.status = stored.status;
        Ok(Outcome::Error(answer))
    }

    /// Steps 5 and 6: invariants on the parameters, the facts, the `require` statements, and
    /// the saved `old(...)` values.
    ///
    /// # Errors
    /// [`RunError`] when the input is malformed.
    pub fn before(&self, request: &Request) -> Result<Before<'a>, RunError> {
        self.before_audited(request, &mut Vec::new())
    }

    /// Like [`Pipeline::before`]; the violations of audited rules are appended to `audited`.
    ///
    /// # Errors
    /// [`RunError`] when the input is malformed.
    pub fn before_audited(
        &self,
        request: &Request,
        audited: &mut Vec<Violation>,
    ) -> Result<Before<'a>, RunError> {
        self.before_full(request, audited).map(|(before, _)| before)
    }

    /// Like [`Pipeline::before_audited`], and whether the request had a new key when it ended: the
    /// store then holds its answer (`spec/rules.md` section 11.9).
    fn before_full(
        &self,
        request: &Request,
        audited: &mut Vec<Violation>,
    ) -> Result<(Before<'a>, bool), RunError> {
        let params = match self.bind(request, audited)? {
            ControlFlow::Continue(params) => params,
            ControlFlow::Break(answer) => return Ok((Before::Rejected(answer), false)),
        };
        let fresh = match self.lookup(request)? {
            Lookup::NewKey => true,
            Lookup::NoKey => false,
            Lookup::Replay(stored) => return Ok((Before::Replayed(stored), false)),
            Lookup::Conflict(answer) => return Ok((Before::Rejected(answer), false)),
        };
        let before = self.prepare(request, params, audited)?;
        Ok((before, fresh))
    }

    /// Step 6, from the facts on.
    fn prepare(
        &self,
        request: &Request,
        params: BTreeMap<String, Json>,
        audited: &mut Vec<Violation>,
    ) -> Result<Before<'a>, RunError> {
        let mut env = Env::new(self.ir);
        env.params = params;
        env.principal.clone_from(&request.principal);
        env.now = Some(request.now);
        if let Some(answer) = self.load_facts(&mut env, request)? {
            return Ok(Before::Rejected(answer));
        }
        if let Some(answer) = self.policies(&env, request.now)? {
            return Ok(Before::Rejected(answer));
        }
        let sent = self.if_match(request);
        if let Some(answer) = self.version_guard(&env, sent)? {
            return Ok(Before::Rejected(answer));
        }
        if let Some(answer) = self.guard(&env)? {
            return Ok(Before::Rejected(answer));
        }
        if let Some(answer) = self.requires(&env, audited)? {
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
            principal: request.principal.clone(),
            old,
            checked_version: match sent {
                Some(IfMatch::Version(n)) if self.endpoint.rules.version.is_some() => Some(n),
                Some(IfMatch::Version(_) | IfMatch::Any) | None => None,
            },
        }))
    }

    // ---- step 5 ----------------------------------------------------------------------

    fn bind(
        &self,
        request: &Request,
        audited: &mut Vec<Violation>,
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
            let (full, checked) =
                check_type_audited(self.ir, &p.ty, value, opts).map_err(|e| e.at_path(&p.name))?;
            found.extend(checked.violations.into_iter().map(|v| reroot(v, &p.name)));
            audited.extend(checked.audited.into_iter().map(|v| reroot(v, &p.name)));
            params.insert(p.name.clone(), full);
        }
        let headers = self.header_issues(request);
        if found.is_empty() && headers.is_empty() {
            Ok(ControlFlow::Continue(params))
        } else {
            Ok(ControlFlow::Break(self.validation_answer(headers, found)?))
        }
    }

    /// The issues of the two headers this crate reads (`spec/rules.md` section 11.9): a malformed
    /// `If-Match` on an endpoint with a version guard, and a malformed or missing key on an
    /// `@idempotent` endpoint. They are not rule violations and carry no `violations`.
    fn header_issues(&self, request: &Request) -> Vec<Json> {
        let mut issues = Vec::new();
        let at = |name: &str| format!("header.{name}");
        let sent = request.if_match.as_deref();
        if self.endpoint.rules.version.is_some()
            && sent.is_some_and(|t| parse_if_match(t).is_none())
        {
            let message = version::IF_MATCH_MESSAGE;
            issues.push(json!({ "path": at("If-Match"), "code": "type", "message": message }));
        }
        if let Some(idem) = &self.endpoint.idempotency {
            match &request.key {
                Some(k) if !is_valid_key(&k.value) => {
                    let message = crate::idempotency::KEY_MESSAGE;
                    issues.push(
                        json!({ "path": at(&idem.header), "code": "type", "message": message }),
                    );
                }
                None if idem.required => {
                    issues.push(json!({ "path": at(&idem.header), "code": "required", "message": "is required" }));
                }
                Some(_) | None => {}
            }
        }
        issues
    }

    /// What the store says about the key of the request, once step 5 has passed.
    fn lookup(&self, request: &Request) -> Result<Lookup, RunError> {
        let (Some(_), Some(key)) = (&self.endpoint.idempotency, &request.key) else {
            return Ok(Lookup::NoKey);
        };
        let message = match &key.store {
            StoreState::Empty => return Ok(Lookup::NewKey),
            StoreState::Stored(stored) => return Ok(Lookup::Replay(stored.clone())),
            StoreState::Other => OTHER_MESSAGE,
            StoreState::Running => RUNNING_MESSAGE,
        };
        let data = json!({ "message": message });
        let answer = self.answer("Conflict", 409, data, Vec::new(), Vec::new())?;
        Ok(Lookup::Conflict(answer))
    }

    /// What `If-Match` says, when the endpoint has a guard and the request has the header. A
    /// malformed header was refused at step 5.
    fn if_match(&self, request: &Request) -> Option<IfMatch> {
        self.endpoint.rules.version.as_ref()?;
        request.if_match.as_deref().and_then(parse_if_match)
    }

    /// The version guard (`spec/rules.md` sections 11.7 and 11.9): after the policies and before the
    /// transition, `412 PreconditionFailed` when the version the caller sent is not the one the
    /// fact has. `*`, no header and a fact that is absent pass.
    fn version_guard(
        &self,
        env: &Env<'_>,
        sent: Option<IfMatch>,
    ) -> Result<Option<ErrorAnswer>, RunError> {
        let (Some(rule), Some(IfMatch::Version(expected))) = (&self.endpoint.rules.version, sent)
        else {
            return Ok(None);
        };
        let Some(fact) = env.facts.get(&rule.fact) else {
            return Ok(None);
        };
        let missing = || {
            InputError::Ir(format!(
                "fact `{}` has no version `{}`",
                rule.fact, rule.wire
            ))
        };
        let actual = version::version_of(fact, &rule.wire).ok_or_else(missing)?;
        if actual == expected {
            return Ok(None);
        }
        let violation = Violation {
            rule_name: "version".to_owned(),
            rule_id: rule.id.clone(),
            path: "request".to_owned(),
            fields: Vec::new(),
            message: "violates rule version".to_owned(),
            error: None,
        };
        let data = json!({ "expected": expected.to_string(), "actual": actual.to_string() });
        self.answer("PreconditionFailed", 412, data, vec![violation], Vec::new())
            .map(Some)
    }

    /// `422 ValidationError` with one issue per violation (section 7).
    fn validation_error(&self, violations: Vec<Violation>) -> Result<ErrorAnswer, RunError> {
        self.validation_answer(Vec::new(), violations)
    }

    /// Like [`Self::validation_error`], with the issues of malformed headers first.
    fn validation_answer(
        &self,
        mut issues: Vec<Json>,
        violations: Vec<Violation>,
    ) -> Result<ErrorAnswer, RunError> {
        issues.extend(violations.iter().map(|v| {
            let code = if v.error.is_some() {
                "rule_error"
            } else {
                "rule"
            };
            json!({ "path": v.path, "code": code, "message": v.message })
        }));
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

    /// Every `allow` in source order (section 11.3): the first denial is `403 Forbidden`.
    fn policies(&self, env: &Env<'_>, now: i64) -> Result<Option<ErrorAnswer>, RunError> {
        for a in &self.endpoint.rules.allows {
            let policy =
                self.ir.policies.get(a.policy.index()).ok_or_else(|| {
                    InputError::Ir("an `allow` names a missing policy".to_owned())
                })?;
            let args = match allow_args(env, &a.args)? {
                Some(args) => args,
                // An argument that cannot be evaluated fails closed.
                None => {
                    return self.forbidden(policy, &a.reads, NOT_PERMITTED).map(Some);
                }
            };
            let verdict = eval_policy(self.ir, policy, &args, now)?;
            if !verdict.permitted {
                let text = verdict.denial_message();
                return self.forbidden(policy, &a.reads, text).map(Some);
            }
        }
        Ok(None)
    }

    /// `403 Forbidden`: its `message` and the message of its one violation are `text`.
    fn forbidden(
        &self,
        policy: &Policy,
        reads: &[String],
        text: &str,
    ) -> Result<ErrorAnswer, RunError> {
        let violation = Violation {
            rule_name: policy.name.clone(),
            rule_id: policy.id.clone(),
            path: "request".to_owned(),
            fields: reads.to_vec(),
            message: text.to_owned(),
            error: None,
        };
        let data = json!({ "message": text });
        self.answer("Forbidden", 403, data, vec![violation], Vec::new())
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
        match machine::guard(self.ir, self.endpoint, t, fact)? {
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

    fn requires(
        &self,
        env: &Env<'_>,
        audited: &mut Vec<Violation>,
    ) -> Result<Option<ErrorAnswer>, RunError> {
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
            if r.rollout.is_some() {
                // An audited `require` never answers, with `else E` or without: it is reported.
                audited.push(violation);
                continue;
            }
            let answer = match &r.else_error {
                None => self.validation_error(vec![violation])?,
                Some(e) => self.else_answer(env, e, violation, audited)?,
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
        audited: &mut Vec<Violation>,
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
        let (data, checked) =
            check_decl_audited(self.ir, decl, &Json::Object(data), Options::default())?;
        audited.extend(checked.audited.into_iter().map(|v| reroot(v, "error")));
        if !checked.violations.is_empty() {
            let hook = (checked.violations.into_iter())
                .map(|v| reroot(v, "error"))
                .collect();
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
                    | DeclKind::Const(_)
                    | DeclKind::Event(_) => fallback,
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
        self.after_into(result, OutputMode::Strict, &mut Vec::new())
            .map(|(outcome, _)| outcome)
    }

    /// Steps 8 and 9 in the output `mode`. Returns the outcome and the violations of enforced
    /// rules that were reported without rejecting (mode `audit`); the violations of audited
    /// rules (`@rollout(audit)`) are appended to `audited` in every mode but `off`.
    ///
    /// # Errors
    /// [`RunError`] when the result does not have the shape of the success type.
    pub fn after_into(
        &self,
        result: &Json,
        mode: OutputMode,
        audited: &mut Vec<Violation>,
    ) -> Result<(Outcome, Vec<Violation>), RunError> {
        let after = self.after_full(result, mode, audited)?;
        Ok((after.outcome, after.hook))
    }

    /// Like [`Prepared::after_into`], and the body of the success: the result as checked, with the
    /// defaults of its fields applied.
    ///
    /// # Errors
    /// [`RunError`] when the result does not have the shape of the success type.
    pub fn after_full(
        &self,
        result: &Json,
        mode: OutputMode,
        audited: &mut Vec<Violation>,
    ) -> Result<After, RunError> {
        let p = &self.pipeline;
        if mode == OutputMode::Off {
            return Ok(After::new(
                Outcome::Passes,
                Vec::new(),
                Some(result.clone()),
            ));
        }
        let mut env = Env::new(p.ir);
        env.params.clone_from(&self.params);
        env.now = Some(self.now);
        env.principal.clone_from(&self.principal);
        env.old.clone_from(&self.old);
        // Facts are deliberately not in scope here: outside `old(...)` they are an error.
        let mut hook = Vec::new();
        if let Some(ty) = p
            .endpoint
            .success
            .ty
            .as_ref()
            .filter(|_| !p.endpoint.success.stream)
        {
            let (full, checked) = check_type_audited(p.ir, ty, result, Options::default())
                .map_err(|e| e.at_path("result"))?;
            audited.extend(checked.audited.into_iter().map(|v| reroot(v, "result")));
            hook.extend(checked.violations.into_iter().map(|v| reroot(v, "result")));
            if !hook.is_empty() && mode == OutputMode::Strict {
                return internal(p, hook).map(|o| After::new(o, Vec::new(), None));
            }
            env.result = Some(full);
        }
        hook.extend(self.ensures(&env, result, audited)?);
        let body = Some(env.result.clone().unwrap_or_else(|| result.clone()));
        if hook.is_empty() || mode == OutputMode::Audit {
            return Ok(After::new(Outcome::Passes, hook, body));
        }
        internal(p, hook).map(|o| After::new(o, Vec::new(), None))
    }

    /// Step 9: every `ensure` in source order, then the target state. The violations of enforced
    /// ones are returned; those of audited ones go to `audited`.
    fn ensures(
        &self,
        env: &Env<'_>,
        result: &Json,
        audited: &mut Vec<Violation>,
    ) -> Result<Vec<Violation>, RunError> {
        let p = &self.pipeline;
        let mut hook = Vec::new();
        for e in &p.endpoint.rules.ensures {
            let verdict = verdict(env, &e.expr)?;
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
            let sink = if e.rollout.is_some() {
                &mut *audited
            } else {
                &mut hook
            };
            sink.push(rule.violation(&verdict));
        }
        // The checked result, with the defaults of its fields applied, like the `ensure` statements see it.
        let checked = env.result.as_ref().unwrap_or(result);
        hook.extend(machine::target(p.ir, p.endpoint, checked)?);
        Ok(hook)
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

/// The arguments of an `allow` in wire form; `None` when one cannot be evaluated (section 3.7).
fn allow_args(env: &Env<'_>, args: &[Expr]) -> Result<Option<Vec<Json>>, RunError> {
    let mut out = Vec::with_capacity(args.len());
    for arg in args {
        match eval(env, arg) {
            Ok(v) => out.push(v.to_json()?),
            Err(Failure::Eval(_)) => return Ok(None),
            Err(Failure::Input(e)) => return Err(e.into()),
        }
    }
    Ok(Some(out))
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
