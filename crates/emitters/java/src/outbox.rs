//! The outbox of events phase E3 (`spec/events.md` section 11, `spec/targets/java.md` section
//! 12.6): the `emit` the handler of a move that emits receives (`<Endpoint>Emit`), the check after
//! the handler, and the relay (`<Service>OutboxRelay`).
//!
//! `emit` shares the code of the publisher: `Events.emit` and `Events.publish` both call
//! `Events.prepare` (steps 1 to 5 of `spec/events.md` section 7), and the descriptions of the
//! channels and events (`<NAME>_CHANNEL`, `<NAME>_EVENT`) are the constants of `<Service>Publisher`,
//! which are not private for a service with an emitting move.

use veld_codegen::{CodeWriter, FileSet, GenDiagnostic, naming};
use veld_ir::{DeclKind, Endpoint, EndpointTransition, Ir, Service};

use crate::endpoint::{Site, unsupported};
use crate::events::{self, Names};
use crate::spec::RequestNames;
use crate::tree::{FileCtx, Tree, java_path};
use crate::util::{add_file, doc_comment, java_str, member_ident, unique};

/// What the Javadoc of an endpoint whose move emits says about the `emit` argument.
pub const EMIT_NOTE: &str = "The `emit` argument writes the events of the move to the outbox (`spec/events.md` section 11.1): call its method for every event the edge taken emits, exactly once and in the transaction that changes the state, so that the state and its event stand or fall together. It runs steps 1 to 5 of publishing and then `outbox.save`; nothing is sent to a transport (the relay does that). After a success the server checks that exactly the events of the edge were written and answers `500 Internal` otherwise. A method that answers a declared error need not have written.";

/// The endpoints of `svc` whose move emits an event.
pub(crate) fn emitting(svc: &Service) -> impl Iterator<Item = &Endpoint> {
    svc.endpoints.iter().filter(|ep| emits(ep))
}

/// Whether the move of `ep` emits an event.
#[must_use]
pub(crate) fn emits(ep: &Endpoint) -> bool {
    ep.rules
        .transition
        .as_ref()
        .is_some_and(|t| !t.emits.is_empty())
}

/// Whether the hooks of `svc` need an outbox.
#[must_use]
pub(crate) fn service_emits(svc: &Service) -> bool {
    emitting(svc).next().is_some()
}

/// `PayEmit` for the request `PayRequest`: the type of `emit`, the argument the handler gets after
/// the transition.
#[must_use]
pub(crate) fn emit_name(request: &str) -> String {
    format!("{}Emit", request.strip_suffix("Request").unwrap_or(request))
}

/// `<Service>OutboxRelay`.
#[must_use]
pub(crate) fn relay_name(base: &str) -> String {
    format!("{base}OutboxRelay")
}

/// The publisher source of a service: `veld:<package>/<Service>`, the `source` of every message.
fn source_of(ir: &Ir, svc: &Service) -> String {
    format!("veld:{}/{}", ir.package(svc.package).name, svc.name)
}

/// `Map.of("pending", List.of("BookingPaid"), ...)`: by wire value of each source state, the
/// declaration names of the events the edge from it to the target emits. A source whose edge emits
/// nothing is left out. This is `veld_rules::outbox::expected_events` for every source at once.
fn edges_code(fc: &mut FileCtx<'_, '_>, t: &EndpointTransition) -> Option<String> {
    let ir = fc.tree.ir;
    let DeclKind::Enum(machine) = &ir.decl(t.decl).kind else {
        return None;
    };
    let (map, list) = (fc.name("java.util.Map"), fc.name("java.util.List"));
    let mut entries = Vec::new();
    for name in &t.from {
        let wire = machine
            .variants
            .iter()
            .find(|v| &v.name == name)
            .map(|v| v.wire.as_str())?;
        let mut events: Vec<&str> = Vec::new();
        for flow in machine
            .flows
            .iter()
            .filter(|f| &f.from == name && f.to == t.to)
        {
            if let Some(e) = flow.emits {
                let event = ir.decl(e).name.as_str();
                if !events.contains(&event) {
                    events.push(event);
                }
            }
        }
        if events.is_empty() {
            continue;
        }
        let list_of: Vec<String> = events.iter().map(|e| java_str(e)).collect();
        entries.push((java_str(wire), format!("{list}.of({})", list_of.join(", "))));
    }
    // `Map.of` takes ten pairs at most.
    if entries.len() <= 10 {
        let pairs: Vec<String> = entries.iter().map(|(k, v)| format!("{k}, {v}")).collect();
        Some(format!("{map}.of({})", pairs.join(", ")))
    } else {
        let pairs: Vec<String> = entries
            .iter()
            .map(|(k, v)| format!("{map}.entry({k}, {v})"))
            .collect();
        Some(format!("{map}.ofEntries({})", pairs.join(", ")))
    }
}

/// The refusals of a service that emits: a parameter or a fact called `emit` would hide the
/// argument of the handler.
#[must_use]
pub(crate) fn refusals(svc: &Service) -> Vec<GenDiagnostic> {
    let mut out = Vec::new();
    for ep in emitting(svc) {
        let clash = ep
            .params
            .iter()
            .map(|p| p.name.as_str())
            .chain(ep.rules.facts.iter().map(|f| f.name.as_str()))
            .find(|n| member_ident(n) == "emit");
        if let Some(name) = clash {
            let mut d = GenDiagnostic::new(
                "E0801",
                format!(
                    "{}.{}: generator `java` cannot call a parameter or a fact `{name}` on an endpoint whose move emits an event: the handler's argument `emit` would be hidden",
                    svc.name, ep.name
                ),
            );
            d.origin = ep.rules.first_origin().cloned();
            out.push(d);
        }
    }
    out
}

/// Writes the lines of the call of an emitting endpoint that make its `emit`, after the guard has
/// set `transition_from`: `Emission emission = run.emission(...); PayEmit emit = new PayEmit(emission);`.
pub(crate) fn write_begin(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    site: &Site<'_>,
    diags: &mut Vec<GenDiagnostic>,
) {
    let Some(t) = site
        .ep
        .rules
        .transition
        .as_ref()
        .filter(|t| !t.emits.is_empty())
    else {
        return;
    };
    let Some(edges) = edges_code(fc, t) else {
        diags.push(unsupported(
            site,
            "an emitting move over something that is not an enum",
        ));
        return;
    };
    let emission = fc.rt("Emission");
    let publisher = source_of(fc.tree.ir, site.svc);
    w.line(format!(
        "{emission} emission = run.emission(emissions, {}, {edges}, transition_from.wire());",
        java_str(&publisher)
    ));
    w.line(format!(
        "{0} emit = new {0}(emission);",
        emit_name(site.request)
    ));
}

/// The check after the handler, for a success: `emission.check("pay");`.
pub(crate) fn write_check(w: &mut CodeWriter, site: &Site<'_>) {
    if emits(site.ep) {
        w.line(format!("emission.check({});", java_str(&site.ep.name)));
    }
}

/// `<Endpoint>Emit` of every emitting endpoint of `svc` and `<Service>OutboxRelay`.
pub(crate) fn emit_files(
    files: &mut FileSet,
    tree: &Tree<'_>,
    package: &str,
    names: &RequestNames,
    svc: &Service,
) -> Result<(), Vec<GenDiagnostic>> {
    if !service_emits(svc) {
        return Ok(());
    }
    let used = Names::of(tree, svc);
    for ep in emitting(svc) {
        let missing = ep.rules.transition.iter().flat_map(|t| &t.emits).any(|e| {
            used.channel_const(e.channel).is_none() || used.event_const(e.event).is_none()
        });
        if missing {
            return Err(vec![GenDiagnostic::new(
                "E0801",
                format!(
                    "{}.{}: generator `java` cannot find the channel or the event of a move that emits in the channels the service publishes",
                    svc.name, ep.name
                ),
            )]);
        }
        let class = emit_name(&names.get(svc.id, &ep.name));
        let mut fc = FileCtx::new(tree, package, &class);
        let mut w = CodeWriter::new();
        write_emit_class(&mut w, &mut fc, svc, ep, &class);
        add_file(files, &java_path(package, &class), fc.finish(&w.finish()))?;
    }
    let class = relay_name(&naming::pascal(&svc.name));
    let mut fc = FileCtx::new(tree, package, &class);
    let mut w = CodeWriter::new();
    write_relay(&mut w, &mut fc, svc, &class);
    add_file(files, &java_path(package, &class), fc.finish(&w.finish()))
}

/// What the handler of `ep` gets as `emit`: one method per event of the move.
fn write_emit_class(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    svc: &Service,
    ep: &Endpoint,
    class: &str,
) {
    let ir = fc.tree.ir;
    let (emission, runtime_events) = (fc.rt("Emission"), fc.rt("Events"));
    let Some(transition) = &ep.rules.transition else {
        return;
    };
    let publisher = format!("{}Publisher", naming::pascal(&svc.name));
    let used = Names::of(fc.tree, svc);
    doc_comment(
        w,
        Some(&format!(
            "What the handler of `{}` gets as `emit`, after the facts and the transition (`spec/events.md` section 11.1): one method per event of the move. Call it in the transaction that changes the state; it runs steps 1 to 5 of publishing and writes the entry to the outbox. Nothing is sent to a transport. The server answers `500 Internal` for a success that did not write exactly the events of the edge it took.",
            ep.name
        )),
        &[],
        None,
    );
    w.line("@SuppressWarnings(\"deprecation\")");
    w.open(format!("public final class {class}"));
    w.line(format!("private final {emission} emission;"));
    w.blank();
    w.line("/** Built by the generated handler and by the test handler of the same package. */");
    w.open(format!("{class}({emission} emission)"));
    w.line("this.emission = emission;");
    w.close();
    let methods = unique(
        transition
            .emits
            .iter()
            .map(|e| member_ident(&ir.decl(e.event).name))
            .collect(),
    );
    for (e, method) in transition.emits.iter().zip(&methods) {
        w.blank();
        let decl = ir.decl(e.event);
        doc_comment(w, decl.docs.as_deref(), &[], decl.deprecated.as_deref());
        let ty = fc.decl(e.event);
        // `emit_files` has checked that both exist.
        let channel = used.channel_const(e.channel).unwrap_or_default();
        let def = used.event_const(e.event).unwrap_or_default();
        w.open(format!(
            "public {runtime_events}.PublishedEvent {method}({ty} event) throws Exception"
        ));
        w.line(format!(
            "return emission.emit({publisher}.{channel}, {publisher}.{def}, event);"
        ));
        w.close();
    }
    w.blank();
    w.line("/** The emission of this request, for the test handler of the same package. */");
    w.open(format!("{emission} emission()"));
    w.line("return emission;");
    w.close();
    w.close();
}

/// `<Service>OutboxRelay.run(outbox, transport[, options])` (`spec/events.md` section 11.1).
fn write_relay(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service, class: &str) {
    let ir = fc.tree.ir;
    let (events, outbox, transport) = (fc.rt("Events"), fc.rt("Outbox"), fc.rt("Transport"));
    let map = fc.name("java.util.Map");
    doc_comment(
        w,
        Some(&format!(
            "The relay of `{}` (`spec/events.md` section 11.1). `run` takes up to `limit` (default 100) pending entries of the outbox, publishes each to the transport with the options of its channel, in the order the outbox gave them, and calls `done` after the transport accepted it. It stops at the first entry that fails (the failure goes to `onError` of the options) and answers how many entries it sent. Run it in a loop or from a job, in the process that writes or in another; an entry can be sent twice, so consumers need an inbox.",
            svc.name
        )),
        &[],
        None,
    );
    w.open(format!("public final class {class}"));
    w.line(format!(
        "/** The options each topic of `{}` is published with by the relay (`spec/events.md` section 10.6). */",
        svc.name
    ));
    let entries: Vec<String> = events::published(svc)
        .into_iter()
        .map(|id| {
            let c = ir.channel(id);
            format!(
                "{map}.entry({}, new {transport}.ChannelOptions({}, {}))",
                java_str(&c.topic),
                c.ordered,
                c.dead_letter
            )
        })
        .collect();
    w.line(format!(
        "private static final {map}<String, {transport}.ChannelOptions> CHANNELS ="
    ));
    w.indent().indent();
    w.line(format!("{map}.ofEntries({});", entries.join(", ")));
    w.dedent().dedent();
    w.blank();
    w.line(format!("private {class}() {{}}"));
    w.blank();
    w.line("/** Runs the relay once, with the default options (up to 100 entries). */");
    w.open(format!(
        "public static int run({outbox} outbox, {transport} transport)"
    ));
    w.line(format!(
        "return run(outbox, transport, new {events}.RelayOptions());"
    ));
    w.close();
    w.blank();
    w.line("/** Runs the relay once: answers how many entries the transport accepted. */");
    w.open(format!(
        "public static int run({outbox} outbox, {transport} transport, {events}.RelayOptions options)"
    ));
    w.line(format!(
        "return {events}.relay(outbox, transport, CHANNELS, options);"
    ));
    w.close();
    w.close();
}

/// The members `<Service>Hooks` has for the outbox: the id source, the clock and the rule hook of
/// `emit` (`events(...)`), and what the generated handler reads (`emissions()`).
pub(crate) fn write_hooks_members(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    svc: &Service,
    class: &str,
) {
    let events = fc.rt("Events");
    let emission = fc.rt("Emission");
    let first = emitting(svc).next().map_or("", |e| e.name.as_str());
    w.line("/** The id source, the clock and the rule hook of `emit` (`spec/events.md` section 11.1). The clock of the endpoint rules is another thing. */");
    w.open(format!(
        "public {class} events({events}.PublisherOptions options)"
    ));
    w.line("this.events = options;");
    w.line("return this;");
    w.close();
    w.blank();
    w.line("/** The options of `emit`, for the test handler of the same package. */");
    w.open(format!("{events}.PublisherOptions events()"));
    w.line("return events;");
    w.close();
    w.blank();
    w.line("/** What the endpoints that emit write to. A server without an outbox fails when it is created. */");
    w.open(format!("{emission}.Source emissions()"));
    w.open("if (outbox == null)");
    w.line(format!(
        "throw new IllegalStateException({});",
        java_str(&format!("{first}: hooks.outbox is required"))
    ));
    w.close();
    w.line(format!(
        "return new {emission}.Source(outbox, events, null);"
    ));
    w.close();
    w.blank();
}
