//! What the server checks after the handler (`spec/events.md` section 11.1): after a success the
//! event of the move MUST have been written exactly once.

use veld_ir::endpoint_rules::EndpointTransition;
use veld_ir::{DeclId, DeclKind, Ir};

/// What went wrong with the events a handler wrote: the server answers `500 Internal` and the
/// error goes to `onError`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WrittenFault {
    /// An event of the move was not written.
    Missing(DeclId),
    /// An event of the move was written more than once.
    Twice(DeclId),
    /// An event the move does not emit from this state was written.
    Unexpected(DeclId),
}

/// The events the move emits when the field held `from` (a variant name) before it: those of the
/// edge `from -> transition.to` of the state machine, in the order of the edges. Empty for an
/// edge without `emits`, and for a state that is no source of the transition.
#[must_use]
pub fn expected_events(ir: &Ir, transition: &EndpointTransition, from: &str) -> Vec<DeclId> {
    let DeclKind::Enum(machine) = &ir.decl(transition.decl).kind else {
        return Vec::new();
    };
    let mut events = Vec::new();
    for flow in &machine.flows {
        let hit = flow.from == from && flow.to == transition.to;
        if let (true, Some(e)) = (hit, flow.emits)
            && !events.contains(&e)
        {
            events.push(e);
        }
    }
    events
}

/// Checks the events the handler wrote (in the order it wrote them) against `expected`. A
/// success must have written each expected event exactly once and nothing else; a handler that
/// answered an error is not checked.
///
/// # Errors
/// The first [`WrittenFault`]: a missing event first (in the order of `expected`), then a repeated
/// one, then an unexpected one (in write order).
pub fn check_written(expected: &[DeclId], written: &[DeclId]) -> Result<(), WrittenFault> {
    let count = |e: DeclId| written.iter().filter(|w| **w == e).count();
    for e in expected {
        match count(*e) {
            0 => return Err(WrittenFault::Missing(*e)),
            1 => {}
            _ => return Err(WrittenFault::Twice(*e)),
        }
    }
    match written.iter().find(|w| !expected.contains(w)) {
        Some(w) => Err(WrittenFault::Unexpected(*w)),
        None => Ok(()),
    }
}
