//! The shared script vectors of sagas (`tests/rules/sagas/*.json`).
//!
//! A vector file is a JSON array of cases. A case names a saga endpoint, the saga id and the bound
//! parameters, a [`Script`] of what every call answers, and what the run must come to. A generated
//! orchestrator is held to the same cases, with a fake clock and fake services that answer by the
//! script.
//!
//! ```json
//! {
//!   "name": "the payment is declined and the room is released",
//!   "endpoint": "Checkout.checkout",
//!   "id": "k-1",
//!   "params": { "body": { "roomId": "r-1", "amount": 100 } },
//!   "script": { "steps": { "hold": [ { "kind": "ok", "value": { "id": "h-1" } } ] } },
//!   "expect": {
//!     "status": "compensated",
//!     "answer": { "status": 402, "error": "PaymentDeclined", "body": { "reason": "funds" } },
//!     "calls": [ { "phase": "step", "step": "hold", "key": "k-1.hold" } ],
//!     "compensated": ["hold"]
//!   }
//! }
//! ```
//!
//! Each expected call lists some of the members of a [`CallRecord`](super::CallRecord); the
//! members it lists must be equal, the others are not looked at. The number of calls must be equal.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value as Json;
use veld_ir::{Endpoint, Ir, Service};

use super::{SagaAnswer, SagaInput, SagaRun, Script, Status, StepStatus, run_saga};

/// What a case expects of a run.
#[derive(Debug, Clone, Deserialize)]
pub struct Expect {
    /// The final status.
    pub status: Status,
    /// The answer. Not looked at when absent.
    #[serde(default)]
    pub answer: Option<SagaAnswer>,
    /// The calls, in order: for each, the members of the record that must be equal.
    pub calls: Vec<Json>,
    /// The steps whose undo succeeded, in order.
    #[serde(default)]
    pub compensated: Vec<String>,
    #[serde(default)]
    pub stuck_hook: bool,
    /// The status after each save, in order. Not looked at when absent.
    #[serde(default)]
    pub save_statuses: Option<Vec<Status>>,
    /// The status of each step at the end, by step name. Not looked at when absent.
    #[serde(default)]
    pub step_statuses: Option<BTreeMap<String, StepStatus>>,
    /// How many times the process was started again.
    #[serde(default)]
    pub restarts: usize,
}

/// One case of a vector file.
#[derive(Debug, Clone, Deserialize)]
pub struct SagaCase {
    pub name: String,
    /// `Service.endpoint`.
    pub endpoint: String,
    pub id: String,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub start_ms: i64,
    pub params: BTreeMap<String, Json>,
    #[serde(default)]
    pub script: Script,
    pub expect: Expect,
}

/// Reads a vector file.
///
/// # Errors
/// The message of the JSON error.
pub fn load_cases(text: &str) -> Result<Vec<SagaCase>, String> {
    serde_json::from_str(text).map_err(|e| e.to_string())
}

fn locate<'a>(ir: &'a Ir, name: &str) -> Option<(&'a Service, &'a Endpoint)> {
    let (service, endpoint) = name.split_once('.')?;
    let service = ir.services.iter().find(|s| s.name == service)?;
    let endpoint = service.endpoints.iter().find(|e| e.name == endpoint)?;
    Some((service, endpoint))
}

/// Runs a case and compares the run with what it expects.
///
/// # Errors
/// A message that lists every difference, or why the case could not run.
pub fn run_case(ir: &Ir, case: &SagaCase) -> Result<SagaRun, String> {
    let (service, endpoint) = locate(ir, &case.endpoint)
        .ok_or_else(|| format!("{}: no endpoint `{}`", case.name, case.endpoint))?;
    let input = SagaInput {
        ir,
        service,
        endpoint,
        params: case.params.clone(),
        id: case.id.clone(),
        scope: case.scope.clone(),
        start_ms: case.start_ms,
    };
    let run = run_saga(&input, &case.script).map_err(|e| format!("{}: {e}", case.name))?;
    let problems = differences(&case.expect, &run);
    if problems.is_empty() {
        Ok(run)
    } else {
        Err(format!("{}:\n  {}", case.name, problems.join("\n  ")))
    }
}

fn differences(want: &Expect, run: &SagaRun) -> Vec<String> {
    let mut out = Vec::new();
    let mut check = |what: &str, ok: bool, expected: String, actual: String| {
        if !ok {
            out.push(format!("{what}: expected {expected}, found {actual}"));
        }
    };
    let show = |v: &dyn std::fmt::Debug| format!("{v:?}");
    check(
        "status",
        want.status == run.state.status,
        show(&want.status),
        show(&run.state.status),
    );
    if let Some(a) = &want.answer {
        check(
            "answer",
            run.state.answer.as_ref() == Some(a),
            show(a),
            show(&run.state.answer),
        );
    }
    check(
        "compensated",
        want.compensated == run.compensated,
        show(&want.compensated),
        show(&run.compensated),
    );
    check(
        "stuck_hook",
        want.stuck_hook == run.stuck_hook,
        show(&want.stuck_hook),
        show(&run.stuck_hook),
    );
    check(
        "restarts",
        want.restarts == run.restarts,
        show(&want.restarts),
        show(&run.restarts),
    );
    if let Some(statuses) = &want.save_statuses {
        let actual: Vec<Status> = run.saves.iter().map(|s| s.status).collect();
        check("saves", *statuses == actual, show(statuses), show(&actual));
    }
    if let Some(steps) = &want.step_statuses {
        let actual: BTreeMap<String, StepStatus> = run
            .state
            .steps
            .iter()
            .map(|s| (s.name.clone(), s.status))
            .collect();
        check("steps", *steps == actual, show(steps), show(&actual));
    }
    out.extend(call_differences(&want.calls, run));
    out
}

fn call_differences(want: &[Json], run: &SagaRun) -> Vec<String> {
    let mut out = Vec::new();
    if want.len() != run.calls.len() {
        out.push(format!(
            "calls: expected {}, found {}: {}",
            want.len(),
            run.calls.len(),
            run.calls
                .iter()
                .map(|c| format!("{} {}", c.step, c.key))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        return out;
    }
    for (i, (w, c)) in want.iter().zip(&run.calls).enumerate() {
        let actual = serde_json::to_value(c).unwrap_or(Json::Null);
        let Some(members) = w.as_object() else {
            out.push(format!("call {}: not an object", i + 1));
            continue;
        };
        for (key, expected) in members {
            if actual.get(key) != Some(expected) {
                out.push(format!(
                    "call {} `{key}`: expected {expected}, found {}",
                    i + 1,
                    actual
                        .get(key)
                        .map_or_else(|| "nothing".to_owned(), Json::to_string)
                ));
            }
        }
    }
    out
}
