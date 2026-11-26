//! Conformance checks for `@version` and `@idempotent` (`spec/rules.md` sections 11.7 to 11.9).
//!
//! Both features are opt-in. A generator that does not know them would leave out the `If-Match`
//! guard, the `ETag` or the idempotency store without a word, so until it declares `rules_version`
//! or `idempotency` it must refuse a contract that uses them with `E0801`, in every role.

use veld_codegen::{DynGenerator, GenRequest, Role, check_capabilities};
use veld_ir::{Feature, Ir, ServiceId};

use crate::compile;

/// An endpoint whose first fact is a model with a version field: the `If-Match` guard.
const GUARD_SOURCE: &str = "model Room {
  id: uuid
  name: string
  @version
  revision: long
}

model Rename { name: string }

service Rooms at \"/rooms\" {
  put \"/{id}\" rename(id: uuid, body: Rename) -> Room | NotFound {
    fact room: Room = id
  }
}
";

/// An endpoint with no block that answers with such a model: only the `ETag`.
const ETAG_SOURCE: &str = "model Room {
  id: uuid
  name: string
  @version
  revision: long
}

service Rooms at \"/rooms\" {
  get \"/{id}\" getRoom(id: uuid) -> Room | NotFound
}
";

/// `@idempotent` with its defaults.
const IDEMPOTENT_SOURCE: &str = "model Booking {
  id: uuid
  guests: int
}

service Bookings at \"/bookings\" {
  @idempotent
  post \"/\" book(body: Booking) -> 201 Booking
}
";

/// `@idempotent` with a header, a required key and auth.
const IDEMPOTENT_REQUIRED_SOURCE: &str = "model Booking {
  id: uuid
  guests: int
}

service Bookings at \"/bookings\" {
  @auth(bearer)
  @idempotent(header: \"X-Request-Id\", required: true)
  post \"/\" book(body: Booking) -> 201 Booking
}
";

/// The contracts that use `rules_version`, each with the line of its first use: the fact of a
/// guard, and the endpoint that only owes an `ETag`.
const VERSION_CASES: [(&str, &str, &str); 2] = [
    ("compiled_version_guard", GUARD_SOURCE, "fact room"),
    (
        "compiled_version_etag",
        ETAG_SOURCE,
        "get \"/{id}\" getRoom",
    ),
];

/// The contracts that use `idempotency`, each with the line of the annotation.
const IDEMPOTENCY_CASES: [(&str, &str, &str); 2] = [
    ("compiled_idempotent", IDEMPOTENT_SOURCE, "@idempotent"),
    (
        "compiled_idempotent_required",
        IDEMPOTENT_REQUIRED_SOURCE,
        "@idempotent(",
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

/// The two contracts that use `rules_version`, with the line of the first use in each.
///
/// # Panics
/// When a contract of this crate does not compile.
#[must_use]
pub fn compiled_version_contracts() -> Vec<(Ir, u32)> {
    compiled(&VERSION_CASES)
}

/// The two contracts that use `idempotency`, with the line of the annotation in each.
///
/// # Panics
/// When a contract of this crate does not compile.
#[must_use]
pub fn compiled_idempotency_contracts() -> Vec<(Ir, u32)> {
    compiled(&IDEMPOTENCY_CASES)
}

/// The generator must not declare `rules_version` and must refuse a contract with a version guard
/// or a response that owes an `ETag`, in every role: the gate reports exactly one `E0801` that
/// names the feature, at the first use, and `generate` answers `E0801` only and makes no files.
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_version_is_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    all_refused(
        generator,
        Feature::RulesVersion,
        &compiled_version_contracts(),
    )
}

/// The generator must not declare `idempotency` and must refuse a contract with `@idempotent`, in
/// every role (see [`check_version_is_refused`]).
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_idempotency_is_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    all_refused(
        generator,
        Feature::Idempotency,
        &compiled_idempotency_contracts(),
    )
}

/// The generator declares `rules_version` in every role, the gate lets the contracts through and
/// `generate` makes files for each.
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_version_is_generated(generator: &dyn DynGenerator) -> Result<(), String> {
    all_generated(
        generator,
        Feature::RulesVersion,
        &compiled_version_contracts(),
    )
}

/// The generator declares `idempotency` in every role, the gate lets the contracts through and
/// `generate` makes files for each.
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_idempotency_is_generated(generator: &dyn DynGenerator) -> Result<(), String> {
    all_generated(
        generator,
        Feature::Idempotency,
        &compiled_idempotency_contracts(),
    )
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

fn all_generated(
    generator: &dyn DynGenerator,
    feature: Feature,
    contracts: &[(Ir, u32)],
) -> Result<(), String> {
    for (ir, line) in contracts {
        for role in generator.roles() {
            let caps = generator.capabilities(*role);
            if !caps.supports(feature) {
                return Err(format!(
                    "{}: does not declare `{}`",
                    role.name(),
                    feature.name()
                ));
            }
            let req = request(ir, *role);
            let gate = check_capabilities(ir, &req, &caps);
            if !gate.is_empty() {
                return Err(format!(
                    "{} / line {line}: the gate reported {gate:?}",
                    role.name()
                ));
            }
            match generator.generate_dyn(&req, &crate::options()) {
                Ok(files) if !files.is_empty() => {}
                other => {
                    return Err(format!(
                        "{} / line {line}: generate did not make files: {other:?}",
                        role.name()
                    ));
                }
            }
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
    match generator.generate_dyn(&req, &crate::options()) {
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

    /// A guard uses `rules_version` and nothing else of the two; an `ETag` alone uses it too.
    #[test]
    fn each_contract_uses_exactly_its_feature() {
        for (ir, _) in compiled_version_contracts() {
            let used = ir.features_used(&[ServiceId(0)]);
            assert!(used.contains(&Feature::RulesVersion), "{used:?}");
            assert!(!used.contains(&Feature::Idempotency), "{used:?}");
        }
        for (ir, _) in compiled_idempotency_contracts() {
            let used = ir.features_used(&[ServiceId(0)]);
            assert!(used.contains(&Feature::Idempotency), "{used:?}");
            assert!(!used.contains(&Feature::RulesVersion), "{used:?}");
        }
    }

    /// The gate names the first use, and a generator that opts in is let through.
    #[test]
    fn the_gate_points_at_the_first_use_and_opting_in_lets_it_through() {
        let all = Capabilities::all_except(&[]);
        for (ir, line) in compiled_version_contracts() {
            let found = gate(&ir, &all);
            assert_eq!(found.len(), 1, "{found:?}");
            assert_eq!(found[0].origin.as_ref().map(|o| o.line), Some(line));
            assert!(gate(&ir, &all.clone().with(Feature::RulesVersion)).is_empty());
        }
        for (ir, line) in compiled_idempotency_contracts() {
            let found = gate(&ir, &all);
            assert_eq!(found.len(), 1, "{found:?}");
            assert_eq!(found[0].origin.as_ref().map(|o| o.line), Some(line));
            assert!(gate(&ir, &all.clone().with(Feature::Idempotency)).is_empty());
        }
    }
}
