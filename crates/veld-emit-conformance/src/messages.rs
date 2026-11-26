//! Conformance checks for message catalogs (`spec/platform.md` section 22).
//!
//! The feature `messages` is opt-in. A generator that does not know it would send the contract's
//! text where a catalog was meant to be used, so until it declares `messages` it must refuse a
//! member that has catalogs with `E0801`, in every role, at the first catalog file.

use veld_codegen::{DynGenerator, GenRequest, Role, check_capabilities};
use veld_ir::{Feature, Ir, ServiceId};

use crate::compile;

const SOURCE: &str = "model Room {
  id: uuid
  nights: int
  rule atLeastOne: nights >= 1
    message \"A stay is at least one night\"
}

service Rooms at \"/rooms\" {
  post \"/\" book(body: Room) -> 201 Room
}
";

/// Where the first catalog file is: the directory and then `ar.toml`, line 1.
pub const CATALOG_FILE: &str = "messages/ar.toml";

/// The contract of this module with two catalogs, `ar` and `pt-br`, as the compiler puts them in
/// the IR (`Package.messages`).
///
/// # Panics
/// When the contract of this crate does not compile.
#[must_use]
pub fn compiled_messages_contract() -> Ir {
    let mut ir = compile("compiled_messages", SOURCE);
    let key = "app/Room.atLeastOne".to_owned();
    let catalogs = [
        ("ar", "إقامة ليلة واحدة على الأقل"),
        ("pt-br", "Pelo menos uma noite"),
    ];
    for p in ir.packages.iter_mut().filter(|p| p.name == "app") {
        for (tag, text) in catalogs {
            p.messages
                .entry(tag.to_owned())
                .or_default()
                .insert(key.clone(), text.to_owned());
        }
        p.messages_dir = Some("messages".to_owned());
    }
    ir
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

/// The generator must not declare `messages` and must refuse a member that has catalogs, in every
/// role: the gate reports exactly one `E0801` that names the feature, at the first catalog file
/// (`messages/ar.toml`, line 1), and `generate` answers `E0801` only and makes no files.
///
/// # Errors
/// A description of the first role for which the generator does not behave so.
pub fn check_messages_is_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    let ir = compiled_messages_contract();
    for role in generator.roles() {
        let caps = generator.capabilities(*role);
        if caps.supports(Feature::Messages) {
            return Err(format!("{}: declares `messages`", role.name()));
        }
        let req = request(&ir, *role);
        let gate = check_capabilities(&ir, &req, &caps);
        let mine: Vec<_> = gate
            .iter()
            .filter(|d| d.message.contains("`messages`"))
            .collect();
        let at_the_file = mine.len() == 1
            && mine.iter().all(|d| {
                d.code == "E0801"
                    && d.origin
                        .as_ref()
                        .is_some_and(|o| o.file == CATALOG_FILE && o.line == 1)
            });
        if !at_the_file || gate.iter().any(|d| d.code != "E0801") {
            return Err(format!(
                "{}: the gate reported {gate:?}, wanted one E0801 for `messages` at {CATALOG_FILE}:1",
                role.name()
            ));
        }
        match generator.generate_dyn(&req, &crate::options()) {
            Err(d) if !d.is_empty() && d.iter().all(|d| d.code == "E0801") => {}
            other => {
                return Err(format!(
                    "{}: generate did not answer E0801: {other:?}",
                    role.name()
                ));
            }
        }
    }
    Ok(())
}

/// The generator declares `messages` in every role, the gate lets the member through and
/// `generate` makes files for each.
///
/// # Errors
/// A description of the first role for which the generator does not behave so.
pub fn check_messages_are_generated(generator: &dyn DynGenerator) -> Result<(), String> {
    let ir = compiled_messages_contract();
    for role in generator.roles() {
        let caps = generator.capabilities(*role);
        if !caps.supports(Feature::Messages) {
            return Err(format!("{}: does not declare `messages`", role.name()));
        }
        let req = request(&ir, *role);
        let gate = check_capabilities(&ir, &req, &caps);
        if !gate.is_empty() {
            return Err(format!("{}: the gate reported {gate:?}", role.name()));
        }
        match generator.generate_dyn(&req, &crate::options()) {
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
    use veld_codegen::Capabilities;

    use super::*;

    /// A member with catalogs uses `messages`, and the contract without them does not.
    #[test]
    fn only_a_member_with_catalogs_uses_the_feature() {
        let ir = compiled_messages_contract();
        assert!(
            ir.features_used(&[ServiceId(0)])
                .contains(&Feature::Messages)
        );
        let bare = compile("bare_messages", SOURCE);
        assert!(
            !bare
                .features_used(&[ServiceId(0)])
                .contains(&Feature::Messages)
        );
    }

    /// The gate names the catalog file, and a generator that opts in is let through.
    #[test]
    fn the_gate_names_the_first_catalog_and_opting_in_lets_it_through() {
        let ir = compiled_messages_contract();
        let all = Capabilities::all_except(&[]);
        let found = check_capabilities(&ir, &request(&ir, Role::Server), &all);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(
            found[0].origin.as_ref().map(|o| (o.file.as_str(), o.line)),
            Some((CATALOG_FILE, 1))
        );
        let with = all.with(Feature::Messages);
        assert!(check_capabilities(&ir, &request(&ir, Role::Server), &with).is_empty());
    }
}
