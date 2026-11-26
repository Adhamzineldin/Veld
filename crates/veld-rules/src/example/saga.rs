//! Saga examples (`spec/platform.md` section 18): the script an example stands for, and running
//! it offline through the reference orchestrator.
//!
//! `given step s = ..` and `given undo s = ..` say what a call answers, always, on every attempt;
//! `unavailable` never answers. A step the example does not state answers a success whose value is
//! filled in like any value of an example (`spec/rules.md` section 12.2), an undo answers `ok`.
//! The values of the steps are filled in after the parameters, `returns` and the key, in the
//! order of the steps. The saga id is the key of the `key` line; an example without one gets the
//! id [`FIXED_ID`].

use serde_json::Value as Json;
use veld_ir::{
    Constraints, Endpoint, ExampleOutcome, ExampleSaga, Ir, SagaAnswer, SagaCall, SagaExpect,
    Service, Type,
};

use super::report::{self, ExampleReport, Mismatch, Verdict};
use super::{Build, ExampleError, fill_all};
use crate::pipeline::{ErrorAnswer, Outcome};
use crate::saga::{Answer, SagaInput, SagaRun, Script, Status, run_saga, saga_id};

/// The saga id of an example that has no `key` line.
pub const FIXED_ID: &str = "example";

impl<'a> Build<'a> {
    /// What every call of the saga answers, or `None` when the endpoint is not a saga.
    pub(super) fn saga_script(&mut self) -> Result<Option<Script>, ExampleError> {
        let Some(saga) = &self.endpoint.saga else {
            return Ok(None);
        };
        let lines = self.ex.saga.clone().unwrap_or_default();
        let mut script = Script::default();
        for step in &saga.steps {
            let given = lines.steps.iter().find(|g| g.step == step.name);
            let answer = self.answer(&step.call, given.map(|g| &g.answer), false)?;
            script.steps.insert(step.name.clone(), vec![answer]);
        }
        for step in saga.steps.iter().filter(|s| s.compensate.is_some()) {
            let Some(call) = &step.compensate else {
                continue;
            };
            if let Some(g) = lines.undos.iter().find(|g| g.step == step.name) {
                let answer = self.answer(call, Some(&g.answer), true)?;
                script.undos.insert(step.name.clone(), vec![answer]);
            }
        }
        Ok(Some(script))
    }

    /// One answer of the example, or the default for a call it does not state.
    fn answer(
        &mut self,
        call: &SagaCall,
        given: Option<&SagaAnswer>,
        undo: bool,
    ) -> Result<Answer, ExampleError> {
        let callee = self.callee(call)?;
        match given {
            None | Some(SagaAnswer::Answer(ExampleOutcome::Ok(None))) if undo => Ok(Answer::ok()),
            Some(SagaAnswer::Unavailable) => Ok(Answer::none()),
            Some(SagaAnswer::Answer(ExampleOutcome::Error(e))) => {
                let pattern = e.pattern.as_ref().map(|v| self.resolve(v)).transpose()?;
                self.filler.begin();
                let data = self.filler.fill(
                    &Type::Decl(e.decl),
                    &Constraints::default(),
                    pattern.as_ref(),
                    "$",
                )?;
                let name = self
                    .ir
                    .decls
                    .get(e.decl.index())
                    .map_or_else(String::new, |d| d.name.clone());
                Ok(Answer::Error {
                    decl: name,
                    data: Some(data),
                    status: None,
                    after_ms: 0,
                })
            }
            Some(SagaAnswer::Answer(ExampleOutcome::Ok(v))) => self.success(callee, v.as_ref()),
            None => self.success(callee, None),
        }
    }

    /// A success of `callee`: the value is the example's, completed, or filled in.
    fn success(
        &mut self,
        callee: &Endpoint,
        given: Option<&veld_ir::ExampleValue>,
    ) -> Result<Answer, ExampleError> {
        let Some(ty) = callee.success.ty.clone().filter(|_| !callee.success.stream) else {
            return Ok(Answer::ok());
        };
        let given = given.map(|v| self.resolve(v)).transpose()?;
        self.filler.begin();
        let value = self
            .filler
            .fill(&ty, &Constraints::default(), given.as_ref(), "$")?;
        Ok(Answer::Ok {
            value: Some(value),
            after_ms: 0,
        })
    }

    fn callee(&self, call: &SagaCall) -> Result<&'a Endpoint, ExampleError> {
        let ir: &'a Ir = self.ir;
        ir.services
            .get(call.service.index())
            .and_then(|s| s.endpoints.iter().find(|e| e.name == call.endpoint))
            .ok_or_else(|| ExampleError::WrongForm {
                example: self.ex.name.clone(),
                reason: format!(
                    "the saga calls `{}`, which the IR does not have",
                    call.endpoint
                ),
            })
    }
}

/// Runs a saga example offline: the saga runs with the script the example stands for, and the
/// answer and the `expect` line are compared with what the saga did.
pub(super) fn run_saga_example(
    ir: &Ir,
    ex: &veld_ir::Example,
    service: &Service,
    endpoint: &Endpoint,
) -> Result<ExampleReport, ExampleError> {
    let (filled, script) = fill_all(ir, ex)?;
    let script = script.unwrap_or_default();
    if let Some(key) = &filled.request.key
        && key.store != crate::idempotency::StoreState::Empty
    {
        return Ok(skipped(
            ex,
            "a saga example runs with a key the store does not know",
        ));
    }
    let key = filled
        .request
        .key
        .as_ref()
        .map_or(FIXED_ID, |k| k.value.as_str());
    let caller = filled.request.caller.as_deref();
    let input = SagaInput {
        ir,
        service,
        endpoint,
        params: filled.request.params.clone(),
        id: saga_id(caller, key),
        scope: caller.map(str::to_owned),
        start_ms: filled.request.now,
    };
    let run = run_saga(&input, &script).map_err(|e| ExampleError::WrongForm {
        example: ex.name.clone(),
        reason: e.to_string(),
    })?;
    let (outcome, result) = outcome_of(&run);
    let mut verdict = report::compare(ir, endpoint, &filled.expected, &outcome, result.as_ref());
    if verdict == Verdict::Passed
        && let Some(saga) = &ex.saga
    {
        verdict = check_expect(saga, &run);
    }
    Ok(ExampleReport {
        name: ex.name.clone(),
        verdict,
        outcome: Some(outcome),
        audited: Vec::new(),
    })
}

fn skipped(ex: &veld_ir::Example, why: &str) -> ExampleReport {
    ExampleReport {
        name: ex.name.clone(),
        verdict: Verdict::Skipped(why.to_owned()),
        outcome: None,
        audited: Vec::new(),
    }
}

/// What the saga answered, as the outcome of a call.
fn outcome_of(run: &SagaRun) -> (Outcome, Option<Json>) {
    let answer = run.state.answer.clone();
    let Some(answer) = answer else {
        return (Outcome::Passes, None);
    };
    match (&answer.error, run.answer_decl) {
        (Some(name), Some(decl)) => {
            let data = answer
                .body
                .unwrap_or_else(|| Json::Object(serde_json::Map::new()));
            let outcome = Outcome::Error(ErrorAnswer {
                decl,
                name: name.clone(),
                status: answer.status,
                data,
                violations: Vec::new(),
                hook: Vec::new(),
            });
            (outcome, None)
        }
        (Some(_) | None, _) => (Outcome::Passes, answer.body),
    }
}

/// `expect compensated a, b` and `expect stuck`.
fn check_expect(saga: &ExampleSaga, run: &SagaRun) -> Verdict {
    let Some(expect) = &saga.expect else {
        return Verdict::Passed;
    };
    let (want, ok) = match expect {
        SagaExpect::Stuck => ("stuck".to_owned(), run.state.status == Status::Stuck),
        SagaExpect::Compensated(steps) => (
            format!("compensated {}", steps.join(", ")),
            run.state.status == Status::Compensated && &run.compensated == steps,
        ),
    };
    if ok {
        return Verdict::Passed;
    }
    Verdict::Failed(Mismatch {
        expected: want,
        actual: describe_run(run),
        differences: Vec::new(),
    })
}

/// What the saga did, in the words of the `expect` lines.
fn describe_run(run: &SagaRun) -> String {
    match run.state.status {
        Status::Stuck => "stuck".to_owned(),
        Status::Compensated if run.compensated.is_empty() => "compensated nothing".to_owned(),
        Status::Compensated => format!("compensated {}", run.compensated.join(", ")),
        Status::Completed => "completed".to_owned(),
        Status::Running | Status::Compensating => "unfinished".to_owned(),
    }
}
