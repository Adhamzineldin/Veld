//! Conformance checks for retention (`spec/platform.md` section 23).
//!
//! The feature `retention` is opt-in and a server's own: a server target with `@retain` models
//! gets `retentionPlan` and `run<Target>Retention`, and a client generates nothing for it. Until a
//! generator declares `retention` for its server role it must refuse such a contract there with
//! `E0801`, at the first retained model, and it must go on generating a client as before.

use veld_codegen::{DynGenerator, GenRequest, Role, check_capabilities};
use veld_ir::{Feature, Ir, ServiceId};

use crate::compile;

const SOURCE: &str = "@retain(days(30), since: checkedOutAt, then: anonymize)
model Guest {
  id: uuid
  name: string @pii
  checkedOutAt?: datetime
}

service Guests at \"/guests\" {
  get \"/{id}\" getGuest(id: uuid) -> Guest | NotFound
}
";

/// The contract of this module: a service whose model has `@retain`.
///
/// # Panics
/// When the contract of this crate does not compile.
#[must_use]
pub fn compiled_retention_contract() -> Ir {
    compile("compiled_retention", SOURCE)
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

/// The line of the retained model: where `E0801` points.
fn model_line(ir: &Ir) -> Option<u32> {
    ir.retention_plan(&[ServiceId(0)])
        .first()
        .map(|e| ir.decl(e.decl).origin.line)
}

/// The server role must not declare `retention` and must refuse the contract: the gate reports
/// exactly one `E0801` that names the feature, at the retained model, and `generate` answers
/// `E0801` only and makes no files. The client role must not be affected: it does not use the
/// feature, the gate lets the contract through and `generate` makes files.
///
/// # Errors
/// A description of the first role for which the generator does not behave so.
pub fn check_retention_is_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    let ir = compiled_retention_contract();
    let line = model_line(&ir).ok_or("the contract has no retained model")?;
    for role in generator.roles() {
        let caps = generator.capabilities(*role);
        let req = request(&ir, *role);
        let gate = check_capabilities(&ir, &req, &caps);
        match role {
            Role::Server => refused(generator, *role, &req, &gate, caps.supports(Feature::Retention), line)?,
            Role::Client => untouched(generator, &req, &gate)?,
        }
    }
    Ok(())
}

fn refused(
    generator: &dyn DynGenerator,
    role: Role,
    req: &GenRequest<'_>,
    gate: &[veld_codegen::GenDiagnostic],
    declares: bool,
    line: u32,
) -> Result<(), String> {
    if declares {
        return Err(format!("{}: declares `retention`", role.name()));
    }
    let mine: Vec<_> = gate
        .iter()
        .filter(|d| d.message.contains("`retention`"))
        .collect();
    let at_the_model = mine.len() == 1
        && mine
            .iter()
            .all(|d| d.code == "E0801" && d.origin.as_ref().map(|o| o.line) == Some(line));
    if !at_the_model || gate.iter().any(|d| d.code != "E0801") {
        return Err(format!(
            "{}: the gate reported {gate:?}, wanted one E0801 for `retention` at line {line}",
            role.name()
        ));
    }
    match generator.generate_dyn(req, &serde_json::Value::Null) {
        Err(d) if !d.is_empty() && d.iter().all(|d| d.code == "E0801") => Ok(()),
        other => Err(format!(
            "{}: generate did not answer E0801: {other:?}",
            role.name()
        )),
    }
}

fn untouched(
    generator: &dyn DynGenerator,
    req: &GenRequest<'_>,
    gate: &[veld_codegen::GenDiagnostic],
) -> Result<(), String> {
    if !gate.is_empty() {
        return Err(format!("client: the gate reported {gate:?}"));
    }
    match generator.generate_dyn(req, &serde_json::Value::Null) {
        Ok(files) if !files.is_empty() => Ok(()),
        other => Err(format!("client: generate did not make files: {other:?}")),
    }
}

/// The generator declares `retention` for its server role, the gate lets the contract through in
/// every role and `generate` makes files for each.
///
/// # Errors
/// A description of the first role for which the generator does not behave so.
pub fn check_retention_is_generated(generator: &dyn DynGenerator) -> Result<(), String> {
    let ir = compiled_retention_contract();
    for role in generator.roles() {
        let caps = generator.capabilities(*role);
        if *role == Role::Server && !caps.supports(Feature::Retention) {
            return Err("server: does not declare `retention`".to_owned());
        }
        let req = request(&ir, *role);
        let gate = check_capabilities(&ir, &req, &caps);
        if !gate.is_empty() {
            return Err(format!("{}: the gate reported {gate:?}", role.name()));
        }
        match generator.generate_dyn(&req, &serde_json::Value::Null) {
            Ok(files) if !files.is_empty() => {}
            other => {
                return Err(format!(
                    "{}: generate did not make files: {other:?}",
                    role.name()
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use veld_codegen::{Capabilities, checked_features};

    use super::*;

    /// A server uses the feature; a client and the contract without `@retain` do not.
    #[test]
    fn only_a_server_with_a_retained_model_uses_the_feature() {
        let ir = compiled_retention_contract();
        let server = checked_features(&ir, &request(&ir, Role::Server));
        let client = checked_features(&ir, &request(&ir, Role::Client));
        assert!(server.contains(&Feature::Retention), "{server:?}");
        assert!(!client.contains(&Feature::Retention), "{client:?}");
        let bare = compile("bare_retention", &SOURCE.replace(
            "@retain(days(30), since: checkedOutAt, then: anonymize)\n",
            "",
        ));
        assert!(!checked_features(&bare, &request(&bare, Role::Server)).contains(&Feature::Retention));
    }

    /// A service that only calls the retained one does not use retention itself.
    #[test]
    fn a_consumed_service_does_not_count() {
        let ir = compiled_retention_contract();
        let mut req = request(&ir, Role::Server);
        req.consumes = std::mem::take(&mut req.services);
        assert!(!checked_features(&ir, &req).contains(&Feature::Retention));
    }

    /// The gate names the model, and a generator that opts in is let through.
    #[test]
    fn the_gate_points_at_the_model_and_opting_in_lets_it_through() {
        let ir = compiled_retention_contract();
        let line = model_line(&ir).unwrap();
        let all = Capabilities::all_except(&[]);
        let server = request(&ir, Role::Server);
        let found = check_capabilities(&ir, &server, &all);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].origin.as_ref().map(|o| o.line), Some(line));
        let with = all.with(Feature::Retention);
        assert!(check_capabilities(&ir, &server, &with).is_empty());
        assert!(check_capabilities(&ir, &request(&ir, Role::Client), &Capabilities::all_except(&[])).is_empty());
    }
}
