//! The relay of `spec/events.md` section 11.1 as a pure function.
//!
//! `run<Service>OutboxRelay(outbox, transport, options)` takes up to `limit` (default 100)
//! pending entries, publishes each in the order `pending` gave them, and calls `done` after the
//! transport accepted it. It stops at the first entry that fails and answers how many it sent;
//! the failure goes to `onError`. The options of the channel (section 10.6) go to the transport
//! with every `publish` and do not change what the relay does.

use super::OutboxEntry;

/// The default of `limit`.
pub const DEFAULT_LIMIT: usize = 100;

/// What happens to one entry: the answer of the transport and of `done`, or the death of the
/// process in between.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// The transport accepts the message and `done` succeeds.
    Accepted,
    /// `publish` fails with this text: the entry is not sent and stays pending; the relay stops
    /// and reports the failure.
    PublishFails(String),
    /// The transport accepts the message and `done` fails with this text: the entry is sent and
    /// stays pending, so it is sent again by the next run; the relay stops and reports the
    /// failure.
    DoneFails(String),
    /// The transport accepts the message and the process ends before `done`: the entry is sent
    /// and stays pending; nothing is reported.
    CrashAfterPublish,
}

/// What one run of the relay did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relay {
    /// The ids of the entries the transport accepted, in publish order. An entry that stays
    /// pending after this (`DoneFails`, `CrashAfterPublish`) will be published again.
    pub published: Vec<String>,
    /// The ids `done` was called with successfully, in order.
    pub done: Vec<String>,
    /// How many entries the run sent: the answer of `run<Service>OutboxRelay`, the length of
    /// `published`.
    pub sent: usize,
    /// The failure reported to `onError`, with the id of its entry; `None` for a run that
    /// stopped at the limit, at the end of the pending entries or in a crash.
    pub failure: Option<(String, String)>,
}

/// Runs the relay over `pending` (what `pending(limit)` answered) with the steps of `script`:
/// entry `i` meets `script[i]`, and an entry past the end of the script is [`Step::Accepted`].
/// At most `limit` entries are looked at.
#[must_use]
pub fn relay(pending: &[OutboxEntry], limit: usize, script: &[Step]) -> Relay {
    let mut run = Relay {
        published: Vec::new(),
        done: Vec::new(),
        sent: 0,
        failure: None,
    };
    for (i, entry) in pending.iter().take(limit).enumerate() {
        match script.get(i).unwrap_or(&Step::Accepted) {
            Step::Accepted => {
                run.published.push(entry.id.clone());
                run.done.push(entry.id.clone());
            }
            Step::PublishFails(why) => {
                run.failure = Some((entry.id.clone(), why.clone()));
                break;
            }
            Step::DoneFails(why) => {
                run.published.push(entry.id.clone());
                run.failure = Some((entry.id.clone(), why.clone()));
                break;
            }
            Step::CrashAfterPublish => {
                run.published.push(entry.id.clone());
                break;
            }
        }
    }
    run.sent = run.published.len();
    run
}
