//! Event examples (`spec/events.md` section 12): `given event E { .. } [on C]` and its
//! `expect` lines. Everything that is not well formed is `E1010`; a value that does not fit the
//! event is `E0952`, as for a call.

use std::collections::BTreeSet;

use super::values::Values;
use crate::diagnostics::Diagnostic;
use crate::semantic::{Program, REventExample, REventExpect};
use crate::syntax::ast::{self, ExpectKind, doc_text};
use veld_ir::Type;

pub(super) fn check_event_example(
    prog: &Program<'_>,
    module: usize,
    ex: &ast::Example,
    ev: &ast::EventExample,
    diags: &mut Vec<Diagnostic>,
) -> Option<REventExample> {
    let event = prog.event_ref(module, &ev.event, "E1010", diags);
    let channel = event.and_then(|e| choose_channel(prog, module, e, ev, diags));
    let ty = event.map(|e| Type::Decl(veld_ir::DeclId(u32::try_from(e).unwrap_or(u32::MAX))));
    let value = ty.as_ref().and_then(|ty| {
        let mut v = Values {
            prog,
            module,
            diags,
            givens: Vec::new(),
        };
        v.value(&ev.value, ty)
    });
    let expect = match channel {
        Some(c) => check_expectations(prog, module, c, ev, diags),
        None => None,
    };
    Some(REventExample {
        name: ex.name.value.clone(),
        pkg: prog.modules[module].pkg,
        module,
        event: event?,
        channel: channel?,
        value: value?,
        expect: expect?,
        docs: doc_text(&ex.docs),
        span: ex.span,
    })
}

/// The channel of the example: the one named by `on`, or the only visible one that carries the
/// event.
fn choose_channel(
    prog: &Program<'_>,
    module: usize,
    event: usize,
    ev: &ast::EventExample,
    diags: &mut Vec<Diagnostic>,
) -> Option<usize> {
    if let Some(on) = &ev.channel {
        let c = prog.channel_ref(module, on, "E1010", diags)?;
        let carries = prog.channels[c]
            .res
            .as_ref()
            .is_some_and(|r| r.events.contains(&event));
        if !carries && prog.channels[c].res.is_some() {
            diags.push(Diagnostic::error(
                "E1010",
                format!(
                    "channel `{}` does not carry `{}`",
                    prog.channels[c].name, prog.decls[event].name
                ),
                on.span,
            ));
            return None;
        }
        return Some(c);
    }
    let all = prog.visible_channels_of(module, event);
    match all.as_slice() {
        [one] => Some(*one),
        [] => {
            diags.push(
                Diagnostic::error(
                    "E1010",
                    format!(
                        "no channel visible here carries `{}`",
                        prog.decls[event].name
                    ),
                    ev.event.span,
                )
                .with_help("import a channel that carries it, or declare one"),
            );
            None
        }
        many => {
            let names: Vec<&str> = many
                .iter()
                .map(|c| prog.channels[*c].name.as_str())
                .collect();
            diags.push(
                Diagnostic::error(
                    "E1010",
                    format!(
                        "`{}` is carried by several channels ({}); say which with `on`",
                        prog.decls[event].name,
                        names.join(", ")
                    ),
                    ev.event.span,
                )
                .with_help(format!(
                    "write `on {}`",
                    names.first().copied().unwrap_or("Channel")
                )),
            );
            None
        }
    }
}

fn check_expectations(
    prog: &Program<'_>,
    module: usize,
    channel: usize,
    ev: &ast::EventExample,
    diags: &mut Vec<Diagnostic>,
) -> Option<REventExpect> {
    let rejected = ev
        .expects
        .iter()
        .find(|e| matches!(e.kind, ExpectKind::Rejected));
    let called = ev
        .expects
        .iter()
        .find(|e| matches!(e.kind, ExpectKind::Called { .. }));
    if let (Some(r), Some(c)) = (rejected, called) {
        diags.push(
            Diagnostic::error(
                "E1010",
                "`rejected` cannot be combined with a `called` line",
                r.span,
            )
            .with_label(c.span, "this line expects a call"),
        );
        return None;
    }
    if let Some(once) = rejected.and_then(|r| r.once) {
        diags.push(Diagnostic::error(
            "E1010",
            "`once` goes with `called`, not with `rejected`",
            once,
        ));
        return None;
    }
    if rejected.is_some() {
        return Some(REventExpect::Rejected);
    }
    let mut out: Vec<(usize, String, bool, bool)> = Vec::new();
    let mut seen: BTreeSet<(usize, String)> = BTreeSet::new();
    let mut ok = true;
    for e in &ev.expects {
        let ExpectKind::Called {
            service,
            handler,
            not,
        } = &e.kind
        else {
            continue;
        };
        if let (true, Some(once)) = (*not, e.once) {
            diags.push(Diagnostic::error(
                "E1010",
                "`not called once` says nothing: write `not called`",
                once,
            ));
            ok = false;
            continue;
        }
        let Some(s) = pick_service(prog, module, channel, service, handler, diags) else {
            ok = false;
            continue;
        };
        if !seen.insert((s, handler.name.clone())) {
            diags.push(Diagnostic::error(
                "E1010",
                format!("`{}.{}` is named twice", service.name, handler.name),
                e.span,
            ));
            ok = false;
            continue;
        }
        out.push((s, handler.name.clone(), !not, e.once.is_some()));
    }
    ok.then_some(REventExpect::Calls(out))
}

/// The service `name` that has a subscription to `channel` with a handler `handler`: of the
/// package of the example first, then of any package of the workspace.
fn pick_service(
    prog: &Program<'_>,
    module: usize,
    channel: usize,
    service: &ast::Ident,
    handler: &ast::Ident,
    diags: &mut Vec<Diagnostic>,
) -> Option<usize> {
    let pkg = prog.modules[module].pkg;
    let candidates: Vec<usize> = prog
        .services
        .iter()
        .enumerate()
        .filter(|(_, s)| s.name == service.name)
        .filter(|(_, s)| {
            s.subscriptions
                .iter()
                .filter(|sub| sub.channel == channel)
                .any(|sub| sub.handlers.iter().any(|h| h.name == handler.name))
        })
        .map(|(i, _)| i)
        .collect();
    let local: Vec<usize> = candidates
        .iter()
        .copied()
        .filter(|i| prog.services[*i].pkg == pkg)
        .collect();
    match (local.as_slice(), candidates.as_slice()) {
        ([one], _) | ([], [one]) => Some(*one),
        ([], []) => {
            let names = handlers_of(prog, channel);
            diags.push(
                Diagnostic::error(
                    "E1010",
                    format!(
                        "no handler `{}.{}` on channel `{}`",
                        service.name, handler.name, prog.channels[channel].name
                    ),
                    service.span.to(handler.span),
                )
                .did_you_mean(
                    &format!("{}.{}", service.name, handler.name),
                    names.iter().map(String::as_str),
                    Some(format!("handlers on this channel: {}", names.join(", "))),
                ),
            );
            None
        }
        _ => {
            diags.push(Diagnostic::error(
                "E1010",
                format!(
                    "`{}.{}` is ambiguous: several packages have such a service",
                    service.name, handler.name
                ),
                service.span,
            ));
            None
        }
    }
}

fn handlers_of(prog: &Program<'_>, channel: usize) -> Vec<String> {
    let mut v: Vec<String> = prog
        .services
        .iter()
        .flat_map(|s| {
            s.subscriptions
                .iter()
                .filter(|sub| sub.channel == channel)
                .flat_map(|sub| {
                    sub.handlers
                        .iter()
                        .map(|h| format!("{}.{}", s.name, h.name))
                })
        })
        .collect();
    v.sort();
    v.dedup();
    v
}
