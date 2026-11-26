//! `transition ... since path` (`spec/platform.md` section 10): the `E0936` checks. A transition
//! takes a timed edge exactly when it has `since`, it has one source, and that pair is a timed
//! edge of the machine; `since` names a required `datetime` field of the same fact.

use super::transition::Scope;
use crate::diagnostics::{Diagnostic, Span};
use crate::syntax::ast;
use veld_ir::rules::Step;
use veld_ir::{Scalar, Type};

/// What the timing of one transition came to: the `since` steps and the milliseconds of the edge.
/// `None` when a diagnostic was reported; the pair is empty for an ordinary transition.
pub(super) type Timing = (Vec<Step>, Option<i64>);

/// Checks `since` against the edges the transition takes. `from` are the source names and `to` the
/// target, all known variants of the machine `decl`.
pub(super) fn check(
    scope: &Scope<'_, '_>,
    t: &ast::TransitionStmt,
    decl: usize,
    from: &[&str],
    to: &str,
    diags: &mut Vec<Diagnostic>,
) -> Option<Timing> {
    let flows = &scope.prog.decls[decl].res.machine.flows;
    let timed: Vec<(&str, i64)> = from
        .iter()
        .filter_map(|s| {
            let f = flows.iter().find(|f| f.from == *s && f.to == to)?;
            f.after_ms.map(|ms| (*s, ms))
        })
        .collect();
    let Some(since) = &t.since else {
        if let Some((source, ms)) = timed.first() {
            diags.push(
                Diagnostic::error(
                    "E0936",
                    format!(
                        "`{source} -> {to}` is a timed edge ({ms} ms): the transition needs `since`"
                    ),
                    t.span,
                )
                .with_help("write `since fact.field`, the field that holds when the value entered its state"),
            );
            return None;
        }
        return Some((Vec::new(), None));
    };
    if from.len() != 1 {
        return fail(
            diags,
            t.span,
            "a transition with `since` has exactly one source state",
        );
    }
    let [(_, ms)] = timed.as_slice() else {
        return fail(
            diags,
            t.span,
            &format!(
                "`since` on `{} -> {to}`, which is not a timed edge",
                from[0]
            ),
        );
    };
    let steps = since_steps(scope, t, since, diags)?;
    Some((steps, Some(*ms)))
}

fn fail(diags: &mut Vec<Diagnostic>, span: Span, message: &str) -> Option<Timing> {
    diags.push(Diagnostic::error("E0936", message, span).with_help(
        "`since` belongs to a transition over an edge declared with `flow a -> b after d`",
    ));
    None
}

/// The steps of the `since` path below the fact: every one required, the last a `datetime`.
fn since_steps(
    scope: &Scope<'_, '_>,
    t: &ast::TransitionStmt,
    since: &[ast::Ident],
    diags: &mut Vec<Diagnostic>,
) -> Option<Vec<Step>> {
    let root = &since[0];
    if root.name != t.path[0].name {
        diags.push(Diagnostic::error(
            "E0936",
            format!(
                "`since` starts at `{}`, but the transition is over the fact `{}`",
                root.name, t.path[0].name
            ),
            root.span,
        ));
        return None;
    }
    let fact = scope.facts.iter().find(|f| f.name == root.name)?;
    if since.len() == 1 {
        return bad(
            diags,
            root.span,
            "`since` names a fact, not a `datetime` field",
        );
    }
    let mut cur: Type = fact.ty.clone();
    let mut steps = Vec::new();
    for id in &since[1..] {
        let field = scope.field_of(&cur, id, diags)?;
        if field.optional {
            return bad(
                diags,
                id.span,
                &format!("field `{}` is optional and may be absent", id.name),
            );
        }
        steps.push(Step {
            name: field.name.clone(),
            wire: field.wire.clone(),
            optional: false,
        });
        cur = field.ty.clone();
    }
    if cur != Type::Scalar(Scalar::Datetime) {
        let last = since.last().map_or(t.span, |l| l.span);
        return bad(diags, last, "the field after `since` must be a `datetime`");
    }
    Some(steps)
}

fn bad<T>(diags: &mut Vec<Diagnostic>, span: Span, message: &str) -> Option<T> {
    diags.push(Diagnostic::error("E0936", message, span));
    None
}
