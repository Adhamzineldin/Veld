//! Conformance checks for sagas (`spec/platform.md` sections 17 and 18).
//!
//! `sagas` is an opt-in feature. A generator that does not know it would leave the endpoint of a
//! saga without an implementation, and a client would call an operation nobody runs, so until it
//! declares `sagas` it must refuse a contract with a saga with `E0801`, in every role.

use veld_codegen::{DynGenerator, GenRequest, Role, check_capabilities};
use veld_ir::{Feature, Ir, ServiceId};

use crate::compile;

/// The shape of the spec: a saga of three steps over other services of the package, two of them
/// with a compensation.
const CHECKOUT_SOURCE: &str = "model Hold { id: uuid }
model Pay { id: uuid }
model Booking { id: uuid }
model Req { roomId: uuid }
error RoomFull 409 { capacity: int }

service Rooms at \"/rooms\" {
  @idempotent
  post \"/hold\" hold(body: Req) -> Hold | RoomFull
  @idempotent
  post \"/release/{id}\" release(id: uuid) -> 204
}

service Payments at \"/payments\" {
  @idempotent
  post \"/charge\" charge(body: Req) -> Pay
  @idempotent
  post \"/refund/{id}\" refund(id: uuid) -> 204
}

service Bookings at \"/bookings\" {
  @idempotent
  post \"/confirm/{id}\" confirm(id: uuid) -> Booking
}

service Checkout at \"/checkout\" {
  saga post \"/\" checkout(body: Req) -> Booking | RoomFull {
    step hold = Rooms.hold(body: { roomId: body.roomId }) compensate Rooms.release(id: hold.id)
    step charge = Payments.charge(body: body) compensate Payments.refund(id: charge.id)
    step confirm = Bookings.confirm(id: hold.id)
    timeout minutes(5)
  }
}
";

/// A saga of one step with no compensation, on a service that has auth.
const SMALL_SOURCE: &str = "model Order { id: uuid }
model Receipt { id: uuid }

service Shop at \"/shop\" {
  @idempotent
  post \"/receipt\" receipt(body: Order) -> Receipt
}

@auth(bearer)
service Orders at \"/orders\" {
  saga post \"/\" place(body: Order) -> Receipt {
    step receipt = Shop.receipt(body: body)
    timeout seconds(30)
  }
}
";

/// The contracts that use `sagas`, each with the line of the saga endpoint.
const SAGA_CASES: [(&str, &str, &str); 2] = [
    ("compiled_saga_checkout", CHECKOUT_SOURCE, "saga post"),
    ("compiled_saga_small", SMALL_SOURCE, "saga post"),
];

/// The two contracts that use `sagas`, with the line of the first use in each.
///
/// # Panics
/// When a contract of this crate does not compile.
#[must_use]
pub fn compiled_saga_contracts() -> Vec<(Ir, u32)> {
    SAGA_CASES
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

fn request(ir: &Ir, role: Role) -> GenRequest<'_> {
    GenRequest {
        ir,
        role,
        services: ir.services.iter().map(|s| s.id).collect::<Vec<ServiceId>>(),
        consumes: vec![],
        framework: None,
    }
}

/// The generator must not declare `sagas` and must refuse a contract with a saga, in every role:
/// the gate reports exactly one `E0801` that names the feature, at the saga endpoint, and
/// `generate` answers `E0801` only and makes no files.
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_sagas_are_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    for (ir, line) in compiled_saga_contracts() {
        for role in generator.roles() {
            if generator.capabilities(*role).supports(Feature::Sagas) {
                return Err(format!("{}: declares `sagas`", role.name()));
            }
            refuses(generator, &ir, *role, line)
                .map_err(|e| format!("{} / line {line}: {e}", role.name()))?;
        }
    }
    Ok(())
}

/// The generator declares `sagas` in every role, the gate lets the contracts through and
/// `generate` makes files for each.
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_sagas_are_generated(generator: &dyn DynGenerator) -> Result<(), String> {
    for (ir, line) in compiled_saga_contracts() {
        for role in generator.roles() {
            let caps = generator.capabilities(*role);
            if !caps.supports(Feature::Sagas) {
                return Err(format!("{}: does not declare `sagas`", role.name()));
            }
            let req = request(&ir, *role);
            let gate = check_capabilities(&ir, &req, &caps);
            if !gate.is_empty() {
                return Err(format!(
                    "{} / line {line}: the gate reported {gate:?}",
                    role.name()
                ));
            }
            match generator.generate_dyn(&req, &serde_json::Value::Null) {
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

fn refuses(generator: &dyn DynGenerator, ir: &Ir, role: Role, line: u32) -> Result<(), String> {
    let req = request(ir, role);
    let gate = check_capabilities(ir, &req, &generator.capabilities(role));
    let mine: Vec<_> = gate
        .iter()
        .filter(|d| d.message.contains("`sagas`"))
        .collect();
    let at_first_use = mine.len() == 1
        && mine
            .iter()
            .all(|d| d.code == "E0801" && d.origin.as_ref().map(|o| o.line) == Some(line));
    if !at_first_use || gate.iter().any(|d| d.code != "E0801") {
        return Err(format!(
            "the gate reported {gate:?}, wanted one E0801 for `sagas` at line {line}"
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

    /// A saga uses `sagas`, and the services it calls use nothing of it.
    #[test]
    fn the_saga_uses_the_feature_and_what_it_calls_does_not() {
        for (ir, _) in compiled_saga_contracts() {
            let saga = ir
                .services
                .iter()
                .find(|s| s.endpoints.iter().any(|e| e.saga.is_some()));
            let others: Vec<ServiceId> = ir
                .services
                .iter()
                .filter(|s| s.endpoints.iter().all(|e| e.saga.is_none()))
                .map(|s| s.id)
                .collect();
            let Some(saga) = saga else {
                panic!("no saga in the contract");
            };
            assert!(ir.features_used(&[saga.id]).contains(&Feature::Sagas));
            assert!(!ir.features_used(&others).contains(&Feature::Sagas));
        }
    }

    /// The gate names the saga endpoint, and a generator that opts in is let through.
    #[test]
    fn the_gate_points_at_the_saga_and_opting_in_lets_it_through() {
        let all = Capabilities::all_except(&[]);
        for (ir, line) in compiled_saga_contracts() {
            let req = request(&ir, Role::Server);
            let found = check_capabilities(&ir, &req, &all);
            let sagas: Vec<_> = found
                .iter()
                .filter(|d| d.message.contains("`sagas`"))
                .collect();
            assert_eq!(sagas.len(), 1, "{found:?}");
            assert_eq!(sagas[0].origin.as_ref().map(|o| o.line), Some(line));
            // The implicit `@idempotent` of the saga is reported too, once.
            assert!(found.iter().any(|d| d.message.contains("`idempotency`")));
            let opted = all.clone().with(Feature::Sagas).with(Feature::Idempotency);
            assert!(check_capabilities(&ir, &req, &opted).is_empty());
        }
    }
}
