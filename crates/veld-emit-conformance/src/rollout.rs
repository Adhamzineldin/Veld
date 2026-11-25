//! Conformance checks for `@rollout(audit)` (`spec/rules.md` section 17.1).
//!
//! An audited rule never rejects. A generator that does not know the feature would enforce it, so
//! until a generator declares `rules_rollout` it must refuse a contract that reaches an audited
//! rule with `E0801`, in every role: a server would reject, and a client would check it locally.

use veld_codegen::{DynGenerator, GenRequest, Role, check_capabilities};
use veld_ir::{Feature, Ir, ServiceId};

use crate::compile;

/// An audited invariant of a model that the endpoint reaches.
const INVARIANT_SOURCE: &str = "model Greeting {
  message: string
  @rollout(audit)
  rule named: len(message) > 0
}

service Hello at \"/hello\" {
  get \"/{name}\" greet(name: string) -> Greeting | NotFound
}
";

/// An audited `require` of an endpoint.
const REQUIRE_SOURCE: &str = "model Greeting {
  message: string
}

service Hello at \"/hello\" {
  get \"/{name}\" greet(name: string) -> Greeting | NotFound {
    @rollout(audit)
    require len(name) > 0
  }
}
";

/// An audited `ensure` of an endpoint.
const ENSURE_SOURCE: &str = "model Greeting {
  message: string
}

service Hello at \"/hello\" {
  get \"/{name}\" greet(name: string) -> Greeting | NotFound {
    @rollout(audit)
    ensure len(result.message) > 0
  }
}
";

/// The contracts, each with the text on the line of its first audited rule.
const CASES: [(&str, &str, &str); 3] = [
    ("compiled_rollout_invariant", INVARIANT_SOURCE, "rule named"),
    ("compiled_rollout_require", REQUIRE_SOURCE, "require len"),
    ("compiled_rollout_ensure", ENSURE_SOURCE, "ensure len"),
];

/// The three contracts that use `rules_rollout`, with the line of the audited rule in each.
///
/// # Panics
/// When a contract of this crate does not compile.
#[must_use]
pub fn compiled_rollout_contracts() -> Vec<(Ir, u32)> {
    CASES
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

/// The generator must not declare `rules_rollout` and must refuse a contract with an audited
/// invariant, `require` or `ensure`, in every role: the gate reports exactly one `E0801` that names
/// the feature, at the audited rule, and `generate` answers `E0801` only and makes no files.
///
/// # Errors
/// A description of the first role and contract for which the generator does not behave so.
pub fn check_rollout_is_refused(generator: &dyn DynGenerator) -> Result<(), String> {
    for (ir, line) in compiled_rollout_contracts() {
        let services: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
        for role in generator.roles() {
            let caps = generator.capabilities(*role);
            if caps.supports(Feature::RulesRollout) {
                return Err(format!("{}: declares `rules_rollout`", role.name()));
            }
            refuses(generator, &ir, *role, services.clone(), line)
                .map_err(|e| format!("{} / line {line}: {e}", role.name()))?;
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
    let mine: Vec<_> = gate
        .iter()
        .filter(|d| d.message.contains("`rules_rollout`"))
        .collect();
    let at_the_rule = mine.len() == 1
        && mine
            .iter()
            .all(|d| d.code == "E0801" && d.origin.as_ref().map(|o| o.line) == Some(line));
    if !at_the_rule || gate.iter().any(|d| d.code != "E0801") {
        return Err(format!(
            "the gate reported {gate:?}, wanted one E0801 for `rules_rollout` at line {line}"
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

    /// The gate names the audited rule that comes first in the source, wherever it stands.
    #[test]
    fn the_gate_names_the_earliest_audited_rule() {
        let src = "model Greeting {\n  message: string\n  @rollout(audit)\n  rule named: len(message) > 0\n}\n\nservice Hello at \"/hello\" {\n  get \"/{name}\" greet(name: string) -> Greeting | NotFound {\n    @rollout(audit)\n    require len(name) > 0\n  }\n}\n";
        let ir = compile("earliest", src);
        let services: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
        let req = GenRequest {
            ir: &ir,
            role: Role::Server,
            services,
            consumes: vec![],
            framework: None,
        };
        let gate = check_capabilities(&ir, &req, &Capabilities::all_except(&[]));
        assert_eq!(gate.len(), 1, "{gate:?}");
        assert!(gate[0].message.contains("`rules_rollout`"), "{gate:?}");
        assert_eq!(gate[0].origin.as_ref().map(|o| o.line), Some(4));
        let opted_in = Capabilities::all_except(&[]).with(Feature::RulesRollout);
        assert!(check_capabilities(&ir, &req, &opted_in).is_empty());
    }
}
