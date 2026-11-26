//! Timed moves (`spec/platform.md` section 10): the duration of `flow ... after` (`E0935`) and
//! `W0933`, a timed edge that no `transition` takes. The `since` side is in
//! `rules::since`.

use std::collections::BTreeSet;

use super::duration::const_duration_ms;
use super::{DKind, Program};
use crate::diagnostics::{Diagnostic, Span, has_errors};
use crate::syntax::ast::{FlowStmt, Ident};

/// The milliseconds of the `after` of a `flow` statement; `None` when there is none or it is
/// wrong (then `E0935` was reported).
pub(super) fn after_ms(f: &FlowStmt, diags: &mut Vec<Diagnostic>) -> Option<i64> {
    let after = f.after.as_ref()?;
    if f.states.len() != 2 {
        diags.push(
            Diagnostic::error(
                "E0935",
                "`after` belongs to a `flow` with exactly one edge",
                after.span,
            )
            .with_help("write one `flow a -> b after d` per timed edge"),
        );
        return None;
    }
    match const_duration_ms(after) {
        Ok(ms) if ms > 0 => Some(ms),
        Ok(_) => {
            diags.push(Diagnostic::error(
                "E0935",
                "the duration after `after` must be greater than zero",
                after.span,
            ));
            None
        }
        Err(why) => {
            diags.push(
                Diagnostic::error(
                    "E0935",
                    format!("`after` takes a positive constant duration: {why}"),
                    after.span,
                )
                .with_help("for example `after hours(24)`"),
            );
            None
        }
    }
}

/// `E0935`: an edge that already has a declaration gets a second `after`.
pub(super) fn second_after(first: Span, a: &Ident, b: &Ident, diags: &mut Vec<Diagnostic>) {
    diags.push(
        Diagnostic::error(
            "E0935",
            format!(
                "the flow `{} -> {}` is declared again with `after`: an edge has one declaration",
                a.name, b.name
            ),
            a.span.to(b.span),
        )
        .with_label(first, "first declared here"),
    );
}

/// `W0933`: a timed edge that no `transition` of the workspace takes, so nothing can ever make
/// the move.
pub(super) fn check_unused_edges(prog: &Program<'_>, diags: &mut Vec<Diagnostic>) {
    // A transition that failed its checks is not in the program, so every edge would look unused.
    if has_errors(diags) {
        return;
    }
    let taken: BTreeSet<(usize, String, String)> = prog
        .services
        .iter()
        .flat_map(|s| &s.endpoints)
        .filter_map(|e| e.rules.transition.as_ref())
        .filter(|t| !t.since.is_empty())
        .flat_map(|t| t.from.iter().map(|f| (t.decl, f.clone(), t.to.clone())))
        .collect();
    for (i, d) in prog.decls.iter().enumerate() {
        if d.kind != DKind::Enum {
            continue;
        }
        for f in d.res.machine.flows.iter().filter(|f| f.after_ms.is_some()) {
            if !taken.contains(&(i, f.from.clone(), f.to.clone())) {
                diags.push(
                    Diagnostic::warning(
                        "W0933",
                        format!(
                            "the timed edge `{} -> {}` of `{}` is taken by no `transition ... since`",
                            f.from, f.to, d.name
                        ),
                        f.span,
                    )
                    .with_help("write an endpoint with `transition x.state: a -> b since x.field`"),
                );
            }
        }
    }
}
