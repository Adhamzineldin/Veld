//! Comparing a server's answer with what an example expects (`spec/rules.md` section 12.1).

use serde_json::Value as Json;
use veld_ir::{DeclId, DeclKind, Endpoint, Ir, Scalar, Type};
use veld_rules::{
    Difference, ErrorAnswer, ExampleReport, Expected, FilledExample, Mismatch, Outcome, Verdict,
    describe_actual, describe_expected, diff_pattern,
};

use crate::request::{PASSED, endpoint_of};

/// What a server answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub status: u16,
    /// Header names in lower case.
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Answer {
    /// The value of a header (ASCII case-insensitive).
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub(crate) fn json(&self) -> Option<Json> {
        serde_json::from_str(&self.body).ok()
    }
}

/// An error answer: a problem document of section 5.5 of `spec/ir.md`.
pub(crate) struct Problem {
    pub ty: String,
    pub status: Option<u64>,
    pub data: Json,
}

pub(crate) fn problem_of(answer: &Answer) -> Option<Problem> {
    let doc = answer.json()?;
    let ty = doc.get("type")?.as_str()?.to_owned();
    Some(Problem {
        ty,
        status: doc.get("status").and_then(Json::as_u64),
        data: doc
            .get("data")
            .cloned()
            .unwrap_or_else(|| Json::Object(Default::default())),
    })
}

/// The declared error whose signature is `sig`.
fn decl_by_sig<'a>(ir: &'a Ir, sig: &str) -> Option<&'a veld_ir::Decl> {
    ir.decls
        .iter()
        .find(|d| d.sig == sig && matches!(d.kind, DeclKind::Error(_)))
}

/// What the answer is, as far as the IR describes it.
pub(crate) fn outcome_of(ir: &Ir, answer: &Answer) -> Option<Outcome> {
    if (200..300).contains(&answer.status) {
        return Some(Outcome::Passes);
    }
    let problem = problem_of(answer)?;
    let decl = decl_by_sig(ir, &problem.ty)?;
    Some(Outcome::Error(ErrorAnswer {
        decl: decl.id,
        name: decl.name.clone(),
        status: answer.status,
        data: problem.data,
        violations: Vec::new(),
        hook: Vec::new(),
    }))
}

/// The answer as text, for a failure message.
fn describe(outcome: Option<&Outcome>, answer: &Answer, body: Option<&Json>) -> String {
    match outcome {
        Some(o) => describe_actual(o, body.filter(|_| matches!(o, Outcome::Passes))),
        None => {
            let text: String = answer.body.chars().take(200).collect();
            format!("HTTP {} {text}", answer.status)
        }
    }
}

struct Judge<'a> {
    ir: &'a Ir,
    endpoint: &'a Endpoint,
    expected: &'a Expected,
    answer: &'a Answer,
    outcome: Option<Outcome>,
}

impl Judge<'_> {
    fn fail(&self, differences: Vec<Difference>) -> Verdict {
        let body = self.answer.json();
        Verdict::Failed(Mismatch {
            expected: describe_expected(self.ir, self.expected),
            actual: describe(self.outcome.as_ref(), self.answer, body.as_ref()),
            differences,
        })
    }

    fn pattern(&self, ty: &Type, pattern: Option<&Json>, actual: &Json) -> Verdict {
        let diffs = pattern.map_or_else(Vec::new, |p| diff_pattern(self.ir, ty, p, actual));
        if diffs.is_empty() {
            Verdict::Passed
        } else {
            self.fail(diffs)
        }
    }

    fn ok(&self, pattern: Option<&Json>, dry: bool) -> Verdict {
        if dry {
            return self.dry_ok(pattern);
        }
        let status = self.endpoint.success.status;
        if self.answer.status != status {
            return self.fail(vec![status_difference(status, self.answer.status)]);
        }
        let ty = self.endpoint.success.ty.clone();
        let ty = ty.unwrap_or(Type::Scalar(Scalar::Json));
        match (pattern, self.answer.json()) {
            (None, _) => Verdict::Passed,
            (Some(_), None) => self.fail(vec![Difference {
                path: "$".to_owned(),
                expected: "a JSON body".to_owned(),
                actual: "a body that is not JSON".to_owned(),
            }]),
            (Some(p), Some(body)) => self.pattern(&ty, Some(p), &body),
        }
    }

    /// A dry run answers `204` and `Veld-Test-Passed: 1`, and has no body to match.
    fn dry_ok(&self, pattern: Option<&Json>) -> Verdict {
        if pattern.is_some() {
            return Verdict::Skipped("`ok` with a value needs `returns` in a dry run".to_owned());
        }
        let passed = self.answer.header(PASSED) == Some("1");
        if self.answer.status == 204 && passed {
            return Verdict::Passed;
        }
        let mut diffs = Vec::new();
        if self.answer.status != 204 {
            diffs.push(status_difference(204, self.answer.status));
        }
        if !passed {
            diffs.push(Difference {
                path: format!("header {PASSED}"),
                expected: "1".to_owned(),
                actual: self
                    .answer
                    .header(PASSED)
                    .map_or("missing", |_| "something else")
                    .to_owned(),
            });
        }
        self.fail(diffs)
    }

    fn error(&self, decl: DeclId, pattern: Option<&Json>) -> Verdict {
        let Some(expected_decl) = self.ir.decls.get(decl.index()) else {
            return self.fail(Vec::new());
        };
        let Some(problem) = problem_of(self.answer) else {
            return self.fail(vec![Difference {
                path: "$.type".to_owned(),
                expected: expected_decl.sig.clone(),
                actual: "no problem document".to_owned(),
            }]);
        };
        let mut diffs = Vec::new();
        if problem.ty != expected_decl.sig {
            diffs.push(Difference {
                path: "$.type".to_owned(),
                expected: expected_decl.sig.clone(),
                actual: problem.ty.clone(),
            });
        }
        if let Some(status) = self.expected_status(decl)
            && status != self.answer.status
        {
            diffs.push(status_difference(status, self.answer.status));
        }
        if problem
            .status
            .is_some_and(|s| s != u64::from(self.answer.status))
        {
            diffs.push(Difference {
                path: "$.status".to_owned(),
                expected: self.answer.status.to_string(),
                actual: problem.status.map_or_else(String::new, |s| s.to_string()),
            });
        }
        if diffs.is_empty() {
            return self.pattern(&Type::Decl(decl), pattern, &problem.data);
        }
        self.fail(diffs)
    }

    /// The status the endpoint gives the error: its own list first, then the error's default.
    fn expected_status(&self, decl: DeclId) -> Option<u16> {
        let listed = self.endpoint.errors.iter().find(|e| e.decl == decl);
        listed
            .map(|e| e.status)
            .or_else(|| match &self.ir.decls.get(decl.index())?.kind {
                DeclKind::Error(e) => e.default_status,
                DeclKind::Model(_)
                | DeclKind::Enum(_)
                | DeclKind::Union(_)
                | DeclKind::Const(_) => None,
            })
    }
}

fn status_difference(expected: impl ToString, actual: impl ToString) -> Difference {
    Difference {
        path: "status".to_owned(),
        expected: expected.to_string(),
        actual: actual.to_string(),
    }
}

/// Compares the answer of a server with the expected outcome of `filled`. `dry_run` says the
/// example was sent with `Veld-Test-Dry-Run: 1`.
#[must_use]
pub fn judge(ir: &Ir, filled: &FilledExample, dry_run: bool, answer: &Answer) -> ExampleReport {
    let outcome = outcome_of(ir, answer);
    let verdict = match endpoint_of(ir, filled) {
        Some(endpoint) => {
            let j = Judge {
                ir,
                endpoint,
                expected: &filled.expected,
                answer,
                outcome: outcome.clone(),
            };
            let dry = dry_run && filled.request.result.is_none();
            match &filled.expected {
                Expected::Ok(p) => j.ok(p.as_ref(), dry),
                Expected::Error { decl, pattern } => j.error(*decl, pattern.as_ref()),
            }
        }
        None => Verdict::Skipped(format!("no endpoint `{}`", filled.endpoint)),
    };
    ExampleReport {
        name: filled.name.clone(),
        verdict,
        outcome,
    }
}
