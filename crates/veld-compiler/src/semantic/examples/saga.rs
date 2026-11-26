//! The saga lines of an example (`spec/platform.md` section 18): `given step`, `given undo`,
//! `expect compensated` and `expect stuck`.
//!
//! A step is named by a step of the saga, an answer is typed by the endpoint that call makes
//! (a value is a value of its success type, an error is one it can answer with), and an undo
//! needs a step that has a `compensate`. A step the example does not name answers a filled-in
//! success and an undo answers `ok`; that is the runner's business, not this file's.

use super::values::Values;
use super::{ROutcome, RSagaAnswer, RSagaExample, RSagaExpect};
use crate::diagnostics::{Diagnostic, Span};
use crate::semantic::{Program, RCall, REndpoint, RSaga, RStep};
use crate::syntax::ast::{self, SagaAnswerLine, SagaExpectKind};

impl<'p> Values<'p, '_, '_> {
    /// Checks the saga lines of the example of endpoint `ep`. The outer `None` is an error;
    /// `Some(None)` is an example without saga lines.
    pub fn saga_lines(
        &mut self,
        lines: Option<&ast::SagaExampleLines>,
        call: &ast::ExampleCall,
        ep: &REndpoint,
    ) -> Option<Option<RSagaExample>> {
        let Some(saga) = &ep.saga else {
            return self.not_a_saga(lines, ep);
        };
        if let Some(r) = &call.returns {
            self.diags.push(
                Diagnostic::error(
                    "E0952",
                    "a saga has no handler, so `returns` has nothing to say",
                    r.span,
                )
                .with_help("state what a step answers with `given step name = value`"),
            );
            return None;
        }
        let Some(lines) = lines else {
            return Some(None);
        };
        let mut out = RSagaExample::default();
        let mut ok = true;
        for g in &lines.givens {
            ok &= self.saga_given(g, saga, &mut out);
        }
        if let Some(e) = &lines.expect {
            ok &= self.saga_expect(e, saga, &mut out);
        }
        ok.then_some(Some(out))
    }

    /// Saga lines on an endpoint that is not a saga are `E0952`.
    fn not_a_saga(
        &mut self,
        lines: Option<&ast::SagaExampleLines>,
        ep: &REndpoint,
    ) -> Option<Option<RSagaExample>> {
        let Some(lines) = lines else {
            return Some(None);
        };
        let at = lines
            .givens
            .first()
            .map(|g| g.span)
            .or(lines.expect.as_ref().map(|e| e.span))
            .unwrap_or_else(Span::none);
        self.diags.push(
            Diagnostic::error(
                "E0952",
                format!("endpoint `{}` is not a saga", ep.name),
                at,
            )
            .with_help("`given step`, `given undo` and `expect compensated` belong to a `saga post` endpoint"),
        );
        None
    }

    fn saga_given(&mut self, g: &ast::SagaGivenLine, saga: &RSaga, out: &mut RSagaExample) -> bool {
        let name = &g.step.name;
        let Some(step) = saga.steps.iter().find(|s| &s.name == name) else {
            self.unknown_step(&g.step, saga);
            return false;
        };
        let (word, call, list) = if g.undo {
            let Some(c) = &step.compensate else {
                self.err_step(
                    format!("step `{name}` has no `compensate` to answer"),
                    g.step.span,
                );
                return false;
            };
            ("undo", c, &mut out.undos)
        } else {
            ("step", &step.call, &mut out.steps)
        };
        if list.iter().any(|(n, _)| n == name) {
            self.err_step(
                format!("`given {word} {name}` is written twice"),
                g.step.span,
            );
            return false;
        }
        match self.saga_answer(&g.answer, call) {
            Some(a) => {
                list.push((name.clone(), a));
                true
            }
            None => false,
        }
    }

    /// What one call answers, typed by the endpoint it calls.
    fn saga_answer(&mut self, a: &SagaAnswerLine, call: &RCall) -> Option<RSagaAnswer> {
        let callee = callee_of(self.prog, call)?;
        match a {
            SagaAnswerLine::Unavailable(_) => Some(RSagaAnswer::Unavailable),
            SagaAnswerLine::Outcome(o) => Some(RSagaAnswer::Answer(self.outcome_of(o, callee)?)),
            SagaAnswerLine::Value(v) => {
                let outcome = ast::Outcome {
                    error: None,
                    value: Some(v.clone()),
                    span: v.span,
                };
                let o: ROutcome = self.outcome_of(&outcome, callee)?;
                Some(RSagaAnswer::Answer(o))
            }
        }
    }

    fn saga_expect(
        &mut self,
        e: &ast::SagaExpectLine,
        saga: &RSaga,
        out: &mut RSagaExample,
    ) -> bool {
        match &e.kind {
            SagaExpectKind::Stuck => {
                out.expect = Some(RSagaExpect::Stuck);
                true
            }
            SagaExpectKind::Compensated(names) => {
                let mut steps = Vec::new();
                let mut ok = true;
                for n in names {
                    ok &= self.compensated_step(n, saga, &steps);
                    steps.push(n.name.clone());
                }
                out.expect = Some(RSagaExpect::Compensated(steps));
                ok
            }
        }
    }

    fn compensated_step(&mut self, n: &ast::Ident, saga: &RSaga, seen: &[String]) -> bool {
        let Some(step) = saga.steps.iter().find(|s| s.name == n.name) else {
            self.unknown_step(n, saga);
            return false;
        };
        if step.compensate.is_none() {
            self.err_step(
                format!(
                    "step `{}` has no `compensate`, so it is never undone",
                    n.name
                ),
                n.span,
            );
            return false;
        }
        if seen.contains(&n.name) {
            self.err_step(format!("step `{}` is named twice", n.name), n.span);
            return false;
        }
        true
    }

    fn unknown_step(&mut self, id: &ast::Ident, saga: &RSaga) {
        let names: Vec<&str> = saga.steps.iter().map(|s: &RStep| s.name.as_str()).collect();
        let diag = Diagnostic::error(
            "E0951",
            format!("`{}` is not a step of the saga", id.name),
            id.span,
        )
        .did_you_mean(
            &id.name,
            names.iter().copied(),
            Some(format!("steps: {}", names.join(", "))),
        );
        self.diags.push(diag);
    }

    fn err_step(&mut self, msg: String, span: Span) {
        self.diags.push(Diagnostic::error("E0952", msg, span));
    }
}

/// The endpoint a call of a saga makes.
fn callee_of<'p>(prog: &'p Program<'_>, call: &RCall) -> Option<&'p REndpoint> {
    prog.services
        .get(call.service)?
        .endpoints
        .iter()
        .find(|e| e.name == call.endpoint)
}
