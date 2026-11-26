//! Fuzzing a live test handler (`spec/rules.md` section 12.3, `--fuzz`).
//!
//! For every endpoint a seeded generator builds requests, valid and invalid. The reference
//! pipeline works out what each one must end in, the request goes to the test handler in
//! dry-run mode, and any difference in the answer (see [`crate::outcome::judge_outcome`]) is a
//! failure. The first requests of a run are the **boundaries** of the rules; the rest are
//! random. See [`rules`] for how both are found, and [`generate`] for how requests are made.
//!
//! The same seed always gives the same requests, so a failure can be repeated.

mod access;
mod loc;
mod render;
mod rng;
mod rules;
mod values;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value as Json;
use veld_ir::{AuthKind, Constraints, Endpoint, Ir, Scalar, Service, Type};
use veld_rules::helpers::instant::parse_datetime;
use veld_rules::{
    Difference, Expected, FilledExample, Outcome, Pipeline, Request, RunError, describe_actual,
    format_datetime,
};

use self::loc::{fact_value, param_value};
pub use self::render::render_example;
use self::rng::derive;
use self::rules::{RuleIndex, Scope, Target, Task};
use self::values::{Gen, Skip, strip_derived};
use crate::error::LiveError;
use crate::live::Live;
use crate::outcome::{Called, describe_answer, judge_outcome};
use crate::request::build_request;

/// The size and the seed of a fuzz run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuzzOptions {
    /// Requests per endpoint, boundaries included.
    pub count: usize,
    pub seed: u64,
}

/// Why requests cannot be generated for an endpoint.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum GenerateError {
    /// The endpoint is skipped: the generator cannot make a value for it.
    #[error("{0}")]
    Skipped(String),
    /// A generated request does not have the shape the IR describes. This is a bug of the
    /// generator, not a finding about the server.
    #[error("the generator made a request the reference evaluator rejects: {0}")]
    Rejected(RunError),
}

/// Why a fuzz run stopped.
#[derive(Debug, thiserror::Error)]
pub enum FuzzError {
    #[error(transparent)]
    Live(#[from] LiveError),
    #[error("{endpoint}: {source}")]
    Generator {
        endpoint: String,
        source: GenerateError,
    },
}

/// The requests of one endpoint.
#[derive(Debug, Clone)]
pub struct Generated {
    pub requests: Vec<Request>,
    /// How many of them, from the first, are boundary requests.
    pub boundaries: usize,
}

/// A request on which the server and the reference evaluator disagree.
#[derive(Debug, Clone)]
pub struct FuzzMismatch {
    /// The position of the request among those sent to the endpoint, from 0.
    pub index: usize,
    pub boundary: bool,
    /// What the reference evaluator says the answer is.
    pub expected: String,
    /// What the server answered.
    pub actual: String,
    pub differences: Vec<Difference>,
    /// The request as an `example` block that can be pasted into the contract. `None` when a
    /// value cannot be written in example syntax.
    pub example: Option<String>,
}

/// What happened to one endpoint.
#[derive(Debug, Clone)]
pub struct EndpointReport {
    pub service: String,
    pub endpoint: String,
    /// Requests sent.
    pub sent: usize,
    /// How many of those are boundary requests.
    pub boundaries: usize,
    /// How many requests ended in each outcome (`ok`, or the name of the error).
    pub outcomes: BTreeMap<String, usize>,
    /// Why the endpoint was not fuzzed, if it was not.
    pub skipped: Option<String>,
    pub mismatch: Option<FuzzMismatch>,
}

impl EndpointReport {
    fn new(service: &Service, ep: &Endpoint) -> EndpointReport {
        EndpointReport {
            service: service.name.clone(),
            endpoint: ep.name.clone(),
            sent: 0,
            boundaries: 0,
            outcomes: BTreeMap::new(),
            skipped: None,
            mismatch: None,
        }
    }

    fn skipped(service: &Service, ep: &Endpoint, why: &str) -> EndpointReport {
        EndpointReport {
            skipped: Some(why.to_owned()),
            ..EndpointReport::new(service, ep)
        }
    }
}

/// The result of a fuzz run.
#[derive(Debug, Clone)]
pub struct FuzzReport {
    pub seed: u64,
    pub endpoints: Vec<EndpointReport>,
    /// Why the run ended early (the server could not be reached any more, or the generator made
    /// a request the reference evaluator rejects). What was found before is kept in `endpoints`.
    pub stopped: Option<String>,
}

impl FuzzReport {
    /// True when the run finished and no endpoint disagrees with the reference evaluator.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.stopped.is_none() && self.endpoints.iter().all(|e| e.mismatch.is_none())
    }

    /// Endpoints whose every request ended the same way, with that outcome: the fuzz did not get
    /// past it, so whatever comes after it in the pipeline was not exercised.
    #[must_use]
    pub fn one_sided(&self) -> Vec<(String, String)> {
        self.endpoints
            .iter()
            .filter(|e| e.sent > 1 && e.outcomes.len() == 1 && e.mismatch.is_none())
            .filter_map(|e| {
                let only = e.outcomes.keys().next()?;
                (only != "ok").then(|| (format!("{}.{}", e.service, e.endpoint), only.clone()))
            })
            .collect()
    }

    /// How many endpoints were skipped, with the reasons.
    #[must_use]
    pub fn skipped(&self) -> Vec<(String, String)> {
        self.endpoints
            .iter()
            .filter_map(|e| Some((format!("{}.{}", e.service, e.endpoint), e.skipped.clone()?)))
            .collect()
    }
}

/// Why the endpoints of `service` are not fuzzed on `live`, if they are not.
fn not_fuzzed(live: &Live, service: &Service) -> Option<String> {
    // A server target generates a test handler only for a service with endpoint rules
    // (`spec/rules.md` section 12.4); another service has nothing to answer a dry run.
    if !service.endpoints.iter().any(|e| !e.rules.is_empty()) {
        return Some("the service has no endpoint rules, so it has no test handler".to_owned());
    }
    // A workspace has one test handler per service: this one may be another service's.
    (!live.serves(&service.name)).then(|| {
        format!(
            "the test handler at {} does not serve `{}`",
            live.base_url(),
            service.name
        )
    })
}

/// Fuzzes every endpoint of `ir` that the live test handler serves.
///
/// A run that cannot go on (the server stops answering, or the generator made a request the
/// reference evaluator rejects) ends there with [`FuzzReport::stopped`] set: what was found
/// before is in the report, never thrown away. A disagreement with the server is in the report
/// too.
///
/// # Errors
/// None today; the type is kept so that callers do not change when one is added.
pub fn fuzz(ir: &Ir, live: &Live, opts: &FuzzOptions) -> Result<FuzzReport, FuzzError> {
    let mut report = FuzzReport {
        seed: opts.seed,
        endpoints: Vec::new(),
        stopped: None,
    };
    for service in &ir.services {
        let skip = not_fuzzed(live, service);
        for ep in &service.endpoints {
            if let Some(why) = &skip {
                let skipped = EndpointReport::skipped(service, ep, why);
                report.endpoints.push(skipped);
                continue;
            }
            let (endpoint, failure) = fuzz_endpoint(ir, live, service, ep, opts);
            report.endpoints.push(endpoint);
            if let Some(failure) = failure {
                report.stopped = Some(failure.to_string());
                return Ok(report);
            }
        }
    }
    Ok(report)
}

fn label(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Passes => "ok".to_owned(),
        Outcome::Error(a) => a.name.clone(),
    }
}

/// The report of one endpoint and, when the run cannot go on, why. The report holds whatever was
/// sent before the failure.
fn fuzz_endpoint(
    ir: &Ir,
    live: &Live,
    service: &Service,
    ep: &Endpoint,
    opts: &FuzzOptions,
) -> (EndpointReport, Option<FuzzError>) {
    let name = format!("{}.{}", service.name, ep.name);
    let bug = |source: GenerateError| FuzzError::Generator {
        endpoint: name.clone(),
        source,
    };
    let mut report = EndpointReport::new(service, ep);
    let generated = match generate(ir, service, ep, opts.seed, opts.count) {
        Ok(g) => g,
        Err(GenerateError::Skipped(why)) => {
            report.skipped = Some(why);
            return (report, None);
        }
        Err(e) => return (report, Some(bug(e))),
    };
    let pipeline = match Pipeline::new(ir, service, &ep.name) {
        Ok(p) => p,
        Err(e) => return (report, Some(bug(GenerateError::Rejected(e)))),
    };
    let cx = Cx {
        ir,
        live,
        service,
        ep,
        pipeline,
        seed: opts.seed,
        boundaries: generated.boundaries,
    };
    for (index, req) in generated.requests.iter().enumerate() {
        match cx.send(index, req) {
            Err(failed) => return (report, Some(failed.in_endpoint(&name))),
            Ok(Sent::Unsupported(reason)) => {
                report.skipped = Some(reason);
                break;
            }
            Ok(Sent::Done { outcome, mismatch }) => {
                report.sent += 1;
                report.boundaries += usize::from(index < generated.boundaries);
                *report.outcomes.entry(outcome).or_default() += 1;
                if mismatch.is_some() {
                    report.mismatch = mismatch;
                    break;
                }
            }
        }
    }
    (report, None)
}

/// What is needed to send the requests of one endpoint.
struct Cx<'a> {
    ir: &'a Ir,
    live: &'a Live,
    service: &'a Service,
    ep: &'a Endpoint,
    pipeline: Pipeline<'a>,
    seed: u64,
    boundaries: usize,
}

enum Sent {
    /// The endpoint cannot be sent to (a stream, a value that cannot be bound).
    Unsupported(String),
    Done {
        /// The label of the expected outcome.
        outcome: String,
        mismatch: Option<FuzzMismatch>,
    },
}

/// A failure of one request, before it is known which endpoint it was for.
enum Failed {
    Live(LiveError),
    Rejected(RunError),
}

impl Failed {
    fn in_endpoint(self, name: &str) -> FuzzError {
        match self {
            Failed::Live(e) => FuzzError::Live(e),
            Failed::Rejected(e) => FuzzError::Generator {
                endpoint: name.to_owned(),
                source: GenerateError::Rejected(e),
            },
        }
    }
}

impl From<LiveError> for Failed {
    fn from(e: LiveError) -> Failed {
        Failed::Live(e)
    }
}

impl Cx<'_> {
    /// Works out the expected outcome of `req`, sends it in dry-run mode and judges the answer.
    fn send(&self, index: usize, req: &Request) -> Result<Sent, Failed> {
        let expected = self.pipeline.run(req).map_err(Failed::Rejected)?;
        let filled = FilledExample {
            name: format!("fuzz {index}"),
            service: self.service.id,
            endpoint: self.ep.name.clone(),
            request: req.clone(),
            clock: format_datetime(req.now),
            expected: Expected::Ok(None),
        };
        let http = match build_request(self.ir, &filled, true) {
            Ok(h) => h,
            Err(LiveError::Unsupported { reason, .. }) => return Ok(Sent::Unsupported(reason)),
            Err(e) => return Err(e.into()),
        };
        let answer = self.live.send(&http)?;
        let called = Called {
            service: self.service,
            endpoint: self.ep,
        };
        let differences = judge_outcome(self.ir, called, &expected, &answer);
        let mismatch = (!differences.is_empty()).then(|| {
            let title = format!(
                "fuzz seed {} {}.{} request {index}",
                self.seed, self.service.name, self.ep.name
            );
            FuzzMismatch {
                index,
                boundary: index < self.boundaries,
                expected: describe_actual(&expected, None),
                actual: describe_answer(self.ir, &answer),
                differences,
                example: render_example(self.ir, self.service, self.ep, req, &expected, &title),
            }
        });
        Ok(Sent::Done {
            outcome: label(&expected),
            mismatch,
        })
    }
}

// ---- generating requests ------------------------------------------------------------------

/// The requests to send to one endpoint: first the boundaries of its rules, then random
/// requests, `count` in all. Deterministic in `seed`.
///
/// **Boundaries.** For every comparison in a rule of the endpoint (its `require` lines and the
/// rules of the models its parameters contain) with a side the generator can write (a path,
/// `len(path)`, or the difference of two paths), the other side is evaluated on a request that
/// passes everything else, and the path is written at that value and one smallest step either
/// side (one for an integer, a millisecond for a datetime, a day for a date, `0.01` for a
/// decimal, the next float). The other side can be a literal, `now`, a `days(30)` or another
/// path such as `room.capacity`.
///
/// **States.** For an endpoint with a `transition`, one more boundary request per state of the
/// machine: everything else is made to pass, then the state is written (`access::states`).
///
/// **Random requests.** Parameters and facts are generated from their types, and so is the
/// principal when the package declares one and the endpoint has auth; a required fact is
/// absent in one request of ten, an optional one in one of two. In three requests of four the
/// reference pipeline is then run and every rule it reports as violated is made true if the
/// generator can (see `rules::repair`), so a request is not a violation of `end > start` every
/// time. The same repair makes a `permit` of a denied `allow` true and writes a source state
/// for an illegal one (see `access`). Each failing check is left alone one time in eight
/// ([`QUIT`]), so requests that fail two checks at once are common. In one request of four the
/// clock is moved next to a datetime found in the request.
///
/// # Errors
/// [`GenerateError::Skipped`] when no value can be made for a type of the endpoint (a string with
/// `@pattern` or `@format` and no `@example`).
pub fn generate(
    ir: &Ir,
    service: &Service,
    ep: &Endpoint,
    seed: u64,
    count: usize,
) -> Result<Generated, GenerateError> {
    let key = format!("{}.{}", service.name, ep.name);
    let mut g = Gen::new(ir, derive(seed, &key));
    g.principal = service.principal.as_ref().map(|p| p.decl);
    let pipeline = Pipeline::new(ir, service, &ep.name).map_err(GenerateError::Rejected)?;
    let index = rules::index(ir, ep);
    let mut out = Out::default();
    for task in boundary_tasks(ir, ep) {
        for delta in [-1, 0, 1] {
            if out.requests.len() >= count {
                break;
            }
            // A baseline that cannot be made to pass (the generator may not know how to satisfy
            // a rule) is tried again a few times with other values.
            for _ in 0..BASELINES {
                let mut req = baseline(&mut g, ep, Facts::Present)?;
                let ready = rules::prepare(&mut g, ep, &mut req, &task).is_some()
                    && settle(&mut g, ep, &mut req, &pipeline, &index, 0)?;
                if ready {
                    if rules::apply(&mut g, ep, &mut req, &task, delta).is_some() {
                        strip_request(ir, ep, g.principal, &mut req);
                        out.push(req);
                    }
                    break;
                }
            }
        }
    }
    state_boundaries(&mut g, ep, (&pipeline, &index), &mut out, count)?;
    let boundaries = out.requests.len();
    let mut tries = 0;
    let max_tries = count.saturating_mul(4).saturating_add(16);
    while out.requests.len() < count && tries < max_tries {
        tries += 1;
        let mut req = baseline(&mut g, ep, Facts::Random)?;
        if g.rng.chance(75) {
            settle(&mut g, ep, &mut req, &pipeline, &index, QUIT)?;
        }
        move_clock(&mut g, &mut req);
        strip_request(ir, ep, g.principal, &mut req);
        out.push(req);
    }
    Ok(Generated {
        requests: out.requests,
        boundaries,
    })
}

/// One request for every state of the machine of the endpoint's `transition`: everything else
/// passes (the baseline is settled first), then the state is written. A state the transition
/// refuses meets `InvalidTransition`, a source state gets past the guard.
fn state_boundaries(
    g: &mut Gen<'_>,
    ep: &Endpoint,
    (pipeline, index): (&Pipeline<'_>, &RuleIndex<'_>),
    out: &mut Out,
    count: usize,
) -> Result<(), GenerateError> {
    for (t, state) in access::states(g.ir, ep) {
        if out.requests.len() >= count {
            break;
        }
        let mut req = baseline(g, ep, Facts::Present)?;
        for _ in 0..BASELINES {
            if settle(g, ep, &mut req, pipeline, index, 0)? {
                break;
            }
            req = baseline(g, ep, Facts::Present)?;
        }
        if access::set_state(g, ep, &mut req, t, state).is_some() {
            strip_request(g.ir, ep, g.principal, &mut req);
            out.push(req);
        }
    }
    Ok(())
}

/// Removes the derived fields from every value of the request (`spec/rules.md` section 17.2): a
/// caller never sends one, an `example` never writes one, and the reference pipeline computes
/// them. A rule on a derived field gets no boundary of its own; it is exercised through the fields
/// the expression reads.
fn strip_request(ir: &Ir, ep: &Endpoint, principal: Option<veld_ir::DeclId>, req: &mut Request) {
    for p in ep.params.iter().filter(|p| !p.stream) {
        if let Some(v) = req.params.get_mut(&p.name) {
            strip_derived(ir, &p.ty, v);
        }
    }
    for f in &ep.rules.facts {
        if let Some(Some(v)) = req.facts.get_mut(&f.name) {
            strip_derived(ir, &f.ty, v);
        }
    }
    if let (Some(decl), Some(v)) = (principal, req.principal.as_mut()) {
        strip_derived(ir, &Type::Decl(decl), v);
    }
}

#[derive(Default)]
struct Out {
    requests: Vec<Request>,
    seen: BTreeSet<String>,
}

impl Out {
    fn push(&mut self, req: Request) {
        let key = format!("{:?}", (&req.params, &req.facts, &req.principal, req.now));
        if self.seen.insert(key) {
            self.requests.push(req);
        }
    }
}

#[derive(Clone, Copy)]
enum Facts {
    /// Every fact present, as a boundary request needs.
    Present,
    Random,
}

fn skipped(Skip(why): Skip) -> GenerateError {
    GenerateError::Skipped(why)
}

/// A request with generated parameters and facts, and the default clock.
fn baseline(g: &mut Gen<'_>, ep: &Endpoint, facts: Facts) -> Result<Request, GenerateError> {
    let mut req = Request {
        now: g.clock,
        ..Request::default()
    };
    for p in ep.params.iter().filter(|p| !p.stream) {
        if p.optional && !g.rng.chance(50) {
            continue;
        }
        req.params
            .insert(p.name.clone(), param_value(g, p).map_err(skipped)?);
    }
    for f in &ep.rules.facts {
        let present = match facts {
            Facts::Present => true,
            Facts::Random if f.optional => g.rng.chance(50),
            Facts::Random => g.rng.chance(90),
        };
        let value = if present {
            Some(fact_value(g, f).map_err(skipped)?)
        } else {
            None
        };
        req.facts.insert(f.name.clone(), value);
    }
    // The filler of an `example` adds a principal to every endpoint with auth, so one is made
    // for it too: a printed block then fills back to this request.
    if let Some(decl) = g.principal.filter(|_| ep.auth.kind != AuthKind::None) {
        let ty = Type::Decl(decl);
        req.principal = Some(g.value(&ty, &Constraints::default()).map_err(skipped)?);
    }
    Ok(req)
}

/// How many baselines are tried for one boundary request.
const BASELINES: usize = 4;

/// The chance in a hundred that a random request is left as it is at any check it fails, instead
/// of being repaired. Without it a request that fails a late check (a `require`) would nearly
/// always have passed the earlier ones, and two checks that both apply (a denied caller on a
/// booking in an illegal state) would be rare: the order of `spec/rules.md` section 9.5 would
/// be fuzzed only by luck.
const QUIT: u64 = 12;

/// Runs the reference pipeline and repairs the rules it reports, until the request passes or
/// nothing more can be repaired. True when the request passes. At each failing check, `quit`
/// in a hundred leaves the request as it is.
fn settle(
    g: &mut Gen<'_>,
    ep: &Endpoint,
    req: &mut Request,
    pipeline: &Pipeline<'_>,
    index: &RuleIndex<'_>,
    quit: u64,
) -> Result<bool, GenerateError> {
    // Each round repairs the rules the pipeline reports; more rules need more rounds.
    for _ in 0..(8 + index.len().min(56)) {
        match pipeline.run(req).map_err(GenerateError::Rejected)? {
            Outcome::Passes => return Ok(true),
            Outcome::Error(a) => {
                if a.violations.is_empty()
                    || (quit > 0 && g.rng.chance(quit))
                    || !rules::repair(g, ep, req, &a.violations, index)
                {
                    return Ok(false);
                }
            }
        }
    }
    Ok(false)
}

/// The tasks whose boundaries are worth sending: numbers, instants and lengths.
fn boundary_tasks(ir: &Ir, ep: &Endpoint) -> Vec<Task> {
    let mut all = Vec::new();
    // The boundaries of an audited rule change no outcome, so they are not requested.
    for r in ep.rules.requires.iter().filter(|r| r.rollout.is_none()) {
        all.extend(rules::tasks(&Scope::Request, &r.expr, true, false));
    }
    for (at, id) in rules::rule_places(ir, ep) {
        let Some(decl) = ir.decls.get(id.index()) else {
            continue;
        };
        for rule in decl.rules().iter().filter(|r| !r.is_audited()) {
            all.extend(rules::tasks(
                &Scope::Instance(at.clone()),
                &rule.expr,
                true,
                false,
            ));
        }
    }
    let mut out: Vec<Task> = Vec::new();
    for t in all {
        let numeric = match &t.target {
            Target::Path(_, s) => matches!(
                s,
                Scalar::Int
                    | Scalar::Long
                    | Scalar::Float
                    | Scalar::Decimal
                    | Scalar::Datetime
                    | Scalar::Date
            ),
            Target::Len(_) | Target::Diff(..) => true,
            Target::Member(_) => false,
        };
        if numeric && t.op != veld_ir::Op::In && !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// In one request of four, the clock is put next to a datetime of the request (a millisecond
/// before, at it, or a millisecond after, or a day either side).
fn move_clock(g: &mut Gen<'_>, req: &mut Request) {
    if !g.rng.chance(25) {
        return;
    }
    let mut found = Vec::new();
    req.params.values().for_each(|v| datetimes(v, &mut found));
    req.facts
        .values()
        .flatten()
        .for_each(|v| datetimes(v, &mut found));
    if found.is_empty() {
        return;
    }
    let base = found[g.rng.index(found.len())];
    const OFFSETS: [i64; 5] = [-1, 0, 1, -86_400_000, 86_400_000];
    req.now = base + OFFSETS[g.rng.index(OFFSETS.len())];
}

fn datetimes(v: &Json, out: &mut Vec<i64>) {
    match v {
        Json::String(s) if s.contains('T') => out.extend(parse_datetime(s)),
        Json::Array(items) => items.iter().for_each(|i| datetimes(i, out)),
        Json::Object(map) => map.values().for_each(|i| datetimes(i, out)),
        Json::Null | Json::Bool(_) | Json::Number(_) | Json::String(_) => {}
    }
}
