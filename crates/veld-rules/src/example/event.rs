//! Event examples (`spec/events.md` section 12): one delivery of one filled-in event on one
//! channel.
//!
//! [`deliver`] applies steps 3 and 4 of section 8 (decode and shape, then the invariants, with a
//! matching rules digest) to the event and works out which `Service.handler` pairs are called
//! across every subscription of the channel. [`run_event_example`] compares that with the
//! `expect` lines of the example. Handlers are not run.

use std::collections::BTreeMap;

use serde_json::Value as Json;
use veld_ir::{
    ChannelId, Constraints, DeclId, EventExpect, Example, ExampleEvent, Ir, ServiceId, Type,
};

use super::fill::Filler;
use super::matching::Difference;
use super::report::{ExampleReport, Mismatch, Verdict};
use super::{DEFAULT_CLOCK, ExampleError, resolve_value};
use crate::check::{Options, Violation, check_type};
use crate::helpers::instant::parse_datetime;

/// Why a delivered message is rejected (`spec/events.md` section 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// The value does not have the shape of the event.
    Shape,
    /// An invariant is violated, or could not be evaluated.
    Rule,
}

impl RejectReason {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            RejectReason::Shape => "shape",
            RejectReason::Rule => "rule",
        }
    }
}

/// What one delivery of one event does.
#[derive(Debug, Clone, PartialEq)]
pub enum Delivery {
    /// No handler is called.
    Rejected {
        reason: RejectReason,
        /// What was wrong, as text.
        detail: String,
        violations: Vec<Violation>,
    },
    /// The `Service.handler` pairs that are called, services in IR order, handlers in source
    /// order. A subscription that has no `on` line for the event ignores it.
    Handled { called: Vec<(ServiceId, String)> },
}

/// An event example with everything filled in.
#[derive(Debug, Clone, PartialEq)]
pub struct FilledEvent {
    pub name: String,
    pub event: DeclId,
    pub channel: ChannelId,
    /// The event in wire form, defaults applied: what a publisher would send.
    pub value: Json,
}

/// Delivers the wire value `value` of event `event` on `channel` (steps 3 and 4 of section 8).
#[must_use]
pub fn deliver(ir: &Ir, channel: ChannelId, event: DeclId, value: &Json) -> Delivery {
    let ty = Type::Decl(event);
    match check_type(ir, &ty, value, Options::default()) {
        Err(e) => {
            return Delivery::Rejected {
                reason: RejectReason::Shape,
                detail: e.to_string(),
                violations: Vec::new(),
            };
        }
        Ok((_, violations)) if !violations.is_empty() => {
            let names: Vec<String> = violations.iter().map(|v| v.rule_name.clone()).collect();
            return Delivery::Rejected {
                reason: RejectReason::Rule,
                detail: format!("rule {}", names.join(", ")),
                violations,
            };
        }
        Ok(_) => {}
    }
    let called = ir
        .services
        .iter()
        .flat_map(|s| {
            s.subscriptions
                .iter()
                .filter(move |sub| sub.channel == channel)
                .flat_map(|sub| sub.handlers.iter())
                .filter(move |h| h.event == event)
                .map(move |h| (s.id, h.name.clone()))
        })
        .collect();
    Delivery::Handled { called }
}

fn event_of(ex: &Example) -> Result<&ExampleEvent, ExampleError> {
    ex.event.as_ref().ok_or_else(|| ExampleError::WrongForm {
        example: ex.name.clone(),
        reason: "it is not an event example".to_owned(),
    })
}

/// Fills in what an event example leaves out (section 12.2, like a parameter) and resolves its
/// value to wire JSON.
///
/// # Errors
/// [`ExampleError`]: `E0953` when a value cannot be filled in or a filled-in value breaks an
/// invariant, and the errors of an example that does not fit the IR.
pub fn fill_event_example(ir: &Ir, ex: &Example) -> Result<FilledEvent, ExampleError> {
    let spec = event_of(ex)?;
    let clock_ms = parse_datetime(DEFAULT_CLOCK).ok_or_else(|| ExampleError::BadClock {
        example: ex.name.clone(),
        text: DEFAULT_CLOCK.to_owned(),
    })?;
    let given = resolve_value(&ex.name, &BTreeMap::new(), &spec.value)?;
    let ty = Type::Decl(spec.decl);
    let mut filler = Filler::new(ir, &ex.name, clock_ms);
    filler.begin();
    let value = filler.fill(&ty, &Constraints::default(), Some(&given), "$")?;
    if let Ok((_, violations)) = check_type(ir, &ty, &value, Options::default()) {
        for v in violations {
            let at = |f: &String| format!("{}.{f}", v.path);
            let fields: Vec<String> = v
                .fields
                .iter()
                .filter(|f| filler.filled.contains(&at(f)))
                .map(|f| at(f).trim_start_matches("$.").to_owned())
                .collect();
            if !fields.is_empty() {
                return Err(ExampleError::BreaksInvariant {
                    example: ex.name.clone(),
                    rule: v.rule_name,
                    rule_id: v.rule_id,
                    message: v.message,
                    fields,
                });
            }
        }
    }
    Ok(FilledEvent {
        name: ex.name.clone(),
        event: spec.decl,
        channel: spec.channel,
        value,
    })
}

/// Runs an event example offline: fills it in, delivers the event and compares the handlers
/// that are called with the `expect` lines.
///
/// # Errors
/// [`ExampleError`] when the example cannot be filled in or does not fit the IR. A mismatch is
/// not an error: it is [`Verdict::Failed`].
pub fn run_event_example(ir: &Ir, ex: &Example) -> Result<ExampleReport, ExampleError> {
    let filled = fill_event_example(ir, ex)?;
    let delivery = deliver(ir, filled.channel, filled.event, &filled.value);
    let expect = ex.expect.as_ref().ok_or_else(|| ExampleError::WrongForm {
        example: ex.name.clone(),
        reason: "an event example has no `expect` line".to_owned(),
    })?;
    let verdict = match live_only(ir, expect, &filled, &delivery) {
        Some(why) => Verdict::Skipped(why),
        None => compare(ir, expect, &delivery),
    };
    Ok(ExampleReport {
        name: ex.name.clone(),
        verdict,
        outcome: None,
        audited: Vec::new(),
    })
}

/// An example that expects `rejected` and is delivered offline because of a constraint, which the
/// reference does not check (`spec/events.md` section 12): only a live run can tell. The reason.
fn live_only(
    ir: &Ir,
    expect: &EventExpect,
    filled: &FilledEvent,
    delivery: &Delivery,
) -> Option<String> {
    if !matches!(
        (expect, delivery),
        (EventExpect::Rejected, Delivery::Handled { .. })
    ) {
        return None;
    }
    let found = super::constraints::findings(ir, filled.event, &filled.value);
    let first = found.first()?;
    let verb = if first.decided { "breaks" } else { "may break" };
    Some(format!(
        "live only: `{}` {verb} {}; a generated consumer rejects it as `shape`, the reference evaluator does not check constraints",
        first.path, first.what
    ))
}

fn handler_name(ir: &Ir, service: ServiceId, handler: &str) -> String {
    let s = ir
        .services
        .get(service.index())
        .map_or("?", |s| s.name.as_str());
    format!("{s}.{handler}")
}

/// The delivery as text, as a [`Mismatch`] prints it.
#[must_use]
pub fn describe_delivery(ir: &Ir, delivery: &Delivery) -> String {
    match delivery {
        Delivery::Rejected { reason, detail, .. } => {
            format!("rejected ({}: {detail})", reason.as_str())
        }
        Delivery::Handled { called } if called.is_empty() => "no handler is called".to_owned(),
        Delivery::Handled { called } => {
            let names: Vec<String> = called
                .iter()
                .map(|(s, h)| handler_name(ir, *s, h))
                .collect();
            format!("called {}", names.join(", "))
        }
    }
}

/// The expectation as text, as a [`Mismatch`] prints it.
#[must_use]
pub fn describe_expectation(ir: &Ir, expect: &EventExpect) -> String {
    match expect {
        EventExpect::Rejected => "rejected".to_owned(),
        EventExpect::Calls(calls) => calls
            .iter()
            .map(|c| {
                let not = if c.called { "" } else { " not" };
                let once = if c.once { " once" } else { "" };
                format!(
                    "{}{not} called{once}",
                    handler_name(ir, c.service, &c.handler)
                )
            })
            .collect::<Vec<_>>()
            .join("; "),
    }
}

fn compare(ir: &Ir, expect: &EventExpect, delivery: &Delivery) -> Verdict {
    let mut differences = Vec::new();
    match (expect, delivery) {
        (EventExpect::Rejected, Delivery::Rejected { .. }) => {}
        (EventExpect::Rejected, Delivery::Handled { .. }) => differences.push(Difference {
            path: "event".to_owned(),
            expected: "rejected".to_owned(),
            actual: describe_delivery(ir, delivery),
        }),
        (EventExpect::Calls(calls), _) => {
            for c in calls {
                let was_called = match delivery {
                    Delivery::Rejected { .. } => false,
                    Delivery::Handled { called } => called
                        .iter()
                        .any(|(s, h)| *s == c.service && h == &c.handler),
                };
                if was_called != c.called {
                    differences.push(Difference {
                        path: handler_name(ir, c.service, &c.handler),
                        expected: if c.called { "called" } else { "not called" }.to_owned(),
                        actual: if was_called { "called" } else { "not called" }.to_owned(),
                    });
                }
            }
        }
    }
    if differences.is_empty() {
        return Verdict::Passed;
    }
    Verdict::Failed(Mismatch {
        expected: describe_expectation(ir, expect),
        actual: describe_delivery(ir, delivery),
        differences,
    })
}
