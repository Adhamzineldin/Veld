//! Comparing an answer with a whole outcome of the reference pipeline (the fuzzer's judge).
//!
//! [`crate::judge`] compares an answer with what an example writes down. A generated request has
//! no example: its expected outcome is what `veld_rules::Pipeline::run` says, including the
//! violations, so those are compared here too.

use serde_json::Value as Json;
use veld_ir::{Ir, Type};
use veld_rules::{Difference, ErrorAnswer, Outcome, Violation, diff_pattern};

use crate::judge::{Answer, outcome_of, problem_of};
use crate::request::PASSED;

fn diff(path: &str, expected: impl ToString, actual: impl ToString) -> Difference {
    Difference {
        path: path.to_owned(),
        expected: expected.to_string(),
        actual: actual.to_string(),
    }
}

fn text<'a>(v: &'a Json, key: &str) -> &'a str {
    v.get(key).and_then(Json::as_str).unwrap_or("")
}

/// One violation as text: the rule id, the path, the fields, the message and, for an evaluation
/// error, its kind.
struct ViolationText<'a> {
    rule: &'a str,
    path: &'a str,
    fields: String,
    message: &'a str,
    error: Option<&'a str>,
}

impl ViolationText<'_> {
    fn text(&self) -> String {
        let error = self
            .error
            .map_or_else(String::new, |e| format!(" (error {e})"));
        format!(
            "{} at {} reads {}: {}{error}",
            self.rule, self.path, self.fields, self.message
        )
    }
}

/// The violations of an answer as text, in the order of the answer (section 7).
fn answer_violations(body: Option<&Json>) -> Vec<String> {
    let list = body
        .and_then(|b| b.get("violations"))
        .and_then(Json::as_array);
    let one = |v: &Json| {
        ViolationText {
            rule: text(v, "rule"),
            path: text(v, "path"),
            fields: v.get("fields").map_or_else(String::new, Json::to_string),
            message: text(v, "message"),
            error: v.get("error").and_then(Json::as_str),
        }
        .text()
    };
    list.map(|vs| vs.iter().map(one).collect())
        .unwrap_or_default()
}

fn expected_violations(vs: &[Violation]) -> Vec<String> {
    vs.iter()
        .map(|v| {
            ViolationText {
                rule: &v.rule_id,
                path: &v.path,
                fields: serde_json::to_string(&v.fields).unwrap_or_default(),
                message: &v.message,
                error: v.error.map(veld_rules::EvalError::as_str),
            }
            .text()
        })
        .collect()
}

/// The issues of a `ValidationError` as text, in the order of the answer.
fn issues(data: &Json) -> Vec<String> {
    let list = data.get("issues").and_then(Json::as_array);
    let one = |i: &Json| {
        format!(
            "{} {} {}",
            text(i, "path"),
            text(i, "code"),
            text(i, "message")
        )
    };
    list.map(|is| is.iter().map(one).collect())
        .unwrap_or_default()
}

/// Two lists that must be equal entry by entry: the order of violations and issues is part of
/// the answer (section 5.2).
fn same_list(path: &str, expected: &[String], actual: &[String]) -> Vec<Difference> {
    if expected == actual {
        return Vec::new();
    }
    let (mut e, mut a) = (expected.to_vec(), actual.to_vec());
    e.sort();
    a.sort();
    if e == a {
        return vec![diff(
            path,
            format!("in this order: [{}]", expected.join("; ")),
            format!("the same entries in another order: [{}]", actual.join("; ")),
        )];
    }
    let show = |items: Vec<&String>| {
        let items: Vec<&str> = items.into_iter().map(String::as_str).collect();
        format!("[{}]", items.join("; "))
    };
    let missing = show(expected.iter().filter(|e| !actual.contains(e)).collect());
    let extra = show(actual.iter().filter(|a| !expected.contains(a)).collect());
    vec![diff(
        path,
        format!("{} entries; not in the answer: {missing}", expected.len()),
        format!("{} entries; not expected: {extra}", actual.len()),
    )]
}

/// Compares what a server answered to a request sent in dry-run mode with the outcome of
/// `veld_rules::Pipeline::run` without a result. An empty list is a match.
///
/// A pass is `204` with `Veld-Test-Passed: 1`. An error must have the declared type and status,
/// the payload of its fields (by the value semantics of section 3.4 of `spec/rules.md`) and the
/// same violations in the same order, each by rule id, path, fields, message and evaluation
/// error; a `ValidationError` also the same issues in the same order.
#[must_use]
pub fn judge_outcome(ir: &Ir, expected: &Outcome, answer: &Answer) -> Vec<Difference> {
    match expected {
        Outcome::Passes => judge_pass(ir, answer),
        Outcome::Error(a) => judge_error(ir, a, answer),
    }
}

fn judge_pass(ir: &Ir, answer: &Answer) -> Vec<Difference> {
    let mut out = Vec::new();
    if answer.status != 204 {
        out.push(diff("status", 204, answer.status));
    }
    match answer.header(PASSED) {
        Some("1") => {}
        Some(_) => out.push(diff(&format!("header {PASSED}"), "1", "something else")),
        None => out.push(diff(&format!("header {PASSED}"), "1", "missing")),
    }
    if answer.status != 204
        && let Some(Outcome::Error(e)) = outcome_of(ir, answer)
    {
        out.push(diff("$.type", "a pass", e.name));
    }
    out
}

fn judge_error(ir: &Ir, a: &ErrorAnswer, answer: &Answer) -> Vec<Difference> {
    let Some(decl) = ir.decls.get(a.decl.index()) else {
        return vec![diff("$.type", "a declared error", "an unknown declaration")];
    };
    let Some(problem) = problem_of(answer) else {
        let seen = if answer.status == 204 {
            "a pass (204)"
        } else {
            "no problem document"
        };
        return vec![diff("$.type", &decl.sig, seen)];
    };
    let mut out = Vec::new();
    if problem.ty != decl.sig {
        out.push(diff("$.type", &decl.sig, &problem.ty));
    }
    if a.status != answer.status {
        out.push(diff("status", a.status, answer.status));
    }
    let body = answer.json();
    out.extend(same_list(
        "violations",
        &expected_violations(&a.violations),
        &answer_violations(body.as_ref()),
    ));
    if a.name == "ValidationError" {
        out.extend(same_list(
            "$.data.issues",
            &issues(&a.data),
            &issues(&problem.data),
        ));
    } else {
        let ty = Type::Decl(a.decl);
        out.extend(diff_pattern(ir, &ty, &a.data, &problem.data));
    }
    out
}

/// The answer as one line of text, for a failure message.
#[must_use]
pub fn describe_answer(ir: &Ir, answer: &Answer) -> String {
    let body: String = answer.body.chars().take(400).collect();
    match outcome_of(ir, answer) {
        Some(Outcome::Passes) => format!("HTTP {} (passed)", answer.status),
        Some(Outcome::Error(e)) => format!("HTTP {} {} {body}", answer.status, e.name),
        None => format!("HTTP {} {body}", answer.status),
    }
}
