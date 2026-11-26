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

/// A flagged invariant: the first use is the `rule` line.
const FLAG_RULE_SOURCE: &str = "model Stay {
  guests: int
  rule small: guests <= 6 when flag(\"smallGroups\")
}

service Stays at \"/stays\" {
  post \"/\" book(body: Stay) -> 204
}
";

/// A flagged `require`: the first use is the `require` line.
const FLAG_REQUIRE_SOURCE: &str = "model Room {
  id: uuid
  open: bool
}

service Rooms at \"/rooms\" {
  post \"/{id}/book\" book(id: uuid) -> 204 {
    fact room: Room = id
    require room.open when flag(\"openOnly\")
  }
}
";

/// A flagged `flow` that no endpoint takes: the first use is the `enum` line.
const FLAG_FLOW_SOURCE: &str = "enum Phase {
  draft, live
  flow draft -> live when flag(\"launch\")
  final live
}

model Room {
  id: uuid
  phase: Phase
}

service Rooms at \"/rooms\" {
  get \"/{id}\" getRoom(id: uuid) -> Room
}
";

/// The contracts that use `rules_flags`, each with the line of its first use.
const FLAG_CASES: [(&str, &str, &str); 3] = [
    ("compiled_flag_rule", FLAG_RULE_SOURCE, "rule small"),
    (
        "compiled_flag_require",
        FLAG_REQUIRE_SOURCE,
        "require room.open",
    ),
    ("compiled_flag_flow", FLAG_FLOW_SOURCE, "enum Phase"),
];

/// `@timeout` on an endpoint: the first use is the endpoint line.
const TIMEOUT_SOURCE: &str = "model Room {
  id: uuid
}

service Rooms at \"/rooms\" {
  @timeout(seconds(2))
  get \"/{id}\" getRoom(id: uuid) -> Room
}
";

/// `@timeout` on a service only: it reaches the endpoint, whose line is the first use.
const TIMEOUT_SERVICE_SOURCE: &str = "model Room {
  id: uuid
}

@timeout(minutes(1))
service Rooms at \"/rooms\" {
  get \"/{id}\" getRoom(id: uuid) -> Room
}
";

/// `@slo` on an endpoint: the first use is the endpoint line.
const SLO_SOURCE: &str = "model Room {
  id: uuid
}

service Rooms at \"/rooms\" {
  @slo(p99: ms(300), errors: 0.1)
  get \"/{id}\" getRoom(id: uuid) -> Room
}
";

/// `@slo` on a service only.
const SLO_SERVICE_SOURCE: &str = "model Room {
  id: uuid
}

@slo(errors: 1)
service Rooms at \"/rooms\" {
  get \"/{id}\" getRoom(id: uuid) -> Room
}
";

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

/// The contracts that use `timeouts`, with the line of the first endpoint that has a deadline.
///
/// # Panics
/// When a contract of this crate does not compile.
#[must_use]
pub fn compiled_timeout_contracts() -> Vec<(Ir, u32)> {
    compiled(&[
        ("compiled_timeout", TIMEOUT_SOURCE, "get \"/{id}\""),
        (
            "compiled_timeout_service",
            TIMEOUT_SERVICE_SOURCE,
            "get \"/{id}\"",
        ),
    ])
}

/// The contracts that use `slo`, with the line of the first endpoint that has an objective.
///
/// # Panics
/// When a contract of this crate does not compile.
#[must_use]
pub fn compiled_slo_contracts() -> Vec<(Ir, u32)> {
    compiled(&[
        ("compiled_slo", SLO_SOURCE, "get \"/{id}\""),
        ("compiled_slo_service", SLO_SERVICE_SOURCE, "get \"/{id}\""),
    ])
}

/// The generator must not declare `timeouts` and must refuse a contract with a deadline, in every
/// role (see [`check_timed_is_refused`]).
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_timeouts_are_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    all_refused(generator, Feature::Timeouts, &compiled_timeout_contracts())
}

/// The generator must not declare `slo` and must refuse a contract with a service level, in every
/// role (see [`check_timed_is_refused`]).
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_slo_is_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    all_refused(generator, Feature::Slo, &compiled_slo_contracts())
}

/// The contracts that use `rules_flags`, with the line of the first use in each.
///
/// # Panics
/// When a contract of this crate does not compile.
#[must_use]
pub fn compiled_flag_contracts() -> Vec<(Ir, u32)> {
    compiled(&FLAG_CASES)
}

/// The generator must not declare `rules_flags` and must refuse a contract with a flagged
/// statement, in every role (see [`check_timed_is_refused`]).
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_flags_are_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    all_refused(generator, Feature::RulesFlags, &compiled_flag_contracts())
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
    fn each_deadline_and_service_level_contract_uses_its_feature_and_the_gate_points_at_it() {
        let all = Capabilities::all_except(&[]);
        for (feature, contracts) in [
            (Feature::Timeouts, compiled_timeout_contracts()),
            (Feature::Slo, compiled_slo_contracts()),
        ] {
            for (ir, line) in contracts {
                let used = ir.features_used(&[ServiceId(0)]);
                assert!(used.contains(&feature), "{used:?}");
                let found = gate(&ir, &all);
                assert_eq!(found.len(), 1, "{found:?}");
                assert_eq!(found[0].origin.as_ref().map(|o| o.line), Some(line));
                assert!(gate(&ir, &all.clone().with(feature)).is_empty());
            }
        }
    }

    #[test]
    fn each_flag_contract_uses_rules_flags_and_the_gate_points_at_it() {
        let all = Capabilities::all_except(&[]);
        for (ir, line) in compiled_flag_contracts() {
            let used = ir.features_used(&[ServiceId(0)]);
            assert!(used.contains(&Feature::RulesFlags), "{used:?}");
            let found = gate(&ir, &all);
            assert_eq!(found.len(), 1, "{found:?}");
            assert_eq!(found[0].origin.as_ref().map(|o| o.line), Some(line));
            assert!(gate(&ir, &all.clone().with(Feature::RulesFlags)).is_empty());
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
