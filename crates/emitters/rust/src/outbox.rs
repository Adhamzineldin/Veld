//! The outbox of events phase E3 (`spec/events.md` section 11, `spec/targets/rust.md` section 13):
//! the `emit` argument of the handler of a move that emits (`<Endpoint>Emit`), the events the edge
//! taken must write, and the relay (`run_<service>_outbox_relay`), all in `<service>_events.rs`,
//! where the descriptions of the channels and events are.
//!
//! `emit` shares the code of the publisher: `veld::events::Emission::emit` and `publish_event_with`
//! both call the runtime's `build_message` (steps 1 to 5 of `spec/events.md` section 7).

use veld_codegen::{CodeWriter, GenDiagnostic, naming};
use veld_ir::{Channel, DeclKind, Endpoint, EndpointTransition, Ir, Service, ServiceId};

use crate::events::{channel_static, def_name, events_mod};
use crate::tree::Tree;
use crate::types::Ctx;
use crate::util::{lit, method, snake, type_name};

/// Whether the move of `ep` emits an event.
#[must_use]
pub fn emits(ep: &Endpoint) -> bool {
    ep.rules
        .transition
        .as_ref()
        .is_some_and(|t| !t.emits.is_empty())
}

/// The endpoints of `svc` whose move emits an event, with their index.
pub fn emitting(svc: &Service) -> impl Iterator<Item = (usize, &Endpoint)> {
    svc.endpoints.iter().enumerate().filter(|(_, e)| emits(e))
}

/// Whether the hooks of `svc` need an outbox.
#[must_use]
pub fn svc_emits(svc: &Service) -> bool {
    emitting(svc).next().is_some()
}

/// Whether one of the services has an endpoint whose move emits.
#[must_use]
pub fn served_emits(ir: &Ir, services: &[ServiceId]) -> bool {
    services.iter().any(|s| svc_emits(ir.service(*s)))
}

/// `PayEmit`: the type of the `emit` argument of the endpoint `pay`.
#[must_use]
pub fn emit_name(ep: &Endpoint) -> String {
    type_name(&format!("{}Emit", naming::pascal(&ep.name)))
}

/// The module of the events of the service, as a path from the crate root.
#[must_use]
pub fn events_path(tree: &Tree<'_>, svc: &Service) -> String {
    format!(
        "{}::{}::{}",
        tree.root,
        tree.pkg_mod(svc.package),
        events_mod(svc)
    )
}

/// `, emit: crate::bookings::bookings_events::PayEmit<'_>` for the service method of an endpoint
/// whose move emits: it follows the transition (`spec/rules.md` section 11.12).
#[must_use]
pub fn emit_param(tree: &Tree<'_>, svc: &Service, ep: &Endpoint) -> String {
    if emits(ep) {
        format!(", emit: {}::{}<'_>", events_path(tree, svc), emit_name(ep))
    } else {
        String::new()
    }
}

/// The doc lines of the service method of an endpoint whose move emits.
pub fn emit_docs(w: &mut CodeWriter, ep: &Endpoint) {
    if !emits(ep) {
        return;
    }
    w.line("///");
    w.line("/// `emit` writes the events of the move to the outbox (`spec/events.md` section 11.1): call its");
    w.line("/// method for every event the edge taken emits, exactly once and in the transaction that changes");
    w.line("/// the state, so that the state and its event stand or fall together. It runs steps 1 to 5 of");
    w.line("/// publishing and then `Outbox::save`; nothing is sent to a transport (the relay does that). After");
    w.line("/// a success the server checks that exactly the events of the edge were written and answers `500");
    w.line(
        "/// Internal` otherwise. A method that answers a declared error need not have written.",
    );
}

/// The refusals of a server: a parameter or a fact called `emit` on an endpoint whose move emits
/// would hide the argument of the handler (the other targets cannot give it that name either), and
/// a move that emits an event on a channel the service does not publish has nowhere to write it.
///
/// # Errors
/// One `E0801` for each.
pub fn refusals(ir: &Ir, services: &[ServiceId]) -> Result<(), Vec<GenDiagnostic>> {
    let mut out = Vec::new();
    for sid in services {
        let svc = ir.service(*sid);
        for (_, ep) in emitting(svc) {
            let clash = ep
                .params
                .iter()
                .map(|p| p.name.as_str())
                .chain(ep.rules.facts.iter().map(|f| f.name.as_str()))
                .find(|n| snake(n) == "emit");
            if let Some(name) = clash {
                out.push(refusal(svc, ep, &format!("a parameter or a fact `{name}` hides the `emit` argument of the handler of a move that emits an event")));
            }
            let published: Vec<_> = svc.published_channels().collect();
            let transition = ep.rules.transition.iter().flat_map(|t| &t.emits);
            if transition
                .into_iter()
                .any(|e| !published.contains(&e.channel))
            {
                out.push(refusal(
                    svc,
                    ep,
                    "the channel of an event the move emits is not one the service publishes",
                ));
            }
        }
    }
    if out.is_empty() { Ok(()) } else { Err(out) }
}

fn refusal(svc: &Service, ep: &Endpoint, why: &str) -> GenDiagnostic {
    let mut d = GenDiagnostic::new(
        "E0801",
        format!(
            "{}.{}: generator `rust` cannot generate the outbox for this endpoint: {why}",
            svc.name, ep.name
        ),
    );
    d.origin = ep.rules.first_origin().cloned();
    d
}

/// `match from { "pending" => &["BookingPaid"], _ => &[] }`: by wire value of each source state, the
/// declaration names of the events the edge from it to the target emits. A source whose edge emits
/// nothing is left out. This is `veld_rules::outbox::expected_events` for every source at once, worked
/// out at generation time from the flows of the enum.
fn edges(ir: &Ir, t: &EndpointTransition) -> Vec<(String, Vec<String>)> {
    let DeclKind::Enum(machine) = &ir.decl(t.decl).kind else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for name in &t.from {
        let Some(wire) = machine
            .variants
            .iter()
            .find(|v| &v.name == name)
            .map(|v| v.wire.clone())
        else {
            continue;
        };
        let mut events: Vec<String> = Vec::new();
        for flow in machine
            .flows
            .iter()
            .filter(|f| &f.from == name && f.to == t.to)
        {
            if let Some(e) = flow.emits {
                let event = ir.decl(e).name.clone();
                if !events.contains(&event) {
                    events.push(event);
                }
            }
        }
        if !events.is_empty() {
            out.push((wire, events));
        }
    }
    out
}

fn emits_fn(ep: &Endpoint) -> String {
    format!("{}_emits", method(&ep.name))
}

fn relay_fn(svc: &Service) -> String {
    format!("run_{}_outbox_relay", snake(&svc.name))
}

/// Writes the `emit` types, the tables of expected events and the relay of a service that has an
/// endpoint whose move emits. The statics of the events and the channels are the ones the events file
/// already has for its publisher.
pub fn write(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service) {
    if !svc_emits(svc) {
        return;
    }
    for (_, ep) in emitting(svc) {
        write_emit(w, ctx, svc, ep);
        write_expected(w, ctx.tree.ir, ep);
    }
    write_dispatch(w, svc);
    write_relay(w, ctx, svc);
}

fn write_emit(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service, ep: &Endpoint) {
    let ir = ctx.tree.ir;
    let name = emit_name(ep);
    let options = format!("{}PublisherOptions", type_name(&svc.name));
    w.line(format!(
        "/// The `emit` argument of `{}`: one method per event the move emits, in the order of the",
        method(&ep.name)
    ));
    w.line("/// contract. Each runs steps 1 to 5 of publishing (shape, invariants, the key, the id and the");
    w.line("/// clock, the bytes) and saves the message in the outbox; nothing is sent to a transport. A value");
    w.line(
        "/// that does not hold is the `ValidationError` of a publisher, and nothing is written.",
    );
    w.open(format!("pub struct {name}<'a>"));
    w.line("emission: veld::events::Emission<'a>,");
    w.close().blank();
    w.open(format!("impl<'a> {name}<'a>"));
    w.line("/// Starts the emission of one request. For generated code.");
    w.line("#[doc(hidden)]");
    w.open(format!(
        "pub fn begin<O: veld::events::Outbox>(outbox: &'a O, options: {options}, log: &veld::events::EmitLog) -> Self"
    ));
    w.line(format!(
        "{name} {{ emission: veld::events::Emission::new(outbox, (SOURCE, VALIDATION), options, log) }}"
    ));
    w.close();
    for e in ep.rules.transition.iter().flat_map(|t| &t.emits) {
        let decl = ir.decl(e.event);
        let channel = ir.channel(e.channel);
        w.blank();
        w.line(format!(
            "/// Writes a `{}` to the outbox, for the channel `{}` (topic `{}`).",
            decl.name, channel.name, channel.topic
        ));
        w.open(format!(
            "pub async fn {}(&self, event: &{}) -> Result<veld::events::PublishedEvent, veld::Error>",
            snake(&decl.name),
            ctx.tree.type_path(e.event, ctx.here)
        ));
        w.line(format!(
            "self.emission.emit(&{}, &{}, event).await",
            channel_static(channel),
            def_name(ctx, e.event)
        ));
        w.close();
    }
    w.close().blank();
}

/// The events the move of `ep` must write, by the state the field held.
fn write_expected(w: &mut CodeWriter, ir: &Ir, ep: &Endpoint) {
    let Some(t) = ep.rules.transition.as_ref() else {
        return;
    };
    w.line(format!(
        "/// The events the move of `{}` writes when the state field held `from` (a wire value):",
        method(&ep.name)
    ));
    w.line("/// what the server checks after the handler.");
    w.line("#[doc(hidden)]");
    w.open(format!(
        "pub fn {}(from: &str) -> &'static [&'static str]",
        emits_fn(ep)
    ));
    w.open("match from");
    for (wire, events) in edges(ir, t) {
        let list: Vec<String> = events.iter().map(|e| lit(e)).collect();
        w.line(format!("{} => &[{}],", lit(&wire), list.join(", ")));
    }
    w.line("_ => &[],");
    w.close().close().blank();
}

/// `emitted_by(endpoint, from)`: the table above for the endpoint with that index; the test handler
/// answers with it when it does not call the handler.
fn write_dispatch(w: &mut CodeWriter, svc: &Service) {
    w.line(
        "/// The events the move of the endpoint with this index writes when the state field held",
    );
    w.line("/// `from`: what `Veld-Test-Emitted` lists when the test handler does not call the handler.");
    w.line("#[doc(hidden)]");
    w.open("pub fn emitted_by(endpoint: usize, from: &str) -> &'static [&'static str]");
    w.open("match endpoint");
    for (i, ep) in emitting(svc) {
        w.line(format!("{i} => {}(from),", emits_fn(ep)));
    }
    w.line("_ => &[],");
    w.close().close().blank();
}

fn write_relay(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service) {
    let ir = ctx.tree.ir;
    let mut channels: Vec<&Channel> = Vec::new();
    for p in &svc.publishes {
        let channel = ir.channel(p.channel);
        if !channels.iter().any(|c| c.topic == channel.topic) {
            channels.push(channel);
        }
    }
    let upper = naming::screaming_snake(&svc.name);
    w.line(format!(
        "/// The options of the channels `{}` publishes to, by topic: what the relay hands the transport.",
        svc.name
    ));
    w.open_with(
        format!("const {upper}_RELAY_CHANNELS: &[(&str, veld::events::ChannelOptions)] ="),
        " &[",
    );
    for channel in channels {
        w.line(format!(
            "({}, veld::events::ChannelOptions {{ ordered: {}, dead_letter: {} }}),",
            lit(&channel.topic),
            channel.ordered,
            channel.dead_letter
        ));
    }
    w.close_with("];");
    w.blank();
    w.line(format!(
        "/// The relay of `{}` (`spec/events.md` section 11.1): takes up to `options.limit` pending entries of",
        svc.name
    ));
    w.line("/// the outbox, publishes each to `transport` with the options of its channel, in the order the");
    w.line("/// outbox gave them, and calls `done` after the transport accepted it. It stops at the first entry");
    w.line("/// that fails (the failure goes to `options.on_error`) and answers how many it sent. Run it in a");
    w.line("/// loop or from a job; an entry can be sent twice, so consumers need an inbox.");
    w.open(format!(
        "pub async fn {}(outbox: &dyn veld::events::Outbox, transport: &dyn veld::events::Transport, options: &veld::events::RelayOptions) -> usize",
        relay_fn(svc)
    ));
    w.line(format!(
        "veld::events::relay_outbox((outbox, transport), {upper}_RELAY_CHANNELS, options).await"
    ));
    w.close().blank();
}

/// The lines of the call of an endpoint whose move emits, before the service method is called:
/// `let emits = ...; let emit = PayEmit::begin(...);`. `checked` is false for a stream, which is not
/// checked after the handler and so needs no list of expected events.
pub fn write_begin(
    w: &mut CodeWriter,
    tree: &Tree<'_>,
    svc: &Service,
    ep: &Endpoint,
    checked: bool,
) {
    let path = events_path(tree, svc);
    if checked {
        w.line(format!(
            "let emits = {path}::{}(veld::events::transition_from(&args));",
            emits_fn(ep)
        ));
    }
    w.line(format!(
        "let emit = {path}::{}::begin(self.hooks.outbox(), self.hooks.events(), &ctx.emitted);",
        emit_name(ep)
    ));
}

/// The check after the handler of a success: `veld::events::check_emitted("pay", emits, &ctx.emitted)?;`.
pub fn write_check(w: &mut CodeWriter, ep: &Endpoint) {
    w.line(format!(
        "veld::events::check_emitted({}, emits, &ctx.emitted)?;",
        lit(&method(&ep.name))
    ));
}

/// The members of the hooks of a service that has an endpoint whose move emits: the outbox and the
/// options of `emit`.
pub fn hooks(w: &mut CodeWriter) {
    w.blank();
    w.line("/// The outbox the handlers of the moves that emit write to (`spec/events.md` section 11.1).");
    w.line("/// Implement `veld::events::Outbox` over your own store, so that `save` joins the transaction");
    w.line("/// that changes the state, or use `veld::events::MemoryOutbox` in tests.");
    w.line("type Outbox: veld::events::Outbox;");
    w.blank();
    w.line("/// The outbox the endpoints write the events of their moves to.");
    w.line("fn outbox(&self) -> &Self::Outbox;");
    w.blank();
    w.line("/// The id source, the clock and the rule hook of `emit` (the options of a publisher). The default");
    w.line("/// is a random id, the system clock and no hook.");
    w.open("fn events(&self) -> veld::events::PublisherOptions");
    w.line("veld::events::PublisherOptions::new()");
    w.close();
}
