//! Conformance checks for events (`spec/events.md` section 14).
//!
//! In phase E1 no built-in server generator serves events: it must refuse a service that
//! publishes or subscribes with `E0801`. A client generator supports the feature by generating
//! nothing: its output must not change when a contract gains events.

use veld_codegen::{DynGenerator, FileSet, GenRequest, Role, check_capabilities};
use veld_ir::{Feature, Ir, ServiceId};

use crate::compile;

/// The events part of a contract: an event, a channel that carries it, a service that publishes
/// to the channel and one that subscribes to it.
const EVENTS_SOURCE: &str = "model Greeting {
  message: string
}

/// Published when a greeting is sent.
event Greeted {
  name: string
  rule named: len(name) > 0
}

channel GreetingEvents {
  carries Greeted
  key name
}

service Hello at \"/hello\" {
  publishes GreetingEvents
  get \"/{name}\" greet(name: string) -> Greeting | NotFound
}

service Audit {
  subscribes GreetingEvents as \"audit\" {
    on Greeted -> record
  }
}
";

/// The same contract without the event, the channel, `publishes` and the service that subscribes.
const NO_EVENTS_SOURCE: &str = "model Greeting {
  message: string
}

service Hello at \"/hello\" {
  get \"/{name}\" greet(name: string) -> Greeting | NotFound
}
";

/// The source of the fixture that uses the feature `events`.
pub(crate) const FIXTURE_SOURCE: &str = EVENTS_SOURCE;

/// A contract with an event, a channel, a publishing service and a subscribing service.
#[must_use]
pub fn compiled_events_contract() -> Ir {
    compile("compiled_events", EVENTS_SOURCE)
}

fn line_of(needle: &str) -> Result<u32, String> {
    let at = EVENTS_SOURCE.lines().position(|l| l.contains(needle));
    at.and_then(|i| u32::try_from(i + 1).ok())
        .ok_or_else(|| format!("`{needle}` is not in the events contract"))
}

fn service_id(ir: &Ir, name: &str) -> Result<ServiceId, String> {
    ir.services
        .iter()
        .find(|s| s.name == name)
        .map(|s| s.id)
        .ok_or_else(|| format!("the events contract has no service `{name}`"))
}

/// A server generator must not declare `events` and must refuse a service that publishes or
/// subscribes: the gate reports exactly one `E0801` that names the feature, at the service for a
/// publisher (the IR records no origin for a `publishes` line) and at the `subscribes` line for a
/// subscriber, and `generate` answers `E0801` only and makes no files. Roles other than the server
/// are not checked here.
///
/// # Errors
/// A description of the first role and service for which the generator does not behave so.
pub fn check_events_are_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    let ir = compiled_events_contract();
    let cases = [
        ("Hello", line_of("service Hello")?),
        ("Audit", line_of("subscribes GreetingEvents")?),
    ];
    for role in generator.roles().iter().filter(|r| **r == Role::Server) {
        let caps = generator.capabilities(*role);
        if caps.supports(Feature::Events) {
            return Err(format!("{}: declares `events`", role.name()));
        }
        for (name, line) in cases {
            let services = vec![service_id(&ir, name)?];
            refuses(generator, &ir, *role, services, line)
                .map_err(|e| format!("{} / {name}: {e}", role.name()))?;
        }
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
    let at_the_line = gate.len() == 1
        && gate.iter().all(|d| {
            d.code == "E0801"
                && d.message.contains("`events`")
                && d.origin.as_ref().map(|o| o.line) == Some(line)
        });
    if !at_the_line {
        return Err(format!(
            "the gate reported {gate:?}, wanted one E0801 at line {line}"
        ));
    }
    match generator.generate_dyn(&req, &serde_json::Value::Null) {
        Err(d) if !d.is_empty() && d.iter().all(|d| d.code == "E0801") => Ok(()),
        other => Err(format!("generate did not answer E0801: {other:?}")),
    }
}

fn generate(
    generator: &dyn DynGenerator,
    ir: &Ir,
    role: Role,
    services: Vec<ServiceId>,
) -> Result<FileSet, String> {
    let req = GenRequest {
        ir,
        role,
        services,
        consumes: vec![],
        framework: None,
    };
    generator
        .generate_dyn(&req, &serde_json::Value::Null)
        .map_err(|d| format!("generation failed: {d:?}"))
}

/// The first difference between two file sets, or `None` when they are equal.
fn first_difference(a: &FileSet, b: &FileSet) -> Option<String> {
    let left: Vec<(String, Vec<u8>)> = a
        .iter()
        .map(|(p, bytes)| (p.to_string(), bytes.to_vec()))
        .collect();
    let right: Vec<(String, Vec<u8>)> = b
        .iter()
        .map(|(p, bytes)| (p.to_string(), bytes.to_vec()))
        .collect();
    if left == right {
        return None;
    }
    let paths = |v: &[(String, Vec<u8>)]| v.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>();
    if paths(&left) != paths(&right) {
        let (l, r) = (paths(&left), paths(&right));
        let only = |a: &[String], b: &[String]| -> Vec<String> {
            a.iter().filter(|p| !b.contains(p)).cloned().collect()
        };
        return Some(format!(
            "the file lists differ: only without events {:?}, only with events {:?}",
            only(&l, &r),
            only(&r, &l)
        ));
    }
    left.iter()
        .zip(&right)
        .find(|(l, r)| l.1 != r.1)
        .map(|(l, _)| format!("`{}` differs", l.0))
}

/// A client generator must generate the same files, byte for byte, for a service that gains
/// `publishes` and for a contract that gains events, channels and a service that subscribes
/// (`spec/events.md` section 14): the first service is selected in both runs. It must also accept
/// the service that only subscribes, which has no endpoints and no `at`.
///
/// # Errors
/// A description of the first client role for which the output moves.
pub fn check_events_are_ignored_by_clients(generator: &dyn DynGenerator) -> Result<(), String> {
    let plain = compile("events_ignored", NO_EVENTS_SOURCE);
    let with = compile("events_ignored", EVENTS_SOURCE);
    for role in generator.roles().iter().filter(|r| **r == Role::Client) {
        if !generator.capabilities(*role).supports(Feature::Events) {
            return Err(format!("{}: does not declare `events`", role.name()));
        }
        let hello = |ir: &Ir| service_id(ir, "Hello").map(|s| vec![s]);
        let before = generate(generator, &plain, *role, hello(&plain)?)?;
        let after = generate(generator, &with, *role, hello(&with)?)?;
        if let Some(d) = first_difference(&before, &after) {
            return Err(format!(
                "{}: events changed the client output: {d}",
                role.name()
            ));
        }
        let audit = vec![service_id(&with, "Audit")?];
        generate(generator, &with, *role, audit)
            .map_err(|e| format!("{}: a service that only subscribes: {e}", role.name()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use veld_codegen::{Capabilities, GenDiagnostic};

    use super::*;

    /// Writes one file that lists the declarations of every selected service, events included.
    struct Leaky {
        caps: Capabilities,
        role: Role,
    }

    impl DynGenerator for Leaky {
        fn id(&self) -> &'static str {
            "leaky"
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
            let names: Vec<&str> = req.ir.decls.iter().map(|d| d.name.as_str()).collect();
            let mut files = FileSet::new();
            files
                .add("all.txt", names.join("\n"))
                .map_err(|e| vec![GenDiagnostic::new("E0899", e.to_string())])?;
            Ok(files)
        }
    }

    #[test]
    fn the_contract_has_one_publisher_and_one_subscriber() {
        let ir = compiled_events_contract();
        assert!(ir.services.iter().any(|s| !s.publishes.is_empty()));
        assert!(ir.services.iter().any(|s| !s.subscriptions.is_empty()));
        assert!(compile("plain", NO_EVENTS_SOURCE).channels.is_empty());
    }

    #[test]
    fn a_server_that_claims_events_is_caught() {
        let claims = Leaky {
            caps: Capabilities::all_except(&[]),
            role: Role::Server,
        };
        let err = check_events_are_refused(&claims).unwrap_err();
        assert!(err.contains("declares `events`"), "{err}");
    }

    #[test]
    fn a_server_that_does_not_gate_events_is_caught() {
        let ungated = Leaky {
            caps: Capabilities::default(),
            role: Role::Server,
        };
        let err = check_events_are_refused(&ungated).unwrap_err();
        assert!(err.contains("did not answer E0801"), "{err}");
    }

    #[test]
    fn a_client_that_writes_events_is_caught() {
        let leaky = Leaky {
            caps: Capabilities::all_except(&[]),
            role: Role::Client,
        };
        let err = check_events_are_ignored_by_clients(&leaky).unwrap_err();
        assert!(err.contains("events changed the client output"), "{err}");
    }

    #[test]
    fn a_client_that_does_not_declare_events_is_caught() {
        let refuses = Leaky {
            caps: Capabilities::all_except(&[Feature::Events]),
            role: Role::Client,
        };
        let err = check_events_are_ignored_by_clients(&refuses).unwrap_err();
        assert!(err.contains("does not declare `events`"), "{err}");
    }
}
