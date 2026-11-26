//! Rule-aware comparison of declarations: invariants of models and errors, and the state
//! machines of enums (`spec/rules.md` sections 5, 6 and 10).
//!
//! A rule is identified by its declaration and its name; it "changed" when its id differs, which
//! happens exactly when the expression changes (section 6). Without a solver the tool cannot tell
//! whether a changed expression is tighter or looser, so it says so: `PossiblyBreaking`.

use std::collections::BTreeMap;

use veld_ir::{Decl, DeclKind, Enum, Rule};

use super::decls::by_name;
use super::expr_text::expr_text;
use super::usage::{Role, Usage};
use super::{Change, ChangeKind, Ctx};

/// Where a declaration is used. A declaration no endpoint reaches (a types-only package) is
/// treated as used in both positions: its consumers are unknown, so the tool assumes the worst.
#[derive(Debug, Clone, Copy)]
pub(super) struct Position {
    pub request: bool,
    pub response: bool,
}

impl Position {
    pub(super) fn of(role: Role) -> Position {
        if role.request || role.response || role.stream {
            Position {
                request: role.request || role.stream,
                response: role.response || role.stream,
            }
        } else {
            Position {
                request: true,
                response: true,
            }
        }
    }

    pub(super) fn describe(self) -> &'static str {
        match (self.request, self.response) {
            (true, false) => "requests",
            (false, true) => "responses",
            (true, true) | (false, false) => "requests and responses",
        }
    }
}

pub(super) fn diff_decl_rules(ctx: &Ctx<'_>, usage: &Usage, out: &mut Vec<Change>) {
    let (old, new) = (by_name(ctx.old), by_name(ctx.new));
    for (q, o) in &old {
        if let Some(n) = new.get(q) {
            diff_invariants(q, (o, n), Position::of(usage.role(q)), out);
            diff_flows(q, o, n, out);
        }
    }
}

/// The rules a declaration wrote itself; inherited ones are reported on the parent.
pub(super) fn own_rules(d: &Decl) -> BTreeMap<&str, &Rule> {
    d.rules()
        .iter()
        .filter(|r| r.inherited_from.is_none())
        .map(|r| (r.name.as_str(), r))
        .collect()
}

/// Only the enforced rules: an audited rule never rejects and is judged as if it were absent
/// (`spec/rules.md` section 17.1).
pub(super) fn enforced<'a>(all: &BTreeMap<&'a str, &'a Rule>) -> BTreeMap<&'a str, &'a Rule> {
    all.iter()
        .filter(|(_, r)| !r.is_audited())
        .map(|(n, r)| (*n, *r))
        .collect()
}

fn diff_invariants(q: &str, pair: (&Decl, &Decl), pos: Position, out: &mut Vec<Change>) {
    let (old_all, new_all) = (own_rules(pair.0), own_rules(pair.1));
    let (old, new) = (enforced(&old_all), enforced(&new_all));
    for (name, o) in &old {
        match new.get(name) {
            None => {
                let mut c = removed(q, o, pos);
                if new_all.contains_key(name) {
                    c.message.push_str(AUDITED_NOW);
                }
                out.push(c);
            }
            Some(n) => changed(q, (o, n), pos, out),
        }
    }
    for (name, n) in &new {
        if !old.contains_key(name) {
            let mut c = added(q, n, pos);
            if old_all.contains_key(name) {
                c.message.push_str(ENFORCED_NOW);
            }
            out.push(c);
        }
    }
}

const AUDITED_NOW: &str = "; the rule is now audited (`@rollout(audit)`): it is reported and never rejects, so this is judged like removing it";
const ENFORCED_NOW: &str =
    "; the rule is no longer audited: it now rejects, so this is judged like adding it";

fn added(q: &str, r: &Rule, pos: Position) -> Change {
    let text = expr_text(&r.expr);
    if pos.request {
        Change::breaking(
            ChangeKind::InvariantAdded,
            q,
            format!(
                "rule `{}` was added: `{text}`; it is checked on {}, so a request that was \
                 accepted before may now be rejected",
                r.name,
                pos.describe()
            ),
        )
    } else {
        Change::non_breaking(
            ChangeKind::InvariantAdded,
            q,
            format!(
                "note: rule `{}` was added: `{text}`; a new guarantee on {}",
                r.name,
                pos.describe()
            ),
        )
    }
}

fn removed(q: &str, r: &Rule, pos: Position) -> Change {
    let text = expr_text(&r.expr);
    if pos.response {
        Change::breaking(
            ChangeKind::InvariantRemoved,
            q,
            format!(
                "rule `{}` was removed (was `{text}`); consumers of {} lose a guarantee they \
                 could rely on",
                r.name,
                pos.describe()
            ),
        )
    } else {
        Change::non_breaking(
            ChangeKind::InvariantRemoved,
            q,
            format!(
                "rule `{}` was removed (was `{text}`); {} are now accepted more widely",
                r.name,
                pos.describe()
            ),
        )
    }
}

fn changed(q: &str, pair: (&Rule, &Rule), pos: Position, out: &mut Vec<Change>) {
    let (o, n) = pair;
    if o.id != n.id {
        out.push(Change::possibly_breaking(
            ChangeKind::InvariantChanged,
            q,
            format!(
                "possibly breaking: rule `{}` changed from `{}` to `{}`; it is checked on {}, \
                 and the tool cannot tell whether the new rule is tighter or looser",
                o.name,
                expr_text(&o.expr),
                expr_text(&n.expr),
                pos.describe()
            ),
        ));
    }
    if o.message != n.message {
        out.push(Change::non_breaking(
            ChangeKind::RuleMessageChanged,
            q,
            format!("note: the message of rule `{}` changed", o.name),
        ));
    }
}

fn enum_of(d: &Decl) -> Option<&Enum> {
    match &d.kind {
        DeclKind::Enum(e) => Some(e),
        DeclKind::Model(_)
        | DeclKind::Error(_)
        | DeclKind::Union(_)
        | DeclKind::Const(_)
        | DeclKind::Event(_) => None,
    }
}

/// A flow removed is breaking (a transition clients could rely on is gone), a flow added is not.
/// A state made final loses its outgoing flows, which are reported here as removed flows.
fn diff_flows(q: &str, o: &Decl, n: &Decl, out: &mut Vec<Change>) {
    let (Some(a), Some(b)) = (enum_of(o), enum_of(n)) else {
        return;
    };
    // Edges are the same edge whatever event they emit: `emits` is judged in `emits.rs`.
    let has = |flows: &[veld_ir::Flow], f: &veld_ir::Flow| {
        flows.iter().any(|g| g.from == f.from && g.to == f.to)
    };
    for f in a.flows.iter().filter(|f| !has(&b.flows, f)) {
        out.push(Change::breaking(
            ChangeKind::FlowRemoved,
            q,
            format!("flow `{} -> {}` was removed", f.from, f.to),
        ));
    }
    for f in b.flows.iter().filter(|f| !has(&a.flows, f)) {
        out.push(Change::non_breaking(
            ChangeKind::FlowAdded,
            q,
            format!("flow `{} -> {}` was added", f.from, f.to),
        ));
    }
}
