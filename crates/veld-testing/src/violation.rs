//! `veld test --from-violation` (`spec/rules.md` section 17.3): an `example` block from the
//! event a server reported to `onRuleViolation` with a capture.
//!
//! The capture holds the request in wire form, minus the values of `@pii` and `@secret` fields
//! (`redacted`, section 19.3). They are filled in by the rules of section 12.2, and the tool says
//! which ones, and whether the example still reproduces the violation.

use std::collections::BTreeSet;

use serde_json::{Map, Value as Json};
use veld_ir::{Endpoint, Ir, Service, Type};
use veld_rules::helpers::instant::parse_datetime;
use veld_rules::{Completer, Outcome, OutputMode, Pipeline, Reported, Request, describe_actual};

use crate::fuzz::render_example;

/// Why an event cannot be turned into an example (`spec/rules.md` section 17.3).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ViolationError {
    #[error("the event has no capture; turn the capture option of the server on")]
    NoCapture,
    #[error("the contract has no endpoint with the signature {0}")]
    UnknownOperation(String),
    #[error("the capture does not fit the contract: {0}")]
    DoesNotFit(String),
}

/// The example, and what the tool has to say about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FromViolation {
    /// The `example` block, ready to paste into a contract.
    pub block: String,
    /// One line each, without the leading `note: `.
    pub notes: Vec<String>,
}

fn misfit<T>(what: impl Into<String>) -> Result<T, ViolationError> {
    Err(ViolationError::DoesNotFit(what.into()))
}

/// The capture of `event`, an event of `onRuleViolation` as JSON.
///
/// # Errors
/// [`ViolationError`]: no capture, an operation the contract does not have, or a capture that
/// does not fit the contract.
pub fn from_violation(ir: &Ir, event: &Json) -> Result<FromViolation, ViolationError> {
    let capture = match event.get("capture") {
        None | Some(Json::Null) => return Err(ViolationError::NoCapture),
        Some(Json::Object(c)) => c,
        Some(_) => return misfit("`capture` is not an object"),
    };
    let Some(sig) = capture.get("operation").and_then(Json::as_str) else {
        return misfit("`capture.operation` is missing");
    };
    let (service, ep) =
        find_endpoint(ir, sig).ok_or_else(|| ViolationError::UnknownOperation(sig.to_owned()))?;
    let (request, filled) = request_of(ir, service, ep, capture)?;
    let pipeline = Pipeline::new(ir, service, &ep.name)
        .map_err(|e| ViolationError::DoesNotFit(e.to_string()))?;
    let reported = pipeline
        .run_reported(&request, OutputMode::Strict)
        .map_err(|e| ViolationError::DoesNotFit(e.to_string()))?;
    let rule = rule_of(event);
    let name = example_name(rule.as_deref(), &ep.name);
    let block =
        render_example(ir, service, ep, &request, &reported.outcome, &name).ok_or_else(|| {
            ViolationError::DoesNotFit(
                "a value cannot be written in the syntax of an example".to_owned(),
            )
        })?;
    let notes = notes_of(
        event,
        rule.as_deref(),
        &filled,
        &reported,
        !filled.is_empty(),
    );
    Ok(FromViolation { block, notes })
}

fn find_endpoint<'a>(ir: &'a Ir, sig: &str) -> Option<(&'a Service, &'a Endpoint)> {
    ir.services
        .iter()
        .flat_map(|s| s.endpoints.iter().map(move |e| (s, e)))
        .find(|(_, e)| e.sig == sig)
}

/// The id of the rule the event is about.
fn rule_of(event: &Json) -> Option<String> {
    let text = event
        .get("rule")
        .or_else(|| event.get("violation").and_then(|v| v.get("rule")))?;
    text.as_str().map(str::to_owned)
}

/// `NewBooking.atMost30Days` out of `veld:bookings/NewBooking.atMost30Days@3f2a…`.
fn example_name(rule: Option<&str>, endpoint: &str) -> String {
    let short = rule.map(|id| {
        let after = id.rsplit_once('/').map_or(id, |(_, b)| b);
        after.split('@').next().unwrap_or(after).to_owned()
    });
    match short {
        Some(s) => format!("violation of {s}"),
        None => format!("violation captured on {endpoint}"),
    }
}

// ---- the request ------------------------------------------------------------------------

/// A step of a redacted path.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    Key(String),
    Index(usize),
}

/// `.a.b[1].c` as steps.
fn steps(rest: &str, whole: &str) -> Result<Vec<Step>, ViolationError> {
    let bad = || {
        ViolationError::DoesNotFit(format!(
            "`redacted` has the path `{whole}`, which is not a path"
        ))
    };
    let mut out = Vec::new();
    let mut chars = rest.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '.' => {
                let mut key = String::new();
                while let Some(&n) = chars.peek() {
                    if n == '.' || n == '[' {
                        break;
                    }
                    key.push(n);
                    chars.next();
                }
                if key.is_empty() {
                    return Err(bad());
                }
                out.push(Step::Key(key));
            }
            '[' => {
                let digits: String = chars.by_ref().take_while(|d| *d != ']').collect();
                out.push(Step::Index(digits.parse().map_err(|_| bad())?));
            }
            _ => return Err(bad()),
        }
    }
    Ok(out)
}

/// Takes the value at `path` out of its parent. A path that leads nowhere removes nothing.
fn remove(root: &mut Json, path: &[Step]) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let mut at = root;
    for s in parents {
        let next = match s {
            Step::Key(k) => at.get_mut(k.as_str()),
            Step::Index(i) => at.get_mut(*i),
        };
        match next {
            Some(n) => at = n,
            None => return,
        }
    }
    if let (Step::Key(k), Some(map)) = (last, at.as_object_mut()) {
        map.remove(k.as_str());
    }
}

/// Where a redacted path starts: the root word, the name after it, and the rest.
fn split_root(path: &str) -> Result<(&str, &str, &str), ViolationError> {
    let (root, after) = if let Some(a) = path.strip_prefix("params.") {
        ("params", a)
    } else if let Some(a) = path.strip_prefix("facts.") {
        ("facts", a)
    } else if path == "principal"
        || path.starts_with("principal.")
        || path.starts_with("principal[")
    {
        return Ok(("principal", "", &path["principal".len()..]));
    } else {
        return misfit(format!(
            "`redacted` has the path `{path}`, which starts neither with params, facts nor principal"
        ));
    };
    let end = after.find(['.', '[']).unwrap_or(after.len());
    Ok((root, &after[..end], &after[end..]))
}

/// The three parts of the request as the capture holds them, redacted values taken out.
struct Parts {
    params: Map<String, Json>,
    facts: Map<String, Json>,
    principal: Json,
}

fn object_of(capture: &Map<String, Json>, key: &str) -> Result<Map<String, Json>, ViolationError> {
    match capture.get(key) {
        None | Some(Json::Null) => Ok(Map::new()),
        Some(Json::Object(m)) => Ok(m.clone()),
        Some(_) => misfit(format!("`capture.{key}` is not an object")),
    }
}

fn parts_of(capture: &Map<String, Json>, redacted: &[String]) -> Result<Parts, ViolationError> {
    let mut parts = Parts {
        params: object_of(capture, "params")?,
        facts: object_of(capture, "facts")?,
        principal: capture.get("principal").cloned().unwrap_or(Json::Null),
    };
    for path in redacted {
        let (root, name, rest) = split_root(path)?;
        let tail = steps(rest, path)?;
        match root {
            "params" => {
                if let Some(v) = parts.params.get_mut(name) {
                    remove(v, &tail);
                }
            }
            "facts" => {
                if let Some(v) = parts.facts.get_mut(name) {
                    remove(v, &tail);
                }
            }
            _ => remove(&mut parts.principal, &tail),
        }
    }
    Ok(parts)
}

fn redacted_of(capture: &Map<String, Json>) -> Result<Vec<String>, ViolationError> {
    match capture.get("redacted") {
        None | Some(Json::Null) => Ok(Vec::new()),
        Some(Json::Array(items)) => items
            .iter()
            .map(|i| match i.as_str() {
                Some(s) => Ok(s.to_owned()),
                None => misfit("`redacted` holds something that is not a path"),
            })
            .collect(),
        Some(_) => misfit("`capture.redacted` is not a list"),
    }
}

/// The request of the capture, with the redacted values filled in, and the paths filled in.
fn request_of(
    ir: &Ir,
    service: &Service,
    ep: &Endpoint,
    capture: &Map<String, Json>,
) -> Result<(Request, BTreeSet<String>), ViolationError> {
    let now_text = capture.get("now").and_then(Json::as_str);
    let now = now_text.and_then(parse_datetime).ok_or_else(|| {
        ViolationError::DoesNotFit("`capture.now` is not RFC 3339 text".to_owned())
    })?;
    let redacted = redacted_of(capture)?;
    let parts = parts_of(capture, &redacted)?;
    let mut filler = Completer::new(ir, now);
    let mut filled = BTreeSet::new();
    let mut request = Request {
        now,
        ..Request::default()
    };
    for (name, value) in &parts.params {
        let Some(p) = ep.params.iter().find(|p| &p.name == name && !p.stream) else {
            return misfit(format!("`{name}` is not a parameter of {}", ep.name));
        };
        let (full, paths) = complete(
            &mut filler,
            &p.ty,
            value,
            &format!("params.{name}"),
            &redacted,
        )?;
        filled.extend(paths);
        request.params.insert(name.clone(), full);
    }
    for p in ep.params.iter().filter(|p| !p.optional && !p.stream) {
        if !request.params.contains_key(&p.name) {
            return misfit(format!("the required parameter `{}` is missing", p.name));
        }
    }
    for (name, value) in &parts.facts {
        let Some(f) = ep.rules.facts.iter().find(|f| &f.name == name) else {
            return misfit(format!("`{name}` is not a fact of {}", ep.name));
        };
        let full = if value.is_null() {
            None
        } else {
            let (full, paths) = complete(
                &mut filler,
                &f.ty,
                value,
                &format!("facts.{name}"),
                &redacted,
            )?;
            filled.extend(paths);
            Some(full)
        };
        request.facts.insert(name.clone(), full);
    }
    request.principal = principal_of(
        &mut filler,
        service,
        &parts.principal,
        &redacted,
        &mut filled,
    )?;
    Ok((request, filled))
}

fn principal_of(
    filler: &mut Completer<'_>,
    service: &Service,
    value: &Json,
    redacted: &[String],
    filled: &mut BTreeSet<String>,
) -> Result<Option<Json>, ViolationError> {
    if value.is_null() {
        return Ok(None);
    }
    let Some(p) = &service.principal else {
        return misfit("the capture has a principal and the contract has none");
    };
    let (full, paths) = complete(filler, &Type::Decl(p.decl), value, "principal", redacted)?;
    filled.extend(paths);
    Ok(Some(full))
}

/// Completes `value`. A required value the capture does not have, and `redacted` does not name,
/// does not fit the contract. Returns the value and the paths filled in under `redacted`.
fn complete(
    filler: &mut Completer<'_>,
    ty: &Type,
    value: &Json,
    root: &str,
    redacted: &[String],
) -> Result<(Json, BTreeSet<String>), ViolationError> {
    let done = filler
        .complete(ty, value, root)
        .map_err(|e| ViolationError::DoesNotFit(e.to_string()))?;
    let named = |p: &String| redacted.iter().any(|r| under(p, r));
    if let Some(path) = done.missing.iter().find(|p| !named(p)) {
        return misfit(format!(
            "`{path}` is required and the capture does not have it, and `redacted` does not name it"
        ));
    }
    let filled = done.filled.into_iter().filter(named).collect();
    Ok((done.value, filled))
}

/// True when `path` is `redacted` or lies below it.
fn under(path: &str, redacted: &str) -> bool {
    path == redacted
        || path
            .strip_prefix(redacted)
            .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('['))
}

// ---- what the tool says -----------------------------------------------------------------

fn notes_of(
    event: &Json,
    rule: Option<&str>,
    filled: &BTreeSet<String>,
    reported: &Reported,
    was_filled: bool,
) -> Vec<String> {
    let mut notes: Vec<String> = filled.iter().map(|p| format!("filled in {p}")).collect();
    if event.get("position").and_then(Json::as_str) == Some("output") {
        notes.push(
            "the violation is of an output rule; add a returns line with the result the handler gave"
                .to_owned(),
        );
    }
    if let Some(rule) = rule.filter(|r| !reproduces(reported, r)) {
        let mut text = format!(
            "the example does not reproduce the violation of {rule}; it ends in {}",
            describe_actual(&reported.outcome, None)
        );
        if was_filled {
            text.push_str(": a value was filled in, so the original values may matter");
        }
        notes.push(text);
    }
    notes
}

/// True when the outcome, or the audited reports, name the rule.
fn reproduces(reported: &Reported, rule: &str) -> bool {
    let in_outcome = match &reported.outcome {
        Outcome::Passes => false,
        Outcome::Error(a) => a
            .violations
            .iter()
            .chain(&a.hook)
            .any(|v| v.rule_id == rule),
    };
    in_outcome
        || reported.audited.iter().any(|v| v.rule_id == rule)
        || reported.hook.iter().any(|v| v.rule_id == rule)
}
