//! Conformance checks for the channel annotations of phase E2 (`spec/events.md` section 10.8).
//!
//! `@retry`, `@deadLetter` and `@ordered` change what a consumer does with a message. The feature
//! `events_delivery` is opt-in: a generator that has not built them must refuse a service that
//! publishes or subscribes to such a channel with `E0801`, at the channel, and never generate a
//! consumer that silently ignores them. A client generates nothing for events and keeps its bytes.

use veld_codegen::{DynGenerator, FileSet, GenRequest, Role, check_capabilities};
use veld_ir::{Feature, Ir, ServiceId};

use crate::compile;
use crate::events::FIXTURE_SOURCE;

/// The annotations the checks try, one at a time and all together.
const VARIANTS: [&str; 7] = [
    "@retry(max: 3, backoff: fixed)\n",
    "@retry(max: 0)\n",
    "@deadLetter\n",
    "@ordered\n",
    "@retry(max: 5, backoff: exponential)\n@deadLetter\n",
    "@ordered\n@deadLetter\n",
    "@retry(max: 2)\n@deadLetter\n@ordered\n",
];

const CHANNEL_LINE: &str = "channel GreetingEvents {";

/// The events contract of `events.rs` with `annotations` written before its channel.
fn source(annotations: &str) -> String {
    FIXTURE_SOURCE.replace(CHANNEL_LINE, &format!("{annotations}{CHANNEL_LINE}"))
}

/// The line of the channel declaration in `source`.
fn channel_line(source: &str) -> Result<u32, String> {
    let at = source.lines().position(|l| l.starts_with(CHANNEL_LINE));
    at.and_then(|i| u32::try_from(i + 1).ok())
        .ok_or_else(|| "the events contract has no channel line".to_string())
}

/// A contract with the annotations `annotations` on its channel, a publishing service `Hello` and
/// a subscribing service `Audit`.
#[must_use]
pub fn compiled_events_delivery_contract(annotations: &str) -> Ir {
    compile("compiled_events_delivery", &source(annotations))
}

fn service_id(ir: &Ir, name: &str) -> Result<ServiceId, String> {
    ir.services
        .iter()
        .find(|s| s.name == name)
        .map(|s| s.id)
        .ok_or_else(|| format!("the events contract has no service `{name}`"))
}

/// A server generator that has not built phase E2 must not declare `events_delivery`, and for a
/// service that publishes or subscribes to a channel with `@retry`, `@deadLetter` or `@ordered`
/// the gate reports exactly one `E0801` that names the feature, at the channel, and `generate`
/// answers `E0801` only and makes no files. A service of the same contract without the
/// annotations must still pass the gate. Roles other than the server are not checked here.
///
/// # Errors
/// A description of the first role, annotations and service for which the generator does not
/// behave so.
pub fn check_events_delivery_is_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    for role in generator.roles().iter().filter(|r| **r == Role::Server) {
        if generator
            .capabilities(*role)
            .supports(Feature::EventsDelivery)
        {
            return Err(format!("{}: declares `events_delivery`", role.name()));
        }
        for annotations in VARIANTS {
            let src = source(annotations);
            let line = channel_line(&src)?;
            let ir = compile("events_delivery_refused", &src);
            for name in ["Hello", "Audit"] {
                let services = vec![service_id(&ir, name)?];
                refuses(generator, &ir, *role, services, line).map_err(|e| {
                    format!("{} / {name} / {:?}: {e}", role.name(), annotations.trim())
                })?;
            }
        }
    }
    Ok(())
}

/// A server generator that has built phase E2 must declare `events_delivery`, let the gate pass
/// the publishing and the subscribing service of a channel with `@retry`, `@deadLetter` and
/// `@ordered` (each alone and together) and generate at least one file for each, the same files on
/// every run.
///
/// # Errors
/// A description of the first role, annotations and service for which the generator does not
/// behave so.
pub fn check_events_delivery_is_generated(generator: &dyn DynGenerator) -> Result<(), String> {
    for role in generator.roles().iter().filter(|r| **r == Role::Server) {
        if !generator
            .capabilities(*role)
            .supports(Feature::EventsDelivery)
        {
            return Err(format!(
                "{}: does not declare `events_delivery`",
                role.name()
            ));
        }
        for annotations in VARIANTS {
            let ir = compile("events_delivery_generated", &source(annotations));
            for name in ["Hello", "Audit"] {
                let services = vec![service_id(&ir, name)?];
                generates(generator, &ir, *role, services).map_err(|e| {
                    format!("{} / {name} / {:?}: {e}", role.name(), annotations.trim())
                })?;
            }
        }
    }
    Ok(())
}

fn generates(
    generator: &dyn DynGenerator,
    ir: &Ir,
    role: Role,
    services: Vec<ServiceId>,
) -> Result<(), String> {
    let req = GenRequest {
        ir,
        role,
        services,
        consumes: vec![],
        framework: None,
    };
    let gate = check_capabilities(ir, &req, &generator.capabilities(role));
    if !gate.is_empty() {
        return Err(format!("the gate reported {gate:?}"));
    }
    let first = generator
        .generate_dyn(&req, &crate::options())
        .map_err(|d| format!("generation failed: {d:?}"))?;
    let second = generator
        .generate_dyn(&req, &crate::options())
        .map_err(|d| format!("the second generation failed: {d:?}"))?;
    if first.is_empty() || first != second {
        return Err("no files, or two runs gave different files".to_owned());
    }
    Ok(())
}

fn refuses(
    generator: &dyn DynGenerator,
    ir: &Ir,
    role: Role,
    services: Vec<ServiceId>,
    line: u32,
) -> Result<(), String> {
    let req = GenRequest {
        ir,
        role,
        services,
        consumes: vec![],
        framework: None,
    };
    let gate = check_capabilities(ir, &req, &generator.capabilities(role));
    let mine: Vec<_> = gate
        .iter()
        .filter(|d| d.message.contains("`events_delivery`"))
        .collect();
    let at_the_channel = mine.len() == 1
        && mine
            .iter()
            .all(|d| d.code == "E0801" && d.origin.as_ref().map(|o| o.line) == Some(line));
    if !at_the_channel || gate.iter().any(|d| d.code != "E0801") {
        return Err(format!(
            "the gate reported {gate:?}, wanted one E0801 for `events_delivery` at line {line}"
        ));
    }
    match generator.generate_dyn(&req, &crate::options()) {
        Err(d) if !d.is_empty() && d.iter().all(|d| d.code == "E0801") => Ok(()),
        other => Err(format!("generate did not answer E0801: {other:?}")),
    }
}

/// A client generator must generate the same files, byte for byte, with and without the
/// annotations on a channel (`spec/events.md` section 14): it generates nothing for events.
///
/// # Errors
/// A description of the first client role and annotations for which the output moves.
pub fn check_events_delivery_is_ignored_by_clients(
    generator: &dyn DynGenerator,
) -> Result<(), String> {
    let plain = compiled_events_delivery_contract("");
    for role in generator.roles().iter().filter(|r| **r == Role::Client) {
        let before = generate(generator, &plain, *role)?;
        for annotations in VARIANTS {
            let ir = compiled_events_delivery_contract(annotations);
            let req = request(&ir, *role);
            let gate = check_capabilities(&ir, &req, &generator.capabilities(*role));
            if !gate.is_empty() {
                return Err(format!(
                    "{} / {:?}: the gate reported {gate:?}",
                    role.name(),
                    annotations.trim()
                ));
            }
            let after = generate(generator, &ir, *role)?;
            if before != after {
                return Err(format!(
                    "{}: {:?} changed the client output",
                    role.name(),
                    annotations.trim()
                ));
            }
        }
    }
    Ok(())
}

fn request(ir: &Ir, role: Role) -> GenRequest<'_> {
    GenRequest {
        ir,
        role,
        services: ir.services.iter().map(|s| s.id).collect(),
        consumes: vec![],
        framework: None,
    }
}

fn generate(generator: &dyn DynGenerator, ir: &Ir, role: Role) -> Result<FileSet, String> {
    generator
        .generate_dyn(&request(ir, role), &crate::options())
        .map_err(|d| format!("generation failed: {d:?}"))
}

#[cfg(test)]
mod tests {
    use veld_codegen::{Capabilities, GenDiagnostic};

    use super::*;

    /// A generator with the capabilities `caps` whose `generate` lists the channels it sees.
    struct Fake {
        caps: Capabilities,
        role: Role,
        refuse: bool,
    }

    impl DynGenerator for Fake {
        fn id(&self) -> &'static str {
            "fake"
        }
        fn roles(&self) -> &'static [Role] {
            match self.role {
                Role::Server => &[Role::Server],
                Role::Client => &[Role::Client],
            }
        }
        fn frameworks(&self, _: Role) -> Vec<veld_codegen::FrameworkInfo> {
            Vec::new()
        }
        fn capabilities(&self, _: Role) -> Capabilities {
            self.caps.clone()
        }
        fn generate_dyn(
            &self,
            req: &GenRequest<'_>,
            _: &serde_json::Value,
        ) -> Result<FileSet, Vec<GenDiagnostic>> {
            if self.refuse {
                return Err(vec![GenDiagnostic::new("E0801", "no")]);
            }
            let mut files = FileSet::new();
            let channels: Vec<String> = req
                .ir
                .channels
                .iter()
                .map(|c| format!("{} {}", c.name, c.has_delivery_options()))
                .collect();
            files
                .add("channels.txt", channels.join("\n"))
                .map_err(|e| vec![GenDiagnostic::new("E0899", e.to_string())])?;
            Ok(files)
        }
    }

    fn server(caps: Capabilities, refuse: bool) -> Fake {
        Fake {
            caps,
            role: Role::Server,
            refuse,
        }
    }

    #[test]
    fn the_variants_compile_and_use_the_feature() {
        for annotations in VARIANTS {
            let ir = compiled_events_delivery_contract(annotations);
            assert!(ir.channels[0].has_delivery_options(), "{annotations}");
            for name in ["Hello", "Audit"] {
                let id = service_id(&ir, name).unwrap();
                assert!(
                    ir.features_used(&[id]).contains(&Feature::EventsDelivery),
                    "{name} {annotations}"
                );
            }
        }
        let plain = compiled_events_delivery_contract("");
        assert!(!plain.channels[0].has_delivery_options());
    }

    #[test]
    fn a_server_that_refuses_the_annotations_conforms() {
        let refuses = server(Capabilities::all_except(&[]), true);
        check_events_delivery_is_refused(&refuses).unwrap();
    }

    #[test]
    fn a_server_that_generates_the_annotations_conforms() {
        let supports = server(
            Capabilities::all_except(&[]).with(Feature::EventsDelivery),
            false,
        );
        check_events_delivery_is_generated(&supports).unwrap();
    }

    #[test]
    fn a_server_that_does_not_declare_the_feature_is_caught_by_the_generated_check() {
        let refuses = server(Capabilities::all_except(&[]), false);
        let err = check_events_delivery_is_generated(&refuses).unwrap_err();
        assert!(err.contains("does not declare `events_delivery`"), "{err}");
    }

    #[test]
    fn a_server_that_declares_the_feature_and_refuses_is_caught_by_the_generated_check() {
        let declares = server(
            Capabilities::all_except(&[]).with(Feature::EventsDelivery),
            true,
        );
        let err = check_events_delivery_is_generated(&declares).unwrap_err();
        assert!(err.contains("generation failed"), "{err}");
    }

    #[test]
    fn a_server_that_declares_the_feature_is_caught() {
        let claims = server(
            Capabilities::all_except(&[]).with(Feature::EventsDelivery),
            true,
        );
        let err = check_events_delivery_is_refused(&claims).unwrap_err();
        assert!(err.contains("declares `events_delivery`"), "{err}");
    }

    #[test]
    fn a_server_that_generates_anyway_is_caught() {
        let leaky = server(Capabilities::all_except(&[]), false);
        let err = check_events_delivery_is_refused(&leaky).unwrap_err();
        assert!(err.contains("generate did not answer E0801"), "{err}");
    }

    #[test]
    fn a_client_that_writes_the_annotations_is_caught() {
        let client = Fake {
            caps: Capabilities::all_except(&[]),
            role: Role::Client,
            refuse: false,
        };
        let err = check_events_delivery_is_ignored_by_clients(&client).unwrap_err();
        assert!(err.contains("changed the client output"), "{err}");
    }
}
