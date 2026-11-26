//! `expect emitted Event` and `expect not emitted` of a call example (`spec/events.md` section
//! 11.1). Everything that is not well formed is `E1010`.

use crate::diagnostics::{Diagnostic, Span};
use crate::semantic::{Program, REndpoint};
use crate::syntax::ast;

/// The events the example says the call writes to the outbox: `None` without an `expect emitted`
/// line, `Some(vec![])` for `expect not emitted`. The outer `None` is an error already reported.
pub(super) fn check_emitted(
    prog: &Program<'_>,
    module: usize,
    call: &ast::ExampleCall,
    ep: &REndpoint,
    has_returns: bool,
    diags: &mut Vec<Diagnostic>,
) -> Option<Option<Vec<usize>>> {
    let Some(first) = call.emitted.first() else {
        return Some(None);
    };
    let moves: Vec<usize> = ep
        .rules
        .transition
        .iter()
        .flat_map(|t| t.emits.iter().map(|e| e.event))
        .collect();
    if ep
        .rules
        .transition
        .as_ref()
        .is_none_or(|t| t.emits.is_empty())
    {
        return fail(
            diags,
            first.span,
            format!(
                "endpoint `{}` has no `transition` that emits an event",
                ep.name
            ),
        );
    }
    let none = call.emitted.iter().any(|l| l.event.is_none());
    if none && call.emitted.len() > 1 {
        return fail(
            diags,
            call.emitted[1].span,
            "`expect not emitted` cannot be combined with another `expect emitted` line".into(),
        );
    }
    if none {
        return Some(Some(Vec::new()));
    }
    let ok_outcome = call.outcome.error.is_none();
    if !(ok_outcome && has_returns) {
        return fail(
            diags,
            first.span,
            "`expect emitted` needs the outcome `ok` and a `returns` line: only a handler that succeeds writes the event".into(),
        );
    }
    let mut events: Vec<usize> = Vec::new();
    for line in &call.emitted {
        let t = line.event.as_ref()?;
        let e = prog.event_ref(module, t, "E1010", diags)?;
        if !moves.contains(&e) {
            return fail(
                diags,
                t.span,
                format!(
                    "the transition of `{}` does not emit `{}`",
                    ep.name, prog.decls[e].name
                ),
            );
        }
        if events.contains(&e) {
            return fail(
                diags,
                t.span,
                format!("`expect emitted {}` is written twice", prog.decls[e].name),
            );
        }
        events.push(e);
    }
    Some(Some(events))
}

fn fail<T>(diags: &mut Vec<Diagnostic>, at: Span, message: String) -> Option<T> {
    diags.push(Diagnostic::error("E1010", message, at));
    None
}
