//! `transition` in an endpoint block (`spec/rules.md` section 10.2): the path, the edge check and
//! the diagnostics `E0932`, `E0933`, `E0934` and the `transition` case of `E0920`.

use std::collections::BTreeSet;

use crate::diagnostics::{Diagnostic, Span};
use crate::semantic::{DKind, Program, RField};
use crate::syntax::ast::{self, Ident, Stmt};
use veld_ir::Type;
use veld_ir::rules::Step;

/// A checked `transition`.
#[derive(Debug, Clone)]
pub struct RTransition {
    pub fact: String,
    /// The path below the fact; the last step is the state field.
    pub steps: Vec<Step>,
    /// The state machine (an index into `Program::decls`).
    pub decl: usize,
    /// Sources as variant names, in source order, without repeats.
    pub from: Vec<String>,
    pub to: String,
    /// Wire values of `from` (same order) and of `to`.
    pub from_wire: Vec<String>,
    pub to_wire: String,
    /// `since` (`spec/platform.md` section 10): the steps below the fact of the `datetime` field.
    /// Empty for a transition that is not timed.
    pub since: Vec<Step>,
    /// The milliseconds of the timed edge this transition takes; `Some` exactly when `since` is
    /// not empty.
    pub after_ms: Option<i64>,
    pub span: Span,
}

/// What the checker knows while it looks at the statement.
pub(super) struct Scope<'a, 'p> {
    pub prog: &'p Program<'a>,
    pub facts: &'p [RField],
    pub poisoned: &'p BTreeSet<String>,
}

/// The `transition` statements of a block, in source order.
pub(super) fn transitions(stmts: &[Stmt]) -> Vec<&ast::TransitionStmt> {
    stmts
        .iter()
        .filter_map(|s| match s {
            Stmt::Transition(t) => Some(t),
            Stmt::Fact(_) | Stmt::Require(_) | Stmt::Ensure(_) | Stmt::Allow(_) => None,
        })
        .collect()
}

/// `E0920`: on an endpoint with a `transition`, a parameter or a fact is not called `transition`,
/// because the handler receives the transition under that name.
pub(super) fn reserved_name(e: &ast::Endpoint, diags: &mut Vec<Diagnostic>) {
    let has = e
        .block
        .as_ref()
        .is_some_and(|b| !transitions(&b.stmts).is_empty());
    if !has {
        return;
    }
    let params = e.params.iter().map(|p| (&p.name, "parameter"));
    let facts = e
        .block
        .iter()
        .flat_map(|b| &b.stmts)
        .filter_map(|s| match s {
            Stmt::Fact(f) => Some((&f.name, "fact")),
            Stmt::Require(_) | Stmt::Ensure(_) | Stmt::Transition(_) | Stmt::Allow(_) => None,
        });
    for (name, what) in params.chain(facts) {
        if name.name == "transition" {
            diags.push(
                Diagnostic::error(
                    "E0920",
                    format!("a {what} is called `transition`, the name the handler gets the transition under"),
                    name.span,
                )
                .with_help("rename it: an endpoint with a `transition` reserves the name"),
            );
        }
    }
}

/// `E0934`: one `transition` per endpoint.
pub(super) fn extra_transition(first: Span, t: &ast::TransitionStmt, diags: &mut Vec<Diagnostic>) {
    diags.push(
        Diagnostic::error("E0934", "an endpoint has at most one `transition`", t.span)
            .with_label(first, "first `transition` here")
            .with_help("list every source state in one statement: `transition x.state: a, b -> c`"),
    );
}

impl Scope<'_, '_> {
    /// Checks one statement; `None` when a diagnostic was reported.
    pub(super) fn check(
        &self,
        t: &ast::TransitionStmt,
        diags: &mut Vec<Diagnostic>,
    ) -> Option<RTransition> {
        let (steps, decl) = self.path(t, diags)?;
        let info = &self.prog.decls[decl];
        let mut ok = true;
        let mut from: Vec<(String, String)> = Vec::new();
        for s in &t.from {
            match self.edge(decl, s, &t.to, diags) {
                Some(wire) => {
                    if !from.iter().any(|(n, _)| *n == s.name) {
                        from.push((s.name.clone(), wire));
                    }
                }
                None => ok = false,
            }
        }
        let Some(to) = info.res.variants.iter().find(|v| v.name == t.to.name) else {
            self.unknown_state(decl, &t.to, diags);
            return None;
        };
        let names: Vec<&str> = from.iter().map(|(n, _)| n.as_str()).collect();
        let timing = ok
            .then(|| super::since::check(self, t, decl, &names, &to.name, diags))
            .flatten()?;
        Some(RTransition {
            fact: t.path[0].name.clone(),
            steps,
            decl,
            from_wire: from.iter().map(|(_, w)| w.clone()).collect(),
            from: from.into_iter().map(|(n, _)| n).collect(),
            to: to.name.clone(),
            to_wire: to.wire.clone(),
            since: timing.0,
            after_ms: timing.1,
            span: t.span,
        })
    }

    /// The wire value of source `s` when `s -> to` is an edge of the machine `decl`.
    fn edge(
        &self,
        decl: usize,
        s: &Ident,
        to: &Ident,
        diags: &mut Vec<Diagnostic>,
    ) -> Option<String> {
        let info = &self.prog.decls[decl];
        let Some(v) = info.res.variants.iter().find(|v| v.name == s.name) else {
            self.unknown_state(decl, s, diags);
            return None;
        };
        let known = info.res.variants.iter().any(|v| v.name == to.name);
        let is_edge = info
            .res
            .machine
            .flows
            .iter()
            .any(|f| f.from == s.name && f.to == to.name);
        if known && !is_edge {
            let next: Vec<&str> = info
                .res
                .machine
                .flows
                .iter()
                .filter(|f| f.from == s.name)
                .map(|f| f.to.as_str())
                .collect();
            let help = if next.is_empty() {
                format!("`{}` has no way out in `{}`", s.name, info.name)
            } else {
                format!("from `{}` the flows lead to: {}", s.name, next.join(", "))
            };
            diags.push(
                Diagnostic::error(
                    "E0933",
                    format!(
                        "`{} -> {}` is not a flow of `{}`",
                        s.name, to.name, info.name
                    ),
                    s.span.to(to.span),
                )
                .with_help(help),
            );
            return None;
        }
        known.then(|| v.wire.clone())
    }

    fn unknown_state(&self, decl: usize, id: &Ident, diags: &mut Vec<Diagnostic>) {
        let info = &self.prog.decls[decl];
        let names: Vec<&str> = info.res.variants.iter().map(|v| v.name.as_str()).collect();
        diags.push(
            Diagnostic::error(
                "E0933",
                format!("`{}` is not a state of `{}`", id.name, info.name),
                id.span,
            )
            .did_you_mean(
                &id.name,
                names.iter().copied(),
                Some(format!("states: {}", names.join(", "))),
            ),
        );
    }

    /// The steps below the fact and the state machine the path ends in (`E0932`).
    fn path(
        &self,
        t: &ast::TransitionStmt,
        diags: &mut Vec<Diagnostic>,
    ) -> Option<(Vec<Step>, usize)> {
        let root = &t.path[0];
        if self.poisoned.contains(&root.name) {
            return None;
        }
        let Some(fact) = self.facts.iter().find(|f| f.name == root.name) else {
            diags.push(
                Diagnostic::error(
                    "E0932",
                    format!("`{}` is not a fact of this endpoint", root.name),
                    root.span,
                )
                .with_help("the path of a `transition` starts at a `fact`"),
            );
            return None;
        };
        if fact.optional {
            diags.push(path_error(
                format!("fact `{}` is optional and may be absent", root.name),
                root.span,
            ));
            return None;
        }
        if t.path.len() == 1 {
            diags.push(
                path_error(
                    format!(
                        "`{}` names a fact, not the field that holds the state",
                        root.name
                    ),
                    root.span,
                )
                .with_help(format!("write `{}.<field>`", root.name)),
            );
            return None;
        }
        self.walk(&fact.ty, &t.path[1..], diags)
    }

    fn walk(
        &self,
        start: &Type,
        rest: &[Ident],
        diags: &mut Vec<Diagnostic>,
    ) -> Option<(Vec<Step>, usize)> {
        let mut cur = start.clone();
        let mut steps = Vec::new();
        for id in rest {
            let field = self.field_of(&cur, id, diags)?;
            if field.optional {
                diags.push(path_error(
                    format!("field `{}` is optional and may be absent", id.name),
                    id.span,
                ));
                return None;
            }
            steps.push(Step {
                name: field.name.clone(),
                wire: field.wire.clone(),
                optional: false,
            });
            cur = field.ty.clone();
        }
        let last = rest.last()?;
        match &cur {
            Type::Decl(d)
                if self.prog.decls[d.index()].kind == DKind::Enum
                    && !self.prog.decls[d.index()].res.machine.flows.is_empty() =>
            {
                Some((steps, d.index()))
            }
            Type::Decl(_) | Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Page(_) => {
                diags.push(
                    path_error(
                        format!(
                            "`{}` has type `{}`, which is not a state machine",
                            last.name,
                            self.prog.type_name(&cur)
                        ),
                        last.span,
                    )
                    .with_help("a state machine is an enum with at least one `flow`"),
                );
                None
            }
        }
    }

    pub(super) fn field_of(
        &self,
        ty: &Type,
        id: &Ident,
        diags: &mut Vec<Diagnostic>,
    ) -> Option<&RField> {
        let info = match ty {
            Type::Decl(d)
                if matches!(self.prog.decls[d.index()].kind, DKind::Model | DKind::Error) =>
            {
                &self.prog.decls[d.index()]
            }
            Type::Decl(_) | Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Page(_) => {
                diags.push(path_error(
                    format!(
                        "a value of type `{}` has no field `{}`",
                        self.prog.type_name(ty),
                        id.name
                    ),
                    id.span,
                ));
                return None;
            }
        };
        let found = info.res.fields.iter().find(|f| f.name == id.name);
        if found.is_none() {
            let names: Vec<&str> = info.res.fields.iter().map(|f| f.name.as_str()).collect();
            diags.push(
                Diagnostic::error(
                    "E0901",
                    format!("`{}` has no field `{}`", info.name, id.name),
                    id.span,
                )
                .did_you_mean(
                    &id.name,
                    names.iter().copied(),
                    Some(format!("fields: {}", names.join(", "))),
                ),
            );
        }
        found
    }
}

fn path_error(message: String, span: Span) -> Diagnostic {
    Diagnostic::error("E0932", message, span)
}
