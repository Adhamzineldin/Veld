//! What the consumer decides after step 4 of `spec/events.md` section 8: how often the handler
//! runs, what is copied to the dead-letter topic and what the transport is told (sections 10.4
//! and 10.5). It is a pure function of the channel's policy and of what the handler answers, so
//! one table of cases (`tests/wire/events/retry.json`) holds every generator to it.

use veld_ir::{Backoff, Channel, Retry};

use super::dead_letter::Reason;

/// The options of a channel that a consumer carries out (`@retry`, `@deadLetter`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Policy {
    pub retry: Option<Retry>,
    pub dead_letter: bool,
}

impl Policy {
    /// The policy of `channel`.
    #[must_use]
    pub fn of(channel: &Channel) -> Policy {
        Policy {
            retry: channel.retry,
            dead_letter: channel.dead_letter,
        }
    }
}

/// `retryDelayMs` and `retryMaxDelayMs`, the options of the consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    pub delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            delay_ms: 1000,
            max_delay_ms: 60_000,
        }
    }
}

/// What a run of the handler came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerResult {
    Returned,
    Failed,
}

/// What the consumer tells the transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Acknowledge.
    Done,
    /// Leave the message unacknowledged: the transport delivers it again.
    Again,
}

impl Answer {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Answer::Done => "done",
            Answer::Again => "again",
        }
    }
}

/// One failed run, as `onHandlerError` sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailedRun {
    /// `meta.attempt` of the run: the transport's attempt plus the runs already made.
    pub attempt: u32,
    /// True when the message will not be handled again by this consumer.
    pub last: bool,
}

/// The outcome of one delivery of one message to a consumer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settlement {
    /// How many times steps 5 and 6 ran (the inbox and the handler).
    pub runs: u32,
    /// Every failed run, in order.
    pub failures: Vec<FailedRun>,
    /// The waits before the second run, the third and so on, in milliseconds.
    pub waits_ms: Vec<u64>,
    /// Why a copy is published to `<topic>.dlq`, when it is.
    pub copy: Option<Reason>,
    /// The number of runs written to the copy as `veldattempts`.
    pub copy_attempts: u32,
    /// What the consumer answers, supposing that publishing the copy succeeded.
    pub answer: Answer,
}

impl Settlement {
    /// What the consumer answers when it needed a copy and publishing it failed
    /// (`spec/events.md` section 10.5: the copy comes before the acknowledgement).
    #[must_use]
    pub fn answer_if_the_copy_fails(&self) -> Answer {
        if self.copy.is_some() {
            Answer::Again
        } else {
            self.answer
        }
    }
}

/// A message that is ignored (step 2): acknowledged, nothing else.
#[must_use]
pub fn ignored() -> Settlement {
    Settlement {
        runs: 0,
        failures: Vec::new(),
        waits_ms: Vec::new(),
        copy: None,
        copy_attempts: 0,
        answer: Answer::Done,
    }
}

/// A rejected message (steps 1, 3 and 4): acknowledged, so that it does not block the stream,
/// and copied when the channel has `@deadLetter`. No run, so `veldattempts` is 0.
#[must_use]
pub fn rejected(policy: Policy, reason: Reason) -> Settlement {
    Settlement {
        copy: policy.dead_letter.then_some(reason),
        ..ignored()
    }
}

/// The wait before run `k + 1`, after run `k` failed (`k` from 1): `delay_ms` for `fixed`,
/// `delay_ms * 2^(k-1)` for `exponential`, never more than `max_delay_ms`.
#[must_use]
pub fn wait_ms(backoff: Backoff, k: u32, timing: Timing) -> u64 {
    let wait = match backoff {
        Backoff::Fixed => timing.delay_ms,
        Backoff::Exponential => {
            let doublings = k.saturating_sub(1).min(63);
            timing.delay_ms.saturating_mul(1_u64 << doublings)
        }
    };
    wait.min(timing.max_delay_ms)
}

/// A delivered message (step 6): the handler runs until it returns, or until the retries of the
/// channel are used up (section 10.4).
///
/// `results[i]` is what run `i + 1` comes to. When a run has no entry, the last entry repeats; an
/// empty list is a handler that returns. `transport_attempt` is the attempt the transport
/// reports for this delivery (it starts at 1).
#[must_use]
pub fn handled(
    policy: Policy,
    timing: Timing,
    transport_attempt: u32,
    results: &[HandlerResult],
) -> Settlement {
    let allowed = policy.retry.map_or(1, |r| r.max.saturating_add(1));
    let mut out = ignored();
    for run in 1..=allowed {
        out.runs = run;
        let result = nth(results, run);
        if result == HandlerResult::Returned {
            return out;
        }
        // Without `@retry` the transport delivers the message again, so no run is the last.
        let last = policy.retry.is_some() && run == allowed;
        out.failures.push(FailedRun {
            attempt: transport_attempt.saturating_add(run - 1),
            last,
        });
        match (policy.retry, last) {
            (Some(r), false) => out.waits_ms.push(wait_ms(r.backoff, run, timing)),
            (Some(_), true) => return exhausted(out, policy),
            // Without `@retry` the transport delivers the message again by its own policy.
            (None, _) => {
                out.answer = Answer::Again;
                return out;
            }
        }
    }
    out
}

fn nth(results: &[HandlerResult], run: u32) -> HandlerResult {
    let index = usize::try_from(run.saturating_sub(1)).unwrap_or(usize::MAX);
    results
        .get(index)
        .or_else(|| results.last())
        .copied()
        .unwrap_or(HandlerResult::Returned)
}

/// The last run failed under `@retry`: the message is copied with `@deadLetter` and gone without.
fn exhausted(mut out: Settlement, policy: Policy) -> Settlement {
    if policy.dead_letter {
        out.copy = Some(Reason::Handler);
        out.copy_attempts = out.runs;
    }
    out.answer = Answer::Done;
    out
}
