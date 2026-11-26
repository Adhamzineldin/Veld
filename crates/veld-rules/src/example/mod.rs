//! Examples (`spec/rules.md` section 12): filling in what they leave out, running them against
//! the reference pipeline, and exposing the request they stand for.
//!
//! * [`fill_example`] gives the complete request of an example as data. The live mode of
//!   `veld test` sends exactly these values.
//! * [`run_example`] runs it offline (steps 5 and 6 of the pipeline, and steps 8 and 9 when the
//!   example has `returns`) and compares the outcome with the expected one.

mod complete;
mod event;
mod fill;
mod matching;
mod report;

use std::collections::BTreeMap;

use serde_json::Value as Json;
use veld_ir::{
    AuthKind, DeclId, DeclKind, Endpoint, Example, ExampleIfMatch, ExampleOutcome, ExampleValue,
    Ir, KeyStore, Service, ServiceId, Type,
};

pub use self::complete::{Completed, Completer};
pub use self::event::{
    Delivery, FilledEvent, RejectReason, deliver, describe_delivery, describe_expectation,
    fill_event_example, run_event_example,
};
use self::fill::Filler;
pub use self::matching::{Difference, diff_pattern};
pub use self::report::{ExampleReport, Mismatch, Verdict, describe_actual, describe_expected};
use crate::check::{Options, check_type};
use crate::error::InputError;
use crate::helpers::instant::parse_datetime;
use crate::idempotency::{IdempotencyKey, StoreState, StoredAnswer};
use crate::pipeline::{OutputMode, Pipeline, Request, RunError};
use crate::version::IfMatch;

/// The clock of an example without `at`.
pub const DEFAULT_CLOCK: &str = "2026-01-01T00:00:00Z";

/// Why an example cannot be run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExampleError {
    /// `E0950`: the example names an endpoint the IR does not have.
    #[error("example \"{example}\": no endpoint `{endpoint}`")]
    UnknownEndpoint { example: String, endpoint: String },
    /// `E0951`: `given` names something that is not a fact of the endpoint.
    #[error("example \"{example}\": `{fact}` is not a fact of the endpoint")]
    UnknownFact { example: String, fact: String },
    /// `E0952`: an argument the endpoint does not have.
    #[error("example \"{example}\": `{param}` is not a parameter of the endpoint")]
    UnknownParam { example: String, param: String },
    /// `E0952`: the clock is not an RFC 3339 datetime.
    #[error("example \"{example}\": `{text}` is not a valid clock")]
    BadClock { example: String, text: String },
    /// `E0952`: a reference that does not lead to a value of a given.
    #[error("example \"{example}\": `{given}.{path}` is not a value")]
    BadRef {
        example: String,
        given: String,
        path: String,
    },
    /// `E0952`: `returns` on an endpoint with no success body.
    #[error("example \"{example}\": the endpoint has no success body to return")]
    UnexpectedReturns { example: String },
    /// `E0953`: a value the example leaves out cannot be filled in.
    #[error("example \"{example}\": cannot fill in `{path}`: {reason}")]
    CannotFill {
        example: String,
        path: String,
        reason: String,
    },
    /// `E0953`: the filled-in value breaks an invariant that reads a filled-in field. The
    /// example must give `fields` itself.
    #[error(
        "example \"{example}\": rule {rule} is violated by values the example left out; give {}",
        fields.join(", ")
    )]
    BreaksInvariant {
        example: String,
        rule: String,
        rule_id: String,
        message: String,
        /// The filled-in fields the rule reads, from the root of the value (`body.end`).
        fields: Vec<String>,
    },
    /// `E0952`: the example has the wrong form for what is asked of it.
    #[error("example \"{example}\": {reason}")]
    WrongForm { example: String, reason: String },
    /// A value that does not have the shape of its type.
    #[error("example \"{example}\": {source}")]
    Shape {
        example: String,
        #[source]
        source: InputError,
    },
    #[error(transparent)]
    Run(#[from] RunError),
}

impl ExampleError {
    /// The diagnostic code of `spec/rules.md` section 14.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            ExampleError::UnknownEndpoint { .. } => "E0950",
            ExampleError::UnknownFact { .. } => "E0951",
            ExampleError::UnknownParam { .. }
            | ExampleError::WrongForm { .. }
            | ExampleError::BadClock { .. }
            | ExampleError::BadRef { .. }
            | ExampleError::UnexpectedReturns { .. }
            | ExampleError::Shape { .. } => "E0952",
            ExampleError::CannotFill { .. } | ExampleError::BreaksInvariant { .. } => "E0953",
            ExampleError::Run(_) => "E0952",
        }
    }
}

/// What an example expects, with references resolved.
#[derive(Debug, Clone, PartialEq)]
pub enum Expected {
    /// `ok`, with an optional pattern for what the handler returned.
    Ok(Option<Json>),
    /// An error and an optional pattern for its fields.
    Error { decl: DeclId, pattern: Option<Json> },
}

/// An example with everything filled in: the request it stands for and what it expects.
#[derive(Debug, Clone, PartialEq)]
pub struct FilledExample {
    pub name: String,
    pub service: ServiceId,
    pub endpoint: String,
    /// The request: parameters, facts (`None` is absent), principal, clock and, when the
    /// example has `returns`, the handler's result.
    pub request: Request,
    /// The clock as text, as the live mode sends it in `Veld-Test-Now`.
    pub clock: String,
    pub expected: Expected,
}

/// The service and endpoint an example calls.
fn locate<'a>(ir: &'a Ir, ex: &Example) -> Result<(&'a Service, &'a Endpoint), ExampleError> {
    let missing = || ExampleError::UnknownEndpoint {
        example: ex.name.clone(),
        endpoint: ex.endpoint.clone(),
    };
    let service = ex
        .service
        .and_then(|s| ir.services.get(s.index()))
        .ok_or_else(missing)?;
    let endpoint = service.endpoints.iter().find(|e| e.name == ex.endpoint);
    Ok((service, endpoint.ok_or_else(missing)?))
}

struct Build<'a> {
    ir: &'a Ir,
    ex: &'a Example,
    service: &'a Service,
    endpoint: &'a Endpoint,
    filler: Filler<'a>,
    /// The filled-in givens that are present, for `ref` values.
    givens: BTreeMap<String, Json>,
}

/// Fills in everything the example leaves out (section 12.2) and resolves its references.
///
/// # Errors
/// [`ExampleError`]: `E0953` when a value cannot be filled in or breaks an invariant, and the
/// errors of an example that does not fit the IR.
pub fn fill_example(ir: &Ir, ex: &Example) -> Result<FilledExample, ExampleError> {
    if ex.event.is_some() {
        return Err(ExampleError::WrongForm {
            example: ex.name.clone(),
            reason: "it is an event example, not a call".to_owned(),
        });
    }
    let (service, endpoint) = locate(ir, ex)?;
    let clock = ex.now.clone().unwrap_or_else(|| DEFAULT_CLOCK.to_owned());
    let clock_ms = parse_datetime(&clock).ok_or_else(|| ExampleError::BadClock {
        example: ex.name.clone(),
        text: clock.clone(),
    })?;
    let mut b = Build {
        ir,
        ex,
        service,
        endpoint,
        filler: Filler::new(ir, &ex.name, clock_ms),
        givens: BTreeMap::new(),
    };
    let facts = b.facts()?;
    let principal = b.principal()?;
    let params = b.params()?;
    let result = b.returns()?;
    let key = b.key()?;
    let expected = b.expected()?;
    let caller = b.caller(principal.as_ref());
    Ok(FilledExample {
        name: ex.name.clone(),
        service: service.id,
        endpoint: ex.endpoint.clone(),
        request: Request {
            params,
            facts,
            principal,
            now: clock_ms,
            result,
            if_match: b.if_match(),
            key,
            caller,
        },
        clock,
        expected,
    })
}

/// An example value as wire JSON, with references read from `givens`.
pub(super) fn resolve_value(
    example: &str,
    givens: &BTreeMap<String, Json>,
    v: &ExampleValue,
) -> Result<Json, ExampleError> {
    Ok(match v {
        ExampleValue::Json(j) => j.clone(),
        ExampleValue::Ref(r) => {
            let bad = || ExampleError::BadRef {
                example: example.to_owned(),
                given: r.given.clone(),
                path: r.path.join("."),
            };
            let mut cur = givens.get(&r.given).ok_or_else(bad)?;
            for step in &r.path {
                cur = cur.get(step).ok_or_else(bad)?;
            }
            cur.clone()
        }
        ExampleValue::Object(fields) => {
            let mut out = serde_json::Map::new();
            for f in fields {
                out.insert(f.wire.clone(), resolve_value(example, givens, &f.value)?);
            }
            Json::Object(out)
        }
        ExampleValue::List(items) => {
            let out: Result<Vec<Json>, ExampleError> = items
                .iter()
                .map(|i| resolve_value(example, givens, i))
                .collect();
            Json::Array(out?)
        }
    })
}

impl Build<'_> {
    fn shape(&self, source: InputError) -> ExampleError {
        ExampleError::Shape {
            example: self.ex.name.clone(),
            source,
        }
    }

    /// An example value as wire JSON, with references read from the givens filled in so far.
    fn resolve(&self, v: &ExampleValue) -> Result<Json, ExampleError> {
        resolve_value(&self.ex.name, &self.givens, v)
    }

    /// The facts: the givens as written, then the facts the example does not mention.
    fn facts(&mut self) -> Result<BTreeMap<String, Option<Json>>, ExampleError> {
        let mut facts = BTreeMap::new();
        let declared = &self.endpoint.rules.facts;
        for g in &self.ex.givens {
            let Some(fact) = declared.iter().find(|f| f.name == g.fact) else {
                return Err(ExampleError::UnknownFact {
                    example: self.ex.name.clone(),
                    fact: g.fact.clone(),
                });
            };
            let value = match &g.value {
                Some(v) => {
                    let given = self.resolve(v)?;
                    self.filler.begin();
                    let full = self.filler.fill(
                        &fact.ty,
                        &veld_ir::Constraints::default(),
                        Some(&given),
                        "$",
                    )?;
                    self.givens.insert(g.fact.clone(), full.clone());
                    Some(full)
                }
                None => None,
            };
            facts.insert(g.fact.clone(), value);
        }
        for fact in declared {
            if facts.contains_key(&fact.name) {
                continue;
            }
            if fact.optional {
                facts.insert(fact.name.clone(), None);
                continue;
            }
            self.filler.begin();
            let full = self
                .filler
                .fill(&fact.ty, &veld_ir::Constraints::default(), None, "$")?;
            self.givens.insert(fact.name.clone(), full.clone());
            facts.insert(fact.name.clone(), Some(full));
        }
        Ok(facts)
    }

    /// The principal (section 11.1): `as` as written, completed like any value of the principal's
    /// model; without `as`, a filled-in one when the endpoint has auth (section 12.2). It comes after
    /// the facts and before the arguments, so the counters of the filled-in values run in that order.
    fn principal(&mut self) -> Result<Option<Json>, ExampleError> {
        let given = match &self.ex.principal {
            Some(v) => Some(self.resolve(v)?),
            None => None,
        };
        let Some(p) = &self.service.principal else {
            return Ok(given);
        };
        if given.is_none() && self.endpoint.auth.kind == AuthKind::None {
            return Ok(None);
        }
        self.filler.begin();
        let ty = Type::Decl(p.decl);
        let full = self
            .filler
            .fill(&ty, &veld_ir::Constraints::default(), given.as_ref(), "$")?;
        Ok(Some(full))
    }

    /// The arguments, in the order of the endpoint's parameters.
    fn params(&mut self) -> Result<BTreeMap<String, Json>, ExampleError> {
        for a in &self.ex.args {
            if !self.endpoint.params.iter().any(|p| p.name == a.param) {
                return Err(ExampleError::UnknownParam {
                    example: self.ex.name.clone(),
                    param: a.param.clone(),
                });
            }
        }
        let mut out = BTreeMap::new();
        for p in self.endpoint.params.iter().filter(|p| !p.stream) {
            let arg = self.ex.args.iter().find(|a| a.param == p.name);
            if arg.is_none() && p.optional {
                continue;
            }
            let given = match arg {
                Some(a) => Some(self.resolve(&a.value)?),
                None => None,
            };
            self.filler.begin();
            let full = self
                .filler
                .fill(&p.ty, &p.constraints, given.as_ref(), "$")?;
            self.check_filled(&p.ty, &full, true, &p.name)?;
            out.insert(p.name.clone(), full);
        }
        Ok(out)
    }

    fn returns(&mut self) -> Result<Option<Json>, ExampleError> {
        let Some(v) = &self.ex.returns else {
            return Ok(None);
        };
        let Some(ty) = self.endpoint.success.ty.clone() else {
            return Err(ExampleError::UnexpectedReturns {
                example: self.ex.name.clone(),
            });
        };
        let given = self.resolve(v)?;
        self.filler.begin();
        let full = self
            .filler
            .fill(&ty, &veld_ir::Constraints::default(), Some(&given), "$")?;
        self.check_filled(&ty, &full, false, "result")?;
        Ok(Some(full))
    }

    /// The `If-Match` header of an example: `*` or `"3"` (`spec/rules.md` section 11.9).
    fn if_match(&self) -> Option<String> {
        let line = self.ex.if_match.as_ref()?;
        let value = match line {
            ExampleIfMatch::Any => IfMatch::Any,
            ExampleIfMatch::Version(v) => IfMatch::Version(v.parse().ok()?),
        };
        Some(value.header())
    }

    /// The text the application gives for the caller: the principal, when the endpoint has auth
    /// and its keys are scoped to the caller.
    fn caller(&self, principal: Option<&Json>) -> Option<String> {
        self.endpoint.idempotency.as_ref().filter(|i| i.caller)?;
        Some(principal.map_or_else(|| "caller".to_owned(), veld_codegen::canonical::canonical))
    }

    /// The `key` line: the header, and what the store holds for it. A stored answer is a value
    /// like any other, so what the line leaves out is filled in (section 12.2).
    fn key(&mut self) -> Result<Option<IdempotencyKey>, ExampleError> {
        let Some(line) = &self.ex.key else {
            return Ok(None);
        };
        let store = match &line.store {
            KeyStore::New => StoreState::Empty,
            KeyStore::Other => StoreState::Other,
            KeyStore::Running => StoreState::Running,
            KeyStore::Stored(outcome) => StoreState::Stored(self.stored(outcome)?),
        };
        Ok(Some(IdempotencyKey {
            value: line.value.clone(),
            store,
        }))
    }

    fn stored(&mut self, outcome: &ExampleOutcome) -> Result<StoredAnswer, ExampleError> {
        let (status, error, ty, given) = match outcome {
            ExampleOutcome::Ok(v) => {
                let ty = self.endpoint.success.ty.clone();
                let given = v.as_ref().map(|v| self.resolve(v)).transpose()?;
                (self.endpoint.success.status, None, ty, given)
            }
            ExampleOutcome::Error(e) => {
                let given = e.pattern.as_ref().map(|v| self.resolve(v)).transpose()?;
                let (name, status) = self.error_of(e.decl);
                (status, Some(name), Some(Type::Decl(e.decl)), given)
            }
        };
        let body = match ty {
            Some(ty) => {
                self.filler.begin();
                let none = veld_ir::Constraints::default();
                Some(self.filler.fill(&ty, &none, given.as_ref(), "$")?)
            }
            None => None,
        };
        Ok(StoredAnswer {
            status,
            error,
            body,
        })
    }

    /// The name and the status an endpoint answers with for an error declaration.
    fn error_of(&self, decl: DeclId) -> (String, u16) {
        let name = self.ir.decls.get(decl.index()).map(|d| d.name.clone());
        let declared = self.endpoint.errors.iter().find(|e| e.decl == decl);
        let default = self.ir.decls.get(decl.index()).and_then(|d| match &d.kind {
            DeclKind::Error(e) => e.default_status,
            DeclKind::Model(_)
            | DeclKind::Enum(_)
            | DeclKind::Union(_)
            | DeclKind::Const(_)
            | DeclKind::Event(_) => None,
        });
        (
            name.unwrap_or_default(),
            declared.map(|e| e.status).or(default).unwrap_or(0),
        )
    }

    fn expected(&self) -> Result<Expected, ExampleError> {
        let pattern = |v: &Option<ExampleValue>| v.as_ref().map(|v| self.resolve(v)).transpose();
        let outcome = self
            .ex
            .outcome
            .as_ref()
            .ok_or_else(|| ExampleError::WrongForm {
                example: self.ex.name.clone(),
                reason: "a call example has no outcome".to_owned(),
            })?;
        Ok(match outcome {
            ExampleOutcome::Ok(v) => Expected::Ok(pattern(v)?),
            ExampleOutcome::Error(e) => Expected::Error {
                decl: e.decl,
                pattern: pattern(&e.pattern)?,
            },
        })
    }

    /// A filled-in value must be valid: an invariant that is violated and reads a field the
    /// example left out means the example is wrong (`E0953`). One whose fields the example
    /// wrote itself may be violated on purpose.
    fn check_filled(
        &self,
        ty: &Type,
        value: &Json,
        request: bool,
        root: &str,
    ) -> Result<(), ExampleError> {
        let opts = Options {
            request,
            partial: false,
        };
        let (_, violations) = check_type(self.ir, ty, value, opts).map_err(|e| self.shape(e))?;
        for v in violations {
            let at = |f: &String| format!("{}.{f}", v.path);
            let fields: Vec<String> = v
                .fields
                .iter()
                .filter(|f| self.filler.filled.contains(&at(f)))
                .map(|f| format!("{root}{}", at(f).trim_start_matches('$')))
                .collect();
            if !fields.is_empty() {
                return Err(ExampleError::BreaksInvariant {
                    example: self.ex.name.clone(),
                    rule: v.rule_name,
                    rule_id: v.rule_id,
                    message: v.message,
                    fields,
                });
            }
        }
        Ok(())
    }
}

/// Runs an example offline: fills it in, runs the pipeline and compares the outcome with the
/// expected one (section 12.3).
///
/// # Errors
/// [`ExampleError`] when the example cannot be filled in or does not fit the IR. A mismatch is
/// not an error: it is [`Verdict::Failed`].
pub fn run_example(ir: &Ir, ex: &Example) -> Result<ExampleReport, ExampleError> {
    if ex.event.is_some() {
        return run_event_example(ir, ex);
    }
    let filled = fill_example(ir, ex)?;
    let (service, endpoint) = locate(ir, ex)?;
    let pipeline = Pipeline::new(ir, service, &ex.endpoint)?;
    let reported =
        crate::coverage::armed(|| pipeline.run_reported(&filled.request, OutputMode::Strict))?;
    let result = reported.body.as_ref().or(filled.request.result.as_ref());
    let verdict = report::compare(ir, endpoint, &filled.expected, &reported.outcome, result);
    Ok(ExampleReport {
        name: ex.name.clone(),
        verdict,
        outcome: Some(reported.outcome),
        audited: reported.audited,
    })
}
