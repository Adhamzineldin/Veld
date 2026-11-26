//! Events (`spec/events.md`): channels, `publishes`, `subscribes` and the graph checks
//! `E1001` to `E1009`, `W1001` and `W1002` (`E1010` lives with the examples).
//!
//! A channel is named and imported like a declaration, but it is not one: it has no `DeclId` and
//! no type. Its names live in `Scope::channel_names`, beside the declarations of the module, and
//! the package-wide duplicate check of `collect` covers both. An event is an ordinary entry of
//! the declaration table with the kind `DKind::Event`.

use std::collections::{BTreeMap, BTreeSet};

use super::annotations;
use super::{DKind, Program};
use crate::diagnostics::{Diagnostic, Span};
use crate::language::Target;
use crate::syntax::ast::{self, TypeExpr, TypeExprKind, doc_text};
use veld_ir::{PackageKind, Scalar, Type};

/// One `channel` declaration, once it is checked.
#[derive(Debug)]
pub struct ChannelInfo<'a> {
    pub pkg: usize,
    pub module: usize,
    pub name: String,
    /// `package:module.Name`.
    pub qualified: String,
    pub decl: &'a ast::Decl,
    pub ast: &'a ast::ChannelDecl,
    pub name_span: Span,
    /// `None` when the channel has an error; every use of it then stays silent.
    pub res: Option<RChannel>,
}

#[derive(Debug, Clone)]
pub struct RChannel {
    pub topic: String,
    /// The carried events (indices into `Program::decls`), in `carries` order.
    pub events: Vec<usize>,
    pub event_spans: Vec<Span>,
    pub key: Option<RChannelKey>,
    pub docs: Option<String>,
    pub deprecated: Option<String>,
    /// `@retry` (`spec/events.md` section 10.2).
    pub retry: Option<veld_ir::Retry>,
    pub dead_letter: bool,
    pub ordered: bool,
}

#[derive(Debug, Clone)]
pub struct RChannelKey {
    pub name: String,
    pub wire: String,
    pub ty: Type,
}

/// A channel name in a scope: declared here, or imported.
#[derive(Debug, Clone, Copy)]
pub struct ChannelEntry {
    pub channel: usize,
    pub import: Option<usize>,
}

/// A checked `subscribes` block.
#[derive(Debug, Clone)]
pub struct RSubscription {
    /// Index into `Program::channels`.
    pub channel: usize,
    pub group: String,
    pub handlers: Vec<RHandler>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct RHandler {
    /// The event, an index into `Program::decls`.
    pub event: usize,
    pub name: String,
    pub docs: Option<String>,
    pub span: Span,
}

impl Program<'_> {
    /// Looks the channel `name` up as seen from `module`, marking its import as used.
    #[must_use]
    pub fn lookup_channel(&self, module: usize, name: &str) -> Option<usize> {
        let scope = self.scopes.get(module)?;
        let e = scope.channel_names.get(name)?;
        if let Some(i) = e.import {
            scope.imports[i].used.set(true);
        }
        Some(e.channel)
    }

    fn name_of(t: &TypeExpr) -> Option<&ast::Ident> {
        match &t.kind {
            TypeExprKind::Named { name, args } if args.is_empty() => Some(name),
            TypeExprKind::Named { .. } | TypeExprKind::List(_) => None,
        }
    }

    /// The event a type position names, or a diagnostic with `code`.
    pub(super) fn event_ref(
        &self,
        module: usize,
        t: &TypeExpr,
        code: &'static str,
        diags: &mut Vec<Diagnostic>,
    ) -> Option<usize> {
        let Some(name) = Self::name_of(t) else {
            diags.push(Diagnostic::error(
                code,
                "expected the name of an event",
                t.span,
            ));
            return None;
        };
        if let Some(d) = self.lookup(module, &name.name) {
            if self.decls[d].kind == DKind::Event {
                return Some(d);
            }
            diags.push(Diagnostic::error(
                code,
                format!("`{}` is not an event", name.name),
                name.span,
            ));
            return None;
        }
        if let Some(c) = self.lookup_channel(module, &name.name) {
            diags.push(Diagnostic::error(
                code,
                format!("`{}` is a channel, not an event", self.channels[c].name),
                name.span,
            ));
            return None;
        }
        if self.scopes[module].poisoned.contains(&name.name) {
            return None;
        }
        let events = self.visible_events(module);
        diags.push(
            Diagnostic::error("E0205", format!("unknown event `{}`", name.name), name.span)
                .did_you_mean(&name.name, events.iter().map(String::as_str), None),
        );
        None
    }

    /// The channel a type position names, or a diagnostic with `code`.
    pub(super) fn channel_ref(
        &self,
        module: usize,
        t: &TypeExpr,
        code: &'static str,
        diags: &mut Vec<Diagnostic>,
    ) -> Option<usize> {
        let Some(name) = Self::name_of(t) else {
            diags.push(Diagnostic::error(
                code,
                "expected the name of a channel",
                t.span,
            ));
            return None;
        };
        if let Some(c) = self.lookup_channel(module, &name.name) {
            return Some(c);
        }
        if self.lookup(module, &name.name).is_some() {
            diags.push(Diagnostic::error(
                code,
                format!("`{}` is not a channel", name.name),
                name.span,
            ));
            return None;
        }
        if self.scopes[module].poisoned.contains(&name.name) {
            return None;
        }
        let names: Vec<&str> = self.scopes[module]
            .channel_names
            .keys()
            .map(String::as_str)
            .collect();
        diags.push(
            Diagnostic::error(
                "E0205",
                format!("unknown channel `{}`", name.name),
                name.span,
            )
            .did_you_mean(&name.name, names.iter().copied(), None),
        );
        None
    }

    fn visible_events(&self, module: usize) -> Vec<String> {
        self.scopes[module]
            .names
            .iter()
            .filter(|(_, e)| self.decls[e.decl].kind == DKind::Event)
            .map(|(n, _)| n.clone())
            .collect()
    }

    /// The channels visible in `module` that carry `event`, as indices into `channels`.
    pub(super) fn visible_channels_of(&self, module: usize, event: usize) -> Vec<usize> {
        let mut seen = BTreeSet::new();
        self.scopes[module]
            .channel_names
            .values()
            .map(|e| e.channel)
            .filter(|c| seen.insert(*c))
            .filter(|c| {
                self.channels[*c]
                    .res
                    .as_ref()
                    .is_some_and(|r| r.events.contains(&event))
            })
            .collect()
    }
}

// ---- channels ---------------------------------------------------------------------------

pub fn check_channels(prog: &mut Program<'_>, diags: &mut Vec<Diagnostic>) {
    let mut results: Vec<Option<RChannel>> = (0..prog.channels.len())
        .map(|i| check_channel(prog, i, diags))
        .collect();
    let mut topics: BTreeMap<String, Span> = BTreeMap::new();
    for (c, r) in prog.channels.iter().zip(results.iter_mut()) {
        let Some(res) = r else { continue };
        let dead_letter = res.dead_letter.then(|| format!("{}.dlq", res.topic));
        let taken = [Some(res.topic.clone()), dead_letter]
            .into_iter()
            .flatten()
            .find_map(|t| topics.get(&t).map(|first| (t, *first)));
        if let Some((topic, first)) = taken {
            diags.push(
                Diagnostic::error(
                    "E1004",
                    format!("topic `{topic}` is already used by another channel"),
                    c.name_span,
                )
                .with_label(first, "first used here"),
            );
            *r = None;
        } else {
            topics.insert(res.topic.clone(), c.name_span);
            if res.dead_letter {
                topics.insert(format!("{}.dlq", res.topic), c.name_span);
            }
        }
    }
    for (c, r) in prog.channels.iter_mut().zip(results) {
        c.res = r;
    }
    warn_retry_without_dead_letter(prog, diags);
}

/// `W1003`: a channel with `@retry` and without `@deadLetter` (`spec/events.md` section 10.2).
/// A package from a registry is not the author's to fix and stays silent.
fn warn_retry_without_dead_letter(prog: &Program<'_>, diags: &mut Vec<Diagnostic>) {
    for c in &prog.channels {
        let Some(res) = &c.res else { continue };
        if res.retry.is_none() || res.dead_letter {
            continue;
        }
        if !matches!(
            prog.packages[c.pkg].kind,
            PackageKind::Service | PackageKind::Library
        ) {
            continue;
        }
        let at = c
            .decl
            .annotations
            .iter()
            .find(|a| a.name.name == "retry")
            .map_or(c.name_span, |a| a.span);
        diags.push(
            Diagnostic::warning(
                "W1003",
                format!(
                    "channel `{}` has `@retry` and no `@deadLetter`: a message whose retries are used up is gone",
                    c.name
                ),
                at,
            )
            .with_help("add `@deadLetter` to keep such messages on the topic `<topic>.dlq`"),
        );
    }
}

/// True when `topic` is a legal topic name (`spec/events.md` section 4).
#[must_use]
pub fn topic_is_legal(topic: &str) -> bool {
    legal_name(topic) && topic != "." && topic != ".." && !topic.ends_with(".dlq")
}

/// `[A-Za-z0-9._-]{1,200}`.
#[must_use]
pub fn legal_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 200
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn default_topic(package: &str, channel: &str) -> String {
    let pkg: String = package
        .trim_start_matches('@')
        .chars()
        .map(|c| if c == '/' { '.' } else { c })
        .collect();
    format!("{pkg}.{channel}")
}

fn check_channel(prog: &Program<'_>, i: usize, diags: &mut Vec<Diagnostic>) -> Option<RChannel> {
    let c = &prog.channels[i];
    let anns = annotations::parse(&c.decl.annotations, Target::Channel, None, diags);
    let mut ok = true;
    let topic = anns
        .topic
        .clone()
        .unwrap_or_else(|| default_topic(&prog.packages[c.pkg].name, &c.name));
    if !topic_is_legal(&topic) {
        let at = c
            .decl
            .annotations
            .iter()
            .find(|a| a.name.name == "topic")
            .map_or(c.name_span, |a| a.span);
        diags.push(
            Diagnostic::error("E1003", format!("`{topic}` is not a legal topic name"), at)
                .with_help("a topic matches [A-Za-z0-9._-]{1,200}, is not `.` or `..` and does not end in `.dlq`"),
        );
        ok = false;
    }
    let (events, event_spans) = carried_events(prog, c, diags);
    if events.is_empty() {
        if c.ast.carries.is_empty() {
            diags.push(Diagnostic::error(
                "E1002",
                format!("channel `{}` carries nothing", c.name),
                c.name_span,
            ));
        }
        ok = false;
    }
    let key = check_key(prog, c, &events, diags);
    let key = match key {
        Ok(k) => k,
        Err(()) => {
            ok = false;
            None
        }
    };
    ok.then(|| RChannel {
        topic,
        events,
        event_spans,
        key,
        docs: doc_text(&c.decl.docs),
        deprecated: anns.deprecated,
        retry: anns.retry,
        dead_letter: anns.dead_letter,
        ordered: anns.ordered,
    })
}

/// The events of every `carries` line, each once; a bad entry is reported and skipped.
fn carried_events(
    prog: &Program<'_>,
    c: &ChannelInfo<'_>,
    diags: &mut Vec<Diagnostic>,
) -> (Vec<usize>, Vec<Span>) {
    let mut events: Vec<usize> = Vec::new();
    let mut spans: Vec<Span> = Vec::new();
    let mut failed = false;
    for line in &c.ast.carries {
        for t in &line.events {
            let Some(e) = prog.event_ref(c.module, t, "E1002", diags) else {
                failed = true;
                continue;
            };
            if events.contains(&e) {
                diags.push(Diagnostic::error(
                    "E1002",
                    format!(
                        "channel `{}` carries `{}` twice",
                        c.name, prog.decls[e].name
                    ),
                    t.span,
                ));
                failed = true;
                continue;
            }
            events.push(e);
            spans.push(t.span);
        }
    }
    if failed {
        events.clear();
        spans.clear();
    }
    (events, spans)
}

/// The key of the channel. `Err` when there is a problem; `Ok(None)` for no key.
fn check_key(
    prog: &Program<'_>,
    c: &ChannelInfo<'_>,
    events: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> Result<Option<RChannelKey>, ()> {
    let Some(first) = c.ast.keys.first() else {
        return Ok(None);
    };
    if let Some(second) = c.ast.keys.get(1) {
        diags.push(
            Diagnostic::error(
                "E1005",
                format!("channel `{}` has more than one `key`", c.name),
                second.span,
            )
            .with_label(first.span, "first `key` here"),
        );
        return Err(());
    }
    let name = &first.field.name;
    let mut found: Option<(Type, String)> = None;
    for &e in events {
        let ev = &prog.decls[e];
        let Some(f) = ev.res.fields.iter().find(|f| &f.name == name) else {
            diags.push(Diagnostic::error(
                "E1005",
                format!(
                    "event `{}` has no field `{name}`, the key of channel `{}`",
                    ev.name, c.name
                ),
                first.field.span,
            ));
            return Err(());
        };
        if f.optional {
            diags.push(Diagnostic::error(
                "E1005",
                format!(
                    "the key `{name}` must be required, but it is optional in `{}`",
                    ev.name
                ),
                first.field.span,
            ));
            return Err(());
        }
        if let Some((ty, _)) = &found
            && ty != &f.ty
        {
            diags.push(Diagnostic::error(
                "E1005",
                format!("the key `{name}` has another type in `{}`", ev.name),
                first.field.span,
            ));
            return Err(());
        }
        found = Some((f.ty.clone(), f.wire.clone()));
    }
    let Some((ty, wire)) = found else {
        return Err(());
    };
    let ok_type = matches!(
        ty,
        Type::Scalar(Scalar::String | Scalar::Uuid | Scalar::Int | Scalar::Long)
    );
    if !ok_type {
        diags.push(Diagnostic::error(
            "E1005",
            format!(
                "the key `{name}` has type `{}`; a key is a `string`, `uuid`, `int` or `long`",
                prog.type_name(&ty)
            ),
            first.field.span,
        ));
        return Err(());
    }
    Ok(Some(RChannelKey {
        name: name.clone(),
        wire,
        ty,
    }))
}

// ---- services -----------------------------------------------------------------------------

/// What a service says about events, checked. `names` holds the endpoint names, which handler
/// names share a namespace with (`E0509`).
pub struct ServiceEvents {
    pub publishes: Vec<usize>,
    pub publish_spans: Vec<Span>,
    pub subscriptions: Vec<RSubscription>,
}

pub fn check_service_events(
    prog: &Program<'_>,
    module: usize,
    s: &ast::ServiceDecl,
    endpoints: &[(String, Span)],
    diags: &mut Vec<Diagnostic>,
) -> ServiceEvents {
    let mut out = ServiceEvents {
        publishes: Vec::new(),
        publish_spans: Vec::new(),
        subscriptions: Vec::new(),
    };
    for line in &s.publishes {
        for t in &line.channels {
            let Some(c) = prog.channel_ref(module, t, "E1006", diags) else {
                continue;
            };
            if out.publishes.contains(&c) {
                diags.push(Diagnostic::error(
                    "E1006",
                    format!(
                        "service `{}` publishes `{}` twice",
                        s.name.name, prog.channels[c].name
                    ),
                    t.span,
                ));
                continue;
            }
            out.publishes.push(c);
            out.publish_spans.push(t.span);
        }
    }
    let mut taken: Vec<(String, Span)> = endpoints.to_vec();
    for sub in &s.subscribes {
        if let Some(r) =
            check_subscription(prog, module, s, sub, &out.subscriptions, &mut taken, diags)
        {
            out.subscriptions.push(r);
        }
    }
    out
}

fn check_subscription(
    prog: &Program<'_>,
    module: usize,
    s: &ast::ServiceDecl,
    sub: &ast::Subscribes,
    done: &[RSubscription],
    taken: &mut Vec<(String, Span)>,
    diags: &mut Vec<Diagnostic>,
) -> Option<RSubscription> {
    let channel = prog.channel_ref(module, &sub.channel, "E1006", diags);
    let mut ok = channel.is_some();
    if let Some(c) = channel
        && done.iter().any(|d| d.channel == c)
    {
        diags.push(Diagnostic::error(
            "E1006",
            format!(
                "service `{}` subscribes to `{}` twice",
                s.name.name, prog.channels[c].name
            ),
            sub.channel.span,
        ));
        ok = false;
    }
    if !legal_name(&sub.group.value) {
        diags.push(
            Diagnostic::error(
                "E1007",
                format!("`{}` is not a legal consumer group name", sub.group.value),
                sub.group.span,
            )
            .with_help("a group matches [A-Za-z0-9._-]{1,200}"),
        );
        ok = false;
    }
    if sub.handlers.is_empty() {
        diags.push(Diagnostic::error(
            "E1008",
            "a subscription needs at least one `on` line",
            sub.span,
        ));
        ok = false;
    }
    let carried: Option<&RChannel> = channel.and_then(|c| prog.channels[c].res.as_ref());
    let handlers = check_handlers(prog, module, sub, channel, carried, taken, diags);
    ok &= handlers.is_some();
    let handlers = handlers?;
    let channel = channel?;
    ok.then(|| RSubscription {
        channel,
        group: sub.group.value.clone(),
        handlers,
        span: sub.span,
    })
}

fn check_handlers(
    prog: &Program<'_>,
    module: usize,
    sub: &ast::Subscribes,
    channel: Option<usize>,
    carried: Option<&RChannel>,
    taken: &mut Vec<(String, Span)>,
    diags: &mut Vec<Diagnostic>,
) -> Option<Vec<RHandler>> {
    let mut out: Vec<RHandler> = Vec::new();
    let mut ok = true;
    for on in &sub.handlers {
        let Some(e) = prog.event_ref(module, &on.event, "E1008", diags) else {
            ok = false;
            continue;
        };
        if let (Some(r), Some(c)) = (carried, channel)
            && !r.events.contains(&e)
        {
            diags.push(Diagnostic::error(
                "E1008",
                format!(
                    "channel `{}` does not carry `{}`",
                    prog.channels[c].name, prog.decls[e].name
                ),
                on.event.span,
            ));
            ok = false;
            continue;
        }
        if let Some(prev) = out.iter().find(|h| h.event == e) {
            diags.push(
                Diagnostic::error(
                    "E1008",
                    format!(
                        "`{}` has two `on` lines in this subscription",
                        prog.decls[e].name
                    ),
                    on.event.span,
                )
                .with_label(prev.span, "first `on` line here"),
            );
            ok = false;
            continue;
        }
        if let Some((_, first)) = taken.iter().find(|(n, _)| n == &on.handler.name) {
            diags.push(
                Diagnostic::error(
                    "E0509",
                    format!(
                        "`{}` is already an endpoint or handler of this service",
                        on.handler.name
                    ),
                    on.handler.span,
                )
                .with_label(*first, "first declared here"),
            );
            ok = false;
            continue;
        }
        taken.push((on.handler.name.clone(), on.handler.span));
        out.push(RHandler {
            event: e,
            name: on.handler.name.clone(),
            docs: doc_text(&on.docs),
            span: on.span,
        });
    }
    ok.then_some(out)
}

// ---- the graph ------------------------------------------------------------------------------

/// `E1009`, `W1001` and `W1002` (`spec/events.md` section 5.1).
pub fn check_graph(prog: &Program<'_>, diags: &mut Vec<Diagnostic>) {
    // A service dropped by an earlier error would make every channel look unpublished or unread.
    if crate::diagnostics::has_errors(diags) {
        return;
    }
    let published: BTreeSet<usize> = prog
        .services
        .iter()
        .flat_map(|s| s.publishes.iter().copied())
        .collect();
    let mut handled: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for s in &prog.services {
        for sub in &s.subscriptions {
            handled
                .entry(sub.channel)
                .or_default()
                .extend(sub.handlers.iter().map(|h| h.event));
            let c = &prog.channels[sub.channel];
            if c.res.is_some()
                && prog.packages[c.pkg].kind == PackageKind::Service
                && !published.contains(&sub.channel)
            {
                diags.push(
                    Diagnostic::error(
                        "E1009",
                        format!("nobody publishes channel `{}`", c.name),
                        sub.span,
                    )
                    .with_help("a service of this workspace must publish it, or the channel belongs in a library"),
                );
            }
        }
    }
    for &ci in &published {
        let c = &prog.channels[ci];
        let Some(res) = &c.res else { continue };
        match handled.get(&ci) {
            None if prog.packages[c.pkg].kind == PackageKind::Service => {
                diags.push(Diagnostic::warning(
                    "W1001",
                    format!(
                        "channel `{}` is published and nobody in the workspace subscribes to it",
                        c.name
                    ),
                    c.name_span,
                ));
            }
            None => {}
            Some(events) => {
                for (e, span) in res.events.iter().zip(&res.event_spans) {
                    if !events.contains(e) {
                        diags.push(Diagnostic::warning(
                            "W1002",
                            format!(
                                "no subscription handles `{}`, an event of channel `{}`",
                                prog.decls[*e].name, c.name
                            ),
                            *span,
                        ));
                    }
                }
            }
        }
    }
}
