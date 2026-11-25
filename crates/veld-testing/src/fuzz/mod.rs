//! Fuzzing a live test handler (`spec/rules.md` section 12.3, `--fuzz`).
//!
//! For every endpoint a seeded generator builds requests, valid and invalid. The reference
//! pipeline works out what each one must end in, the request goes to the test handler in
//! dry-run mode, and any difference in status, error type, payload or the set of violations is
//! a failure. The first requests of a run are the **boundaries** of the rules; the rest are
//! random. See [`rules`] for how both are found, and [`generate`] for how requests are made.
//!
//! The same seed always gives the same requests, so a failure can be repeated.

mod loc;
mod render;
mod rng;
mod rules;
mod values;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value as Json;
use veld_ir::{Endpoint, Ir, Scalar, Service};
use veld_rules::helpers::instant::parse_datetime;
use veld_rules::{
    Difference, Expected, FilledExample, Outcome, Pipeline, Request, RunError, describe_actual,
    format_datetime,
};

use self::loc::{fact_value, param_value};
pub use self::render::render_example;
use self::rng::derive;
use self::rules::{RuleIndex, Scope, Target, Task};
use self::values::{Gen, Skip};
use crate::error::LiveError;
use crate::live::Live;
use crate::outcome::{describe_answer, judge_outcome};
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
}

impl FuzzReport {
    /// True when no endpoint disagrees with the reference evaluator.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.endpoints.iter().all(|e| e.mismatch.is_none())
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

/// Fuzzes every endpoint of `ir` against a live test handler.
///
/// # Errors
/// [`FuzzError::Live`] when the server cannot be reached, [`FuzzError::Generator`] for a bug of
/// the generator. A disagreement with the server is not an error: it is in the report.
pub fn fuzz(ir: &Ir, live: &Live, opts: &FuzzOptions) -> Result<FuzzReport, FuzzError> {
    let mut endpoints = Vec::new();
    for service in &ir.services {
        // A server target generates a test handler only for a service with endpoint rules
        // (`spec/rules.md` section 12.4); another service has nothing to answer a dry run.
        let handled = service.endpoints.iter().any(|e| !e.rules.is_empty());
        for ep in &service.endpoints {
            if handled {
                endpoints.push(fuzz_endpoint(ir, live, service, ep, opts)?);
            } else {
                endpoints.push(EndpointReport::skipped(
                    service,
                    ep,
                    "the service has no endpoint rules, so it has no test handler",
                ));
            }
        }
    }
    Ok(FuzzReport {
        seed: opts.seed,
        endpoints,
    })
}

fn label(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Passes => "ok".to_owned(),
        Outcome::Error(a) => a.name.clone(),
    }
}

fn fuzz_endpoint(
    ir: &Ir,
    live: &Live,
    service: &Service,
    ep: &Endpoint,
    opts: &FuzzOptions,
) -> Result<EndpointReport, FuzzError> {
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
            return Ok(report);
        }
        Err(e) => return Err(bug(e)),
    };
    let pipeline =
        Pipeline::new(ir, service, &ep.name).map_err(|e| bug(GenerateError::Rejected(e)))?;
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
        match cx.send(index, req).map_err(|e| e.in_endpoint(&name))? {
            Sent::Unsupported(reason) => {
                report.skipped = Some(reason);
                return Ok(report);
            }
            Sent::Done { outcome, mismatch } => {
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
    Ok(report)
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
        let differences = judge_outcome(self.ir, &expected, &answer);
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
/// **Random requests.** Parameters and facts are generated from their types; a required fact is
/// absent in one request of ten, an optional one in one of two. In three requests of four the
/// reference pipeline is then run and every rule it reports as violated is made true if the
/// generator can (see `rules::repair`), so a request is not a violation of `end > start` every
/// time. In one request of four the clock is moved next to a datetime found in the request.
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
                    && settle(&mut g, ep, &mut req, &pipeline, &index)?;
                if ready {
                    if rules::apply(&mut g, ep, &mut req, &task, delta).is_some() {
                        out.push(req);
                    }
                    break;
                }
            }
        }
    }
    let boundaries = out.requests.len();
    let mut tries = 0;
    while out.requests.len() < count && tries < count * 4 + 16 {
        tries += 1;
        let mut req = baseline(&mut g, ep, Facts::Random)?;
        if g.rng.chance(75) {
            settle(&mut g, ep, &mut req, &pipeline, &index)?;
        }
        move_clock(&mut g, &mut req);
        out.push(req);
    }
    Ok(Generated {
        requests: out.requests,
        boundaries,
    })
}

#[derive(Default)]
struct Out {
    requests: Vec<Request>,
    seen: BTreeSet<String>,
}

impl Out {
    fn push(&mut self, req: Request) {
        let key = format!("{:?}", (&req.params, &req.facts, req.now));
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
    Ok(req)
}

/// How many baselines are tried for one boundary request.
const BASELINES: usize = 4;

/// Runs the reference pipeline and repairs the rules it reports, until the request passes or
/// nothing more can be repaired. True when the request passes.
fn settle(
    g: &mut Gen<'_>,
    ep: &Endpoint,
    req: &mut Request,
    pipeline: &Pipeline<'_>,
    index: &RuleIndex<'_>,
) -> Result<bool, GenerateError> {
    // Each round repairs the rules the pipeline reports; more rules need more rounds.
    for _ in 0..(8 + index.len().min(56)) {
        match pipeline.run(req).map_err(GenerateError::Rejected)? {
            Outcome::Passes => return Ok(true),
            Outcome::Error(a) => {
                if a.violations.is_empty() || !rules::repair(g, ep, req, &a.violations, index) {
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
    for r in &ep.rules.requires {
        all.extend(rules::tasks(&Scope::Request, &r.expr, true, false));
    }
    for (at, id) in rules::rule_places(ir, ep) {
        let Some(decl) = ir.decls.get(id.index()) else {
            continue;
        };
        for rule in decl.rules() {
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
        };
        if numeric && !out.contains(&t) {
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
