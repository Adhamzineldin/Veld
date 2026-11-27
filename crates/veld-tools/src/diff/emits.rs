//! `emits` of a flow (`spec/events.md` sections 11 and 16), judged by what a consumer sees.
//!
//! A consumer sees messages, not moves: an event it handles arrives or does not, on a topic, with
//! a key. So:
//!
//! | Change | Result |
//! |---|---|
//! | a flow gains `emits` | compatible: the service sends one more message when it takes that edge, and a consumer that does not handle the event ignores it (`spec/events.md` section 8) |
//! | a flow loses `emits` | possibly breaking: a consumer that handles the event no longer gets it for that move |
//! | a flow emits another event | one entry for each: the old event is lost, the new one is added |
//! | the channel a move sends an event on moves to another topic or key | breaking: the consumers of the old topic no longer get it |
//!
//! An edge that was added or removed is reported as a flow (`rules.rs`), and not again here. The
//! events are matched by qualified name, the enums too, and the channels by topic and key: the
//! indices in the two IRs mean nothing to each other.

use std::collections::BTreeMap;

use veld_ir::{Decl, DeclKind, Flow, Ir};

use super::{Change, ChangeKind, Ctx};

fn enums(ir: &Ir) -> BTreeMap<&str, &Decl> {
    ir.decls
        .iter()
        .filter(|d| matches!(&d.kind, DeclKind::Enum(e) if e.is_state_machine()))
        .map(|d| (d.qualified.as_str(), d))
        .collect()
}

fn flows(d: &Decl) -> &[Flow] {
    match &d.kind {
        DeclKind::Enum(e) => &e.flows,
        DeclKind::Model(_)
        | DeclKind::Error(_)
        | DeclKind::Union(_)
        | DeclKind::Const(_)
        | DeclKind::Event(_) => &[],
    }
}

fn event_name(ir: &Ir, f: &Flow) -> Option<String> {
    f.emits.map(|e| ir.decl(e).qualified.clone())
}

pub(super) fn diff_emits(ctx: &Ctx<'_>, out: &mut Vec<Change>) {
    let (old, new) = (enums(ctx.old), enums(ctx.new));
    for (q, o) in &old {
        let Some(n) = new.get(q) else { continue };
        for of in flows(o) {
            let Some(nf) = flows(n).iter().find(|f| f.from == of.from && f.to == of.to) else {
                continue;
            };
            let (was, is) = (event_name(ctx.old, of), event_name(ctx.new, nf));
            if was == is {
                continue;
            }
            let edge = format!("{} -> {}", of.from, of.to);
            if let Some(w) = &was {
                out.push(Change::possibly_breaking(
                    ChangeKind::EmitsRemoved,
                    q,
                    format!("the move `{edge}` no longer emits `{w}`: a consumer that handles it no longer gets it for that move"),
                ));
            }
            if let Some(i) = &is {
                out.push(Change::non_breaking(
                    ChangeKind::EmitsAdded,
                    q,
                    format!("the move `{edge}` now emits `{i}`"),
                ));
            }
        }
    }
}

/// The address a consumer reads an event of a transition from: the topic and the key of the
/// channel the service sends it on.
type Address = (String, String);

fn addresses(ir: &Ir) -> BTreeMap<(String, String, String), Address> {
    let mut out = BTreeMap::new();
    for s in &ir.services {
        let service = format!("{}.{}", ir.package(s.package).name, s.name);
        for ep in &s.endpoints {
            let Some(t) = &ep.rules.transition else {
                continue;
            };
            for e in &t.emits {
                let c = ir.channel(e.channel);
                let key = super::events::key_text(ir, c);
                out.insert(
                    (
                        service.clone(),
                        ep.name.clone(),
                        ir.decl(e.event).qualified.clone(),
                    ),
                    (c.topic.clone(), key),
                );
            }
        }
    }
    out
}

/// An event an endpoint keeps emitting that goes to another topic or key.
pub(super) fn diff_emit_channels(ctx: &Ctx<'_>, out: &mut Vec<Change>) {
    let (old, new) = (addresses(ctx.old), addresses(ctx.new));
    for (k, was) in &old {
        let Some(is) = new.get(k) else { continue };
        if was == is {
            continue;
        }
        let (service, endpoint, event) = k;
        out.push(Change::breaking(
            ChangeKind::EmitsTopicChanged,
            &format!("{service}.{endpoint}"),
            format!(
                "`{endpoint}` sends `{event}` on topic `{}` with key {} instead of topic `{}` with key {}: a consumer of the old topic no longer gets it",
                is.0,
                key_name(&is.1),
                was.0,
                key_name(&was.1)
            ),
        ));
    }
}

fn key_name(key: &str) -> String {
    if key.is_empty() {
        "none".to_owned()
    } else {
        format!("`{key}`")
    }
}
