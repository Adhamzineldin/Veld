//! Conformance checks for platform phase P1 (`spec/platform.md` sections 2 to 5): field
//! visibility, tenancy, the audit log and rate limits.
//!
//! All four are opt-in. A generator that does not know one would send a field that must be
//! hidden, a value of another tenant, no audit event or no limit, without a word. So until it
//! declares the feature it must refuse a contract that uses it with `E0801`, at the first use.
//! Visibility, tenancy and audit are a server's own: a client has nothing to enforce and is not
//! asked (`spec/platform.md` section 1), so those are checked for the server role only.

use veld_codegen::{DynGenerator, GenRequest, Role, check_capabilities};
use veld_ir::{Feature, Ir, ServiceId};

use crate::compile;

/// A contract with a `@visible` field: the first use is the field.
const VISIBLE_SOURCE: &str = "model User {
  id: uuid
  roles: string[]
}

principal User

model Booking {
  guestId: uuid
  guestPhone?: string @visible(when: user.id == guestId || \"support\" in user.roles)
}

@auth(bearer)
service Bookings at \"/bookings\" {
  get \"/{id}\" getBooking(id: uuid) -> Booking
}
";

/// `@visible` on a field of a declared error that an endpoint can answer with.
const VISIBLE_ERROR_SOURCE: &str = "model User {
  id: uuid
}

principal User

error Refused 409 {
  reason: string
  who?: string @visible(when: user.id == user.id)
}

@auth(bearer)
service Bookings at \"/bookings\" {
  post \"/{id}/cancel\" cancelBooking(id: uuid) -> 204 | Refused
}
";

fn line_of(source: &str, needle: &str) -> u32 {
    source
        .lines()
        .position(|l| l.contains(needle))
        .and_then(|i| u32::try_from(i + 1).ok())
        .unwrap_or(0)
}

/// The contracts that use `visibility`, with the line of the first use in each.
///
/// # Panics
/// When a contract of this crate does not compile.
#[must_use]
pub fn compiled_visibility_contracts() -> Vec<(Ir, u32)> {
    [
        ("compiled_visible", VISIBLE_SOURCE, "guestPhone?"),
        ("compiled_visible_error", VISIBLE_ERROR_SOURCE, "who?"),
    ]
    .iter()
    .map(|(name, source, needle)| (compile(name, source), line_of(source, needle)))
    .collect()
}

/// The generator must not declare `visibility` and must refuse a contract with a `@visible`
/// field as a server: the gate reports exactly one `E0801` that names the feature, at the first
/// use, and `generate` answers `E0801` only and makes no files. As a client it is not asked: the
/// gate reports nothing about `visibility`.
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_visibility_is_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    refused_by_servers(
        generator,
        Feature::Visibility,
        &compiled_visibility_contracts(),
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

/// Every server role refuses; a client role is not asked about `feature`.
fn refused_by_servers(
    generator: &dyn DynGenerator,
    feature: Feature,
    contracts: &[(Ir, u32)],
) -> Result<(), String> {
    let name = format!("`{}`", feature.name());
    for (ir, line) in contracts {
        for role in generator.roles() {
            let req = request(ir, *role);
            let gate = check_capabilities(ir, &req, &generator.capabilities(*role));
            if *role == Role::Client {
                if gate.iter().any(|d| d.message.contains(&name)) {
                    return Err(format!("client / line {line}: the gate asked about {name}"));
                }
                continue;
            }
            if generator.capabilities(*role).supports(feature) {
                return Err(format!("{}: declares {name}", role.name()));
            }
            refuses(generator, &req, &gate, &name, *line)
                .map_err(|e| format!("{} / line {line}: {e}", role.name()))?;
        }
    }
    Ok(())
}

fn refuses(
    generator: &dyn DynGenerator,
    req: &GenRequest<'_>,
    gate: &[veld_codegen::GenDiagnostic],
    name: &str,
    line: u32,
) -> Result<(), String> {
    let mine: Vec<_> = gate.iter().filter(|d| d.message.contains(name)).collect();
    let at_first_use = mine.len() == 1
        && mine
            .iter()
            .all(|d| d.code == "E0801" && d.origin.as_ref().map(|o| o.line) == Some(line));
    if !at_first_use || gate.iter().any(|d| d.code != "E0801") {
        return Err(format!(
            "the gate reported {gate:?}, wanted one E0801 for {name} at line {line}"
        ));
    }
    match generator.generate_dyn(req, &serde_json::Value::Null) {
        Err(d) if !d.is_empty() && d.iter().all(|d| d.code == "E0801") => Ok(()),
        other => Err(format!("generate did not answer E0801: {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_visibility_contract_uses_the_feature() {
        for (ir, _) in compiled_visibility_contracts() {
            let used = ir.features_used(&[ServiceId(0)]);
            assert!(used.contains(&Feature::Visibility), "{used:?}");
        }
    }

    #[test]
    fn a_client_is_not_asked_about_visibility() {
        for (ir, _) in compiled_visibility_contracts() {
            let req = request(&ir, Role::Client);
            let none = veld_codegen::Capabilities::all_except(&[]);
            let gate = check_capabilities(&ir, &req, &none);
            assert!(
                !gate.iter().any(|d| d.message.contains("`visibility`")),
                "{gate:?}"
            );
        }
    }
}
