//! The report of a run (`spec/rules.md` section 18.4): what was asked, what each question came
//! to, and which parts of the fragment were left open.
//!
//! Nothing here says that anything is "verified" or "correct": a result is about the rules of the
//! contract, over the fragment, and about nothing else (section 18).

use std::collections::BTreeSet;
use std::time::Duration;

use veld_ir::Origin;

use crate::solver::SolverInfo;
use crate::types::OpenPart;

/// Which of the four questions of section 18.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum QuestionKind {
    /// Can the invariants of a declaration all hold?
    Invariants,
    /// Can a `require` pass?
    Require,
    /// Does a policy permit anybody?
    Policy,
    /// Is a changed rule tighter (`veld diff --prove`)?
    Tightened,
}

/// What a question came to: exactly one of four.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The solver answered `unsat`. For a rule question, `core` names what contradicts.
    Proved { core: Vec<String> },
    /// The solver gave a value and the reference evaluator confirmed it.
    Counterexample { value: String },
    /// The solver timed out, answered `unknown`, or its value was not confirmed.
    Unknown { reason: String },
    /// The question needs something outside the fragment.
    Unsupported { reason: String },
}

/// One question and its result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub kind: QuestionKind,
    /// The declaration, endpoint or policy, as the user knows it.
    pub subject: String,
    /// The rule or statement.
    pub detail: String,
    pub origin: Origin,
    pub outcome: Outcome,
    /// The parts of the fragment the question left open.
    pub open: BTreeSet<OpenPart>,
    /// Where each name of a core was declared, by description.
    pub core_origins: Vec<(String, Origin)>,
}

impl Item {
    /// True when this result is a finding (section 18.2): *proved* for the three rule
    /// questions, a confirmed *counterexample* for `diff`.
    #[must_use]
    pub fn is_finding(&self) -> bool {
        matches!(
            (self.kind, &self.outcome),
            (QuestionKind::Tightened, Outcome::Counterexample { .. })
                | (
                    QuestionKind::Invariants | QuestionKind::Require | QuestionKind::Policy,
                    Outcome::Proved { .. }
                )
        )
    }

    /// The name of the question, for lists.
    #[must_use]
    pub fn name(&self) -> String {
        if self.detail.is_empty() {
            self.subject.clone()
        } else {
            format!("{} {}", self.subject, self.detail)
        }
    }
}

/// How many of each result.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub proved: usize,
    pub counterexample: usize,
    pub unknown: usize,
    pub unsupported: usize,
}

/// The result of `--prove`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub solver: Option<SolverInfo>,
    pub seed: u32,
    pub timeout: Duration,
    pub items: Vec<Item>,
}

impl Report {
    /// The results, counted.
    #[must_use]
    pub fn counts(&self) -> Counts {
        let mut c = Counts::default();
        for i in &self.items {
            match i.outcome {
                Outcome::Proved { .. } => c.proved += 1,
                Outcome::Counterexample { .. } => c.counterexample += 1,
                Outcome::Unknown { .. } => c.unknown += 1,
                Outcome::Unsupported { .. } => c.unsupported += 1,
            }
        }
        c
    }

    /// The findings: `E0970` for the rule questions.
    pub fn findings(&self) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(|i| i.is_finding())
    }

    /// The questions that ended *unknown* or *unsupported*: `W0970` lists them.
    pub fn undecided(&self) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(|i| {
            matches!(
                i.outcome,
                Outcome::Unknown { .. } | Outcome::Unsupported { .. }
            )
        })
    }

    /// The open parts of the fragment that any question used.
    #[must_use]
    pub fn open_parts(&self) -> BTreeSet<OpenPart> {
        self.items
            .iter()
            .flat_map(|i| i.open.iter().copied())
            .collect()
    }

    /// One line: the solver, its version, the seed and the time limit.
    #[must_use]
    pub fn solver_line(&self) -> String {
        let secs = self.timeout.as_secs_f64();
        match &self.solver {
            Some(s) if s.version.is_empty() => {
                format!("{} (seed {}, {secs}s per question)", s.name, self.seed)
            }
            Some(s) => format!(
                "{} {} (seed {}, {secs}s per question)",
                s.name, s.version, self.seed
            ),
            None => format!("an SMT solver (seed {}, {secs}s per question)", self.seed),
        }
    }

    /// The lines of the summary: the counts and the open parts that were used.
    #[must_use]
    pub fn summary_lines(&self) -> Vec<String> {
        let c = self.counts();
        let mut out = vec![format!(
            "{} questions: {} proved, {} counterexample, {} unknown, {} unsupported",
            self.items.len(),
            c.proved,
            c.counterexample,
            c.unknown,
            c.unsupported
        )];
        let open = self.open_parts();
        if open.is_empty() {
            out.push("open parts of the fragment used: none".to_owned());
        } else {
            let names: Vec<&str> = open.iter().map(|p| p.name()).collect();
            out.push(format!(
                "open parts of the fragment used: {}",
                names.join(", ")
            ));
        }
        out
    }
}
