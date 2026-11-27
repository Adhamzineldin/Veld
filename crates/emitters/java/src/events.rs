//! Events (`spec/events.md` section 14, `spec/targets/java.md` section 12): for every service that
//! publishes, `<Service>Publisher`; for every service that subscribes, `<Service>EventHandlers` and
//! `<Service>Subscriber`. They live in `<package>.server` next to the HTTP classes and do not
//! depend on the framework: the same bytes come out for `jdk` and `spring`.

use std::collections::BTreeSet;

use veld_codegen::naming;
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_ir::{Backoff, Channel, ChannelId, DeclId, DeclKind, Scalar, Service, Type};

use crate::spec::RequestNames;
use crate::tree::{FileCtx, Tree, java_path};
use crate::util::{add_file, doc_comment, java_str, member_ident, unique};

/// Whether a service has an HTTP surface: it has endpoints, or it has no events either (a service
/// without events keeps the files it always had, an empty one included).
#[must_use]
pub fn serves_http(svc: &Service) -> bool {
    !svc.endpoints.is_empty() || !svc.uses_events()
}

/// The lines the README adds for a service with events.
#[must_use]
pub fn readme_lines(_base: &str, svc: &Service) -> String {
    let name = naming::pascal(&svc.name);
    let mut out = String::new();
    if !svc.publishes.is_empty() {
        out.push_str(&format!(
            "// publish events with {name}Publisher.create(transport)\n"
        ));
    }
    if !svc.subscriptions.is_empty() {
        out.push_str(&format!(
            "// implement {name}EventHandlers and connect it with {name}Subscriber.subscribe(transport, handlers)\n"
        ));
    }
    out
}

/// Writes the publisher, the handler interface and the subscriber of every service of the tree
/// that publishes or subscribes. Nothing is written for a tree without events.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<(), Vec<GenDiagnostic>> {
    for sid in &tree.services {
        let svc = tree.ir.service(*sid);
        if !svc.uses_events() {
            continue;
        }
        let package = tree.api_package(svc.package, "server");
        let base = naming::pascal(&svc.name);
        let mut put = |suffix: &str,
                       write: &dyn Fn(&mut CodeWriter, &mut FileCtx<'_, '_>, &str)|
         -> Result<(), Vec<GenDiagnostic>> {
            let class = format!("{base}{suffix}");
            let mut fc = FileCtx::new(tree, &package, &class);
            let mut w = CodeWriter::new();
            write(&mut w, &mut fc, &class);
            add_file(files, &java_path(&package, &class), fc.finish(&w.finish()))
        };
        if !svc.publishes.is_empty() {
            put("Publisher", &|w, fc, class| {
                write_publisher(w, fc, svc, class);
            })?;
        }
        if !svc.subscriptions.is_empty() {
            put("EventHandlers", &|w, fc, class| {
                write_handlers(w, fc, svc, class);
            })?;
            put("Subscriber", &|w, fc, class| {
                write_subscriber(w, fc, svc, class, &base);
            })?;
            // A service with endpoints gets its test handler from `server.rs`.
            if !serves_http(svc) && crate::testing::wanted(tree.ir, tree.testing, svc) {
                let names = RequestNames::new(tree.ir, &tree.services);
                put("TestHandler", &|w, fc, class| {
                    crate::testing::write_test_handler(w, fc, &names, svc, class, &base);
                })?;
            }
        }
    }
    Ok(())
}

/// The channels a service publishes to, once each, in source order.
pub(crate) fn published(svc: &Service) -> Vec<ChannelId> {
    let mut out: Vec<ChannelId> = Vec::new();
    for p in &svc.publishes {
        if !out.contains(&p.channel) {
            out.push(p.channel);
        }
    }
    out
}

/// The events a service's code needs a description of, sorted by declaration id: everything its
/// channels carry when it publishes, and what its `on` lines name when it subscribes.
fn used_events(svc: &Service, tree: &Tree<'_>, publisher: bool) -> Vec<DeclId> {
    let mut out = BTreeSet::new();
    if publisher {
        for p in &svc.publishes {
            out.extend(tree.ir.channel(p.channel).events.iter().copied());
        }
    } else {
        for sub in &svc.subscriptions {
            out.extend(sub.handlers.iter().map(|h| h.event));
        }
    }
    out.into_iter().collect()
}

/// The names of the constants that describe events: `<NAME>_EVENT`, unique.
fn event_consts(tree: &Tree<'_>, ids: &[DeclId]) -> Vec<String> {
    unique(
        ids.iter()
            .map(|id| {
                naming::escape(
                    naming::Lang::Java,
                    &format!("{}_EVENT", naming::screaming_snake(&tree.ir.decl(*id).name)),
                )
            })
            .collect(),
    )
}

fn key_kind(ty: &Type) -> &'static str {
    match ty {
        Type::Scalar(Scalar::Uuid) => "UUID",
        Type::Scalar(Scalar::Int) => "INT",
        Type::Scalar(Scalar::Long) => "LONG",
        Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Page(_) | Type::Decl(_) => "STRING",
    }
}

/// `private static final Events.EventDef<T> NAME = ...;` for each event.
fn write_event_defs(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    ids: &[DeclId],
    consts: &[String],
    vis: &str,
) {
    let events = fc.rt("Events");
    for (id, name) in ids.iter().zip(consts) {
        let decl = fc.tree.ir.decl(*id);
        let digest = match &decl.kind {
            DeclKind::Event(e) => e
                .rules_digest
                .as_deref()
                .map_or("null".to_string(), java_str),
            DeclKind::Model(_)
            | DeclKind::Error(_)
            | DeclKind::Enum(_)
            | DeclKind::Union(_)
            | DeclKind::Const(_) => "null".to_string(),
        };
        let ty = fc.decl(*id);
        w.line(format!(
            "{vis}static final {events}.EventDef<{ty}> {name} ="
        ));
        w.indent().indent();
        w.line(format!(
            "new {events}.EventDef<>({}, {ty}.VELD_SIG, {ty}.SCHEMA, {digest}, {ty}.codec());",
            java_str(&decl.name)
        ));
        w.dedent().dedent();
    }
    // The flags of the invariants of the tree: read once per message (`spec/platform.md` section 11).
    let names = crate::flags::invariant_flags(fc.tree);
    if !names.is_empty() {
        let list = fc.name("java.util.List");
        let quoted: Vec<String> = names.iter().map(|n| crate::flags::quoted(n)).collect();
        w.open("static");
        for (id, _) in ids.iter().zip(consts) {
            let ty = fc.decl(*id);
            w.line(format!(
                "{events}.flags({ty}.VELD_SIG, {list}.of({}));",
                quoted.join(", ")
            ));
        }
        w.close();
    }
}

fn channel_doc(channel: &Channel) -> String {
    let key = channel
        .key
        .as_ref()
        .map_or("no key".to_string(), |k| format!("key `{}`", k.name));
    let head = format!(
        "Channel `{}`: topic `{}`, {key}.",
        channel.name, channel.topic
    );
    match &channel.docs {
        Some(d) => format!("{}\n\n{head}", d.trim()),
        None => head,
    }
}

/// The names a publisher class uses for the channels it publishes to and the events they carry.
pub(crate) struct Names {
    channels: Vec<ChannelId>,
    /// `<NAME>_CHANNEL`, per channel.
    channel_consts: Vec<String>,
    /// The nested class of each channel.
    classes: Vec<String>,
    /// The accessor and the field of each channel.
    members: Vec<String>,
    /// The events the channels carry, and their `<NAME>_EVENT` constants.
    ids: Vec<DeclId>,
    consts: Vec<String>,
}

impl Names {
    /// The constant that describes `channel`, or `None` when the service does not publish it.
    pub(crate) fn channel_const(&self, channel: ChannelId) -> Option<&str> {
        let at = self.channels.iter().position(|c| *c == channel)?;
        self.channel_consts.get(at).map(String::as_str)
    }

    /// The constant that describes `event`, or `None` when no channel of the service carries it.
    pub(crate) fn event_const(&self, event: DeclId) -> Option<&str> {
        let at = self.ids.iter().position(|e| *e == event)?;
        self.consts.get(at).map(String::as_str)
    }

    /// The names of the publisher of `svc`.
    pub(crate) fn of(tree: &Tree<'_>, svc: &Service) -> Self {
        let ir = tree.ir;
        let channels = published(svc);
        let ids = used_events(svc, tree, true);
        let named = |f: &dyn Fn(&Channel) -> String| -> Vec<String> {
            unique(channels.iter().map(|c| f(ir.channel(*c))).collect())
        };
        Self {
            channel_consts: named(&|c| {
                naming::escape(
                    naming::Lang::Java,
                    &format!("{}_CHANNEL", naming::screaming_snake(&c.name)),
                )
            }),
            classes: named(&|c| format!("{}Channel", naming::pascal(&c.name))),
            members: named(&|c| member_ident(&c.name)),
            consts: event_consts(tree, &ids),
            channels,
            ids,
        }
    }
}

fn write_publisher(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service, class: &str) {
    let ir = fc.tree.ir;
    let (events, transport) = (fc.rt("Events"), fc.rt("Transport"));
    let names = Names::of(fc.tree, svc);
    let source = format!("veld:{}/{}", ir.package(svc.package).name, svc.name);
    doc_comment(
        w,
        Some(&format!(
            "The typed publisher of `{}`: one method per event of each channel it publishes to. \
             Every call runs the publish pipeline of `spec/events.md` section 7 (defaults, shape, \
             invariants, the key, the envelope, the transport) and throws the typed \
             `ValidationError` without sending anything when an event is not valid.",
            svc.name
        )),
        &[],
        None,
    );
    w.line("@SuppressWarnings(\"deprecation\")");
    w.open(format!("public final class {class}"));
    w.line(format!(
        "private static final String SOURCE = {};",
        java_str(&source)
    ));
    // The handler of an emitting move writes with the same descriptions (`<Endpoint>Emit`, same
    // package), so they are not private there.
    let vis = if crate::outbox::service_emits(svc) {
        ""
    } else {
        "private "
    };
    write_channel_defs(w, ir, &events, &names, vis);
    write_event_defs(w, fc, &names.ids, &names.consts, vis);
    w.blank();
    for (nested, member) in names.classes.iter().zip(&names.members) {
        w.line(format!("private final {nested} {member};"));
    }
    w.blank();
    w.open(format!(
        "private {class}({transport} transport, {events}.PublisherOptions options)"
    ));
    write_context(w, fc);
    for (nested, member) in names.classes.iter().zip(&names.members) {
        w.line(format!("this.{member} = new {nested}(ctx);"));
    }
    w.close();
    w.blank();
    write_factories(w, class, &transport, &events);
    for (i, id) in names.channels.iter().enumerate() {
        w.blank();
        let channel = ir.channel(*id);
        let docs = channel_doc(channel);
        doc_comment(w, Some(&docs), &[], channel.deprecated.as_deref());
        w.open(format!(
            "public {} {}()",
            names.classes[i], names.members[i]
        ));
        w.line(format!("return {};", names.members[i]));
        w.close();
    }
    for (i, id) in names.channels.iter().enumerate() {
        w.blank();
        write_channel_class(w, fc, ir.channel(*id), &names, i);
    }
    w.close();
}

/// `private static final Events.ChannelDef <NAME>_CHANNEL = ...;` for each channel.
fn write_channel_defs(
    w: &mut CodeWriter,
    ir: &veld_ir::Ir,
    events: &str,
    names: &Names,
    vis: &str,
) {
    for (id, name) in names.channels.iter().zip(&names.channel_consts) {
        let channel = ir.channel(*id);
        let (wire, kind) = match &channel.key {
            Some(k) => (
                java_str(&k.wire),
                format!("{events}.KeyKind.{}", key_kind(&k.ty)),
            ),
            None => ("null".to_string(), "null".to_string()),
        };
        // The two flags are written only for a channel that has the annotation (`@ordered`, `@deadLetter`).
        let flags = if channel.ordered || channel.dead_letter {
            format!(", {}, {}", channel.ordered, channel.dead_letter)
        } else {
            String::new()
        };
        w.line(format!(
            "{vis}static final {events}.ChannelDef {name} = new {events}.ChannelDef({}, {wire}, {kind}{flags});",
            java_str(&channel.topic)
        ));
    }
}

/// `Events.PublisherContext ctx = ...` with the builder of the typed `ValidationError`.
fn write_context(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>) {
    let (events, router) = (fc.rt("Events"), fc.rt("Router"));
    let validation = match fc.tree.prelude_decl("ValidationError") {
        Some(id) => format!(
            "{}.fromWire({router}.issuesToWire(issues), 422, null)",
            fc.decl(id)
        ),
        None => {
            let untyped = fc.rt("UntypedError");
            format!(
                "new {untyped}(\"about:blank\", \"ValidationError\", {router}.issuesToWire(issues), 422, null)"
            )
        }
    };
    w.line(format!(
        "{events}.PublisherContext ctx = new {events}.PublisherContext(transport, SOURCE, options, issues -> {validation});"
    ));
}

fn write_factories(w: &mut CodeWriter, class: &str, transport: &str, events: &str) {
    w.line("/** Builds the publisher on a transport, with the system clock and random ids. */");
    w.open(format!(
        "public static {class} create({transport} transport)"
    ));
    w.line(format!(
        "return new {class}(transport, new {events}.PublisherOptions());"
    ));
    w.close();
    w.blank();
    w.line("/** Builds the publisher on a transport; the options fix the id source and the clock and hear the rule violations. */");
    w.open(format!(
        "public static {class} create({transport} transport, {events}.PublisherOptions options)"
    ));
    w.line(format!("return new {class}(transport, options);"));
    w.close();
}

fn write_channel_class(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    channel: &Channel,
    names: &Names,
    at: usize,
) {
    let ir = fc.tree.ir;
    let events = fc.rt("Events");
    let nested = &names.classes[at];
    doc_comment(w, Some(&channel_doc(channel)), &[], None);
    w.open(format!("public static final class {nested}"));
    w.line(format!("private final {events}.PublisherContext ctx;"));
    w.blank();
    w.open(format!("private {nested}({events}.PublisherContext ctx)"));
    w.line("this.ctx = ctx;");
    w.close();
    let methods = unique(
        channel
            .events
            .iter()
            .map(|e| member_ident(&ir.decl(*e).name))
            .collect(),
    );
    for (ev, method) in channel.events.iter().zip(&methods) {
        w.blank();
        let decl = ir.decl(*ev);
        doc_comment(w, decl.docs.as_deref(), &[], decl.deprecated.as_deref());
        let ty = fc.decl(*ev);
        let def = names
            .ids
            .iter()
            .position(|i| i == ev)
            .map_or("", |p| names.consts[p].as_str());
        let channel_const = &names.channel_consts[at];
        w.open(format!(
            "public {events}.PublishedEvent {method}({ty} event)"
        ));
        w.line(format!(
            "return {events}.publish(ctx, {channel_const}, {def}, event);"
        ));
        w.close();
    }
    w.close();
}

fn write_handlers(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service, class: &str) {
    let events = fc.rt("Events");
    doc_comment(
        w,
        Some(&format!(
            "What `{}` implements: one method per `on` line. A handler that throws leaves the \
             message unacknowledged and it is delivered again; a handler may be called more than \
             once for the same event (`spec/events.md` section 1), so it must be idempotent.",
            svc.name
        )),
        &[],
        None,
    );
    w.line("@SuppressWarnings(\"deprecation\")");
    w.open(format!("public interface {class}"));
    let names = handler_names(svc);
    let mut i = 0;
    for sub in &svc.subscriptions {
        for h in &sub.handlers {
            if i > 0 {
                w.blank();
            }
            doc_comment(w, h.docs.as_deref(), &[], None);
            let ty = fc.decl(h.event);
            w.line(format!(
                "void {}({ty} event, {events}.EventMeta meta) throws Exception;",
                names[i]
            ));
            i += 1;
        }
    }
    w.close();
}

/// The Java method name of every `on` line of a service, in order, unique.
pub(crate) fn handler_names(svc: &Service) -> Vec<String> {
    unique(
        svc.subscriptions
            .iter()
            .flat_map(|s| s.handlers.iter().map(|h| member_ident(&h.name)))
            .collect(),
    )
}

fn write_subscriber(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    svc: &Service,
    class: &str,
    base: &str,
) {
    let (events, transport, list) = (
        fc.rt("Events"),
        fc.rt("Transport"),
        fc.name("java.util.List"),
    );
    let handlers = format!("{base}EventHandlers");
    let ids = used_events(svc, fc.tree, false);
    let consts = event_consts(fc.tree, &ids);
    doc_comment(
        w,
        Some(&format!(
            "Connects the handlers of `{}` to a transport, one subscription per `subscribes` line \
             under its group. The consume pipeline of `spec/events.md` section 8 runs per message. \
             Close the returned handle to stop.",
            svc.name
        )),
        &[],
        None,
    );
    w.line("@SuppressWarnings(\"deprecation\")");
    w.open(format!("public final class {class}"));
    write_event_defs(w, fc, &ids, &consts, "private ");
    w.blank();
    w.line(format!("private {class}() {{}}"));
    w.blank();
    w.line("/** Subscribes the handlers with no hooks. */");
    w.open(format!(
        "public static {transport}.Subscription subscribe({transport} transport, {handlers} handlers)"
    ));
    w.line(format!(
        "return subscribe(transport, handlers, new {events}.ConsumerOptions());"
    ));
    w.close();
    w.blank();
    w.line("/** Subscribes the handlers; the options carry the hooks of `spec/events.md` section 8.1. */");
    w.open(format!(
        "public static {transport}.Subscription subscribe({transport} transport, {handlers} handlers, {events}.ConsumerOptions options)"
    ));
    write_subscription_list(w, fc, svc, &ids, &consts, &events, &list);
    w.close();
    w.close();
}

/// The three arguments `retry, deadLetter, ordered` of a subscription to a channel that has one of
/// the annotations; `None` for a channel without them (the three-argument constructor is used).
fn delivery_args(events: &str, channel: &Channel) -> Option<String> {
    if !channel.has_delivery_options() {
        return None;
    }
    let retry = channel.retry.map_or_else(
        || "null".to_string(),
        |r| {
            let backoff = match r.backoff {
                Backoff::Fixed => "FIXED",
                Backoff::Exponential => "EXPONENTIAL",
            };
            format!(
                "new {events}.RetryPolicy({}, {events}.Backoff.{backoff})",
                r.max
            )
        },
    );
    Some(format!(
        "{retry}, {}, {}",
        channel.dead_letter, channel.ordered
    ))
}

fn write_subscription_list(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    svc: &Service,
    ids: &[DeclId],
    consts: &[String],
    events: &str,
    list: &str,
) {
    let names = handler_names(svc);
    w.line(format!("return {events}.subscribe("));
    w.indent().indent();
    w.line("transport,");
    w.line(format!("{list}.of("));
    w.indent().indent();
    let mut n = 0;
    for (si, sub) in svc.subscriptions.iter().enumerate() {
        let channel = fc.tree.ir.channel(sub.channel);
        w.line(format!(
            "new {events}.SubscriptionDef({}, {}, {list}.of(",
            java_str(&channel.topic),
            java_str(&sub.group)
        ));
        w.indent().indent();
        let tail = delivery_args(events, channel);
        for (hi, h) in sub.handlers.iter().enumerate() {
            let def = ids
                .iter()
                .position(|i| *i == h.event)
                .map_or("", |at| consts[at].as_str());
            let last_handler = hi + 1 == sub.handlers.len();
            let last_sub = si + 1 == svc.subscriptions.len();
            let sep = match (last_handler, last_sub, tail.is_some()) {
                (false, _, _) => ",",
                (true, _, true) => "",
                (true, false, false) => ")),",
                (true, true, false) => "))",
            };
            w.line(format!("{events}.on({def}, handlers::{}){sep}", names[n]));
            n += 1;
        }
        w.dedent().dedent();
        if let Some(tail) = tail {
            let end = if si + 1 == svc.subscriptions.len() {
                ")"
            } else {
                "),"
            };
            w.line(format!("), {tail}{end}"));
        }
    }
    w.dedent().dedent();
    w.line("),");
    w.line("options);");
    w.dedent().dedent();
}
