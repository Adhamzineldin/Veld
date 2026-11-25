//! State machines (`spec/rules.md` section 10): the helpers of section 10.3, the transition guard
//! that runs before the handler and the target check that runs after it.
//!
//! The helpers are public: the generators mirror them in every target, and their tests compare
//! against these.

use serde_json::{Value as Json, json};
use veld_ir::{DeclKind, Endpoint, EndpointTransition, Enum, Ir, Type};

use crate::check::Violation;
use crate::error::InputError;

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
        Some(DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Union(_) | DeclKind::Const(_))
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
    t: &EndpointTransition,
    fact: &Json,
) -> Result<Option<Refusal>, InputError> {
    let current = read_state(t, fact, &t.fact)?;
    let mut sources = Vec::with_capacity(t.from.len());
    for s in &t.from {
        sources.push(wire_of(ir, t, s)?);
    }
    if sources.iter().any(|s| s == current) {
        return Ok(None);
    }
    let to = wire_of(ir, t, &t.to)?;
    let data = json!({ "field": field_path(t), "from": current, "to": to });
    let message = "violates rule transition".to_owned();
    Ok(Some(Refusal {
        data,
        violation: violation(t, message),
    }))
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
