//! Running examples against a live test handler (`spec/rules.md` section 12.4).

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use serde_json::Value as Json;
use veld_ir::{Example, Ir, ServiceId};
use veld_rules::{
    ExampleReport, FilledExample, Mismatch, Outcome, OutputMode, Pipeline, Reported, Request,
    Verdict, Violation, describe_actual, fill_example,
};

use crate::audited;
use crate::conditional;
use crate::error::LiveError;
use crate::judge::{Answer, judge};
use crate::outcome::{Called, describe_answer, judge_outcome};
use crate::request::{HttpRequest, PROBE_PATH, build_request};

/// What a probe says: the services it serves and, for events, the topics they subscribe to.
type Probed = (BTreeSet<String>, Option<BTreeMap<String, Vec<String>>>);

const TIMEOUT: Duration = Duration::from_secs(30);

/// A server that has been probed and found to be a test handler.
#[derive(Debug, Clone)]
pub struct Live {
    base: String,
    dry_run: bool,
    agent: ureq::Agent,
    /// The services the handler said it serves: the keys of the probe's `digests`.
    services: BTreeSet<String>,
    /// The probe's `subscribes`: the topics each service subscribes to. `None` when the probe
    /// has no such member (a test handler that was generated before events).
    subscribes: Option<BTreeMap<String, Vec<String>>>,
}

impl Live {
    /// Probes `GET <base_url>/__veld/test`. A server that does not answer as a test handler is
    /// refused before any example is sent. With `dry_run`, an example without `returns` stops
    /// before the application's handler.
    ///
    /// # Errors
    /// [`LiveError::Connect`] when the server cannot be reached and
    /// [`LiveError::NotTestHandler`] when it answers the probe as anything but a test handler.
    pub fn connect(base_url: &str, dry_run: bool) -> Result<Live, LiveError> {
        let mut live = Live {
            base: base_url.trim_end_matches('/').to_owned(),
            dry_run,
            agent: ureq::AgentBuilder::new()
                .redirects(0)
                .timeout(TIMEOUT)
                .build(),
            services: BTreeSet::new(),
            subscribes: None,
        };
        let (services, subscribes) = live.probe()?;
        live.services = services;
        live.subscribes = subscribes;
        Ok(live)
    }

    /// The base URL, without a trailing slash.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base
    }

    /// Whether the handler said it serves the service `name`. A workspace has one test handler
    /// per service, so the examples and the fuzz requests of another service do not go to it.
    #[must_use]
    pub fn serves(&self, name: &str) -> bool {
        self.services.contains(name)
    }

    /// Whether the probe listed `subscribes` (`spec/events.md` section 10.9).
    #[must_use]
    pub fn serves_events(&self) -> bool {
        self.subscribes.is_some()
    }

    /// The services of `ir` that the test handler said subscribe to `topic`.
    pub(crate) fn subscribers_of(&self, ir: &Ir, topic: &str) -> BTreeSet<ServiceId> {
        let topics = self.subscribes.iter().flatten();
        let names: BTreeSet<&str> = topics
            .filter(|(_, topics)| topics.iter().any(|t| t == topic))
            .map(|(service, _)| service.as_str())
            .collect();
        ir.services
            .iter()
            .filter(|s| names.contains(s.name.as_str()))
            .map(|s| s.id)
            .collect()
    }

    /// The answer MUST be `{ "veldTestHandler": 1, "digests": { "<Service>": .. } }`: anything
    /// else may be a production server, which must never be sent test headers. It MAY say
    /// `"subscribes": { "<Service>": ["<topic>", ..] }` (`spec/events.md` section 10.9).
    fn probe(&self) -> Result<Probed, LiveError> {
        let probe = HttpRequest {
            method: "GET".to_owned(),
            target: PROBE_PATH.to_owned(),
            headers: vec![("Accept".to_owned(), "application/json".to_owned())],
            body: None,
        };
        let url = format!("{}{PROBE_PATH}", self.base);
        let not = |reason: String| LiveError::NotTestHandler {
            url: self.base.clone(),
            reason,
        };
        let answer = self.send(&probe)?;
        if answer.status != 200 {
            return Err(not(format!(
                "`GET {PROBE_PATH}` answered HTTP {}",
                answer.status
            )));
        }
        let body: Json = serde_json::from_str(&answer.body)
            .map_err(|_| not(format!("`GET {url}` did not answer with JSON")))?;
        if body.get("veldTestHandler").and_then(Json::as_u64) != Some(1) {
            return Err(not(format!(
                "the answer of `GET {url}` does not say `\"veldTestHandler\": 1`"
            )));
        }
        let digests = body.get("digests").and_then(Json::as_object);
        let digests = digests.ok_or_else(|| {
            not(format!(
                "the answer of `GET {url}` has no `digests` object naming its services"
            ))
        })?;
        Ok((digests.keys().cloned().collect(), subscribes_of(&body)))
    }

    /// Sends one request and returns whatever the server answered, including `4xx` and `5xx`.
    ///
    /// # Errors
    /// [`LiveError::Connect`] when there is no answer.
    pub fn send(&self, req: &HttpRequest) -> Result<Answer, LiveError> {
        let url = format!("{}{}", self.base, req.target);
        let mut call = self.agent.request(&req.method, &url);
        for (name, value) in &req.headers {
            call = call.set(name, value);
        }
        let sent = match &req.body {
            Some(body) => call.send_string(body),
            None => call.call(),
        };
        match sent {
            Ok(resp) | Err(ureq::Error::Status(_, resp)) => Ok(read_answer(resp)),
            Err(ureq::Error::Transport(t)) => Err(LiveError::Connect {
                url,
                message: t.to_string(),
            }),
        }
    }

    /// Fills in `ex`, sends it and compares the answer with its expected outcome.
    ///
    /// An example writes down only part of an answer. When the answer agrees with the example
    /// and the reference evaluator answers the same request with an error, the server must give
    /// that whole error: its type, its status, its payload and its violations, in order
    /// (`spec/rules.md` section 12.3).
    ///
    /// # Errors
    /// [`LiveError::Example`] when the example cannot be filled in, [`LiveError::Unsupported`]
    /// when no request can be built for its endpoint and [`LiveError::Connect`] when the
    /// connection fails.
    pub fn run_example(&self, ir: &Ir, ex: &Example) -> Result<ExampleReport, LiveError> {
        if ex.event.is_some() {
            return crate::events::run(self, ir, ex);
        }
        let filled = fill_example(ir, ex)?;
        if let Some(service) = ir.services.get(filled.service.index())
            && !self.serves(&service.name)
        {
            let why = format!(
                "the test handler at {} does not serve `{}`",
                self.base, service.name
            );
            return Ok(ExampleReport {
                name: filled.name,
                verdict: Verdict::Skipped(why),
                outcome: None,
                audited: Vec::new(),
            });
        }
        let req = build_request(ir, &filled, self.dry_run)?;
        let answer = self.send(&req)?;
        let mut report = judge(ir, &filled, self.dry_run, &answer);
        let reported = reference(ir, &filled, &filled.request);
        if report.passed()
            && let Some(mismatch) = reported
                .as_ref()
                .and_then(|r| against_reference(ir, &filled, &r.outcome, &answer))
        {
            report.verdict = Verdict::Failed(mismatch);
        }
        if report.passed()
            && let Some(mismatch) = reported
                .as_ref()
                .and_then(|r| conditional::differences(r, &answer))
        {
            report.verdict = Verdict::Failed(mismatch);
        }
        if report.passed()
            && let Some(mismatch) = self.emitted_differences(ir, ex, &answer)
        {
            report.verdict = Verdict::Failed(mismatch);
        }
        if let Some(reported) = reported {
            // What the server tells its hook about audited rules is judged like the rest of the
            // answer (section 12.4): a server that never reports one must not pass.
            let (audited, exact) = self.audited_reference(ir, &filled, &answer, reported);
            if report.passed()
                && let Some(mismatch) = audited::differences(&audited, &answer, exact)
            {
                report.verdict = Verdict::Failed(mismatch);
            }
            report.audited = audited;
        }
        Ok(report)
    }

    /// Where the events the test handler says it wrote (`Veld-Test-Emitted`) differ from the
    /// `expect emitted` lines of `ex` (`spec/events.md` section 11.1). `None` for an example
    /// without such a line.
    fn emitted_differences(&self, ir: &Ir, ex: &Example, answer: &Answer) -> Option<Mismatch> {
        let expected = ex.emitted.as_ref()?;
        let names: Vec<String> = expected
            .iter()
            .filter_map(|id| ir.decls.get(id.index()))
            .map(|d| d.name.clone())
            .collect();
        crate::emitted::differences(&names, answer)
    }

    /// The audited rules the server must report for `filled`, and whether its list must equal them.
    ///
    /// An example with `returns`, and a dry run, tell the reference everything the server sees. An
    /// example without `returns` that is not a dry run runs the application's real handler: its
    /// success answer is the result that steps 8 and 9 saw, so the reference is given it. When the
    /// answer is not a success there is no result to give, and the server may report more than the
    /// reference knows: it must report at least what the reference does.
    fn audited_reference(
        &self,
        ir: &Ir,
        filled: &FilledExample,
        answer: &Answer,
        reported: Reported,
    ) -> (Vec<Violation>, bool) {
        if self.dry_run || filled.request.result.is_some() {
            return (reported.audited, true);
        }
        if (200..300).contains(&answer.status)
            && let Some(body) = answer.json()
        {
            let request = Request {
                result: Some(body),
                ..filled.request.clone()
            };
            if let Some(with_result) = reference(ir, filled, &request) {
                return (with_result.audited, true);
            }
        }
        (reported.audited, false)
    }
}

/// What the reference evaluator does with `request` of `filled`: `None` when it cannot run it.
fn reference(ir: &Ir, filled: &FilledExample, request: &Request) -> Option<Reported> {
    let service = ir.services.get(filled.service.index())?;
    let pipeline = Pipeline::new(ir, service, &filled.endpoint).ok()?;
    pipeline.run_reported(request, OutputMode::Strict).ok()
}

/// Where the answer differs from the error the reference evaluator gives for the same request.
/// `None` when the reference passes the request, cannot run it, or agrees.
fn against_reference(
    ir: &Ir,
    filled: &FilledExample,
    outcome: &Outcome,
    answer: &Answer,
) -> Option<Mismatch> {
    let service = ir.services.get(filled.service.index())?;
    let endpoint = service
        .endpoints
        .iter()
        .find(|e| e.name == filled.endpoint)?;
    if matches!(outcome, Outcome::Passes) {
        return None;
    }
    let called = Called { service, endpoint };
    let differences = judge_outcome(ir, called, outcome, answer);
    if differences.is_empty() {
        return None;
    }
    Some(Mismatch {
        expected: format!(
            "what the reference evaluator answers: {}",
            describe_actual(outcome, None)
        ),
        actual: describe_answer(ir, answer),
        differences,
    })
}

/// The `subscribes` member of a probe: `None` when it is missing or is not an object of lists of
/// strings.
fn subscribes_of(body: &Json) -> Option<BTreeMap<String, Vec<String>>> {
    let map = body.get("subscribes")?.as_object()?;
    map.iter()
        .map(|(service, topics)| {
            let topics = topics.as_array()?;
            let topics: Option<Vec<String>> = topics
                .iter()
                .map(|t| t.as_str().map(str::to_owned))
                .collect();
            Some((service.clone(), topics?))
        })
        .collect()
}

fn read_answer(resp: ureq::Response) -> Answer {
    let status = resp.status();
    let headers = resp
        .headers_names()
        .into_iter()
        .filter_map(|name| {
            let value = resp.header(&name)?.to_owned();
            Some((name.to_ascii_lowercase(), value))
        })
        .collect();
    let body = resp.into_string().unwrap_or_default();
    Answer {
        status,
        headers,
        body,
    }
}
