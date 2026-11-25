//! The result of running an example, in a form a command-line tool can print.

use std::fmt;

use serde_json::Value as Json;
use veld_ir::{Endpoint, Ir, Type};

use super::Expected;
use super::matching::{Difference, diff_pattern};
use crate::check::Violation;
use crate::pipeline::{ErrorAnswer, Outcome};

/// What came of comparing the outcome with the expected one.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Passed,
    /// The example could not be checked offline, with the reason.
    Skipped(String),
    Failed(Mismatch),
}

/// Expected and actual outcome, and where they differ.
#[derive(Debug, Clone, PartialEq)]
pub struct Mismatch {
    pub expected: String,
    pub actual: String,
    pub differences: Vec<Difference>,
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "expected: {}", self.expected)?;
        write!(f, "actual:   {}", self.actual)?;
        for d in &self.differences {
            write!(f, "\n  {d}")?;
        }
        Ok(())
    }
}

/// The report of one example.
#[derive(Debug, Clone, PartialEq)]
pub struct ExampleReport {
    pub name: String,
    pub verdict: Verdict,
    /// What the pipeline answered.
    pub outcome: Outcome,
}

impl ExampleReport {
    /// True when the example passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        matches!(self.verdict, Verdict::Passed)
    }
}

impl fmt::Display for ExampleReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.verdict {
            Verdict::Passed => write!(f, "ok   {}", self.name),
            Verdict::Skipped(why) => write!(f, "skip {} ({why})", self.name),
            Verdict::Failed(m) => write!(f, "FAIL {}\n{m}", self.name),
        }
    }
}

fn violations_text(vs: &[Violation]) -> String {
    let items: Vec<String> = vs
        .iter()
        .map(|v| format!("{}: {}", v.rule_name, v.message))
        .collect();
    format!("[{}]", items.join("; "))
}

fn render_error(a: &ErrorAnswer) -> String {
    let mut s = format!("{} {} {}", a.name, a.status, a.data);
    if !a.violations.is_empty() {
        s.push_str(&format!(" violations {}", violations_text(&a.violations)));
    }
    if !a.hook.is_empty() {
        s.push_str(&format!(
            " reported to the hook {}",
            violations_text(&a.hook)
        ));
    }
    s
}

fn render_actual(outcome: &Outcome, result: Option<&Json>) -> String {
    match (outcome, result) {
        (Outcome::Passes, Some(r)) => format!("ok {r}"),
        (Outcome::Passes, None) => "ok".to_owned(),
        (Outcome::Error(a), _) => render_error(a),
    }
}

fn render_expected(ir: &Ir, expected: &Expected) -> String {
    let with = |head: &str, pattern: &Option<Json>| match pattern {
        Some(p) => format!("{head} {p}"),
        None => head.to_owned(),
    };
    match expected {
        Expected::Ok(p) => with("ok", p),
        Expected::Error { decl, pattern } => {
            let name = ir
                .decls
                .get(decl.index())
                .map_or("an unknown error", |d| d.name.as_str());
            with(name, pattern)
        }
    }
}

/// Compares what the pipeline answered with what the example expects (section 12.1).
pub(super) fn compare(
    ir: &Ir,
    endpoint: &Endpoint,
    expected: &Expected,
    outcome: &Outcome,
    result: Option<&Json>,
) -> Verdict {
    let fail = |differences: Vec<Difference>| {
        Verdict::Failed(Mismatch {
            expected: render_expected(ir, expected),
            actual: render_actual(outcome, result),
            differences,
        })
    };
    match (expected, outcome) {
        (Expected::Ok(None), Outcome::Passes) => Verdict::Passed,
        (Expected::Ok(Some(pattern)), Outcome::Passes) => match (result, &endpoint.success.ty) {
            (None, _) => Verdict::Skipped("`ok` with a value needs `returns` offline".to_owned()),
            (Some(actual), Some(ty)) => {
                differences_or_pass(diff_pattern(ir, ty, pattern, actual), &fail)
            }
            (Some(actual), None) => {
                let ty = Type::Scalar(veld_ir::Scalar::Json);
                differences_or_pass(diff_pattern(ir, &ty, pattern, actual), &fail)
            }
        },
        (Expected::Error { decl, pattern }, Outcome::Error(a)) if *decl == a.decl => {
            match pattern {
                None => Verdict::Passed,
                Some(p) => {
                    let ty = Type::Decl(a.decl);
                    differences_or_pass(diff_pattern(ir, &ty, p, &a.data), &fail)
                }
            }
        }
        (Expected::Ok(_), Outcome::Error(_))
        | (Expected::Error { .. }, Outcome::Passes | Outcome::Error(_)) => fail(Vec::new()),
    }
}

fn differences_or_pass(d: Vec<Difference>, fail: &impl Fn(Vec<Difference>) -> Verdict) -> Verdict {
    if d.is_empty() {
        Verdict::Passed
    } else {
        fail(d)
    }
}
