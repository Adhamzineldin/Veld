//! Conformance checks for derived fields (`spec/rules.md` section 17.2).
//!
//! `rules_derived` is opt-in. A generator that does not know it would leave `total` without a
//! value, so until it declares the feature it must refuse a contract with a derived field with
//! `E0801`, in every role.

use veld_codegen::{DynGenerator, GenRequest, Role, check_capabilities};
use veld_ir::{Feature, Ir, ServiceId};

use crate::compile;

/// A model that an endpoint takes and answers with.
const BODY_SOURCE: &str = "model Line {
  price: decimal
  qty: int
  total: decimal = price * qty
}

service Shop at \"/shop\" {
  post \"/\" add(body: Line) -> 201 Line
}
";

/// A model that only a fact has: no endpoint reaches it, the application loads it.
const FACT_SOURCE: &str = "model Room {
  id: uuid
  seats: int
  free: int = seats - 1
}

model Booking {
  id: uuid
}

service Rooms at \"/rooms\" {
  get \"/{id}\" book(id: uuid) -> Booking {
    fact room: Room = id
  }
}
";

/// A derived field of an error.
const ERROR_SOURCE: &str = "error OverLimit 422 {
  limit: decimal
  used: decimal
  left: decimal = limit - used
}

model Order {
  id: uuid
}

service Orders at \"/orders\" {
  post \"/\" place(body: Order) -> 201 Order | OverLimit
}
";

/// The contracts that use `rules_derived`, each with the text of the line of the first derived
/// field, and whether only a server uses the feature: a client never sees a fact.
const CASES: [(&str, &str, &str, bool); 3] = [
    (
        "compiled_derived_body",
        BODY_SOURCE,
        "total: decimal",
        false,
    ),
    ("compiled_derived_fact", FACT_SOURCE, "free: int", true),
    (
        "compiled_derived_error",
        ERROR_SOURCE,
        "left: decimal",
        false,
    ),
];

/// The contracts that use `rules_derived`, with the line of the first derived field in each.
///
/// # Panics
/// When a contract of this crate does not compile.
#[must_use]
pub fn compiled_derived_contracts() -> Vec<(Ir, u32)> {
    CASES
        .iter()
        .map(|(name, source, needle, _)| {
            let line = source
                .lines()
                .position(|l| l.contains(needle))
                .and_then(|i| u32::try_from(i + 1).ok())
                .unwrap_or(0);
            (compile(name, source), line)
        })
        .collect()
}

/// True for the contracts that only a server uses the feature for (the derived field is on a fact).
fn server_only(index: usize) -> bool {
    CASES.get(index).is_some_and(|c| c.3)
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

/// The generator must not declare `rules_derived` and must refuse a contract with a derived field,
/// in every role: the gate reports exactly one `E0801` that names the feature, at the first
/// derived field, and `generate` answers `E0801` only and makes no files.
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_derived_is_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    for (index, (ir, line)) in compiled_derived_contracts().into_iter().enumerate() {
        for role in generator.roles() {
            if generator
                .capabilities(*role)
                .supports(Feature::RulesDerived)
            {
                return Err(format!("{}: declares `rules_derived`", role.name()));
            }
            if *role == Role::Client && server_only(index) {
                continue;
            }
            refuses(generator, &ir, *role, line)
                .map_err(|e| format!("{} / line {line}: {e}", role.name()))?;
        }
    }
    Ok(())
}

/// The generator declares `rules_derived` in every role, the gate lets the contracts through and
/// `generate` makes files for each.
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_derived_is_generated(generator: &dyn DynGenerator) -> Result<(), String> {
    for (ir, line) in compiled_derived_contracts() {
        for role in generator.roles() {
            let caps = generator.capabilities(*role);
            if !caps.supports(Feature::RulesDerived) {
                return Err(format!("{}: does not declare `rules_derived`", role.name()));
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
        .filter(|d| d.message.contains("`rules_derived`"))
        .collect();
    let at_first_use = mine.len() == 1
        && mine
            .iter()
            .all(|d| d.code == "E0801" && d.origin.as_ref().map(|o| o.line) == Some(line));
    if !at_first_use || gate.iter().any(|d| d.code != "E0801") {
        return Err(format!(
            "the gate reported {gate:?}, wanted one E0801 for `rules_derived` at line {line}"
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
    fn each_contract_uses_rules_derived() {
        for (ir, _) in compiled_derived_contracts() {
            let used = ir.features_used(&[ServiceId(0)]);
            assert!(used.contains(&Feature::RulesDerived), "{used:?}");
        }
    }

    /// The gate names the first derived field, also one that only a fact has, and a generator
    /// that opts in is let through.
    #[test]
    fn the_gate_points_at_the_first_derived_field_and_opting_in_lets_it_through() {
        let all = Capabilities::all_except(&[]);
        for (ir, line) in compiled_derived_contracts() {
            let found = gate(&ir, &all);
            assert_eq!(found.len(), 1, "{found:?}");
            assert_eq!(found[0].origin.as_ref().map(|o| o.line), Some(line));
            assert!(gate(&ir, &all.clone().with(Feature::RulesDerived)).is_empty());
        }
    }

    /// A client sees a derived field too: it reads the value, and its form helper computes it.
    #[test]
    fn a_client_is_not_asked_about_a_fact_it_never_sees() {
        let all = Capabilities::all_except(&[]);
        let contracts = compiled_derived_contracts();
        let (ir, _) = &contracts[1];
        assert!(check_capabilities(ir, &request(ir, Role::Client), &all).is_empty());
        assert_eq!(
            check_capabilities(ir, &request(ir, Role::Server), &all).len(),
            1
        );
    }

    #[test]
    fn a_client_is_refused_as_well() {
        let all = Capabilities::all_except(&[]);
        let (ir, _) = &compiled_derived_contracts()[0];
        let found = check_capabilities(ir, &request(ir, Role::Client), &all);
        assert_eq!(found.len(), 1, "{found:?}");
    }
}
