//! Conformance checks for `emits` and the outbox of phase E3 (`spec/events.md` section 11).
//!
//! `events_outbox` is opt-in: a generator that has not built the outbox must refuse a service with
//! an endpoint whose `transition` emits an event with `E0801`, at the `transition`, and never
//! generate a server whose state changes without its event. A client generates nothing for events
//! and keeps its bytes.

use veld_codegen::{DynGenerator, FileSet, GenRequest, Role, check_capabilities};
use veld_ir::{Feature, Ir, ServiceId};

use crate::compile;

const SOURCE: &str = r#"event Paid { orderId: uuid }

channel PaymentEvents {
  carries Paid
  key orderId
}

enum Status {
  pending, paid
  flow pending -> paid@EMITS@
  final paid
}

model Order { id: uuid, status: Status }

service Orders at "/orders" {
  publishes PaymentEvents

  post "/{id}/pay" pay(id: uuid) -> Order {
    fact order: Order = id
    transition order.status: pending -> paid
  }
}
"#;

/// The text the checks look for at the line of the `transition`.
const TRANSITION: &str = "transition order.status";

/// The contract with `emits Paid` on its flow (`true`) or without it.
fn source(emits: bool) -> String {
    SOURCE.replace("@EMITS@", if emits { " emits Paid" } else { "" })
}

/// The line of the `transition` statement in `source`.
fn transition_line(source: &str) -> Result<u32, String> {
    let at = source.lines().position(|l| l.contains(TRANSITION));
    at.and_then(|i| u32::try_from(i + 1).ok())
        .ok_or_else(|| "the outbox contract has no transition".to_string())
}

/// A contract with a service `Orders` whose only endpoint has a `transition` over a flow that
/// emits `Paid` (`emits` true) or over the same flow without `emits`.
#[must_use]
pub fn compiled_events_outbox_contract(emits: bool) -> Ir {
    compile("compiled_events_outbox", &source(emits))
}

fn orders(ir: &Ir) -> Result<ServiceId, String> {
    ir.services
        .iter()
        .find(|s| s.name == "Orders")
        .map(|s| s.id)
        .ok_or_else(|| "the outbox contract has no service `Orders`".to_string())
}

fn request(ir: &Ir, role: Role, services: Vec<ServiceId>) -> GenRequest<'_> {
    GenRequest {
        ir,
        role,
        services,
        consumes: vec![],
        framework: None,
    }
}

/// A server generator that has not built phase E3 must not declare `events_outbox`, and for the
/// service `Orders` the gate reports exactly one `E0801` that names the feature, at the
/// `transition`, and `generate` answers `E0801` only and makes no files. The same contract
/// without `emits` must not be reported for `events_outbox`. Roles other than the server are not
/// checked here.
///
/// # Errors
/// A description of the first role for which the generator does not behave so.
pub fn check_events_outbox_is_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    for role in generator.roles().iter().filter(|r| **r == Role::Server) {
        let caps = generator.capabilities(*role);
        if caps.supports(Feature::EventsOutbox) {
            return Err(format!("{}: declares `events_outbox`", role.name()));
        }
        let src = source(true);
        let line = transition_line(&src)?;
        let ir = compile("events_outbox_refused", &src);
        let req = request(&ir, *role, vec![orders(&ir)?]);
        let gate = check_capabilities(&ir, &req, &caps);
        let mine: Vec<_> = gate
            .iter()
            .filter(|d| d.message.contains("`events_outbox`"))
            .collect();
        let at_the_transition = mine.len() == 1
            && mine
                .iter()
                .all(|d| d.code == "E0801" && d.origin.as_ref().map(|o| o.line) == Some(line));
        if !at_the_transition || gate.iter().any(|d| d.code != "E0801") {
            return Err(format!(
                "{}: the gate reported {gate:?}, wanted one E0801 for `events_outbox` at line {line}",
                role.name()
            ));
        }
        match generator.generate_dyn(&req, &serde_json::Value::Null) {
            Err(d) if !d.is_empty() && d.iter().all(|d| d.code == "E0801") => {}
            other => {
                return Err(format!(
                    "{}: generate did not answer E0801: {other:?}",
                    role.name()
                ));
            }
        }
        let plain = compile("events_outbox_plain", &source(false));
        let req = request(&plain, *role, vec![orders(&plain)?]);
        let gate = check_capabilities(&plain, &req, &caps);
        if gate.iter().any(|d| d.message.contains("`events_outbox`")) {
            return Err(format!(
                "{}: a flow without `emits` was reported for `events_outbox`: {gate:?}",
                role.name()
            ));
        }
    }
    Ok(())
}

/// A client generator must generate the same files, byte for byte, with and without `emits` on
/// the flow (`spec/events.md` section 11): a client does not see the outbox.
///
/// # Errors
/// A description of the first client role for which the gate objects or the output moves.
pub fn check_events_outbox_is_ignored_by_clients(
    generator: &dyn DynGenerator,
) -> Result<(), String> {
    let plain = compiled_events_outbox_contract(false);
    for role in generator.roles().iter().filter(|r| **r == Role::Client) {
        let before = generate(generator, &plain, *role)?;
        let ir = compiled_events_outbox_contract(true);
        let req = request(&ir, *role, ir.services.iter().map(|s| s.id).collect());
        let gate = check_capabilities(&ir, &req, &generator.capabilities(*role));
        if !gate.is_empty() {
            return Err(format!("{}: the gate reported {gate:?}", role.name()));
        }
        if before != generate(generator, &ir, *role)? {
            return Err(format!(
                "{}: `emits` changed the client output",
                role.name()
            ));
        }
    }
    Ok(())
}

fn generate(generator: &dyn DynGenerator, ir: &Ir, role: Role) -> Result<FileSet, String> {
    let req = request(ir, role, ir.services.iter().map(|s| s.id).collect());
    generator
        .generate_dyn(&req, &serde_json::Value::Null)
        .map_err(|d| format!("generation failed: {d:?}"))
}
