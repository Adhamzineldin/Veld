//! The state a saga saves after every answer (`spec/platform.md` section 18, `SagaStore`).
//!
//! Plain data with the same members in every target. The JSON below is what the reference
//! orchestrator saves and what a generated orchestrator is compared with.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

/// Where the saga is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// The steps run.
    Running,
    /// A step failed: the steps that succeeded are being undone.
    Compensating,
    /// Every step succeeded.
    Completed,
    /// Every step that succeeded was undone.
    Compensated,
    /// An undo failed for good or ran out of time: a person is needed.
    Stuck,
}

impl Status {
    /// True for the states a saga does not leave.
    #[must_use]
    pub fn is_final(self) -> bool {
        matches!(
            self,
            Status::Completed | Status::Compensated | Status::Stuck
        )
    }
}

/// Where one step is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    /// Not answered yet.
    Pending,
    /// Succeeded, and not undone.
    Done,
    /// Answered a declared error, or was never made: nothing to undo.
    Failed,
    /// Got no answer: it may have happened.
    Unknown,
    /// Succeeded and its compensation succeeded.
    Undone,
}

/// The error a step answered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepError {
    /// The name of the error declaration.
    pub error: String,
    /// Its signature.
    #[serde(rename = "type")]
    pub ty: String,
    pub status: u16,
    /// The fields of the error, in wire form.
    pub data: Json,
}

/// One step in the state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepState {
    pub name: String,
    pub status: StepStatus,
    /// The wire value of the success body, present from `done` on (`null` for a call without a
    /// body).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Json>,
    /// The error, present when the step is `failed` by a declared error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<StepError>,
}

/// What the saga answered, as the store keeps it for the key of the caller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SagaAnswer {
    pub status: u16,
    /// The name of the error, `null` for a success.
    pub error: Option<String>,
    /// The body in wire form, `null` for an answer without a body.
    pub body: Option<Json>,
}

/// The state of one saga instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SagaState {
    /// The saga id: the key of the caller, in its scope.
    pub id: String,
    /// The signature of the operation.
    pub operation: String,
    /// The caller, for an endpoint whose keys are scoped to it; `null` otherwise.
    pub scope: Option<String>,
    /// The fingerprint of the request (`spec/rules.md` section 11.9): another request with the
    /// same key and another fingerprint is `409 Conflict`.
    pub fingerprint: String,
    pub status: Status,
    /// The bound parameters, in wire form.
    pub parameters: BTreeMap<String, Json>,
    /// The time by which the last step must have answered, in milliseconds since the epoch.
    pub deadline: i64,
    /// One entry per step, in the order of the steps.
    pub steps: Vec<StepState>,
    /// The step that failed, `null` while none has.
    #[serde(rename = "failedStep")]
    pub failed_step: Option<String>,
    /// The answer, `null` until the saga is over.
    pub answer: Option<SagaAnswer>,
}

impl SagaState {
    /// The state of the step called `name`.
    #[must_use]
    pub fn step(&self, name: &str) -> Option<&StepState> {
        self.steps.iter().find(|s| s.name == name)
    }
}
