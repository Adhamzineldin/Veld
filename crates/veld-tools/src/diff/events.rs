//! Events, channels, `publishes` and `subscribes` (`spec/events.md` sections 3 to 5).
//!
//! Event-aware classification is a later phase (`spec/events.md` section 16). Until then every
//! change is one entry, classified "possibly breaking": the tool cannot yet tell which side of
//! the stream a change hurts. Events are matched by qualified name, like other declarations.

use std::collections::{BTreeMap, BTreeSet};

use veld_ir::{Channel, Decl, DeclKind, Ir, Service, Subscription};

use super::{Change, ChangeKind, Ctx};

fn events(ir: &Ir) -> BTreeMap<&str, &Decl> {
    ir.decls
        .iter()
        .filter(|d| d.is_event())
        .map(|d| (d.qualified.as_str(), d))
        .collect()
}

fn channels(ir: &Ir) -> BTreeMap<&str, &Channel> {
    ir.channels
        .iter()
        .map(|c| (c.qualified.as_str(), c))
        .collect()
}

pub(super) fn diff_events(ctx: &Ctx<'_>, out: &mut Vec<Change>) {
    diff_event_decls(ctx, out);
    diff_channels(ctx, out);
    diff_services(ctx, out);
}

/// The signature differs, or the rules digest does: a rule is not part of the signature, and a
/// changed rule is a change of the event (`spec/events.md` section 16).
fn event_changed(o: &Decl, n: &Decl) -> bool {
    let digest = |d: &Decl| match &d.kind {
        DeclKind::Event(e) => e.rules_digest.clone(),
        DeclKind::Model(_)
        | DeclKind::Error(_)
        | DeclKind::Enum(_)
        | DeclKind::Union(_)
        | DeclKind::Const(_) => None,
    };
    o.sig != n.sig || digest(o) != digest(n)
}

fn diff_event_decls(ctx: &Ctx<'_>, out: &mut Vec<Change>) {
    let (old, new) = (events(ctx.old), events(ctx.new));
    for (q, o) in &old {
        match new.get(q) {
            None => out.push(Change::possibly_breaking(
                ChangeKind::EventRemoved,
                q,
                format!("event `{}` was removed or renamed", o.name),
            )),
            Some(n) if event_changed(o, n) => out.push(Change::possibly_breaking(
                ChangeKind::EventChanged,
                q,
                format!("event `{}` changed", o.name),
            )),
            Some(_) => {}
        }
    }
    for (q, n) in &new {
        if !old.contains_key(q) {
            out.push(Change::possibly_breaking(
                ChangeKind::EventAdded,
                q,
                format!("event `{}` was added", n.name),
            ));
        }
    }
}

/// The carried events as a sorted set of qualified names.
fn carried(ir: &Ir, c: &Channel) -> BTreeSet<String> {
    c.events
        .iter()
        .map(|d| ir.decl(*d).qualified.clone())
        .collect()
}

fn key_text(ir: &Ir, c: &Channel) -> String {
    c.key.as_ref().map_or_else(String::new, |k| {
        format!("{}:{}", k.wire, super::type_key(ir, &k.ty))
    })
}

fn channel_changed(ctx: &Ctx<'_>, o: &Channel, n: &Channel) -> bool {
    o.topic != n.topic
        || key_text(ctx.old, o) != key_text(ctx.new, n)
        || carried(ctx.old, o) != carried(ctx.new, n)
}

fn diff_channels(ctx: &Ctx<'_>, out: &mut Vec<Change>) {
    let (old, new) = (channels(ctx.old), channels(ctx.new));
    for (q, o) in &old {
        match new.get(q) {
            None => out.push(Change::possibly_breaking(
                ChangeKind::ChannelRemoved,
                q,
                format!("channel `{}` was removed or renamed", o.name),
            )),
            Some(n) if channel_changed(ctx, o, n) => out.push(Change::possibly_breaking(
                ChangeKind::ChannelChanged,
                q,
                format!("the topic, key or events of channel `{}` changed", o.name),
            )),
            Some(_) => {}
        }
    }
    for (q, n) in &new {
        if !old.contains_key(q) {
            out.push(Change::possibly_breaking(
                ChangeKind::ChannelAdded,
                q,
                format!("channel `{}` was added", n.name),
            ));
        }
    }
}

type ServiceKey = (String, String);

fn services(ir: &Ir) -> BTreeMap<ServiceKey, &Service> {
    ir.services
        .iter()
        .map(|s| ((ir.package(s.package).name.clone(), s.name.clone()), s))
        .collect()
}

fn publishes(ir: &Ir, s: Option<&Service>) -> BTreeSet<String> {
    s.map(|s| {
        s.publishes
            .iter()
            .map(|c| ir.channel(*c).qualified.clone())
            .collect()
    })
    .unwrap_or_default()
}

/// A subscription as comparable text: its group and its sorted `on` lines.
fn sub_text(ir: &Ir, s: &Subscription) -> String {
    let mut lines: Vec<String> = s
        .handlers
        .iter()
        .map(|h| format!("{}->{}", ir.decl(h.event).qualified, h.name))
        .collect();
    lines.sort();
    format!("{} {}", s.group, lines.join(","))
}

fn subscriptions(ir: &Ir, s: Option<&Service>) -> BTreeMap<String, String> {
    s.map(|s| {
        s.subscriptions
            .iter()
            .map(|x| (ir.channel(x.channel).qualified.clone(), sub_text(ir, x)))
            .collect()
    })
    .unwrap_or_default()
}

fn diff_services(ctx: &Ctx<'_>, out: &mut Vec<Change>) {
    let (old, new) = (services(ctx.old), services(ctx.new));
    let keys: BTreeSet<&ServiceKey> = old.keys().chain(new.keys()).collect();
    for k in keys {
        let (o, n) = (old.get(k).copied(), new.get(k).copied());
        let subject = format!("{}.{}", k.0, k.1);
        diff_publishes(
            &subject,
            (publishes(ctx.old, o), publishes(ctx.new, n)),
            out,
        );
        diff_subscriptions(
            &subject,
            (subscriptions(ctx.old, o), subscriptions(ctx.new, n)),
            out,
        );
    }
}

fn diff_publishes(
    subject: &str,
    sets: (BTreeSet<String>, BTreeSet<String>),
    out: &mut Vec<Change>,
) {
    let (old, new) = sets;
    for c in old.difference(&new) {
        out.push(Change::possibly_breaking(
            ChangeKind::PublishesRemoved,
            subject,
            format!("service `{subject}` no longer publishes to `{c}`"),
        ));
    }
    for c in new.difference(&old) {
        out.push(Change::possibly_breaking(
            ChangeKind::PublishesAdded,
            subject,
            format!("service `{subject}` now publishes to `{c}`"),
        ));
    }
}

fn diff_subscriptions(
    subject: &str,
    maps: (BTreeMap<String, String>, BTreeMap<String, String>),
    out: &mut Vec<Change>,
) {
    let (old, new) = maps;
    for (c, text) in &old {
        match new.get(c) {
            None => out.push(Change::possibly_breaking(
                ChangeKind::SubscriptionRemoved,
                subject,
                format!("service `{subject}` no longer subscribes to `{c}`"),
            )),
            Some(t) if t != text => out.push(Change::possibly_breaking(
                ChangeKind::SubscriptionChanged,
                subject,
                format!(
                    "the group or handlers of the subscription of `{subject}` to `{c}` changed"
                ),
            )),
            Some(_) => {}
        }
    }
    for c in new.keys().filter(|c| !old.contains_key(*c)) {
        out.push(Change::possibly_breaking(
            ChangeKind::SubscriptionAdded,
            subject,
            format!("service `{subject}` now subscribes to `{c}`"),
        ));
    }
}
