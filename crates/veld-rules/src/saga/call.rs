//! One call of a saga with its retries (`spec/platform.md` section 18, "A step").

use std::collections::BTreeMap;

use serde_json::Value as Json;
use veld_ir::{DeclKind, SagaCall};

use super::run::{Flow, Interrupt, Orchestrator};
use super::script::Answer;
use super::state::StepError;
use super::{BACKOFF_MAX_MS, BACKOFF_START_MS, CallRecord, Phase, SagaError, step_key};

/// What a call came to.
pub(super) enum CallOutcome {
    /// It answered a success, with this wire value (`null` for no body).
    Success(Json),
    /// It answered a declared error, below 500: it failed for good.
    Declared(StepError),
    /// No answer arrived in time. `attempted` is false when no call was made at all.
    GaveUp { attempted: bool },
}

/// What one answer of the script means.
enum Judged {
    Final(CallOutcome),
    /// No answer, or a `5xx`: the call is made again.
    Retry,
}

impl Orchestrator<'_> {
    /// Calls `call` (the call or the compensation of step `i`, by `phase`) until it answers or
    /// the clock reaches `window_end`. After a call that got no answer the orchestrator waits
    /// 200 ms, then twice as long, up to 5 s.
    pub(super) fn attempt(
        &mut self,
        phase: Phase,
        i: usize,
        call: &SagaCall,
        args: &BTreeMap<String, Json>,
        window_end: i64,
    ) -> Flow<CallOutcome> {
        let step = self.saga.steps[i].name.clone();
        let undo = phase == Phase::Undo;
        let key = step_key(&self.input.id, &step, undo);
        let mut delay = BACKOFF_START_MS;
        let mut attempted = false;
        while self.now < window_end {
            attempted = true;
            let n = self.next_attempt(&key);
            self.record(phase, &step, call, &key, n, args);
            if let Some(stop) = self.script.stop_after_call(self.calls.len()) {
                return Err(Interrupt::Crash(stop.down_ms.max(0)));
            }
            let answer = self.script.answer(&step, undo, n);
            let arrival = self.now.saturating_add(answer.after_ms());
            if arrival > window_end {
                self.now = window_end;
                return Ok(CallOutcome::GaveUp { attempted });
            }
            match self.judge(call, answer)? {
                Judged::Final(outcome) => {
                    self.now = arrival;
                    return Ok(outcome);
                }
                Judged::Retry => {
                    self.now = arrival.saturating_add(delay);
                    delay = (delay * 2).min(BACKOFF_MAX_MS);
                }
            }
        }
        Ok(CallOutcome::GaveUp { attempted })
    }

    fn next_attempt(&mut self, key: &str) -> usize {
        let n = self.attempts.entry(key.to_owned()).or_insert(0);
        *n += 1;
        *n
    }

    fn record(
        &mut self,
        phase: Phase,
        step: &str,
        call: &SagaCall,
        key: &str,
        attempt: usize,
        args: &BTreeMap<String, Json>,
    ) {
        let service = self
            .input
            .ir
            .services
            .get(call.service.index())
            .map_or_else(String::new, |s| s.name.clone());
        self.calls.push(CallRecord {
            n: self.calls.len() + 1,
            phase,
            step: step.to_owned(),
            service,
            endpoint: call.endpoint.clone(),
            key: key.to_owned(),
            attempt,
            args: args.clone(),
            at_ms: self.now - self.input.start_ms,
        });
    }

    /// What an answer of the script means for the orchestrator.
    fn judge(&self, call: &SagaCall, answer: Answer) -> Result<Judged, SagaError> {
        Ok(match answer {
            Answer::Ok { value, .. } => {
                Judged::Final(CallOutcome::Success(value.unwrap_or(Json::Null)))
            }
            Answer::None { .. } => Judged::Retry,
            Answer::Error {
                decl, data, status, ..
            } => {
                let error = self.step_error(call, &decl, data, status)?;
                if error.status >= 500 {
                    Judged::Retry
                } else {
                    Judged::Final(CallOutcome::Declared(error))
                }
            }
        })
    }

    /// The error a call answered: its declaration, and its status, which is the one the
    /// endpoint called declares for it unless the script says another.
    fn step_error(
        &self,
        call: &SagaCall,
        name: &str,
        data: Option<Json>,
        status: Option<u16>,
    ) -> Result<StepError, SagaError> {
        let ir = self.input.ir;
        let (index, decl) = ir
            .decls
            .iter()
            .enumerate()
            .find(|(_, d)| {
                (d.name == name || d.qualified == name) && matches!(d.kind, DeclKind::Error(_))
            })
            .ok_or_else(|| SagaError::UnknownError(name.to_owned()))?;
        let declared = ir
            .services
            .get(call.service.index())
            .and_then(|s| s.endpoints.iter().find(|e| e.name == call.endpoint))
            .and_then(|e| e.errors.iter().find(|r| r.decl.index() == index))
            .map(|r| r.status);
        let default = match &decl.kind {
            DeclKind::Error(e) => e.default_status,
            DeclKind::Model(_)
            | DeclKind::Enum(_)
            | DeclKind::Union(_)
            | DeclKind::Const(_)
            | DeclKind::Event(_) => None,
        };
        Ok(StepError {
            error: decl.name.clone(),
            ty: decl.sig.clone(),
            status: status.or(declared).or(default).unwrap_or(500),
            data: data.unwrap_or_else(|| Json::Object(serde_json::Map::new())),
        })
    }
}
