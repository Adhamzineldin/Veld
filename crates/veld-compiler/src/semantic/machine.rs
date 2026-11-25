//! State machines (`spec/rules.md` section 10.1): the flows and final states of an enum, and the
//! diagnostics `E0930`, `E0931`, `W0930`, `W0931` and `W0932`.
//!
//! Flows are not part of the enum's signature (`crate::signature`), like rules.

use std::collections::BTreeSet;

use super::{RFlow, RVariant};
use crate::diagnostics::{Diagnostic, Span};
use crate::syntax::ast::{self, Ident};

/// What the checked body of an enum says about its machine.
#[derive(Debug, Clone, Default)]
pub struct RMachine {
    /// Edges in first-declaration order, without duplicates.
    pub flows: Vec<RFlow>,
    /// Final states in declaration order, without duplicates.
    pub finals: Vec<String>,
}

/// Checks the `flow` and `final` statements of `e` against its resolved `variants`.
pub(super) fn resolve_machine(
    e: &ast::EnumDecl,
    variants: &[RVariant],
    diags: &mut Vec<Diagnostic>,
) -> RMachine {
    let known = |id: &Ident, diags: &mut Vec<Diagnostic>| -> bool {
        let found = variants.iter().any(|v| v.name == id.name);
        if !found {
            unknown_state(e, variants, id, diags);
        }
        found
    };
    let mut m = RMachine::default();
    let before = diags.len();
    for f in &e.flows {
        for pair in f.states.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            // Both names are always looked at, so a bad chain reports every bad name.
            let (ok_a, ok_b) = (known(a, diags), known(b, diags));
            if ok_a && ok_b {
                add_edge(&mut m, a, b, diags);
            }
        }
    }
    for fin in &e.finals {
        for s in &fin.states {
            if known(s, diags) && !m.finals.contains(&s.name) {
                m.finals.push(s.name.clone());
            }
        }
    }
    // A flow with an unknown name was left out, so the states it touched would look stuck.
    let damaged = diags.iter().skip(before).any(|d| d.code == "E0930");
    check_finals(e, &m, diags);
    if !damaged {
        warn_unused_states(e, variants, &m, diags);
    }
    m
}

fn unknown_state(
    e: &ast::EnumDecl,
    variants: &[RVariant],
    id: &Ident,
    diags: &mut Vec<Diagnostic>,
) {
    let names: Vec<&str> = variants.iter().map(|v| v.name.as_str()).collect();
    diags.push(
        Diagnostic::error(
            "E0930",
            format!("enum `{}` has no variant `{}`", e.name.name, id.name),
            id.span,
        )
        .did_you_mean(
            &id.name,
            names.iter().copied(),
            Some(format!("variants: {}", names.join(", "))),
        ),
    );
}

fn add_edge(m: &mut RMachine, a: &Ident, b: &Ident, diags: &mut Vec<Diagnostic>) {
    if let Some(first) = m.flows.iter().find(|f| f.from == a.name && f.to == b.name) {
        diags.push(
            Diagnostic::warning(
                "W0930",
                format!("the flow `{} -> {}` is declared twice", a.name, b.name),
                a.span.to(b.span),
            )
            .with_label(first.span, "first declared here"),
        );
        return;
    }
    m.flows.push(RFlow {
        from: a.name.clone(),
        to: b.name.clone(),
        span: a.span.to(b.span),
    });
}

/// `E0931`: a final state with a way out, and `final` in an enum without any flow.
fn check_finals(e: &ast::EnumDecl, m: &RMachine, diags: &mut Vec<Diagnostic>) {
    if e.flows.is_empty() {
        for fin in &e.finals {
            diags.push(
                Diagnostic::error(
                    "E0931",
                    format!(
                        "`final` in enum `{}`, which has no `flow`: it is not a state machine",
                        e.name.name
                    ),
                    fin.span,
                )
                .with_help("declare the flows first, for example `flow a -> b`"),
            );
        }
        return;
    }
    for flow in &m.flows {
        if m.finals.contains(&flow.from) {
            diags.push(
                Diagnostic::error(
                    "E0931",
                    format!(
                        "`{}` is final but the flow `{} -> {}` leaves it",
                        flow.from, flow.from, flow.to
                    ),
                    flow.span,
                )
                .with_help("a final state has no way out"),
            );
        }
    }
}

/// `W0931` and `W0932`: states the machine forgot.
fn warn_unused_states(
    e: &ast::EnumDecl,
    variants: &[RVariant],
    m: &RMachine,
    diags: &mut Vec<Diagnostic>,
) {
    if m.flows.is_empty() {
        return;
    }
    let leaves: BTreeSet<&str> = m.flows.iter().map(|f| f.from.as_str()).collect();
    let enters: BTreeSet<&str> = m.flows.iter().map(|f| f.to.as_str()).collect();
    for v in variants {
        let name = v.name.as_str();
        if m.finals.iter().any(|f| f == name) || leaves.contains(name) {
            continue;
        }
        diags.push(unused_state(e, name, v.span, enters.contains(name)));
    }
}

fn unused_state(e: &ast::EnumDecl, name: &str, span: Span, entered: bool) -> Diagnostic {
    if entered {
        Diagnostic::warning(
            "W0931",
            format!(
                "state `{name}` of `{}` is not final and has no way out",
                e.name.name
            ),
            span,
        )
        .with_help(format!("add a flow from `{name}`, or write `final {name}`"))
    } else {
        Diagnostic::warning(
            "W0932",
            format!(
                "variant `{name}` of the state machine `{}` takes part in no flow and is not final",
                e.name.name
            ),
            span,
        )
        .with_help(format!(
            "add a flow that uses `{name}`, or write `final {name}`"
        ))
    }
}
