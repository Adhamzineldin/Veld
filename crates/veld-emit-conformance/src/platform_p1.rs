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

/// A tenant-scoped model that an endpoint returns: the first use is the `@tenant` field.
const TENANT_SOURCE: &str = "model User {
  id: uuid
  tenantId?: uuid @tenant
}

principal User

model Booking {
  id: uuid
  tenantId: uuid @tenant
}

@auth(bearer)
service Bookings at \"/bookings\" {
  get \"/{id}\" getBooking(id: uuid) -> Booking
}
";

/// A tenant-scoped model that an endpoint only loads as a fact.
const TENANT_FACT_SOURCE: &str = "model User {
  id: uuid
  tenantId?: uuid @tenant
}

principal User

model Booking {
  id: uuid
  tenantId: uuid @tenant
}

@auth(bearer)
service Bookings at \"/bookings\" {
  post \"/{id}/touch\" touch(id: uuid) -> 204 {
    fact booking: Booking = id
  }
}
";

/// An event with a `@tenant` field: the server publishes it with `veldtenant`.
const TENANT_EVENT_SOURCE: &str = "event BookingPaid {
  tenantId: uuid @tenant
  bookingId: uuid
}

channel PaidEvents { carries BookingPaid }

service Bookings at \"/bookings\" {
  publishes PaidEvents
  get \"/\" ping() -> 204
}
";

/// `@audited` on an endpoint: the first use is the endpoint line.
const AUDITED_SOURCE: &str = "model Room {
  id: uuid
}

service Rooms at \"/rooms\" {
  @audited
  get \"/{id}\" getRoom(id: uuid) -> Room
}
";

/// `@audited` on a service only: it reaches the endpoint, whose line is the first use.
const AUDITED_SERVICE_SOURCE: &str = "model Room {
  id: uuid
}

@audited
service Rooms at \"/rooms\" {
  get \"/{id}\" getRoom(id: uuid) -> Room
}
";

/// `@audited` on a `ws` endpoint, which is `E0801` in every generator in this version: the
/// feature is refused at the endpoint like any other use.
const AUDITED_WS_SOURCE: &str = "model Msg {
  text: string
}

service Chat at \"/chat\" {
  @audited
  ws \"/room\" room(body: stream Msg) -> stream Msg
}
";

/// `@limit` on an endpoint: the first use is the endpoint line.
const LIMIT_SOURCE: &str = "model User {
  id: uuid
}

principal User

model Room {
  id: uuid
}

@auth(bearer)
service Rooms at \"/rooms\" {
  @limit(10, per: minute, by: user)
  get \"/{id}\" getRoom(id: uuid) -> Room
}
";

/// `@limit` on a service only, by address: it reaches the endpoint, whose line is the first use.
const LIMIT_SERVICE_SOURCE: &str = "model Room {
  id: uuid
}

@limit(100, per: hour, by: ip)
service Rooms at \"/rooms\" {
  get \"/{id}\" getRoom(id: uuid) -> Room
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
    refused_by(
        generator,
        Feature::Visibility,
        &compiled_visibility_contracts(),
        |_| false,
    )
}

/// The contracts that use `tenancy`, with the line of the first use in each.
///
/// # Panics
/// When a contract of this crate does not compile.
#[must_use]
pub fn compiled_tenancy_contracts() -> Vec<(Ir, u32)> {
    [
        ("compiled_tenant", TENANT_SOURCE, "tenantId: uuid @tenant\n"),
        (
            "compiled_tenant_fact",
            TENANT_FACT_SOURCE,
            "tenantId: uuid @tenant\n",
        ),
        (
            "compiled_tenant_event",
            TENANT_EVENT_SOURCE,
            "tenantId: uuid @tenant",
        ),
    ]
    .iter()
    .map(|(name, source, needle)| (compile(name, source), tenant_line(source, needle)))
    .collect()
}

/// The line of the `@tenant` field of the model or event: the last one in the source, after the
/// principal's, which no generator needs to implement anything for.
fn tenant_line(source: &str, _needle: &str) -> u32 {
    let at = source
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains("@tenant"))
        .map(|(i, _)| i)
        .last();
    at.and_then(|i| u32::try_from(i + 1).ok()).unwrap_or(0)
}

/// The generator must not declare `tenancy` and must refuse a contract with a `@tenant` field that
/// a server loads, returns or publishes. A client is asked too when an endpoint returns a model
/// with a `@tenant` field: the server sets the field, so a request must not carry it.
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_tenancy_is_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    refused_by(
        generator,
        Feature::Tenancy,
        &compiled_tenancy_contracts(),
        client_reaches_tenant,
    )
}

/// The contracts that use `audit`, with the line of the first endpoint that is audited.
///
/// # Panics
/// When a contract of this crate does not compile.
#[must_use]
pub fn compiled_audit_contracts() -> Vec<(Ir, u32)> {
    [
        ("compiled_audited", AUDITED_SOURCE, "get \"/{id}\""),
        (
            "compiled_audited_service",
            AUDITED_SERVICE_SOURCE,
            "get \"/{id}\"",
        ),
        ("compiled_audited_ws", AUDITED_WS_SOURCE, "ws \"/room\""),
    ]
    .iter()
    .map(|(name, source, needle)| (compile(name, source), line_of(source, needle)))
    .collect()
}

/// The generator must not declare `audit` and must refuse a contract with an audited endpoint as
/// a server (see [`check_visibility_is_refused`]); a client is not asked. A generator that does
/// declare it must still refuse the `ws` endpoint of the third contract.
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_audit_is_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    refused_by(
        generator,
        Feature::Audit,
        &compiled_audit_contracts(),
        |_| false,
    )
}

/// The contracts that use `limits`, with the line of the first endpoint that has a limit.
///
/// # Panics
/// When a contract of this crate does not compile.
#[must_use]
pub fn compiled_limit_contracts() -> Vec<(Ir, u32)> {
    [
        ("compiled_limit", LIMIT_SOURCE, "get \"/{id}\""),
        (
            "compiled_limit_service",
            LIMIT_SERVICE_SOURCE,
            "get \"/{id}\"",
        ),
    ]
    .iter()
    .map(|(name, source, needle)| (compile(name, source), line_of(source, needle)))
    .collect()
}

/// The generator must not declare `limits` and must refuse a contract with a limit **in every
/// role**: a client waits and sends the request again, so it is asked too. The gate reports
/// exactly one `E0801` that names the feature, at the first use, and `generate` answers `E0801`
/// only and makes no files.
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_limits_are_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    refused_by(
        generator,
        Feature::Limits,
        &compiled_limit_contracts(),
        |_| true,
    )
}

/// The generator must declare `visibility` as a server and generate the contracts with a
/// `@visible` field; a client is not asked and still generates.
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_visibility_is_generated(generator: &dyn DynGenerator) -> Result<(), String> {
    generated_by(
        generator,
        Feature::Visibility,
        &compiled_visibility_contracts(),
        |_| false,
    )
}

/// The generator must declare `tenancy` as a server, and as a client when an endpoint returns a
/// tenant-scoped model, and generate the contracts with a `@tenant` field.
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_tenancy_is_generated(generator: &dyn DynGenerator) -> Result<(), String> {
    generated_by(
        generator,
        Feature::Tenancy,
        &compiled_tenancy_contracts(),
        client_reaches_tenant,
    )
}

/// The generator must declare `audit` as a server and generate the contracts with an audited
/// endpoint (an endpoint, or a service). The `ws` contract is not among them: it is refused with
/// `E0801` at the endpoint whatever the generator declares ([`check_audited_ws_is_refused`]).
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_audit_is_generated(generator: &dyn DynGenerator) -> Result<(), String> {
    let contracts: Vec<(Ir, u32)> = compiled_audit_contracts().into_iter().take(2).collect();
    generated_by(generator, Feature::Audit, &contracts, |_| false)
}

/// A generator that declares `audit` must still answer `E0801`, at the endpoint, for an audited
/// `ws` endpoint (`spec/platform.md` section 9.1), and make no files.
///
/// # Errors
/// A description of the first role for which the generator does not behave so.
pub fn check_audited_ws_is_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    let contracts = compiled_audit_contracts();
    let Some((ir, line)) = contracts.get(2) else {
        return Err("the contract of the audited ws endpoint is missing".to_string());
    };
    for role in generator.roles().iter().filter(|r| **r == Role::Server) {
        let req = request(ir, *role);
        match generator.generate_dyn(&req, &crate::options()) {
            Err(d)
                if !d.is_empty()
                    && d.iter().all(|d| d.code == "E0801")
                    && d.iter()
                        .any(|d| d.origin.as_ref().map(|o| o.line) == Some(*line)) => {}
            other => {
                return Err(format!(
                    "{} / line {line}: an audited ws endpoint was not refused at the endpoint: {other:?}",
                    role.name()
                ));
            }
        }
    }
    Ok(())
}

/// The generator must declare `limits` in every role and generate the contracts with a `@limit`.
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_limits_are_generated(generator: &dyn DynGenerator) -> Result<(), String> {
    generated_by(
        generator,
        Feature::Limits,
        &compiled_limit_contracts(),
        |_| true,
    )
}

/// A role that is asked about `feature` declares it, passes the gate and makes files; a client
/// that is not asked still makes files and is not asked by the gate.
fn generated_by(
    generator: &dyn DynGenerator,
    feature: Feature,
    contracts: &[(Ir, u32)],
    client_asked: fn(&Ir) -> bool,
) -> Result<(), String> {
    let name = format!("`{}`", feature.name());
    for (ir, line) in contracts {
        for role in generator.roles() {
            let req = request(ir, *role);
            let caps = generator.capabilities(*role);
            let gate = check_capabilities(ir, &req, &caps);
            let asked = *role != Role::Client || client_asked(ir);
            if asked && !caps.supports(feature) {
                return Err(format!("{}: does not declare {name}", role.name()));
            }
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

fn request(ir: &Ir, role: Role) -> GenRequest<'_> {
    GenRequest {
        ir,
        role,
        services: ir.services.iter().map(|s| s.id).collect::<Vec<ServiceId>>(),
        consumes: vec![],
        framework: None,
    }
}

/// A server role refuses `feature` at the first use. A client role refuses it when
/// `client_asked` says the contract asks a client, and is not asked about it otherwise.
fn refused_by(
    generator: &dyn DynGenerator,
    feature: Feature,
    contracts: &[(Ir, u32)],
    client_asked: fn(&Ir) -> bool,
) -> Result<(), String> {
    let name = format!("`{}`", feature.name());
    for (ir, line) in contracts {
        for role in generator.roles() {
            let req = request(ir, *role);
            let gate = check_capabilities(ir, &req, &generator.capabilities(*role));
            let asked = *role != Role::Client || client_asked(ir);
            if !asked {
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

/// A client reaches a tenant-scoped model when an endpoint's response does.
fn client_reaches_tenant(ir: &Ir) -> bool {
    let services: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
    ir.reachable_decls(&services)
        .into_iter()
        .any(|id| ir.decl(id).tenant_field().is_some())
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
    match generator.generate_dyn(req, &crate::options()) {
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
    fn each_tenancy_contract_uses_the_feature_as_a_server() {
        for (ir, _) in compiled_tenancy_contracts() {
            let req = request(&ir, Role::Server);
            let gate = check_capabilities(&ir, &req, &veld_codegen::Capabilities::all_except(&[]));
            assert!(
                gate.iter().any(|d| d.message.contains("`tenancy`")),
                "{gate:?}"
            );
        }
    }

    #[test]
    fn each_limit_contract_uses_the_feature_in_both_roles() {
        for (ir, _) in compiled_limit_contracts() {
            for role in [Role::Server, Role::Client] {
                let req = request(&ir, role);
                let gate =
                    check_capabilities(&ir, &req, &veld_codegen::Capabilities::all_except(&[]));
                assert!(
                    gate.iter().any(|d| d.message.contains("`limits`")),
                    "{role:?}: {gate:?}"
                );
            }
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
