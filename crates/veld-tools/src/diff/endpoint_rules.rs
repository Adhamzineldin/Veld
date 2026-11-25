//! Rule-aware comparison of endpoints: `require`, `ensure`, facts and `transition`
//! (`spec/rules.md` sections 9 and 10).

use std::collections::BTreeMap;

use veld_ir::{ElseError, Endpoint, EndpointEnsure, EndpointRequire, EndpointTransition, Ir};

use super::expr_text::expr_text;
use super::{Change, ChangeKind, Ctx};

/// The part of a statement that identifies it: its name, or for an unnamed one its id.
pub(super) trait Stmt {
    fn stmt_name(&self) -> Option<&str>;
    fn stmt_id(&self) -> &str;
    fn stmt_audited(&self) -> bool;
}

impl Stmt for EndpointRequire {
    fn stmt_name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    fn stmt_id(&self) -> &str {
        &self.id
    }
    fn stmt_audited(&self) -> bool {
        self.rollout.is_some()
    }
}

impl Stmt for EndpointEnsure {
    fn stmt_name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    fn stmt_id(&self) -> &str {
        &self.id
    }
    fn stmt_audited(&self) -> bool {
        self.rollout.is_some()
    }
}

/// Statements paired between the old and the new endpoint.
pub(super) struct Paired<'a, T> {
    pub(super) both: Vec<(&'a T, &'a T)>,
    pub(super) removed: Vec<&'a T>,
    pub(super) added: Vec<&'a T>,
}

/// Named statements pair by name. Unnamed ones pair by id (same expression); the leftovers pair in
/// source order, which reads as "the expression of that statement changed".
pub(super) fn pair<'a, T: Stmt>(old: &[&'a T], new: &[&'a T]) -> Paired<'a, T> {
    let mut p = Paired {
        both: Vec::new(),
        removed: Vec::new(),
        added: Vec::new(),
    };
    let mut new_named: BTreeMap<&str, &T> = new
        .iter()
        .filter_map(|s| s.stmt_name().map(|n| (n, *s)))
        .collect();
    let mut old_loose: Vec<&T> = Vec::new();
    for o in old {
        match o.stmt_name() {
            Some(n) => match new_named.remove(n) {
                Some(m) => p.both.push((*o, m)),
                None => p.removed.push(*o),
            },
            None => old_loose.push(*o),
        }
    }
    p.added.extend(
        new.iter()
            .filter(|s| s.stmt_name().is_some_and(|n| new_named.contains_key(n)))
            .copied(),
    );
    let mut new_loose: Vec<&T> = new
        .iter()
        .filter(|s| s.stmt_name().is_none())
        .copied()
        .collect();
    old_loose.retain(
        |o| match new_loose.iter().position(|m| m.stmt_id() == o.stmt_id()) {
            Some(i) => {
                p.both.push((o, new_loose.remove(i)));
                false
            }
            None => true,
        },
    );
    let common = old_loose.len().min(new_loose.len());
    for (o, m) in old_loose.iter().zip(new_loose.iter()).take(common) {
        p.both.push((o, m));
    }
    p.removed.extend(old_loose.into_iter().skip(common));
    p.added.extend(new_loose.into_iter().skip(common));
    p
}

/// The statements that reject: an audited one never does and is judged as if it were absent
/// (`spec/rules.md` section 17.1).
pub(super) fn enforced<T: Stmt>(items: &[T]) -> Vec<&T> {
    items.iter().filter(|s| !s.stmt_audited()).collect()
}

/// True when `other` holds an audited statement that is `s` with the annotation added: the same
/// name, or for an unnamed one the same expression.
fn audited_in<T: Stmt>(other: &[T], s: &T) -> bool {
    other.iter().any(|x| {
        x.stmt_audited()
            && match (x.stmt_name(), s.stmt_name()) {
                (Some(a), Some(b)) => a == b,
                (None, None) => x.stmt_id() == s.stmt_id(),
                _ => false,
            }
    })
}

const AUDITED_NOW: &str = "; it is now audited (`@rollout(audit)`): reported and never rejecting, so judged like removing it";
const ENFORCED_NOW: &str = "; it is no longer audited: it now rejects, so judged like adding it";

pub(super) fn label(kind: &str, name: Option<&str>, expr: &str) -> String {
    match name {
        Some(n) => format!("{kind} `{n}`"),
        None => format!("{kind} `{expr}`"),
    }
}

pub(super) fn diff_endpoint_rules(
    ctx: &Ctx<'_>,
    s: &str,
    o: &Endpoint,
    n: &Endpoint,
    out: &mut Vec<Change>,
) {
    diff_facts(ctx, s, o, n, out);
    diff_requires(ctx, s, o, n, out);
    diff_ensures(s, o, n, out);
    diff_transition(ctx, s, o, n, out);
}

fn diff_facts(ctx: &Ctx<'_>, s: &str, o: &Endpoint, n: &Endpoint, out: &mut Vec<Change>) {
    let (old, new) = (&o.rules.facts, &n.rules.facts);
    let note = |msg: String| Change::non_breaking(ChangeKind::FactChanged, s, msg);
    for f in old {
        match new.iter().find(|g| g.name == f.name) {
            None => out.push(note(format!(
                "note: fact `{}` was removed; not a wire change, the server implementation must change",
                f.name
            ))),
            Some(g) => {
                let same = ctx.type_key(true, &f.ty) == ctx.type_key(false, &g.ty)
                    && f.optional == g.optional
                    && f.key.as_ref().map(expr_text) == g.key.as_ref().map(expr_text);
                if !same {
                    out.push(note(format!(
                        "note: fact `{}` changed its type, optionality or key; not a wire change, \
                         the server implementation must change",
                        f.name
                    )));
                }
            }
        }
    }
    for g in new.iter().filter(|g| !old.iter().any(|f| f.name == g.name)) {
        out.push(note(format!(
            "note: fact `{}` was added; not a wire change, the server implementation must change",
            g.name
        )));
    }
}

/// The error a failed `require` answers with, as text: its name, status and payload.
fn else_text(ir: &Ir, e: Option<&ElseError>) -> String {
    let Some(e) = e else {
        return "ValidationError (422)".to_string();
    };
    let fields: Vec<String> = e
        .fields
        .iter()
        .map(|f| format!("{}: {}", f.name, expr_text(&f.value)))
        .collect();
    let payload = if fields.is_empty() {
        String::new()
    } else {
        format!(" {{ {} }}", fields.join(", "))
    };
    format!("{} ({}){payload}", ir.decl(e.decl).name, e.status)
}

fn diff_requires(ctx: &Ctx<'_>, s: &str, o: &Endpoint, n: &Endpoint, out: &mut Vec<Change>) {
    let p = pair(&enforced(&o.rules.requires), &enforced(&n.rules.requires));
    for r in p.added {
        let what = label("require", r.name.as_deref(), &expr_text(&r.expr));
        let now = if audited_in(&o.rules.requires, r) {
            ENFORCED_NOW
        } else {
            ""
        };
        out.push(Change::breaking(
            ChangeKind::RequireAdded,
            s,
            format!(
                "{what} was added: `{}`; a request that was accepted before may now be rejected{now}",
                expr_text(&r.expr)
            ),
        ));
    }
    for r in p.removed {
        let what = label("require", r.name.as_deref(), &expr_text(&r.expr));
        let now = if audited_in(&n.rules.requires, r) {
            AUDITED_NOW
        } else {
            ""
        };
        out.push(Change::non_breaking(
            ChangeKind::RequireRemoved,
            s,
            format!(
                "{what} was removed (was `{}`); requests are now accepted more widely{now}",
                expr_text(&r.expr)
            ),
        ));
    }
    for (a, b) in p.both {
        require_changed(ctx, s, (a, b), out);
    }
}

fn require_changed(
    ctx: &Ctx<'_>,
    s: &str,
    pair: (&EndpointRequire, &EndpointRequire),
    out: &mut Vec<Change>,
) {
    let (a, b) = pair;
    let what = label("require", a.name.as_deref(), &expr_text(&a.expr));
    if a.id != b.id {
        out.push(Change::possibly_breaking(
            ChangeKind::RequireChanged,
            s,
            format!(
                "possibly breaking: {what} changed from `{}` to `{}`; the tool cannot tell \
                 whether the new rule is tighter or looser",
                expr_text(&a.expr),
                expr_text(&b.expr)
            ),
        ));
    }
    let (old_else, new_else) = (
        else_text(ctx.old, a.else_error.as_ref()),
        else_text(ctx.new, b.else_error.as_ref()),
    );
    if old_else != new_else {
        out.push(Change::breaking(
            ChangeKind::RequireErrorChanged,
            s,
            format!(
                "{what} now fails with {new_else} instead of {old_else}; callers that match on \
                 the error are affected"
            ),
        ));
    }
    if a.message != b.message {
        out.push(Change::non_breaking(
            ChangeKind::RuleMessageChanged,
            s,
            format!("note: the message of {what} changed"),
        ));
    }
}

fn diff_ensures(s: &str, o: &Endpoint, n: &Endpoint, out: &mut Vec<Change>) {
    let p = pair(&enforced(&o.rules.ensures), &enforced(&n.rules.ensures));
    for e in p.removed {
        let what = label("ensure", e.name.as_deref(), &expr_text(&e.expr));
        let now = if audited_in(&n.rules.ensures, e) {
            AUDITED_NOW
        } else {
            ""
        };
        out.push(Change::breaking(
            ChangeKind::EnsureRemoved,
            s,
            format!(
                "{what} was removed (was `{}`); consumers lose a guarantee on the result{now}",
                expr_text(&e.expr)
            ),
        ));
    }
    for e in p.added {
        let what = label("ensure", e.name.as_deref(), &expr_text(&e.expr));
        let now = if audited_in(&o.rules.ensures, e) {
            ENFORCED_NOW
        } else {
            ""
        };
        out.push(Change::non_breaking(
            ChangeKind::EnsureAdded,
            s,
            format!(
                "note: {what} was added: `{}`; a new guarantee on the result{now}",
                expr_text(&e.expr)
            ),
        ));
    }
    for (a, b) in p.both {
        let what = label("ensure", a.name.as_deref(), &expr_text(&a.expr));
        if a.id != b.id {
            out.push(Change::possibly_breaking(
                ChangeKind::EnsureChanged,
                s,
                format!(
                    "possibly breaking: {what} changed from `{}` to `{}`; the tool cannot tell \
                     whether the new guarantee is stronger or weaker",
                    expr_text(&a.expr),
                    expr_text(&b.expr)
                ),
            ));
        }
        if a.message != b.message {
            out.push(Change::non_breaking(
                ChangeKind::RuleMessageChanged,
                s,
                format!("note: the message of {what} changed"),
            ));
        }
    }
}

fn diff_transition(ctx: &Ctx<'_>, s: &str, o: &Endpoint, n: &Endpoint, out: &mut Vec<Change>) {
    match (&o.rules.transition, &n.rules.transition) {
        (None, None) => {}
        (None, Some(t)) => out.push(Change::breaking(
            ChangeKind::TransitionChanged,
            s,
            format!(
                "the endpoint now only accepts {} -> {} on `{}`; other states answer 409",
                t.from.join(", "),
                t.to,
                field_path(t)
            ),
        )),
        (Some(t), None) => out.push(Change::non_breaking(
            ChangeKind::TransitionChanged,
            s,
            format!(
                "the transition guard {} -> {} on `{}` was removed; any state is now accepted",
                t.from.join(", "),
                t.to,
                field_path(t)
            ),
        )),
        (Some(a), Some(b)) => transition_changed(ctx, s, (a, b), out),
    }
}

fn field_path(t: &EndpointTransition) -> String {
    let steps: Vec<&str> = t.steps.iter().map(|x| x.wire.as_str()).collect();
    format!("{}.{}", t.fact, steps.join("."))
}

fn wire_path(t: &EndpointTransition) -> Vec<&str> {
    t.steps.iter().map(|x| x.wire.as_str()).collect()
}

fn transition_changed(
    ctx: &Ctx<'_>,
    s: &str,
    pair: (&EndpointTransition, &EndpointTransition),
    out: &mut Vec<Change>,
) {
    let (a, b) = pair;
    let kind = ChangeKind::TransitionChanged;
    if a.fact != b.fact
        || wire_path(a) != wire_path(b)
        || ctx.old.decl(a.decl).qualified != ctx.new.decl(b.decl).qualified
    {
        out.push(Change::breaking(
            kind,
            s,
            format!(
                "the transition now guards `{}` instead of `{}`",
                field_path(b),
                field_path(a)
            ),
        ));
        return;
    }
    if a.to != b.to {
        out.push(Change::breaking(
            kind,
            s,
            format!("the target state changed from `{}` to `{}`", a.to, b.to),
        ));
    }
    for f in a.from.iter().filter(|f| !b.from.contains(f)) {
        out.push(Change::breaking(
            kind,
            s,
            format!("the transition no longer accepts the source state `{f}`"),
        ));
    }
    for f in b.from.iter().filter(|f| !a.from.contains(f)) {
        out.push(Change::non_breaking(
            kind,
            s,
            format!("the transition now also accepts the source state `{f}`"),
        ));
    }
}
