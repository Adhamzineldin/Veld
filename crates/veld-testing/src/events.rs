//! Event examples against a live test handler (`spec/events.md` section 10.9).
//!
//! The runner builds the envelope of the filled-in event with the reference encoder, sends it
//! to `POST /__veld/test/event` of a test handler that subscribes to the channel, and compares
//! what the handler answers with what the reference evaluator and the `expect` lines say.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value as Json;
use veld_ir::{ChannelId, EventExpect, Example, ExpectedCall, Ir, ServiceId};
use veld_rules::envelope::{Inputs, MEDIA_TYPE, encode, encode_unchecked, source_of};
use veld_rules::example::{
    Delivery, FilledEvent, RejectReason, constraint_findings, describe_expectation,
    fill_event_example,
};
use veld_rules::helpers::instant::parse_datetime;
use veld_rules::{DEFAULT_CLOCK, Difference, ExampleReport, Mismatch, Verdict};

use crate::error::LiveError;
use crate::judge::Answer;
use crate::live::Live;
use crate::request::HttpRequest;

/// The path a test handler that has subscribers answers to take one message.
pub const EVENT_PATH: &str = "/__veld/test/event";
/// The topic of the message.
pub const TOPIC: &str = "Veld-Test-Topic";
/// The key of the message, when it has one.
pub const KEY: &str = "Veld-Test-Key";
/// How often the message is delivered: `1` or `2`.
pub const DELIVERIES: &str = "Veld-Test-Deliveries";

/// The `id` of the message of every live event example. Each request gets a fresh inbox.
const MESSAGE_ID: &str = "veld-test-1";

/// What a test handler answers to `POST /__veld/test/event`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventAnswer {
    /// `handled`, `ignored` or `rejected`.
    pub outcome: String,
    /// `malformed`, `shape` or `rule` for a rejected message.
    pub reason: Option<String>,
    /// How often each `Service.handler` ran.
    pub called: BTreeMap<String, u64>,
    /// `(rule id, path)` of every violation.
    pub violations: BTreeSet<(String, String)>,
    pub dead_letters: u64,
}

impl EventAnswer {
    /// Reads the answer of section 10.9.
    ///
    /// # Errors
    /// What is wrong with the body, as text.
    pub fn parse(answer: &Answer) -> Result<EventAnswer, String> {
        if answer.status != 200 {
            return Err(format!(
                "the test handler answered HTTP {}: {}",
                answer.status,
                answer.body.chars().take(200).collect::<String>()
            ));
        }
        let body: Json =
            serde_json::from_str(&answer.body).map_err(|_| "the answer is not JSON".to_owned())?;
        let outcome = body
            .get("outcome")
            .and_then(Json::as_str)
            .filter(|o| ["handled", "ignored", "rejected"].contains(o))
            .ok_or("the answer has no `outcome` of `handled`, `ignored` or `rejected`")?
            .to_owned();
        let called = body
            .get("called")
            .and_then(Json::as_object)
            .ok_or("the answer has no `called` object")?
            .iter()
            .map(|(k, v)| v.as_u64().map(|n| (k.clone(), n)))
            .collect::<Option<BTreeMap<_, _>>>()
            .ok_or("a count of `called` is not a number")?;
        let violations = body
            .get("violations")
            .and_then(Json::as_array)
            .map(|vs| vs.iter().filter_map(violation_key).collect())
            .unwrap_or_default();
        Ok(EventAnswer {
            outcome,
            reason: body.get("reason").and_then(Json::as_str).map(str::to_owned),
            called,
            violations,
            dead_letters: body.get("deadLetters").and_then(Json::as_u64).unwrap_or(0),
        })
    }

    /// How often `service.handler` ran.
    #[must_use]
    pub fn count(&self, service: &str, handler: &str) -> u64 {
        self.called
            .get(&format!("{service}.{handler}"))
            .copied()
            .unwrap_or(0)
    }

    fn describe(&self) -> String {
        let mut text = self.outcome.clone();
        if let Some(r) = &self.reason {
            text.push_str(&format!(" ({r})"));
        }
        let ran: Vec<String> = self
            .called
            .iter()
            .filter(|(_, n)| **n > 0)
            .map(|(h, n)| format!("{h} x{n}"))
            .collect();
        if !ran.is_empty() {
            text.push_str(&format!(", called {}", ran.join(", ")));
        }
        text
    }
}

fn violation_key(v: &Json) -> Option<(String, String)> {
    let rule = v.get("rule")?.as_str()?.to_owned();
    let path = v.get("path")?.as_str()?.to_owned();
    Some((rule, path))
}

/// What the live answer must be, worked out from the reference evaluator.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Wanted {
    outcome: &'static str,
    reason: Option<&'static str>,
    violations: Option<BTreeSet<(String, String)>>,
    /// The handlers (`Service.handler`) that must have run, among those the test handler serves.
    called: BTreeSet<String>,
}

/// Runs `ex`, an event example, against `live`.
pub(crate) fn run(live: &Live, ir: &Ir, ex: &Example) -> Result<ExampleReport, LiveError> {
    let filled = fill_event_example(ir, ex)?;
    let topic = ir.channel(filled.channel).topic.clone();
    let served = live.subscribers_of(ir, &topic);
    if served.is_empty() {
        return Ok(skipped(&ex.name, no_subscriber(live, &topic)));
    }
    let Some(expect) = ex.expect.as_ref() else {
        return Ok(skipped(
            &ex.name,
            "an event example has no `expect` line".to_owned(),
        ));
    };
    let deliveries = if has_once(expect) { 2 } else { 1 };
    let delivery = veld_rules::example::deliver(ir, filled.channel, filled.event, &filled.value);
    let req = request(ir, &filled, &topic, deliveries)?;
    let answer = live.send(&req)?;
    let got = match EventAnswer::parse(&answer) {
        Ok(a) => a,
        Err(why) => return Ok(failed(ex, ir, expect, why, Vec::new())),
    };
    let wanted = wanted(ir, &filled, &delivery, &served);
    let mut differences = against_reference(&wanted, &got);
    // The `expect` lines are about the whole workspace; a handler that serves no `on` line for
    // the event is only asked to ignore it (spec/events.md 10.9).
    if handles(ir, &filled, &served) {
        differences.extend(against_expectation(ir, expect, &got, &served));
    }
    if differences.is_empty() {
        return Ok(ExampleReport {
            name: ex.name.clone(),
            verdict: Verdict::Passed,
            outcome: None,
            audited: Vec::new(),
        });
    }
    Ok(failed(ex, ir, expect, got.describe(), differences))
}

fn no_subscriber(live: &Live, topic: &str) -> String {
    if live.serves_events() {
        format!(
            "the test handler at {} has no subscriber of `{topic}`",
            live.base_url()
        )
    } else {
        format!(
            "the test handler at {} does not serve events (its probe has no `subscribes`)",
            live.base_url()
        )
    }
}

fn skipped(name: &str, why: String) -> ExampleReport {
    ExampleReport {
        name: name.to_owned(),
        verdict: Verdict::Skipped(why),
        outcome: None,
        audited: Vec::new(),
    }
}

fn failed(
    ex: &Example,
    ir: &Ir,
    expect: &EventExpect,
    actual: String,
    differences: Vec<Difference>,
) -> ExampleReport {
    ExampleReport {
        name: ex.name.clone(),
        verdict: Verdict::Failed(Mismatch {
            expected: describe_expectation(ir, expect),
            actual,
            differences,
        }),
        outcome: None,
        audited: Vec::new(),
    }
}

fn has_once(expect: &EventExpect) -> bool {
    matches!(expect, EventExpect::Calls(calls) if calls.iter().any(|c| c.once))
}

/// The request of section 10.9: the message bytes as the body and the three headers.
fn request(
    ir: &Ir,
    filled: &FilledEvent,
    topic: &str,
    deliveries: u8,
) -> Result<HttpRequest, LiveError> {
    let bytes = message(ir, filled)?;
    let key = serde_json::from_str::<Json>(&bytes)
        .ok()
        .and_then(|m| m.get("subject").and_then(Json::as_str).map(str::to_owned));
    let mut headers = vec![
        ("Content-Type".to_owned(), MEDIA_TYPE.to_owned()),
        ("Accept".to_owned(), "application/json".to_owned()),
        (TOPIC.to_owned(), topic.to_owned()),
    ];
    headers.extend(key.map(|k| (KEY.to_owned(), k)));
    headers.push((DELIVERIES.to_owned(), deliveries.to_string()));
    Ok(HttpRequest {
        method: "POST".to_owned(),
        target: EVENT_PATH.to_owned(),
        headers,
        body: Some(bytes),
    })
}

/// The envelope of the event, as the reference encoder writes it. A value that a publisher would
/// refuse is written without the checks, so that a consumer can be shown it.
fn message(ir: &Ir, filled: &FilledEvent) -> Result<String, LiveError> {
    let source = publisher_source(ir, filled.channel);
    let time_ms = parse_datetime(DEFAULT_CLOCK).unwrap_or_default();
    let inputs = Inputs {
        id: MESSAGE_ID,
        source: &source,
        time_ms,
    };
    let bytes = encode(ir, filled.channel, filled.event, &filled.value, &inputs)
        .or_else(|_| encode_unchecked(ir, filled.channel, filled.event, &filled.value, &inputs))
        .map_err(|e| LiveError::Unsupported {
            example: filled.name.clone(),
            reason: format!("no message can be built for the event: {e}"),
        })?;
    String::from_utf8(bytes).map_err(|_| LiveError::Unsupported {
        example: filled.name.clone(),
        reason: "the message is not UTF-8".to_owned(),
    })
}

/// The `source` of the service that publishes the channel; any service when nobody does.
fn publisher_source(ir: &Ir, channel: ChannelId) -> String {
    let publisher = ir
        .services
        .iter()
        .find(|s| s.published_channels().any(|c| c == channel))
        .or_else(|| ir.services.first());
    publisher.map_or_else(|| "veld:test/Test".to_owned(), |s| source_of(ir, s.id))
}

/// What the test handler must answer, by the reference evaluator. An event that is delivered
/// offline and breaks a constraint is rejected as `shape` by a generated consumer, which the
/// reference does not see: that is what the example expecting `rejected` is about.
fn wanted(
    ir: &Ir,
    filled: &FilledEvent,
    delivery: &Delivery,
    served: &BTreeSet<ServiceId>,
) -> Wanted {
    // A handler that serves no `on` line for the event ignores it (step 2), whatever the
    // reference says about the whole workspace.
    if !handles(ir, filled, served) {
        return Wanted {
            outcome: "ignored",
            reason: None,
            violations: Some(BTreeSet::new()),
            called: BTreeSet::new(),
        };
    }
    // A generated consumer checks the shape and the constraints (step 3) before the invariants
    // (step 4): a value that breaks a constraint is rejected as `shape` whatever the rules say.
    if !constraint_findings(ir, filled.event, &filled.value).is_empty() {
        return Wanted {
            outcome: "rejected",
            reason: Some("shape"),
            violations: None,
            called: BTreeSet::new(),
        };
    }
    match delivery {
        Delivery::Rejected {
            reason, violations, ..
        } => Wanted {
            outcome: "rejected",
            reason: Some(match reason {
                RejectReason::Shape => "shape",
                RejectReason::Rule => "rule",
            }),
            violations: (*reason == RejectReason::Rule).then(|| {
                violations
                    .iter()
                    .map(|v| (v.rule_id.clone(), v.path.clone()))
                    .collect()
            }),
            called: BTreeSet::new(),
        },
        Delivery::Handled { called } => handled(ir, called, served),
    }
}

/// Whether a served service has an `on` line for the event on the channel.
fn handles(ir: &Ir, filled: &FilledEvent, served: &BTreeSet<ServiceId>) -> bool {
    served.iter().any(|s| {
        ir.service(*s)
            .subscriptions
            .iter()
            .filter(|sub| sub.channel == filled.channel)
            .any(|sub| sub.handlers.iter().any(|h| h.event == filled.event))
    })
}

fn handled(ir: &Ir, called: &[(ServiceId, String)], served: &BTreeSet<ServiceId>) -> Wanted {
    Wanted {
        outcome: "handled",
        reason: None,
        violations: None,
        called: called
            .iter()
            .filter(|(s, _)| served.contains(s))
            .map(|(s, h)| format!("{}.{h}", ir.service(*s).name))
            .collect(),
    }
}

fn difference(path: &str, expected: impl ToString, actual: impl ToString) -> Difference {
    Difference {
        path: path.to_owned(),
        expected: expected.to_string(),
        actual: actual.to_string(),
    }
}

fn against_reference(want: &Wanted, got: &EventAnswer) -> Vec<Difference> {
    let mut out = Vec::new();
    if got.outcome != want.outcome {
        out.push(difference("outcome", want.outcome, &got.outcome));
    }
    if want.outcome == "rejected"
        && got.outcome == "rejected"
        && got.reason.as_deref() != want.reason
    {
        out.push(difference(
            "reason",
            want.reason.unwrap_or("none"),
            got.reason.as_deref().unwrap_or("none"),
        ));
    }
    if let Some(v) = &want.violations
        && v != &got.violations
    {
        out.push(difference(
            "violations",
            format!("{v:?}"),
            format!("{:?}", got.violations),
        ));
    }
    let ran: BTreeSet<String> = got
        .called
        .iter()
        .filter(|(_, n)| **n > 0)
        .map(|(h, _)| h.clone())
        .collect();
    if want.outcome != "rejected" && ran != want.called {
        out.push(difference(
            "called",
            format!("{:?}", want.called),
            format!("{ran:?}"),
        ));
    }
    if want.outcome == "rejected" && !ran.is_empty() {
        out.push(difference("called", "no handler", format!("{ran:?}")));
    }
    out
}

/// The `expect` lines against the answer. A line about a service the test handler does not
/// serve cannot be judged by it and is left out.
fn against_expectation(
    ir: &Ir,
    expect: &EventExpect,
    got: &EventAnswer,
    served: &BTreeSet<ServiceId>,
) -> Vec<Difference> {
    match expect {
        EventExpect::Rejected if got.outcome != "rejected" => {
            vec![difference("event", "rejected", got.describe())]
        }
        EventExpect::Rejected => Vec::new(),
        EventExpect::Calls(calls) => calls
            .iter()
            .filter(|c| served.contains(&c.service))
            .filter_map(|c| line(ir, c, got))
            .collect(),
    }
}

/// `called` wants at least one run, `called once` exactly one (the event came twice), and
/// `not called` none.
fn line(ir: &Ir, c: &ExpectedCall, got: &EventAnswer) -> Option<Difference> {
    let service = ir.service(c.service).name.as_str();
    let n = got.count(service, &c.handler);
    let path = format!("{service}.{}", c.handler);
    let (ok, wanted) = match (c.called, c.once) {
        (true, true) => (n == 1, "called once"),
        (true, false) => (n >= 1, "called"),
        (false, _) => (n == 0, "not called"),
    };
    (!ok).then(|| difference(&path, wanted, format!("run {n} times")))
}
