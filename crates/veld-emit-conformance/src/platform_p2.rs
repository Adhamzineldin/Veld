//! Conformance checks for platform phase P2 (`spec/platform.md` sections 10 to 13): timed moves,
//! flags, deadlines and service levels.
//!
//! All four are opt-in. A generator that does not know one would leave out the due check, the
//! flag, the deadline or the metrics hook without a word, so until it declares the feature it must
//! refuse a contract that uses it with `E0801`, in every role, at the first use.

use veld_codegen::{DynGenerator, GenRequest, Role, check_capabilities};
use veld_ir::{Feature, Ir, ServiceId};

use crate::compile;

/// A timed edge and the transition that takes it: the first use is the `transition` line.
const TIMED_SOURCE: &str = "enum BookingStatus {
  pending, paid, cancelled
  flow pending -> paid
  flow pending -> cancelled after hours(24)
  final paid, cancelled
}

model Booking {
  id: uuid
  status: BookingStatus = .pending
  pendingSince: datetime
}

service Bookings at \"/bookings\" {
  post \"/{id}/expire\" expireBooking(id: uuid) -> Booking {
    fact booking: Booking = id
    transition booking.status: pending -> cancelled since booking.pendingSince
  }
}
";

/// A timed edge that no endpoint takes: the first use is the `enum` line.
const TIMED_EDGE_ONLY_SOURCE: &str = "enum Phase {
  open, closed
  flow open -> closed after minutes(5)
  final closed
}

model Gate {
  id: uuid
  phase: Phase
}

service Gates at \"/gates\" {
  get \"/{id}\" getGate(id: uuid) -> Gate
}
";

/// The contracts that use `rules_timed`, each with the line of its first use.
const TIMED_CASES: [(&str, &str, &str); 2] = [
    (
        "compiled_timed_transition",
        TIMED_SOURCE,
        "transition booking.status",
    ),
    (
        "compiled_timed_edge_only",
        TIMED_EDGE_ONLY_SOURCE,
        "enum Phase",
    ),
];

fn compiled(cases: &[(&str, &str, &str)]) -> Vec<(Ir, u32)> {
    cases
        .iter()
        .map(|(name, source, needle)| {
            let line = source
                .lines()
                .position(|l| l.contains(needle))
                .and_then(|i| u32::try_from(i + 1).ok())
                .unwrap_or(0);
            (compile(name, source), line)
        })
        .collect()
}

/// The contracts that use `rules_timed`, with the line of the first use in each.
///
/// # Panics
/// When a contract of this crate does not compile.
#[must_use]
pub fn compiled_timed_contracts() -> Vec<(Ir, u32)> {
    compiled(&TIMED_CASES)
}

/// The generator must not declare `rules_timed` and must refuse a contract with a timed edge or a
/// timed transition, in every role: the gate reports exactly one `E0801` that names the feature,
/// at the first use, and `generate` answers `E0801` only and makes no files.
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_timed_is_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    all_refused(generator, Feature::RulesTimed, &compiled_timed_contracts())
}

fn request(ir: &Ir, role: Role) -> GenRequest<'_> {
    GenRequest {
        ir,
        role,
        services: ir.services.iter().map(|s| s.id).collect::<Vec<ServiceId>>(),
        consumes: vec![],
        framework: None,
    }
}

fn all_refused(
    generator: &dyn DynGenerator,
    feature: Feature,
    contracts: &[(Ir, u32)],
) -> Result<(), String> {
    for (ir, line) in contracts {
        for role in generator.roles() {
            if generator.capabilities(*role).supports(feature) {
                return Err(format!("{}: declares `{}`", role.name(), feature.name()));
            }
            refuses(generator, ir, *role, feature, *line)
                .map_err(|e| format!("{} / line {line}: {e}", role.name()))?;
        }
    }
    Ok(())
}

fn refuses(
    generator: &dyn DynGenerator,
    ir: &Ir,
    role: Role,
    feature: Feature,
    line: u32,
) -> Result<(), String> {
    let req = request(ir, role);
    let gate = check_capabilities(ir, &req, &generator.capabilities(role));
    let name = format!("`{}`", feature.name());
    let mine: Vec<_> = gate.iter().filter(|d| d.message.contains(&name)).collect();
    let at_first_use = mine.len() == 1
        && mine
            .iter()
            .all(|d| d.code == "E0801" && d.origin.as_ref().map(|o| o.line) == Some(line));
    if !at_first_use || gate.iter().any(|d| d.code != "E0801") {
        return Err(format!(
            "the gate reported {gate:?}, wanted one E0801 for {name} at line {line}"
        ));
    }
    match generator.generate_dyn(&req, &serde_json::Value::Null) {
        Err(d) if !d.is_empty() && d.iter().all(|d| d.code == "E0801") => Ok(()),
        other => Err(format!("generate did not answer E0801: {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use veld_codegen::Capabilities;

    use super::*;

    fn gate(ir: &Ir, caps: &Capabilities) -> Vec<veld_codegen::GenDiagnostic> {
        check_capabilities(ir, &request(ir, Role::Server), caps)
    }

    #[test]
    fn each_timed_contract_uses_rules_timed() {
        for (ir, _) in compiled_timed_contracts() {
            let used = ir.features_used(&[ServiceId(0)]);
            assert!(used.contains(&Feature::RulesTimed), "{used:?}");
        }
    }

    #[test]
    fn the_gate_points_at_the_first_use_and_opting_in_lets_it_through() {
        let all = Capabilities::all_except(&[]);
        for (ir, line) in compiled_timed_contracts() {
            let found = gate(&ir, &all);
            assert_eq!(found.len(), 1, "{found:?}");
            assert_eq!(found[0].origin.as_ref().map(|o| o.line), Some(line));
            assert!(gate(&ir, &all.clone().with(Feature::RulesTimed)).is_empty());
        }
    }
}
