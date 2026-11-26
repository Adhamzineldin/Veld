//! Events (`spec/events.md` section 14, `spec/targets/rust.md` section 13): for every service that
//! publishes or subscribes, `<package>/<service>_events.rs` with its publisher, its handler trait
//! and the function that subscribes it. The envelope, the transport and the memory transport are
//! the runtime file `src/veld/events.rs`.
//!
//! The files do not depend on the HTTP adapter: the same bytes come out for every framework.

use std::collections::BTreeSet;

use veld_codegen::{CodeWriter, FileSet, GenDiagnostic, naming};
use veld_ir::{Channel, ChannelId, DeclId, DeclKind, Service};

use crate::models::with_imports;
use crate::tree::Tree;
use crate::types::Ctx;
use crate::util::{add_file, doc, lit, module, snake, type_name};

/// The module of the events of a service.
#[must_use]
pub fn events_mod(svc: &Service) -> String {
    format!("{}_events", module(&svc.name))
}

/// Whether a service has an HTTP surface: it has endpoints, or it has no events either (a service
/// without events keeps the files it always had, an empty one included).
#[must_use]
pub fn serves_http(svc: &Service) -> bool {
    !svc.endpoints.is_empty() || !svc.uses_events()
}

/// Writes `<package>/<service>_events.rs` for every service of the tree that publishes or
/// subscribes. Returns the services that got one.
///
/// # Errors
/// A file path that cannot be added.
pub fn emit(
    files: &mut FileSet,
    tree: &Tree<'_>,
) -> Result<Vec<veld_ir::ServiceId>, Vec<GenDiagnostic>> {
    let mut done = Vec::new();
    for sid in &tree.services {
        let svc = tree.ir.service(*sid);
        if !svc.uses_events() {
            continue;
        }
        let here = format!(
            "{}::{}::{}",
            tree.root,
            tree.pkg_mod(svc.package),
            events_mod(svc)
        );
        let ctx = Ctx { tree, here: &here };
        let mut w = CodeWriter::with_unit("    ");
        service_file(&mut w, &ctx, svc);
        let path = format!(
            "{}/{}/{}.rs",
            tree.dir,
            tree.pkg_mod(svc.package),
            events_mod(svc)
        );
        add_file(files, &path, with_imports(&w.finish()))?;
        done.push(*sid);
    }
    Ok(done)
}

fn service_file(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service) {
    let publishes = !svc.publishes.is_empty();
    let subscribes = !svc.subscriptions.is_empty();
    let what = match (publishes, subscribes) {
        (true, true) => "its publisher, its handlers and the function that subscribes them",
        (true, false) => "its publisher",
        _ => "its handlers and the function that subscribes them",
    };
    w.line(format!(
        "//! The events of the `{}` service: {what}.",
        svc.name
    ))
    .blank();
    for id in used_events(ctx, svc) {
        write_event_def(w, ctx, id);
    }
    if publishes {
        write_publisher(w, ctx, svc);
    }
    if subscribes {
        write_subscriber(w, ctx, svc);
    }
}

/// The events the service's code needs a description of: everything its channels carry when it
/// publishes, and what its `on` lines name when it subscribes. Sorted by declaration id.
fn used_events(ctx: &Ctx<'_, '_>, svc: &Service) -> BTreeSet<DeclId> {
    let mut out = BTreeSet::new();
    for p in &svc.publishes {
        out.extend(ctx.tree.ir.channel(p.channel).events.iter().copied());
    }
    for sub in &svc.subscriptions {
        out.extend(sub.handlers.iter().map(|h| h.event));
    }
    out
}

/// The static that describes an event to the pipelines.
fn def_name(ctx: &Ctx<'_, '_>, id: DeclId) -> String {
    format!(
        "{}_DEF",
        naming::screaming_snake(&ctx.tree.ir.decl(id).name)
    )
}

/// The static that describes a channel to the publisher.
fn channel_static(channel: &Channel) -> String {
    format!("{}_CHANNEL", naming::screaming_snake(&channel.name))
}

fn deprecated(w: &mut CodeWriter, d: Option<&String>) {
    if let Some(msg) = d {
        w.line("///").line(format!("/// **Deprecated:** {msg}"));
    }
}

fn write_event_def(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, id: DeclId) {
    let decl = ctx.tree.ir.decl(id);
    let digest = match &decl.kind {
        DeclKind::Event(e) => e
            .rules_digest
            .as_deref()
            .map_or("None".to_string(), |d| format!("Some({})", lit(d))),
        DeclKind::Model(_)
        | DeclKind::Error(_)
        | DeclKind::Enum(_)
        | DeclKind::Union(_)
        | DeclKind::Const(_) => "None".to_string(),
    };
    w.line(format!("/// The description of the event `{}`.", decl.name));
    w.open_with(
        format!("static {}: veld::events::EventDef =", def_name(ctx, id)),
        " veld::events::EventDef {",
    );
    w.line(format!("name: {},", lit(&decl.name)));
    w.line(format!(
        "sig: {}::VELD_SIG,",
        ctx.tree.type_path(id, ctx.here)
    ));
    w.line(format!("schema: &{},", ctx.tree.schema_path(id, ctx.here)));
    w.line(format!("digest: {digest},"));
    w.close_with("};");
    w.blank();
}

fn write_channel_def(w: &mut CodeWriter, channel: &Channel) {
    let key = channel
        .key
        .as_ref()
        .map_or("None".to_string(), |k| format!("Some({})", lit(&k.wire)));
    w.line(format!(
        "/// The description of the channel `{}`.",
        channel.name
    ));
    w.line(format!(
        "static {}: veld::events::ChannelDef = veld::events::ChannelDef {{ topic: {}, key: {key} }};",
        channel_static(channel),
        lit(&channel.topic)
    ));
    w.blank();
    if channel.dead_letter || channel.ordered {
        w.line(format!(
            "/// The options of the channel `{}` that the transport gets (`spec/events.md` section 10.6).",
            channel.name
        ));
        w.line(format!(
            "static {}: veld::events::ChannelOptions = {};",
            options_static(channel),
            channel_options(channel)
        ));
        w.blank();
    }
}

/// `veld::events::ChannelOptions { ordered: true, dead_letter: false }` of a channel.
fn channel_options(channel: &Channel) -> String {
    format!(
        "veld::events::ChannelOptions {{ ordered: {}, dead_letter: {} }}",
        channel.ordered, channel.dead_letter
    )
}

/// The static that holds the options of a channel with `@ordered` or `@deadLetter`.
fn options_static(channel: &Channel) -> String {
    format!("{}_OPTIONS", naming::screaming_snake(&channel.name))
}

/// The `Delivery` of a subscription: what `@retry`, `@deadLetter` and `@ordered` of its channel say.
fn delivery(channel: &Channel) -> String {
    if !channel.has_delivery_options() {
        return "veld::events::Delivery::NONE".to_string();
    }
    let retry = channel.retry.map_or("None".to_string(), |r| {
        let backoff = match r.backoff {
            veld_ir::Backoff::Fixed => "Fixed",
            veld_ir::Backoff::Exponential => "Exponential",
        };
        format!(
            "Some(veld::events::RetryPolicy {{ max: {}, backoff: veld::events::Backoff::{backoff} }})",
            r.max
        )
    });
    format!(
        "veld::events::Delivery {{ retry: {retry}, dead_letter: {}, ordered: {} }}",
        channel.dead_letter, channel.ordered
    )
}

/// The channels a service publishes to, once each, in source order.
fn published(svc: &Service) -> Vec<ChannelId> {
    let mut out: Vec<ChannelId> = Vec::new();
    for p in &svc.publishes {
        if !out.contains(&p.channel) {
            out.push(p.channel);
        }
    }
    out
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

/// The type that holds the methods of one channel: `BookingEventsChannel`.
fn channel_type(channel: &Channel) -> String {
    type_name(&format!("{}Channel", naming::pascal(&channel.name)))
}

fn write_publisher(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service) {
    let ir = ctx.tree.ir;
    let name = type_name(&svc.name);
    let channels = published(svc);
    w.line(format!(
        "/// The `source` of every message of `{}`.",
        svc.name
    ));
    w.line(format!(
        "const SOURCE: &str = {};",
        lit(&format!(
            "veld:{}/{}",
            ir.package(svc.package).name,
            svc.name
        ))
    ));
    let validation = ctx
        .tree
        .prelude_decl("ValidationError")
        .map_or("about:blank".to_string(), |id| ir.decl(id).sig.clone());
    w.line("/// The signature of the `ValidationError` a failed publish returns.");
    w.line(format!("const VALIDATION: &str = {};", lit(&validation)));
    w.blank();
    for id in &channels {
        write_channel_def(w, ir.channel(*id));
    }
    w.line(format!(
        "/// The options of the publisher of `{}`: the id source, the clock and the rule hook",
        svc.name
    ));
    w.line("/// (`spec/events.md` section 7).");
    w.line(format!(
        "pub type {name}PublisherOptions = veld::events::PublisherOptions;"
    ));
    w.blank();
    write_publisher_type(w, ctx, svc, &channels);
    for id in &channels {
        write_channel_type(w, ctx, ir.channel(*id));
    }
}

fn write_publisher_type(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    svc: &Service,
    channels: &[ChannelId],
) {
    let ir = ctx.tree.ir;
    let name = type_name(&svc.name);
    let fn_name = snake(&svc.name);
    w.line(format!(
        "/// The typed publisher of `{}`: one method per event of each channel it publishes to.",
        svc.name
    ));
    w.line("///");
    w.line("/// The publish pipeline runs in order: defaults, shape, invariants, the key, the envelope,");
    w.line("/// the transport. Nothing is sent when a step fails: the call returns the `ValidationError`.");
    w.open(format!("pub struct {name}Publisher"));
    w.line("ctx: veld::events::PublisherContext,");
    w.close().blank();
    w.line(format!(
        "/// Builds the publisher of `{}` on a transport.",
        svc.name
    ));
    w.open(format!(
        "pub fn create_{fn_name}_publisher(transport: std::sync::Arc<dyn veld::events::Transport>, options: {name}PublisherOptions) -> {name}Publisher"
    ));
    w.open_with(format!("{name}Publisher"), " {");
    w.line("ctx: veld::events::PublisherContext::new(transport, SOURCE, VALIDATION, options),");
    w.close();
    w.close().blank();
    w.open(format!("impl {name}Publisher"));
    for (i, id) in channels.iter().enumerate() {
        let channel = ir.channel(*id);
        if i > 0 {
            w.blank();
        }
        for line in channel_doc(channel).lines() {
            w.line(if line.is_empty() {
                "///".to_string()
            } else {
                format!("/// {line}")
            });
        }
        deprecated(w, channel.deprecated.as_ref());
        w.open(format!(
            "pub fn {}(&self) -> {}<'_>",
            snake(&channel.name),
            channel_type(channel)
        ));
        w.line(format!("{} {{ ctx: &self.ctx }}", channel_type(channel)));
        w.close();
    }
    w.close().blank();
}

fn write_channel_type(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, channel: &Channel) {
    let ir = ctx.tree.ir;
    let ty = channel_type(channel);
    w.line(format!(
        "/// The events of the channel `{}` (topic `{}`).",
        channel.name, channel.topic
    ));
    w.open(format!("pub struct {ty}<'a>"));
    w.line("ctx: &'a veld::events::PublisherContext,");
    w.close().blank();
    w.open(format!("impl {ty}<'_>"));
    for (i, ev) in channel.events.iter().enumerate() {
        let decl = ir.decl(*ev);
        if i > 0 {
            w.blank();
        }
        doc(w, decl.docs.as_deref());
        deprecated(w, decl.deprecated.as_ref());
        w.open(format!(
            "pub async fn {}(&self, event: &{}) -> Result<veld::events::PublishedEvent, veld::Error>",
            snake(&decl.name),
            ctx.tree.type_path(*ev, ctx.here)
        ));
        if channel.dead_letter || channel.ordered {
            w.line(format!(
                "veld::events::publish_event_with(self.ctx, &{}, {}, &{}, event).await",
                channel_static(channel),
                options_static(channel),
                def_name(ctx, *ev)
            ));
        } else {
            w.line(format!(
                "veld::events::publish_event(self.ctx, &{}, &{}, event).await",
                channel_static(channel),
                def_name(ctx, *ev)
            ));
        }
        w.close();
    }
    w.close().blank();
}

fn write_subscriber(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service) {
    let name = type_name(&svc.name);
    write_handlers_trait(w, ctx, svc);
    w.line(format!(
        "/// The hooks of the subscriber of `{}`: a rejected message, a contract mismatch, a rule",
        svc.name
    ));
    w.line("/// violation, a handler that failed (`spec/events.md` section 8).");
    w.line(format!(
        "pub type {name}SubscriberOptions = veld::events::ConsumerOptions;"
    ));
    w.blank();
    w.line(format!(
        "/// Connects the handlers of `{}` to a transport, one subscription per `subscribes` line",
        svc.name
    ));
    w.line("/// under its group. Call `close` on the handle to stop.");
    w.open(format!(
        "pub async fn subscribe_{}_events<H: {name}EventHandlers>(transport: std::sync::Arc<dyn veld::events::Transport>, handlers: std::sync::Arc<H>, options: {name}SubscriberOptions) -> Result<veld::events::SubscriptionHandle, veld::Error>",
        snake(&svc.name)
    ));
    let annotated = svc
        .subscriptions
        .iter()
        .any(|s| ctx.tree.ir.channel(s.channel).has_delivery_options());
    let entry = if annotated { "subscribe_events_with" } else { "subscribe_events" };
    w.open_with(format!("veld::events::{entry}(transport,"), " vec![");
    for sub in &svc.subscriptions {
        if annotated {
            w.open_with("(veld::events::SubscriptionDef", " {");
        } else {
            w.open_with("veld::events::SubscriptionDef", " {");
        }
        w.line(format!(
            "topic: {},",
            lit(&ctx.tree.ir.channel(sub.channel).topic)
        ));
        w.line(format!("group: {},", lit(&sub.group)));
        w.open_with("on:", " vec![");
        for h in &sub.handlers {
            write_binding(w, ctx, h);
        }
        w.close_with("],");
        if annotated {
            w.close_with(&format!(
                "}}, {}),",
                delivery(ctx.tree.ir.channel(sub.channel))
            ));
        } else {
            w.close_with("},");
        }
    }
    w.close_with("], options).await");
    w.close();
}

fn write_binding(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, h: &veld_ir::Handler) {
    w.open_with(
        format!(
            "veld::events::EventBinding::typed(&{},",
            def_name(ctx, h.event)
        ),
        " {",
    );
    w.line("let handlers = std::sync::Arc::clone(&handlers);");
    w.open_with(
        format!(
            "move |event: {}, meta|",
            ctx.tree.type_path(h.event, ctx.here)
        ),
        " {",
    );
    w.line("let handlers = std::sync::Arc::clone(&handlers);");
    w.line(format!(
        "async move {{ handlers.{}(event, meta).await }}",
        snake(&h.name)
    ));
    w.close();
    w.close_with("}),");
}

fn write_handlers_trait(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service) {
    w.line(format!(
        "/// What `{}` implements: one method per `on` line. A handler that returns an error leaves the",
        svc.name
    ));
    w.line(
        "/// message unacknowledged, and a handler may be called more than once for the same event",
    );
    w.line("/// (`spec/events.md` section 1).");
    w.open(format!(
        "pub trait {}EventHandlers: Send + Sync + 'static",
        type_name(&svc.name)
    ));
    for (i, h) in svc
        .subscriptions
        .iter()
        .flat_map(|s| s.handlers.iter())
        .enumerate()
    {
        if i > 0 {
            w.blank();
        }
        doc(w, h.docs.as_deref());
        w.line(format!(
            "fn {}(&self, event: {}, meta: veld::events::EventMeta) -> impl std::future::Future<Output = Result<(), veld::Error>> + Send;",
            snake(&h.name),
            ctx.tree.type_path(h.event, ctx.here)
        ));
    }
    w.close().blank();
}
