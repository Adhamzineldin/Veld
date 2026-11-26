//! The rules digest of an event, exactly `spec/events.md` section 6.1.
//!
//! `hex16( sha256( RFC 8785 JSON array of the sorted, distinct rule ids of every invariant and the
//! ids of every derived field the event reaches ) )`: the formula of `spec/rules.md` section 9.7 applied to one event. The event
//! reaches its own invariants and those of everything its fields name, transitively.

use std::collections::BTreeSet;

use veld_ir::{DeclId, DeclKind, Ir};

use super::{Json, canonical, hex16, text};

/// The digest of the event `event`, or `None` when it reaches no invariant.
#[must_use]
pub fn event_rules_digest(ir: &Ir, event: DeclId) -> Option<String> {
    let mut seen = BTreeSet::new();
    let mut stack = vec![event];
    let mut ids: BTreeSet<&str> = BTreeSet::new();
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        let d = ir.decl(id);
        let enforced = d.rules().iter().filter(|r| !r.is_audited());
        ids.extend(enforced.map(|r| r.id.as_str()));
        // A subscriber reads the derived values the publisher computed (`spec/rules.md` 17.2).
        ids.extend(d.fields().iter().filter_map(|f| f.derived_id.as_deref()));
        for f in d.fields() {
            f.ty.collect_decls(&mut stack);
        }
        match &d.kind {
            DeclKind::Model(m) => stack.extend(m.parent),
            DeclKind::Union(u) => stack.extend(u.arms.iter().map(|a| a.decl)),
            DeclKind::Const(c) => c.ty.collect_decls(&mut stack),
            DeclKind::Error(_) | DeclKind::Event(_) | DeclKind::Enum(_) => {}
        }
    }
    if ids.is_empty() {
        return None;
    }
    let list = Json::Arr(ids.into_iter().map(text).collect());
    Some(hex16(canonical(&list).as_bytes()))
}

/// Sets `rules_digest` on every event of `ir`.
pub fn fill_event_digests(ir: &mut Ir) {
    let digests: Vec<(usize, Option<String>)> = ir
        .decls
        .iter()
        .filter(|d| d.is_event())
        .map(|d| (d.id.index(), event_rules_digest(ir, d.id)))
        .collect();
    for (i, digest) in digests {
        if let DeclKind::Event(e) = &mut ir.decls[i].kind {
            e.rules_digest = digest;
        }
    }
}
