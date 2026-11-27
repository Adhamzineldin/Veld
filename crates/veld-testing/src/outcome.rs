//! Comparing an answer with a whole outcome of the reference pipeline (the fuzzer's judge).
//!
//! [`crate::judge`] compares an answer with what an example writes down. A generated request has
//! no example: its expected outcome is what `veld_rules::Pipeline::run` says, including the
//! violations, so those are compared here too.

use std::collections::BTreeSet;

use serde_json::Value as Json;
use veld_ir::{DeclKind, Endpoint, Ir, Service, Type};
use veld_rules::{Difference, ErrorAnswer, Outcome, Violation, diff_pattern};

use crate::judge::{Answer, outcome_of, problem_of};
use crate::request::PASSED;

/// The header that carries the rules digest of a service (`spec/rules.md` section 9.7).
const RULES_HEADER: &str = "Veld-Rules";
/// The media type of an error answer (`spec/ir.md` section 5.5).
const PROBLEM_JSON: &str = "application/problem+json";

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

/// Two lists that must hold the same entries. With `ordered` they must also be in the same order:
/// the order of violations and issues is part of the answer (section 5.2), except among the
/// entries of a map, whose order is not specified.
fn same_list(path: &str, expected: &[String], actual: &[String], ordered: bool) -> Vec<Difference> {
    if expected == actual {
        return Vec::new();
    }
    let (mut e, mut a) = (expected.to_vec(), actual.to_vec());
    e.sort();
    a.sort();
    if e == a && !ordered {
        return Vec::new();
    }
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

/// The endpoint an answer came from. The members of a problem document that name the operation,
/// the rules digest of the service and the order of violations are checked against it.
#[derive(Debug, Clone, Copy)]
pub struct Called<'a> {
    pub service: &'a Service,
    pub endpoint: &'a Endpoint,
}

/// Whether a parameter of the endpoint contains a map. The order of the violations among the
/// entries of a map is not specified (`spec/rules.md` section 5.2), so the violations of such an
/// endpoint are compared as a set.
fn params_reach_map(ir: &Ir, ep: &Endpoint) -> bool {
    let mut seen = BTreeSet::new();
    ep.params
        .iter()
        .any(|p| type_reaches_map(ir, &p.ty, &mut seen))
}

fn type_reaches_map(ir: &Ir, ty: &Type, seen: &mut BTreeSet<usize>) -> bool {
    match ty {
        Type::Map(_) => true,
        Type::Scalar(_) => false,
        Type::List(t) | Type::Page(t) => type_reaches_map(ir, t, seen),
        Type::Decl(id) => {
            let Some(decl) = ir.decls.get(id.index()) else {
                return false;
            };
            if !seen.insert(id.index()) {
                return false;
            }
            match &decl.kind {
                DeclKind::Model(_) | DeclKind::Error(_) => decl
                    .fields()
                    .iter()
                    .any(|f| type_reaches_map(ir, &f.ty, seen)),
                DeclKind::Union(u) => u
                    .arms
                    .iter()
                    .any(|arm| type_reaches_map(ir, &Type::Decl(arm.decl), seen)),
                DeclKind::Enum(_) | DeclKind::Const(_) | DeclKind::Event(_) => false,
            }
        }
    }
}

/// The `Veld-Rules` header: a service with a rules digest sends it on every response, errors and
/// dry-run passes included (`spec/rules.md` section 9.7).
fn judge_digest(service: &Service, answer: &Answer) -> Vec<Difference> {
    let Some(digest) = &service.rules_digest else {
        return Vec::new();
    };
    match answer.header(RULES_HEADER) {
        Some(sent) if sent == digest => Vec::new(),
        Some(sent) => vec![diff(&format!("header {RULES_HEADER}"), digest, sent)],
        None => vec![diff(&format!("header {RULES_HEADER}"), digest, "missing")],
    }
}

/// Compares what a server answered with an outcome of `veld_rules::Pipeline::run`. An empty list
/// is a match.
///
/// A pass is `204` with `Veld-Test-Passed: 1` (a request sent in dry-run mode). An error must be
/// the whole problem document of `spec/rules.md` section 7: the media type
/// `application/problem+json`, the declared `type`, `title` and status, the `operation` of the
/// endpoint, the payload of its fields (by the value semantics of section 3.4) and the same
/// violations, each by rule id, path, fields, message and evaluation error; a `ValidationError`
/// also the same issues. Violations and issues must be in the same order unless a parameter of
/// the endpoint contains a map. Either way the answer carries the service's rules digest.
#[must_use]
pub fn judge_outcome(
    ir: &Ir,
    called: Called<'_>,
    expected: &Outcome,
    answer: &Answer,
) -> Vec<Difference> {
    let mut out = match expected {
        Outcome::Passes => judge_pass(ir, answer),
        Outcome::Error(a) => judge_error(ir, called, a, answer),
    };
    out.extend(judge_digest(called.service, answer));
    out
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

/// The members of a problem document that do not depend on the request: the media type, `title`
/// and `operation`.
fn judge_envelope(called: Called<'_>, a: &ErrorAnswer, answer: &Answer) -> Vec<Difference> {
    let mut out = Vec::new();
    let media = answer.header("content-type").unwrap_or("");
    let media = media.split(';').next().unwrap_or("").trim();
    if !media.eq_ignore_ascii_case(PROBLEM_JSON) {
        out.push(diff("header Content-Type", PROBLEM_JSON, media));
    }
    let body = answer.json().unwrap_or(Json::Null);
    if text(&body, "title") != a.name {
        out.push(diff("$.title", &a.name, text(&body, "title")));
    }
    if text(&body, "operation") != called.endpoint.sig {
        let sent = text(&body, "operation");
        out.push(diff("$.operation", &called.endpoint.sig, sent));
    }
    out
}

fn judge_error(ir: &Ir, called: Called<'_>, a: &ErrorAnswer, answer: &Answer) -> Vec<Difference> {
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
    out.extend(judge_envelope(called, a, answer));
    let ordered = !params_reach_map(ir, called.endpoint);
    let body = answer.json();
    out.extend(same_list(
        "violations",
        &expected_violations(&a.violations),
        &answer_violations(body.as_ref()),
        ordered,
    ));
    if a.name == "ValidationError" {
        out.extend(same_list(
            "$.data.issues",
            &issues(&a.data),
            &issues(&problem.data),
            ordered,
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::same_list;

    fn list(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn the_same_entries_in_the_same_order_match() {
        assert!(same_list("v", &list(&["a", "b"]), &list(&["a", "b"]), true).is_empty());
        assert!(same_list("v", &[], &[], true).is_empty());
    }

    #[test]
    fn another_order_differs_only_when_the_order_is_specified() {
        let (expected, actual) = (list(&["a", "b"]), list(&["b", "a"]));
        let found = same_list("v", &expected, &actual, true);
        assert_eq!(found.len(), 1);
        assert!(found[0].actual.contains("another order"), "{:?}", found[0]);
        // Among the entries of a map the order is not specified (section 5.2).
        assert!(same_list("v", &expected, &actual, false).is_empty());
    }

    #[test]
    fn a_missing_an_extra_and_a_repeated_entry_differ_in_both_modes() {
        for ordered in [true, false] {
            assert_eq!(
                same_list("v", &list(&["a", "b"]), &list(&["a"]), ordered).len(),
                1
            );
            assert_eq!(
                same_list("v", &list(&["a"]), &list(&["a", "c"]), ordered).len(),
                1
            );
            assert_eq!(
                same_list("v", &list(&["a"]), &list(&["a", "a"]), ordered).len(),
                1
            );
        }
    }
}
