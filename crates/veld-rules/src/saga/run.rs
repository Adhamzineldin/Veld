//! The orchestrator: the steps in order, the state saved after every answer, the compensations in
//! reverse order (`spec/platform.md` section 18).

use std::collections::BTreeMap;

use serde_json::{Value as Json, json};
use veld_ir::{DeclId, Saga};

use super::state::{SagaAnswer, SagaState, Status, StepError, StepState, StepStatus};
use super::{CallRecord, SagaError, SagaInput, SagaRun, Script};

/// Why `drive` stopped before the saga was over.
pub(super) enum Interrupt {
    /// The process died, and is down for this many milliseconds.
    Crash(i64),
    /// The run cannot go on.
    Fatal(SagaError),
}

impl From<SagaError> for Interrupt {
    fn from(e: SagaError) -> Interrupt {
        Interrupt::Fatal(e)
    }
}

pub(super) type Flow<T> = Result<T, Interrupt>;

pub(super) struct Orchestrator<'a> {
    pub input: &'a SagaInput<'a>,
    pub saga: &'a Saga,
    pub script: &'a Script,
    /// The clock, in milliseconds since the epoch.
    pub now: i64,
    /// The state in memory. A crash loses what it has over [`Self::saved`].
    pub state: SagaState,
    /// The last state that was saved: what a restarted process loads.
    pub saved: SagaState,
    pub saves: Vec<SagaState>,
    pub calls: Vec<CallRecord>,
    /// How many calls were made with each key, in the world of the script: a crash does not
    /// forget them.
    pub attempts: BTreeMap<String, usize>,
    pub compensated: Vec<String>,
    pub stuck_hook: bool,
    pub restarts: usize,
    /// True from a restart until the forward steps are looked at again.
    pub restarted: bool,
    pub answer_decl: Option<DeclId>,
}

/// Runs the saga of `input.endpoint`.
pub(super) fn run(input: &SagaInput<'_>, script: &Script) -> Result<SagaRun, SagaError> {
    let saga = input
        .endpoint
        .saga
        .as_ref()
        .ok_or_else(|| SagaError::NotASaga(input.endpoint.name.clone()))?;
    for name in script.steps.keys().chain(script.undos.keys()) {
        if !saga.steps.iter().any(|s| &s.name == name) {
            return Err(SagaError::UnknownStep(name.clone()));
        }
    }
    let mut o = Orchestrator::new(input, saga, script);
    if let Some(existing) = &script.existing {
        return Ok(o.existing(existing));
    }
    let mut flow = o.save().and_then(|()| o.drive());
    loop {
        match flow {
            Ok(()) => return Ok(o.finish()),
            Err(Interrupt::Fatal(e)) => return Err(e),
            Err(Interrupt::Crash(down_ms)) => {
                o.restart(down_ms);
                flow = o.drive();
            }
        }
    }
}

impl<'a> Orchestrator<'a> {
    fn new(input: &'a SagaInput<'a>, saga: &'a Saga, script: &'a Script) -> Self {
        let state = SagaState {
            id: input.id.clone(),
            operation: input.endpoint.sig.clone(),
            scope: input.scope.clone(),
            fingerprint: input.fingerprint.clone(),
            status: Status::Running,
            parameters: input.params.clone(),
            deadline: input.start_ms.saturating_add(saga.timeout_ms),
            steps: saga
                .steps
                .iter()
                .map(|s| StepState {
                    name: s.name.clone(),
                    status: StepStatus::Pending,
                    value: None,
                    error: None,
                })
                .collect(),
            failed_step: None,
            answer: None,
        };
        Orchestrator {
            input,
            saga,
            script,
            now: input.start_ms,
            saved: state.clone(),
            state,
            saves: Vec::new(),
            calls: Vec::new(),
            attempts: BTreeMap::new(),
            compensated: Vec::new(),
            stuck_hook: false,
            restarts: 0,
            restarted: false,
            answer_decl: None,
        }
    }

    /// A request whose key names a saga that exists: no state is saved and no call is made.
    fn existing(mut self, e: &super::Existing) -> SagaRun {
        self.state.status = e.status;
        self.state.answer = e.answer.clone();
        self.state.steps.clear();
        let conflict = |why: &str| SagaAnswer {
            status: 409,
            error: Some("Conflict".to_owned()),
            body: Some(json!({ "message": why })),
        };
        let (reply, replayed) = if e.fingerprint != self.input.fingerprint {
            (
                conflict("this idempotency key was used with other parameters"),
                false,
            )
        } else if !e.status.is_final() || e.answer.is_none() {
            (
                conflict("a request with this idempotency key is still running"),
                false,
            )
        } else {
            (e.answer.clone().unwrap_or_else(|| conflict("")), true)
        };
        let name = reply.error.clone();
        self.answer_decl = name.and_then(|n| {
            self.input
                .ir
                .decls
                .iter()
                .position(|d| d.name == n)
                .map(|i| DeclId(u32::try_from(i).unwrap_or(u32::MAX)))
        });
        let mut run = self.finish();
        run.reply = reply;
        run.replayed = replayed;
        run
    }

    fn finish(self) -> SagaRun {
        let reply = self.state.answer.clone().unwrap_or(SagaAnswer {
            status: 0,
            error: None,
            body: None,
        });
        SagaRun {
            reply,
            replayed: false,
            elapsed_ms: self.now - self.input.start_ms,
            calls: self.calls,
            saves: self.saves,
            state: self.state,
            answer_decl: self.answer_decl,
            compensated: self.compensated,
            stuck_hook: self.stuck_hook,
            restarts: self.restarts,
        }
    }

    /// Saves the state. The script may kill the process right after.
    pub(super) fn save(&mut self) -> Flow<()> {
        self.saved = self.state.clone();
        self.saves.push(self.state.clone());
        match self.script.stop_after_save(self.saves.len()) {
            Some(stop) => Err(Interrupt::Crash(stop.down_ms.max(0))),
            None => Ok(()),
        }
    }

    /// The process is started again: it loads the last saved state, after `down_ms`.
    fn restart(&mut self, down_ms: i64) {
        self.now = self.now.saturating_add(down_ms);
        self.state = self.saved.clone();
        self.restarts += 1;
        self.restarted = true;
    }

    fn drive(&mut self) -> Flow<()> {
        loop {
            match self.state.status {
                Status::Running => self.forward()?,
                Status::Compensating => self.compensate()?,
                Status::Completed | Status::Compensated | Status::Stuck => return Ok(()),
            }
        }
    }

    // ---- the steps -----------------------------------------------------------------

    fn forward(&mut self) -> Flow<()> {
        if std::mem::take(&mut self.restarted) && self.now >= self.state.deadline {
            return self.fail_at_deadline();
        }
        for i in 0..self.saga.steps.len() {
            if self.state.steps[i].status == StepStatus::Pending && !self.run_step(i)? {
                return Ok(());
            }
        }
        self.complete()
    }

    /// A saga that was down past its deadline fails at the step that was running. That step may
    /// have been called, so it is `unknown`.
    fn fail_at_deadline(&mut self) -> Flow<()> {
        let Some(i) = self
            .state
            .steps
            .iter()
            .position(|s| s.status == StepStatus::Pending)
        else {
            return self.complete();
        };
        self.fail_step(i, StepStatus::Unknown, None)
    }

    /// Runs the call of step `i`. `false` when the step failed and the saga now compensates.
    fn run_step(&mut self, i: usize) -> Flow<bool> {
        let saga = self.saga;
        let step = &saga.steps[i];
        let Ok(args) = self.args_of(&step.name, &step.call) else {
            self.fail_step(i, StepStatus::Failed, None)?;
            return Ok(false);
        };
        let window = self.state.deadline;
        match self.attempt(super::Phase::Step, i, &step.call, &args, window)? {
            super::call::CallOutcome::Success(v) => {
                self.state.steps[i].status = StepStatus::Done;
                self.state.steps[i].value = Some(v);
                // The answer of the last step completes the saga in the same save.
                if self
                    .state
                    .steps
                    .iter()
                    .all(|s| s.status == StepStatus::Done)
                {
                    self.complete()?;
                    return Ok(false);
                }
                self.save()?;
                Ok(true)
            }
            super::call::CallOutcome::Declared(e) => {
                self.fail_step(i, StepStatus::Failed, Some(e))?;
                Ok(false)
            }
            super::call::CallOutcome::GaveUp { attempted } => {
                let status = if attempted {
                    StepStatus::Unknown
                } else {
                    StepStatus::Failed
                };
                self.fail_step(i, status, None)?;
                Ok(false)
            }
        }
    }

    fn fail_step(&mut self, i: usize, status: StepStatus, error: Option<StepError>) -> Flow<()> {
        self.state.steps[i].status = status;
        self.state.steps[i].error = error;
        self.state.failed_step = Some(self.state.steps[i].name.clone());
        self.state.status = Status::Compensating;
        self.save()
    }

    fn complete(&mut self) -> Flow<()> {
        let last = self.state.steps.last().and_then(|s| s.value.clone());
        let body = if self.input.endpoint.success.ty.is_some() {
            Some(last.unwrap_or(Json::Null))
        } else {
            None
        };
        self.state.answer = Some(SagaAnswer {
            status: self.input.endpoint.success.status,
            error: None,
            body,
        });
        self.state.status = Status::Completed;
        self.save()
    }

    // ---- compensation --------------------------------------------------------------

    fn compensate(&mut self) -> Flow<()> {
        for i in (0..self.saga.steps.len()).rev() {
            if self.state.steps[i].status == StepStatus::Unknown && !self.learn(i)? {
                return Ok(());
            }
            if self.state.steps[i].status == StepStatus::Done && !self.undo(i)? {
                return Ok(());
            }
        }
        self.state.answer = Some(self.failure_answer(true));
        self.state.status = Status::Compensated;
        self.save()
    }

    /// Repeats the call of a step that got no answer, with the same key, to learn what it did.
    /// `false` when the saga is stuck.
    fn learn(&mut self, i: usize) -> Flow<bool> {
        let saga = self.saga;
        let step = &saga.steps[i];
        let args = self.args_of(&step.name, &step.call)?;
        let window = self.now.saturating_add(saga.timeout_ms);
        match self.attempt(super::Phase::Learn, i, &step.call, &args, window)? {
            super::call::CallOutcome::Success(v) => {
                self.state.steps[i].status = StepStatus::Done;
                self.state.steps[i].value = Some(v);
            }
            super::call::CallOutcome::Declared(e) => {
                self.state.steps[i].status = StepStatus::Failed;
                self.state.steps[i].error = Some(e);
            }
            super::call::CallOutcome::GaveUp { .. } => {
                self.stick()?;
                return Ok(false);
            }
        }
        self.save()?;
        Ok(true)
    }

    /// Runs the compensation of step `i`, if it has one. `false` when the saga is stuck.
    fn undo(&mut self, i: usize) -> Flow<bool> {
        let saga = self.saga;
        let step = &saga.steps[i];
        let Some(call) = &step.compensate else {
            return Ok(true);
        };
        let args = self.args_of(&step.name, call)?;
        let window = self.now.saturating_add(saga.timeout_ms);
        match self.attempt(super::Phase::Undo, i, call, &args, window)? {
            super::call::CallOutcome::Success(_) => {
                self.state.steps[i].status = StepStatus::Undone;
                self.compensated.push(step.name.clone());
                self.save()?;
                Ok(true)
            }
            super::call::CallOutcome::Declared(_) | super::call::CallOutcome::GaveUp { .. } => {
                self.stick()?;
                Ok(false)
            }
        }
    }

    /// The saga needs a person: it is `stuck`, and the hook is called.
    fn stick(&mut self) -> Flow<()> {
        self.state.answer = Some(self.failure_answer(false));
        self.state.status = Status::Stuck;
        self.save()?;
        self.stuck_hook = true;
        Ok(())
    }

    // ---- the answer ----------------------------------------------------------------

    /// The answer of a saga that failed. When it was undone after a declared error that the saga
    /// declares itself, that error as the step answered it; otherwise `502 SagaFailed`.
    fn failure_answer(&mut self, compensated: bool) -> SagaAnswer {
        let failed = self
            .state
            .failed_step
            .as_deref()
            .and_then(|n| self.state.step(n))
            .cloned();
        if compensated && let Some(e) = failed.as_ref().and_then(|s| s.error.clone()) {
            let decl = self.error_decl(&e);
            if decl.is_some_and(|d| self.saga.passes.contains(&d)) {
                self.answer_decl = decl;
                return SagaAnswer {
                    status: e.status,
                    error: Some(e.error),
                    body: Some(e.data),
                };
            }
        }
        let step = self.state.failed_step.clone().unwrap_or_default();
        self.answer_decl = self
            .input
            .ir
            .decls
            .iter()
            .position(|d| d.name == "SagaFailed")
            .map(|i| DeclId(u32::try_from(i).unwrap_or(u32::MAX)));
        SagaAnswer {
            status: 502,
            error: Some("SagaFailed".to_owned()),
            body: Some(json!({ "step": step, "compensated": compensated })),
        }
    }

    /// The declaration an error of a step names.
    fn error_decl(&self, e: &StepError) -> Option<DeclId> {
        self.input
            .ir
            .decls
            .iter()
            .position(|d| d.sig == e.ty)
            .map(|i| DeclId(u32::try_from(i).unwrap_or(u32::MAX)))
    }
}
