//! `emits` (`spec/events.md` section 11): the event of a flow, and the events a transition
//! sends. The diagnostic is `E1011`; the arity of a flow (one edge) is checked with the machine.

use super::{DKind, Program, REmitted, REndpoint};
use crate::diagnostics::{Diagnostic, Span};

/// Looks the event of every `emits` up, as seen from the module of the state machine.
pub(super) fn resolve_flows(prog: &mut Program<'_>, diags: &mut Vec<Diagnostic>) {
    let mut found: Vec<(usize, usize, Option<usize>)> = Vec::new();
    for (i, d) in prog.decls.iter().enumerate() {
        for (j, f) in d.res.machine.flows.iter().enumerate() {
            if let Some(e) = &f.emits {
                found.push((i, j, event_named(prog, d.module, &e.name, e.span, diags)));
            }
        }
    }
    for (i, j, event) in found {
        if let Some(e) = prog.decls[i].res.machine.flows[j].emits.as_mut() {
            e.event = event;
        }
    }
}

fn event_named(
    prog: &Program<'_>,
    module: usize,
    name: &str,
    span: Span,
    diags: &mut Vec<Diagnostic>,
) -> Option<usize> {
    if let Some(d) = prog.lookup(module, name) {
        if prog.decls[d].kind == DKind::Event {
            return Some(d);
        }
        diags.push(Diagnostic::error(
            "E1011",
            format!("`emits` names `{name}`, which is not an event"),
            span,
        ));
        return None;
    }
    if prog.scopes[module].poisoned.contains(name) {
        return None;
    }
    let events: Vec<&str> = prog.scopes[module]
        .names
        .iter()
        .filter(|(_, e)| prog.decls[e.decl].kind == DKind::Event)
        .map(|(n, _)| n.as_str())
        .collect();
    diags.push(
        Diagnostic::error("E1011", format!("`emits` names no event `{name}`"), span).did_you_mean(
            name,
            events.iter().copied(),
            None,
        ),
    );
    None
}

/// Gives every transition the events its edges emit, each with the channel the service sends it
/// on: the first of `publishes` that carries it. A move whose service publishes no such channel
/// is `E1011`, reported at the `transition` statement.
pub(super) fn attach(
    prog: &Program<'_>,
    publishes: &[usize],
    endpoints: &mut [REndpoint],
    diags: &mut Vec<Diagnostic>,
) {
    for ep in endpoints {
        let Some(t) = ep.rules.transition.as_mut() else {
            continue;
        };
        let machine = &prog.decls[t.decl].res.machine;
        let mut events: Vec<usize> = Vec::new();
        for f in machine
            .flows
            .iter()
            .filter(|f| f.to == t.to && t.from.contains(&f.from))
        {
            if let Some(e) = f.emits.as_ref().and_then(|e| e.event)
                && !events.contains(&e)
            {
                events.push(e);
            }
        }
        for event in events {
            match first_channel(prog, publishes, event) {
                Some(channel) => t.emits.push(REmitted { event, channel }),
                None if broken_channel(prog, publishes) => {}
                None => diags.push(not_published(prog, event, &ep.name, t.span)),
            }
        }
    }
}

fn first_channel(prog: &Program<'_>, publishes: &[usize], event: usize) -> Option<usize> {
    publishes.iter().copied().find(|&c| {
        prog.channels[c]
            .res
            .as_ref()
            .is_some_and(|r| r.events.contains(&event))
    })
}

/// A channel that has an error carries nothing the checker can see; stay silent about it.
fn broken_channel(prog: &Program<'_>, publishes: &[usize]) -> bool {
    publishes.iter().any(|&c| prog.channels[c].res.is_none())
}

fn not_published(prog: &Program<'_>, event: usize, endpoint: &str, at: Span) -> Diagnostic {
    Diagnostic::error(
        "E1011",
        format!(
            "endpoint `{endpoint}` moves along a flow that emits `{}`, and its service publishes no channel that carries it",
            prog.decls[event].name
        ),
        at,
    )
    .with_help("add `publishes <Channel>` to the service, for a channel that carries the event")
}
