//! The outbox of events phase E3 (`spec/events.md` section 11, `spec/targets/typescript.md`
//! section 5f): the `emit` the handler of a move that emits receives, and the relay.
//!
//! The pieces are written into `server/<service>-events.ts`, next to the publisher whose steps
//! `emit` shares (`emitEvent` and `publishEvent` of `runtime/events.ts` call the same
//! `prepareEvent`), and the server module imports them: the type `<Endpoint>Emit` that the
//! request of the handler gets, and `make<Endpoint>Emit`, which the `transition` of the endpoint
//! description names (`emits: { edges, make }`).

use veld_codegen::{CodeWriter, naming};
use veld_emit_typescript::client::RequestNames;
use veld_emit_typescript::derived::{Shape, shaped_type};
use veld_emit_typescript::machine::Move;
use veld_emit_typescript::tree::FileCtx;
use veld_emit_typescript::util::{doc_comment, method_key, prop_key, ts_str};
use veld_ir::{Endpoint, Ir, Service, Type};

use crate::events;

/// The endpoints of `svc` whose move emits an event.
pub(crate) fn emitting(svc: &Service) -> impl Iterator<Item = &Endpoint> {
    svc.endpoints.iter().filter(|ep| emits(ep))
}

/// Whether the move of `ep` emits an event.
pub(crate) fn emits(ep: &Endpoint) -> bool {
    ep.rules
        .transition
        .as_ref()
        .is_some_and(|t| !t.emits.is_empty())
}

/// Whether the hooks of `svc` need an outbox.
pub(crate) fn service_emits(svc: &Service) -> bool {
    emitting(svc).next().is_some()
}

/// `PayEmit` for the request `PayRequest`: the type of what the handler gets besides the facts and
/// the transition.
pub(crate) fn emit_name(request: &str) -> String {
    format!("{}Emit", request.strip_suffix("Request").unwrap_or(request))
}

/// `makePayEmit`: the function that builds the `emit` of one request.
pub(crate) fn make_name(request: &str) -> String {
    format!("make{}", emit_name(request))
}

/// `{ 'pending': ['BookingPaid'], 'paid': ['RefundDue'] }`: by wire value of each source state, the
/// declaration names of the events the edge from it to the target emits. A source whose edge emits
/// nothing is left out. This is `veld_rules::outbox::expected_events` for every source at once.
pub(crate) fn edges(ir: &Ir, mv: &Move<'_>) -> String {
    let t = mv.transition;
    let mut items = Vec::new();
    for (name, wire) in t.from.iter().zip(&mv.sources) {
        let mut names: Vec<&str> = Vec::new();
        for flow in &mv.machine.flows {
            if flow.from != *name || flow.to != t.to {
                continue;
            }
            if let Some(event) = flow.emits {
                let event = ir.decl(event).name.as_str();
                if !names.contains(&event) {
                    names.push(event);
                }
            }
        }
        if names.is_empty() {
            continue;
        }
        let list: Vec<String> = names.iter().map(|n| ts_str(n)).collect();
        items.push(format!("{}: [{}]", prop_key(wire), list.join(", ")));
    }
    format!("{{ {} }}", items.join(", "))
}

/// The types and the builders of `emit` of every endpoint of `svc` whose move emits, and the relay.
pub(crate) fn write(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service) {
    let names = RequestNames::new(fc.tree.ir, &fc.tree.services);
    for ep in emitting(svc) {
        let request = names.get(svc.id, &ep.name);
        write_emit(w, fc, ep, &request);
        w.blank();
    }
    write_relay(w, fc, svc);
}

fn write_emit(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, ep: &Endpoint, request: &str) {
    let ir = fc.tree.ir;
    let ns = fc.events_ns();
    let Some(transition) = &ep.rules.transition else {
        return;
    };
    doc_comment(
        w,
        Some(&format!(
            "What the handler of `{}` gets as `emit`, with the facts and the transition (`spec/events.md` section 11.1): one function per event of the move. Call it in the transaction that changes the state; it runs steps 1 to 5 of publishing and writes the entry to the outbox. Nothing is sent to a transport. The server answers `500 Internal` for a success that did not write exactly the events of the edge it took.",
            ep.name
        )),
        None,
    );
    w.open(format!("export interface {}", emit_name(request)));
    w.open_with("emit:", " {");
    for e in &transition.emits {
        let decl = ir.decl(e.event);
        doc_comment(w, decl.docs.as_deref(), decl.deprecated.as_deref());
        let ty = shaped_type(fc, &Type::Decl(e.event), Shape::Output);
        w.line(format!(
            "{}(event: {ty}): Promise<{ns}.PublishedEvent>;",
            method_key(&naming::camel(&decl.name))
        ));
    }
    w.close_with("};");
    w.close();
    w.blank();
    let session = fc.rt("EmitSession");
    w.line(format!(
        "/** Builds the `emit` of one request of `{}`: the runtime calls it after the transition guard. */",
        ep.name
    ));
    w.open(format!(
        "export function {}(session: {session}): {}['emit']",
        make_name(request),
        emit_name(request)
    ));
    w.open_with(format!("const ctx: {ns}.EmitContext ="), " {");
    w.line(format!("outbox: session.outbox as {ns}.Outbox,"));
    w.line("source: SOURCE,");
    w.line(format!(
        "options: (session.options ?? {{}}) as {ns}.PublisherOptions,"
    ));
    w.line("validation,");
    w.line("record: session.record,");
    w.close_with("};");
    w.open_with("return", " {");
    for e in &transition.emits {
        let decl = ir.decl(e.event);
        w.line(format!(
            "{}: (event) => {ns}.emitEvent(ctx, {}, {}, event),",
            method_key(&naming::camel(&decl.name)),
            events::channel_name(ir.channel(e.channel)),
            events::def_name(fc.tree, e.event)
        ));
    }
    w.close_with("};");
    w.close();
}

/// `run<Service>OutboxRelay(outbox, transport, options)` (`spec/events.md` section 11.1).
fn write_relay(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service) {
    let ir = fc.tree.ir;
    let ns = fc.events_ns();
    let name = naming::pascal(&svc.name);
    w.line(format!(
        "/** The options each topic of `{}` is published with by the relay (`spec/events.md` section 10.6). */",
        svc.name
    ));
    w.open_with(
        format!("const relayChannels: Readonly<Record<string, {ns}.ChannelOptions>> ="),
        " {",
    );
    for id in events::published(svc) {
        let channel = ir.channel(id);
        w.line(format!(
            "{}: {{ ordered: {}, deadLetter: {} }},",
            prop_key(&channel.topic),
            channel.ordered,
            channel.dead_letter
        ));
    }
    w.close_with("};");
    w.blank();
    doc_comment(
        w,
        Some(&format!(
            "Runs the relay of `{}` once (`spec/events.md` section 11.1): takes up to `limit` (default 100) pending entries of the outbox, publishes each to `transport` with the options of its channel, in the order the outbox gave them, and calls `done` after the transport accepted it. It stops at the first entry that fails (the failure goes to `options.onError`) and answers how many entries it sent. Run it in a loop or from a job; an entry can be sent twice, so consumers need an inbox.",
            svc.name
        )),
        None,
    );
    w.open(format!(
        "export function run{name}OutboxRelay(outbox: {ns}.Outbox, transport: {ns}.Transport, options: {ns}.RelayOptions = {{}}): Promise<number>"
    ));
    w.line(format!(
        "return {ns}.relayOutbox(outbox, transport, relayChannels, options);"
    ));
    w.close();
}

/// The parameter or fact of `ep` that is called `emit`: the handler's request would have two.
pub(crate) fn clashes(ep: &Endpoint) -> Option<&str> {
    let taken = |n: &str| n == "emit";
    ep.params
        .iter()
        .map(|p| p.name.as_str())
        .chain(ep.rules.facts.iter().map(|f| f.name.as_str()))
        .find(|n| taken(n))
}
