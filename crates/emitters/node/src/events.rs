//! Events (`spec/events.md` section 14, `spec/targets/typescript.md` section 5f):
//! `runtime/events.ts` and, for every service that publishes or subscribes,
//! `server/<service>-events.ts` with its publisher and its handler interface.
//!
//! The files do not depend on the HTTP adapter: the same bytes come out for every framework.

use std::collections::BTreeSet;

use veld_codegen::naming;
use veld_codegen::{CodeWriter, FileSet, GateError, GenDiagnostic, gate_regions};
use veld_emit_typescript::tree::{FileCtx, Tree};
use veld_emit_typescript::util::{add_file, doc_comment, method_key, ts_str};
use veld_ir::{Backoff, Channel, ChannelId, DeclId, DeclKind, Scalar, Service, Type};

/// The embedded `runtime/events.ts`, markers and all.
const RAW: &str = include_str!("../runtime/events.ts");

/// Whether a service has an HTTP surface: it has endpoints, or it has no events either (a service
/// without events keeps the files it always had, an empty one included).
pub(crate) fn serves_http(svc: &Service) -> bool {
    !svc.endpoints.is_empty() || !svc.uses_events()
}

/// `runtime/events.ts` with the `rules` and `rollout` regions the tree needs.
///
/// # Errors
/// The embedded file has markers that do not pair up.
pub fn runtime_source(tree: &Tree<'_>) -> Result<String, GateError> {
    let gates = tree.gates().normalised();
    gate_regions(RAW, &|name| match name {
        "rules" => gates.rules,
        "rollout" => gates.rollout,
        // The outbox (`spec/events.md` section 11) and what a build without it has.
        "outbox" => gates.outbox,
        "not_outbox" => !gates.outbox,
        // Derived fields (`spec/rules.md` section 17.2) and what a build without them has.
        "derived" => gates.derived,
        "not_derived" => !gates.derived,
        "rules_not_derived" => gates.rules && !gates.derived,
        // Flags (`spec/platform.md` section 11) and what a build without them has.
        "flags" => gates.flags,
        "not_flags" => !gates.flags,
        "flags_rules" => gates.flags && gates.rules,
        "not_flags_rules" => gates.rules && !gates.flags,
        // `veldtenant` (`spec/platform.md` section 3) and what a build without it has.
        "tenant_events" => gates.tenant_events,
        "not_tenant_events" => !gates.tenant_events,
        _ => false,
    })
}

/// Module path (root-relative, no extension) of the events of a service.
pub(crate) fn module_of(tree: &Tree<'_>, svc: &Service) -> String {
    format!("{}server/{}-events", tree.root, naming::kebab(&svc.name))
}

/// Writes `runtime/events.ts` and the events module of every service that publishes or
/// subscribes. Returns the module paths for the barrel. Nothing is written for a tree without
/// events.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<Vec<String>, Vec<GenDiagnostic>> {
    if !tree.uses_events() {
        return Ok(Vec::new());
    }
    let text = runtime_source(tree).map_err(|e| {
        vec![GenDiagnostic::new(
            "E0805",
            format!("runtime/events.ts: {e}"),
        )]
    })?;
    add_file(files, &format!("{}.ts", tree.events_module()), text)?;
    let mut modules = Vec::new();
    for sid in &tree.services {
        let svc = tree.ir.service(*sid);
        if !svc.uses_events() {
            continue;
        }
        let module = module_of(tree, svc);
        let mut fc = FileCtx::new(tree, &module);
        let mut w = CodeWriter::new();
        write_service(&mut w, &mut fc, svc);
        add_file(files, &format!("{module}.ts"), fc.finish(&w.finish()))?;
        modules.push(module);
    }
    Ok(modules)
}

fn write_service(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service) {
    let events = used_events(svc, fc.tree);
    for id in &events {
        write_event_def(w, fc, *id);
    }
    if !events.is_empty() {
        w.blank();
    }
    if !svc.publishes.is_empty() {
        write_publisher(w, fc, svc);
    }
    if !svc.subscriptions.is_empty() {
        write_subscriber(w, fc, svc);
    }
    if crate::outbox::service_emits(svc) {
        crate::outbox::write(w, fc, svc);
    }
}

/// The events the service's code needs a description of: everything its channels carry when it
/// publishes, and what its `on` lines name when it subscribes. Sorted by declaration id.
fn used_events(svc: &Service, tree: &Tree<'_>) -> BTreeSet<DeclId> {
    let mut out = BTreeSet::new();
    for p in &svc.publishes {
        out.extend(tree.ir.channel(p.channel).events.iter().copied());
    }
    for sub in &svc.subscriptions {
        out.extend(sub.handlers.iter().map(|h| h.event));
    }
    out
}

/// The name of the constant that describes an event to the pipelines.
pub(crate) fn def_name(tree: &Tree<'_>, id: DeclId) -> String {
    format!("{}Def", naming::camel(&tree.ir.decl(id).name))
}

/// The name of the constant that describes a channel to the publisher.
pub(crate) fn channel_name(channel: &Channel) -> String {
    format!("{}Channel", naming::camel(&channel.name))
}

fn write_event_def(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, id: DeclId) {
    let ir = fc.tree.ir;
    let decl = ir.decl(id);
    let digest = match &decl.kind {
        DeclKind::Event(e) => e.rules_digest.as_deref().map_or("null".to_string(), ts_str),
        DeclKind::Model(_)
        | DeclKind::Error(_)
        | DeclKind::Enum(_)
        | DeclKind::Union(_)
        | DeclKind::Const(_) => "null".to_string(),
    };
    let ns = fc.events_ns();
    let name = def_name(fc.tree, id);
    let companion = fc.value_ref(id);
    w.open_with(format!("const {name}: {ns}.EventDef ="), " {");
    w.line(format!("name: {},", ts_str(&decl.name)));
    w.line(format!("sig: {companion}.sig,"));
    w.line(format!("schema: {companion}.schema,"));
    w.line(format!("digest: {digest},"));
    let flags = crate::rules::decl_flags(fc.tree);
    if fc.tree.uses_flags() && !flags.is_empty() {
        let list: Vec<String> = flags.iter().map(|f| ts_str(f)).collect();
        w.line(format!("flags: [{}],", list.join(", ")));
    }
    if fc.tree.uses_tenant_events()
        && let Some(f) = decl.tenant_field()
    {
        w.line(format!(
            "tenant: {{ wire: {}, kind: {} }},",
            ts_str(&f.wire),
            ts_str(veld_emit_typescript::platform::tenant_kind(&f.ty))
        ));
    }
    w.close_with("};");
}

/// How the key of a channel is written in the channel's description.
fn key_kind(ty: &Type) -> &'static str {
    match ty {
        Type::Scalar(Scalar::Uuid) => "uuid",
        Type::Scalar(Scalar::Int) => "int",
        Type::Scalar(Scalar::Long) => "long",
        Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Page(_) | Type::Decl(_) => "string",
    }
}

fn write_channel_def(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, channel: &Channel) {
    let ns = fc.events_ns();
    let key = match &channel.key {
        Some(k) => format!(
            "{{ wire: {}, kind: {} }}",
            ts_str(&k.wire),
            ts_str(key_kind(&k.ty))
        ),
        None => "null".to_string(),
    };
    // Only an annotation that is there is written: a channel without one keeps its bytes.
    let mut options = String::new();
    if channel.ordered {
        options.push_str(", ordered: true");
    }
    if channel.dead_letter {
        options.push_str(", deadLetter: true");
    }
    w.line(format!(
        "const {}: {ns}.ChannelDef = {{ topic: {}, key: {key}{options} }};",
        channel_name(channel),
        ts_str(&channel.topic)
    ));
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

/// The builder of the typed `ValidationError` a publisher throws: the prelude class when the
/// contract has one, and the base error with the same title otherwise (an events-only contract
/// has no prelude).
fn write_validation(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>) {
    let ns = fc.events_ns();
    let rules = fc.tree.uses_rules();
    let violations = if rules { "violations" } else { "_violations" };
    w.open(format!(
        "function validation(issues: {ns}.Issue[], {violations}: {ns}.EventViolation[]): Error"
    ));
    match fc.tree.prelude_decl("ValidationError") {
        Some(id) => {
            let class = fc.value_ref(id);
            let init = if rules {
                ", { violations, local: true }"
            } else {
                ""
            };
            w.line(format!("return new {class}({{ issues }}{init});"));
        }
        None => {
            let base = fc.rt("VeldError");
            let extra = if rules {
                ", violations, local: true"
            } else {
                ""
            };
            w.line(format!(
                "return new {base}('about:blank', 'ValidationError', {{ issues }}, {{ status: 422{extra} }}, 422);"
            ));
        }
    }
    w.close();
    w.blank();
}

fn write_publisher(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service) {
    let ir = fc.tree.ir;
    let name = naming::pascal(&svc.name);
    let ns = fc.events_ns();
    let source = format!("veld:{}/{}", ir.package(svc.package).name, svc.name);
    let channels = published(svc);
    w.line(format!("const SOURCE = {};", ts_str(&source)));
    for id in &channels {
        write_channel_def(w, fc, ir.channel(*id));
    }
    w.blank();
    write_validation(w, fc);
    doc_comment(
        w,
        Some(&format!(
            "The options of the publisher of `{}`: the id source, the clock and the rule hook (`spec/events.md` section 7).",
            svc.name
        )),
        None,
    );
    w.line(format!(
        "export type {name}PublisherOptions = {ns}.PublisherOptions;"
    ));
    w.blank();
    write_publisher_interface(w, fc, svc, &channels);
    w.blank();
    write_publisher_factory(w, fc, svc, &channels);
    w.blank();
}

fn channel_member(channel: &Channel) -> String {
    method_key(&naming::camel(&channel.name))
}

fn write_publisher_interface(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    svc: &Service,
    channels: &[ChannelId],
) {
    let ir = fc.tree.ir;
    let ns = fc.events_ns();
    doc_comment(
        w,
        Some(&format!(
            "The typed publisher of `{}`: one method per event of each channel it publishes to.",
            svc.name
        )),
        None,
    );
    w.open(format!(
        "export interface {}Publisher",
        naming::pascal(&svc.name)
    ));
    for id in channels {
        let channel = ir.channel(*id);
        doc_comment(
            w,
            Some(&channel_doc(channel)),
            channel.deprecated.as_deref(),
        );
        w.open_with(format!("readonly {}:", channel_member(channel)), " {");
        for ev in &channel.events {
            let decl = ir.decl(*ev);
            doc_comment(w, decl.docs.as_deref(), decl.deprecated.as_deref());
            // What the publisher takes: a derived field is optional, the runtime computes it.
            let ty = veld_emit_typescript::derived::shaped_type(
                fc,
                &veld_ir::Type::Decl(*ev),
                veld_emit_typescript::derived::Shape::Output,
            );
            w.line(format!(
                "{}(event: {ty}): Promise<{ns}.PublishedEvent>;",
                method_key(&naming::camel(&decl.name))
            ));
        }
        w.close_with("};");
    }
    w.close();
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

fn write_publisher_factory(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    svc: &Service,
    channels: &[ChannelId],
) {
    let ir = fc.tree.ir;
    let ns = fc.events_ns();
    let name = naming::pascal(&svc.name);
    doc_comment(
        w,
        Some(&format!(
            "Builds the publisher of `{}` on a transport. The publish pipeline runs in order: defaults, shape, invariants, the key, the envelope, the transport.",
            svc.name
        )),
        None,
    );
    w.open(format!(
        "export function create{name}Publisher(transport: {ns}.Transport, options: {name}PublisherOptions = {{}}): {name}Publisher"
    ));
    w.line(format!(
        "const ctx: {ns}.PublisherContext = {{ transport, source: SOURCE, options, validation }};"
    ));
    w.open_with("return", " {");
    for id in channels {
        let channel = ir.channel(*id);
        w.open_with(format!("{}:", channel_member(channel)), " {");
        for ev in &channel.events {
            let decl = ir.decl(*ev);
            w.line(format!(
                "{}: (event) => {ns}.publishEvent(ctx, {}, {}, event),",
                method_key(&naming::camel(&decl.name)),
                channel_name(channel),
                def_name(fc.tree, *ev)
            ));
        }
        w.close_with("},");
    }
    w.close_with("};");
    w.close();
}

fn write_subscriber(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service) {
    let name = naming::pascal(&svc.name);
    let ns = fc.events_ns();
    write_handlers_interface(w, fc, svc);
    w.blank();
    doc_comment(
        w,
        Some(&format!(
            "The hooks of the subscriber of `{}`: a rejected message, a contract mismatch, a rule violation, a handler that threw (`spec/events.md` section 8).",
            svc.name
        )),
        None,
    );
    w.line(format!(
        "export type {name}SubscriberOptions = {ns}.ConsumerOptions;"
    ));
    w.blank();
    doc_comment(
        w,
        Some(&format!(
            "Connects the handlers of `{}` to a transport, one subscription per `subscribes` line under its group. Close the handle to stop.",
            svc.name
        )),
        None,
    );
    w.open(format!(
        "export function subscribe{name}Events(transport: {ns}.Transport, handlers: {name}EventHandlers, options: {name}SubscriberOptions = {{}}): Promise<{ns}.SubscriptionHandle>"
    ));
    w.open_with(format!("return {ns}.subscribeEvents(transport,"), " [");
    for sub in &svc.subscriptions {
        w.open_with("", "{");
        w.line(format!(
            "topic: {},",
            ts_str(&fc.tree.ir.channel(sub.channel).topic)
        ));
        w.line(format!("group: {},", ts_str(&sub.group)));
        write_delivery_options(w, fc.tree.ir.channel(sub.channel));
        w.open_with("on:", " [");
        for h in &sub.handlers {
            let ty = fc.type_ref(h.event);
            w.line(format!(
                "{{ def: {}, run: (event, meta) => handlers.{}(event as {ty}, meta) }},",
                def_name(fc.tree, h.event),
                method_key(&h.name)
            ));
        }
        w.close_with("],");
        w.close_with("},");
    }
    w.close_with("], options);");
    w.close();
}

/// `retry`, `deadLetter` and `ordered` of a subscription, each only when the channel has the
/// annotation (`spec/events.md` section 10.2).
fn write_delivery_options(w: &mut CodeWriter, channel: &Channel) {
    if let Some(retry) = channel.retry {
        let backoff = match retry.backoff {
            Backoff::Fixed => "fixed",
            Backoff::Exponential => "exponential",
        };
        w.line(format!(
            "retry: {{ max: {}, backoff: '{backoff}' }},",
            retry.max
        ));
    }
    if channel.dead_letter {
        w.line("deadLetter: true,");
    }
    if channel.ordered {
        w.line("ordered: true,");
    }
}

fn write_handlers_interface(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service) {
    let ns = fc.events_ns();
    doc_comment(
        w,
        Some(&format!(
            "What `{}` implements: one method per `on` line. A handler that throws leaves the message unacknowledged; a handler may be called more than once for the same event (`spec/events.md` section 1).",
            svc.name
        )),
        None,
    );
    w.open(format!(
        "export interface {}EventHandlers",
        naming::pascal(&svc.name)
    ));
    for sub in &svc.subscriptions {
        for h in &sub.handlers {
            doc_comment(w, h.docs.as_deref(), None);
            let ty = fc.type_ref(h.event);
            w.line(format!(
                "{}(event: {ty}, meta: {ns}.EventMeta): Promise<void> | void;",
                method_key(&h.name)
            ));
        }
    }
    w.close();
}
