//! The script of a saga run: what every call answers, and when the orchestrator is stopped.
//!
//! The reference orchestrator has no network and no clock. The world it runs in is this file:
//! the answer of the n-th attempt of every call, how long each answer takes, and the points at
//! which the process dies and is started again from the saved state.
//!
//! ```json
//! {
//!   "steps": { "hold": [ { "kind": "ok", "value": { "id": "h-1" } } ] },
//!   "undos": { "hold": [ { "kind": "none" }, { "kind": "ok" } ] },
//!   "stops": [ { "after_save": 2 }, { "after_call": 3, "down_ms": 1000 } ]
//! }
//! ```

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

/// The answer of one attempt of one call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Answer {
    /// A success. `value` is the wire value of the success body (absent for no body).
    Ok {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<Json>,
        #[serde(default, skip_serializing_if = "is_zero")]
        after_ms: i64,
    },
    /// An error. `decl` is the name of the error declaration, `data` its fields in wire form and
    /// `status` the status it is answered with: the one the endpoint called declares for it
    /// unless the script says another. A status of 500 or more is no answer.
    Error {
        decl: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data: Option<Json>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<u16>,
        #[serde(default, skip_serializing_if = "is_zero")]
        after_ms: i64,
    },
    /// No answer: the connection fails or times out after `after_ms`.
    None {
        #[serde(default, skip_serializing_if = "is_zero")]
        after_ms: i64,
    },
}

fn is_zero(n: &i64) -> bool {
    *n == 0
}

impl Answer {
    /// A success without a value, answered at once.
    #[must_use]
    pub fn ok() -> Answer {
        Answer::Ok {
            value: None,
            after_ms: 0,
        }
    }

    /// A call that never answers.
    #[must_use]
    pub fn none() -> Answer {
        Answer::None { after_ms: 0 }
    }

    /// How long the attempt takes before it answers or fails, in milliseconds.
    #[must_use]
    pub fn after_ms(&self) -> i64 {
        match self {
            Answer::Ok { after_ms, .. }
            | Answer::Error { after_ms, .. }
            | Answer::None { after_ms } => (*after_ms).max(0),
        }
    }
}

/// A point at which the orchestrator dies. It is started again from the last saved state after
/// `down_ms` milliseconds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stop {
    /// Dies right after the n-th save of the run (1 is the save that creates the state).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_save: Option<usize>,
    /// Dies right after the n-th call of the run was made: the call has had its effect, and
    /// its answer is lost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_call: Option<usize>,
    /// How long the process is down.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub down_ms: i64,
}

/// What every call answers, and when the orchestrator is stopped.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Script {
    /// The attempts of the call of each step, by step name. The n-th attempt of the call gets
    /// the n-th answer; once the list is used up the last answer repeats. Attempts of the same
    /// call are counted across the step itself and the calls that learn what it did. A step with
    /// no entry answers a success without a value.
    #[serde(default)]
    pub steps: BTreeMap<String, Vec<Answer>>,
    /// The same for the compensation of each step. No entry: a success.
    #[serde(default)]
    pub undos: BTreeMap<String, Vec<Answer>>,
    /// The points at which the process dies, each used once.
    #[serde(default)]
    pub stops: Vec<Stop>,
    /// A saga with the id of the request already exists in the store. The request then starts
    /// nothing and makes no call: it is answered `409 Conflict` or with the saved answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub existing: Option<Existing>,
}

/// A saga that is in the store when the request arrives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Existing {
    /// The fingerprint it was started with.
    pub fingerprint: String,
    pub status: super::state::Status,
    /// Its answer, when it has ended.
    #[serde(default)]
    pub answer: Option<super::state::SagaAnswer>,
}

impl Script {
    /// The answer of attempt `attempt` (1 for the first) of the call of `step`, or of its
    /// compensation when `undo` is set.
    #[must_use]
    pub fn answer(&self, step: &str, undo: bool, attempt: usize) -> Answer {
        let list = if undo { &self.undos } else { &self.steps };
        let Some(answers) = list.get(step).filter(|a| !a.is_empty()) else {
            return Answer::ok();
        };
        let at = attempt.saturating_sub(1).min(answers.len() - 1);
        answers.get(at).cloned().unwrap_or_else(Answer::ok)
    }

    /// The stop that fires after save number `saves`, if any.
    #[must_use]
    pub fn stop_after_save(&self, saves: usize) -> Option<&Stop> {
        self.stops.iter().find(|s| s.after_save == Some(saves))
    }

    /// The stop that fires after call number `calls`, if any.
    #[must_use]
    pub fn stop_after_call(&self, calls: usize) -> Option<&Stop> {
        self.stops.iter().find(|s| s.after_call == Some(calls))
    }
}
