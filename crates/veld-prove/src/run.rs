//! Asking the questions: one solver process per batch, and the four results.

use std::time::Duration;

use veld_ir::Ir;

use crate::diff::{Changed, tightened_question};
use crate::question::{Question, invariant_questions, policy_questions, require_questions};
use crate::report::{Item, Outcome, Report};
use crate::solver::{Backend, SEED, Verdict, parse_answers, preamble};
use crate::values::concretize;

/// How a run is set up.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// The time limit of each question (`--prove-timeout`).
    pub timeout: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            timeout: Duration::from_secs(5),
        }
    }
}

/// A rule that changed, for `veld diff --prove`.
#[derive(Debug, Clone)]
pub struct ChangedRule<'a> {
    /// The subject of the change, as `veld diff` names it.
    pub subject: String,
    /// The rule, for the report.
    pub detail: String,
    pub changed: Changed<'a>,
}

/// The questions of `veld check --prove` about every contract of `ir`.
pub fn check(ir: &Ir, backend: &dyn Backend, options: &Options) -> Report {
    let mut questions = invariant_questions(ir);
    questions.extend(require_questions(ir));
    questions.extend(policy_questions(ir));
    ask(questions, backend, options)
}

/// The question of `veld diff --prove` for each changed rule.
pub fn diff(changed: &[ChangedRule<'_>], backend: &dyn Backend, options: &Options) -> Report {
    let questions = changed
        .iter()
        .map(|c| tightened_question(&c.subject, &c.detail, c.changed))
        .collect();
    ask(questions, backend, options)
}

/// Runs the encodable questions in one solver process and reads every answer.
pub fn ask(questions: Vec<Question<'_>>, backend: &dyn Backend, options: &Options) -> Report {
    let mut script = preamble(backend.flavour(), options.timeout);
    let mut runnable = 0usize;
    for q in &questions {
        if let Ok(p) = &q.encoded {
            script.push_str(&p.script(runnable));
            runnable += 1;
        }
    }
    let mut answers = None;
    let mut why = String::new();
    if runnable > 0 {
        let out = backend.run(&script, runnable, options.timeout);
        if let Some(f) = out.failed {
            why = f;
        } else {
            if out.killed {
                why = "the solver did not finish in time".to_owned();
            }
            answers = Some(parse_answers(&out.stdout, runnable));
        }
    }
    let verdicts = answers
        .as_ref()
        .map(|a| a.verdicts.as_slice())
        .unwrap_or_default();
    let mut next = 0usize;
    let mut items = Vec::new();
    for q in &questions {
        let outcome = match &q.encoded {
            Err(e) => Outcome::Unsupported {
                reason: e.to_string(),
            },
            Ok(p) => {
                let verdict = verdicts.get(next);
                next += 1;
                outcome_of(q, p, verdict, &why)
            }
        };
        items.push(item_of(q, outcome));
    }
    Report {
        solver: answers.and_then(|a| a.info),
        seed: SEED,
        timeout: options.timeout,
        items,
    }
}

fn item_of(q: &Question<'_>, outcome: Outcome) -> Item {
    Item {
        kind: q.kind,
        subject: q.subject.clone(),
        detail: q.detail.clone(),
        origin: q.origin.clone(),
        outcome,
        open: q
            .encoded
            .as_ref()
            .map(|p| p.open_parts().clone())
            .unwrap_or_default(),
        core_origins: q.core_origins.clone(),
    }
}

fn outcome_of(
    q: &Question<'_>,
    p: &crate::Problem<'_>,
    verdict: Option<&Verdict>,
    why: &str,
) -> Outcome {
    let unknown = |reason: &str| Outcome::Unknown {
        reason: if why.is_empty() {
            reason.to_owned()
        } else {
            why.to_owned()
        },
    };
    match verdict {
        None => unknown("the solver gave no answer"),
        Some(Verdict::Unknown(r)) => unknown(r),
        Some(Verdict::Unsat(core)) => Outcome::Proved {
            core: describe_core(p, core),
        },
        Some(Verdict::Sat(values)) => match concretize(p, values) {
            Err(e) => unknown(&format!("the solver's value could not be built: {e}")),
            Ok(c) => match q.check.confirm(&c) {
                Ok(()) => Outcome::Counterexample {
                    value: crate::confirm::describe(&c, q.check.shows_clock(p)),
                },
                Err(e) => unknown(&format!("the solver's value was not confirmed: {e}")),
            },
        },
    }
}

/// The names of an unsatisfiable core as the descriptions of what they assert, in the order
/// they were asserted.
fn describe_core(p: &crate::Problem<'_>, core: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (name, what) in p.named() {
        if core.contains(name) && !out.contains(what) {
            out.push(what.clone());
        }
    }
    out
}
