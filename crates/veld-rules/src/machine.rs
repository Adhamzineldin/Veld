//! State machines (`spec/rules.md` section 10): the helpers of section 10.3, the transition guard
//! that runs before the handler and the target check that runs after it.
//!
//! The helpers are public: the generators mirror them in every target, and their tests compare
//! against these.

use serde_json::{Value as Json, json};
use veld_ir::{DeclKind, Endpoint, EndpointTransition, Enum, Ir, ServiceId, Type};

use crate::check::{Options, Violation, check_type};
use crate::error::InputError;
use crate::helpers::instant::parse_datetime;

/// The states that can follow `state`, in declaration order of the edges (`nextStates`).
///
/// `state` and the result are variant names. A state that is unknown, or final, has none.
#[must_use]
pub fn next_states<'a>(machine: &'a Enum, state: &str) -> Vec<&'a str> {
    machine
        .flows
        .iter()
        .filter(|f| f.from == state)
        .map(|f| f.to.as_str())
        .collect()
}

/// The timed edges of a machine as `(from, to, milliseconds)`, in the order of the edges: what
/// `timedMoves()` answers in a generated target (`spec/platform.md` section 10.1).
#[must_use]
pub fn timed_moves(machine: &Enum) -> Vec<(&str, &str, i64)> {
    machine
        .flows
        .iter()
        .filter_map(|f| {
            let ms = f.after_ms.as_deref()?.parse::<i64>().ok()?;
            Some((f.from.as_str(), f.to.as_str(), ms))
        })
        .collect()
}

/// An endpoint with a timed transition: what a due-moves runner asks its loader for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedEndpoint {
    pub service: ServiceId,
    /// The name of the service.
    pub service_name: String,
    pub endpoint: String,
    /// The fact of the transition and the wire path of its `since` field below it.
    pub fact: String,
    pub since: Vec<String>,
    /// The source and the target, by wire value.
    pub from: String,
    pub to: String,
    /// The milliseconds of the edge: the loader is asked for calls whose fact entered the source
    /// state at or before `now - after_ms`.
    pub after_ms: i64,
}

/// Every endpoint of `ir` that takes a timed edge, in service and endpoint order. This is what
/// `run<Service>DueMoves` iterates in a generated server and what `veld check` lists.
#[must_use]
pub fn timed_endpoints(ir: &Ir) -> Vec<TimedEndpoint> {
    let mut out = Vec::new();
    for service in &ir.services {
        for ep in &service.endpoints {
            let Some(t) = ep.rules.transition.as_ref().filter(|t| t.is_timed()) else {
                continue;
            };
            let [source] = t.from.as_slice() else {
                continue;
            };
            let Some(after_ms) = edge_ms(ir, t, source) else {
                continue;
            };
            let (Ok(from), Ok(to)) = (wire_of(ir, t, source), wire_of(ir, t, &t.to)) else {
                continue;
            };
            out.push(TimedEndpoint {
                service: service.id,
                service_name: service.name.clone(),
                endpoint: ep.name.clone(),
                fact: t.fact.clone(),
                since: t.since.iter().map(|s| s.wire.clone()).collect(),
                from,
                to,
                after_ms,
            });
        }
    }
    out
}

/// The milliseconds of the edge `source -> t.to`, when it is timed.
fn edge_ms(ir: &Ir, t: &EndpointTransition, source: &str) -> Option<i64> {
    let machine = machine_of(ir, t).ok()?;
    let flow = machine
        .flows
        .iter()
        .find(|f| f.from == source && f.to == t.to)?;
    flow.after_ms.as_deref()?.parse().ok()
}

/// Whether `state` is one of the final states of the machine.
#[must_use]
pub fn is_final(machine: &Enum, state: &str) -> bool {
    machine.finals.iter().any(|f| f == state)
}

/// What the guard found out when the current state is not one of the sources.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Refusal {
    /// The `field`, `from` and `to` of the `InvalidTransition` error, in wire form.
    pub data: Json,
    pub violation: Violation,
}

/// The wire value of variant `name` of the machine `decl`.
fn wire_of(ir: &Ir, t: &EndpointTransition, name: &str) -> Result<String, InputError> {
    let machine = machine_of(ir, t)?;
    machine
        .variants
        .iter()
        .find(|v| v.name == name)
        .map(|v| v.wire.clone())
        .ok_or_else(|| InputError::Ir(format!("`{name}` is not a state of the machine")))
}

fn machine_of<'a>(ir: &'a Ir, t: &EndpointTransition) -> Result<&'a Enum, InputError> {
    match ir.decls.get(t.decl.index()).map(|d| &d.kind) {
        Some(DeclKind::Enum(e)) => Ok(e),
        Some(
            DeclKind::Model(_)
            | DeclKind::Error(_)
            | DeclKind::Union(_)
            | DeclKind::Const(_)
            | DeclKind::Event(_),
        )
        | None => Err(InputError::Ir(
            "a transition names something that is not an enum".to_owned(),
        )),
    }
}

/// The wire path of the field inside the fact, segments joined with `.`.
fn field_path(t: &EndpointTransition) -> String {
    let wires: Vec<&str> = t.steps.iter().map(|s| s.wire.as_str()).collect();
    wires.join(".")
}

/// The string the steps of `t` reach inside `value`.
fn read_state<'v>(
    t: &EndpointTransition,
    value: &'v Json,
    what: &str,
) -> Result<&'v str, InputError> {
    let mut cur = value;
    let mut path = what.to_owned();
    for step in &t.steps {
        path = format!("{path}.{}", step.wire);
        cur = cur
            .get(&step.wire)
            .ok_or_else(|| InputError::MissingField {
                path: path.clone(),
                field: step.wire.clone(),
            })?;
    }
    cur.as_str()
        .ok_or_else(|| InputError::mismatch("a state", &path))
}

fn violation(t: &EndpointTransition, message: String) -> Violation {
    Violation {
        rule_name: "transition".to_owned(),
        rule_id: t.id.clone(),
        path: "request".to_owned(),
        fields: Vec::new(),
        message,
        error: None,
    }
}

/// The guard (section 10.2): `None` when the current state of `fact` is one of the sources.
///
/// # Errors
/// [`InputError`] when the fact has no such field or the IR is not well formed.
pub(crate) fn guard(
    ir: &Ir,
    endpoint: &Endpoint,
    t: &EndpointTransition,
    fact: &Json,
    now: i64,
) -> Result<Option<Refusal>, InputError> {
    // A fact is a value of its type: a field it leaves out has its default.
    let ty = endpoint
        .rules
        .facts
        .iter()
        .find(|f| f.name == t.fact)
        .map(|f| &f.ty);
    let full = match ty {
        Some(ty) => {
            check_type(ir, ty, fact, Options::default())
                .map_err(|e| e.at_path(&t.fact))?
                .0
        }
        None => fact.clone(),
    };
    let current = read_state(t, &full, &t.fact)?;
    let mut sources = Vec::with_capacity(t.from.len());
    for s in &t.from {
        sources.push(wire_of(ir, t, s)?);
    }
    let to = wire_of(ir, t, &t.to)?;
    let data = json!({ "field": field_path(t), "from": current, "to": to });
    let refuse = |message: &str| {
        Ok(Some(Refusal {
            data: data.clone(),
            violation: violation(t, message.to_owned()),
        }))
    };
    if !sources.iter().any(|s| s == current) {
        return refuse("violates rule transition");
    }
    if t.is_timed() && !is_due(ir, t, &full, now)? {
        return refuse("violates rule transition: not due");
    }
    Ok(None)
}

/// The due check of a timed transition (`spec/platform.md` section 10): `now - since >= d`, where
/// `d` is the `after` of the edge. A fact whose `since` is in the future is not due, and so is
/// one whose `since` is not a datetime.
fn is_due(ir: &Ir, t: &EndpointTransition, fact: &Json, now: i64) -> Result<bool, InputError> {
    let Some(source) = t.from.first() else {
        return Ok(true);
    };
    let Some(after) = edge_ms(ir, t, source) else {
        return Ok(true);
    };
    let mut cur = fact;
    let mut path = t.fact.clone();
    for step in &t.since {
        path = format!("{path}.{}", step.wire);
        cur = cur
            .get(&step.wire)
            .ok_or_else(|| InputError::MissingField {
                path: path.clone(),
                field: step.wire.clone(),
            })?;
    }
    let text = cur
        .as_str()
        .ok_or_else(|| InputError::mismatch("a datetime", &path))?;
    let Some(since) = parse_datetime(text) else {
        return Ok(false);
    };
    Ok(now.checked_sub(since).is_some_and(|age| age >= after))
}

/// The target check (section 10.2): the violation when `result` does not hold the target state.
///
/// It only applies when the success type of the endpoint is the very declaration of the fact.
///
/// # Errors
/// [`InputError`] when the result has no such field or the IR is not well formed.
pub(crate) fn target(
    ir: &Ir,
    endpoint: &Endpoint,
    result: &Json,
) -> Result<Option<Violation>, InputError> {
    let Some(t) = &endpoint.rules.transition else {
        return Ok(None);
    };
    let fact_ty = endpoint
        .rules
        .facts
        .iter()
        .find(|f| f.name == t.fact)
        .map(|f| &f.ty);
    let same = matches!(
        (fact_ty, endpoint.success.ty.as_ref()),
        (Some(Type::Decl(a)), Some(Type::Decl(b))) if a == b
    );
    if !same || endpoint.success.stream {
        return Ok(None);
    }
    let want = wire_of(ir, t, &t.to)?;
    let got = read_state(t, result, "result")?;
    if got == want {
        return Ok(None);
    }
    let message = format!(
        "the result must hold the state `{want}` in `{}`, found `{got}`",
        field_path(t)
    );
    Ok(Some(violation(t, message)))
}

#[cfg(test)]
mod tests;
