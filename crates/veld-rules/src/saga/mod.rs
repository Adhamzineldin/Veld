//! The reference orchestrator of a saga (`spec/platform.md` sections 17 and 18).
//!
//! A pure, deterministic interpreter. It takes the saga of the IR, the bound parameters, the saga
//! id and a [`Script`] of what every call answers, and returns the calls it made (service,
//! endpoint, arguments in wire form, idempotency key), every state it saved, the final state and
//! the answer. There is no network, no timer and no thread: time is a number that moves when the
//! orchestrator waits and when the script says a call took long, so the same input always gives
//! the same output. A generated orchestrator is held to these results.
//!
//! * [`run_saga`] runs a saga to its end, with the stops and restarts the script asks for.
//! * [`saga_id`] and [`step_key`] are the id of a saga and the key of each of its calls.
//! * [`SagaState`] is the plain data a store keeps.

mod args;
mod call;
mod run;
mod script;
mod state;
pub mod vectors;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use sha2::{Digest, Sha256};
use veld_ir::{DeclId, Endpoint, Ir, Service};

use crate::error::InputError;
pub use script::{Answer, Existing, Script, Stop};
pub use state::{SagaAnswer, SagaState, Status, StepError, StepState, StepStatus};

/// The first wait after a call that got no answer, in milliseconds. It doubles up to
/// [`BACKOFF_MAX_MS`].
pub const BACKOFF_START_MS: i64 = 200;
/// The longest wait between two attempts of a call, in milliseconds.
pub const BACKOFF_MAX_MS: i64 = 5000;
/// The longest idempotency key a call carries (`spec/rules.md` section 11.9).
const MAX_KEY_LEN: usize = 255;

/// Why a saga could not be run.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SagaError {
    #[error("endpoint `{0}` is not a saga")]
    NotASaga(String),
    #[error("the script names `{0}`, which is not a step of the saga")]
    UnknownStep(String),
    #[error("the script answers with `{0}`, which is not an error declaration")]
    UnknownError(String),
    #[error("the IR does not describe the call of step `{0}`")]
    BadCall(String),
    #[error("an argument of step `{step}` cannot be evaluated: {why}")]
    Argument { step: String, why: String },
    #[error(transparent)]
    Input(#[from] InputError),
}

/// What a run starts from.
#[derive(Debug, Clone)]
pub struct SagaInput<'a> {
    pub ir: &'a Ir,
    pub service: &'a Service,
    /// The saga endpoint.
    pub endpoint: &'a Endpoint,
    /// The bound parameters of the request, in wire form, by parameter name.
    pub params: BTreeMap<String, Json>,
    /// The saga id ([`saga_id`]).
    pub id: String,
    /// The caller, for an endpoint whose keys are scoped to it.
    pub scope: Option<String>,
    /// The fingerprint of the request (`crate::fingerprint` of `params`).
    pub fingerprint: String,
    /// The clock when the request arrives, in milliseconds since the epoch.
    pub start_ms: i64,
}

/// Which kind of call a record is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// The call of a step.
    Step,
    /// The same call again, to learn what a step that got no answer did.
    Learn,
    /// The compensation of a step.
    Undo,
}

/// One call the orchestrator made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallRecord {
    /// 1 for the first call of the run.
    pub n: usize,
    pub phase: Phase,
    pub step: String,
    pub service: String,
    pub endpoint: String,
    /// The idempotency key ([`step_key`]).
    pub key: String,
    /// 1 for the first call with this key.
    pub attempt: usize,
    /// The arguments in wire form, by parameter name.
    pub args: BTreeMap<String, Json>,
    /// Milliseconds since the request arrived.
    pub at_ms: i64,
}

/// Everything a run came to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SagaRun {
    /// Every call, in the order they were made.
    pub calls: Vec<CallRecord>,
    /// Every state that was saved, in order. The first is the one that creates the saga.
    pub saves: Vec<SagaState>,
    /// The state at the end.
    pub state: SagaState,
    /// What the caller is answered: the answer of the saga, `409 Conflict` for a request whose
    /// key names a saga that is running or was started with another fingerprint, or the saved
    /// answer of a saga that has ended.
    pub reply: SagaAnswer,
    /// True when `reply` is a saved answer sent again (`Veld-Idempotent-Replay: true`).
    pub replayed: bool,
    /// The error declaration of the reply, `None` for a success.
    pub answer_decl: Option<DeclId>,
    /// The steps that were undone, in the order their undo succeeded.
    pub compensated: Vec<String>,
    /// True when the saga ended `stuck` and `onSagaStuck` was called.
    pub stuck_hook: bool,
    /// How many times the process was started again from a saved state.
    pub restarts: usize,
    /// Milliseconds from the arrival of the request to the end of the run.
    pub elapsed_ms: i64,
}

/// The id of a saga: the key of the caller. When the endpoint's keys are scoped to the caller the
/// id starts with the first 16 hex characters of the SHA-256 of the caller text and a dot, so two
/// callers who send the same key start two sagas.
#[must_use]
pub fn saga_id(caller: Option<&str>, key: &str) -> String {
    match caller {
        None => key.to_owned(),
        Some(c) => {
            let digest = Sha256::digest(c.as_bytes());
            let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
            format!("{hex}.{key}")
        }
    }
}

/// The idempotency key of a call: `<saga id>.<step>` for a step and `<saga id>.<step>.undo` for
/// its compensation. A key longer than 255 characters is replaced by the 64 hex characters of
/// the SHA-256 of its text.
#[must_use]
pub fn step_key(id: &str, step: &str, undo: bool) -> String {
    let text = if undo {
        format!("{id}.{step}.undo")
    } else {
        format!("{id}.{step}")
    };
    if text.len() <= MAX_KEY_LEN {
        return text;
    }
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Runs a saga to its end.
///
/// # Errors
/// [`SagaError`] when the endpoint is not a saga, or the script or the IR does not fit it. A
/// saga that fails is not an error: it is a run whose state is `compensated` or `stuck`.
pub fn run_saga(input: &SagaInput<'_>, script: &Script) -> Result<SagaRun, SagaError> {
    run::run(input, script)
}
