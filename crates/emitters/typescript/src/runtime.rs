//! The embedded runtime. One real `.ts` file, byte-identical in every role.
//!
//! `runtime/veld.ts` has regions between `// #if <name>` and `// #endif` marker lines
//! (`veld_codegen::gate_regions`). A contract without rules gets the file without them, exactly
//! as it was before rules existed; a contract with invariants also gets the `rules` regions and
//! `runtime/rules.ts` (`spec/targets/typescript.md` section 5b). A contract with endpoint rules
//! gets the `endpoint_rules` regions as well (section 5c); a contract with invariants only never
//! sees them. A contract with a `transition` or an `allow` (or a read of the principal) also gets
//! the `transitions` and `policies` regions (sections 5d and 5e); the rest never sees them.

use std::sync::LazyLock;

use veld_codegen::{FileSet, GateError, GenDiagnostic, gate_regions};

use crate::util::add_file;

/// The embedded `runtime/veld.ts`, markers and all.
const RAW: &str = include_str!("../runtime/veld.ts");

/// The embedded `runtime/rules.ts`: the rule helpers, emitted only for a contract with rules.
pub const RULES_SOURCE: &str = include_str!("../runtime/rules.ts");

/// Source of `runtime/veld.ts` for a contract without rules.
pub static SOURCE: LazyLock<String> = LazyLock::new(|| source(false).unwrap_or_default());

/// Root-relative module path of the runtime, without extension.
pub const MODULE: &str = "runtime/veld";

/// Root-relative module path of the rule helpers, without extension.
pub const RULES_MODULE: &str = "runtime/rules";

/// `runtime/veld.ts` with the `rules` regions kept (`rules`) or dropped.
///
/// # Errors
/// The embedded file has markers that do not pair up.
pub fn source(rules: bool) -> Result<String, GateError> {
    source_with(rules, false)
}

/// Which regions of `runtime/veld.ts` a contract needs.
///
/// A state machine or a policy is an endpoint rule, and an endpoint rule needs the invariant
/// machinery, so [`Gates::normalised`] switches on what each flag builds on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Gates {
    /// Invariants: the `rules` regions and `runtime/rules.ts`.
    pub rules: bool,
    /// Facts, `require` and `ensure`: the `endpoint_rules` regions.
    pub endpoint_rules: bool,
    /// A `transition`: the `transitions` regions.
    pub transitions: bool,
    /// An `allow` or a read of the principal: the `policies` regions.
    pub policies: bool,
}

impl Gates {
    /// The same gates with everything they build on switched on.
    #[must_use]
    pub fn normalised(self) -> Self {
        let endpoint_rules = self.endpoint_rules || self.transitions || self.policies;
        Self {
            rules: self.rules || endpoint_rules,
            endpoint_rules,
            ..self
        }
    }

    /// The gates two trees need together (the server's own tree and its consumed clients).
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self {
            rules: self.rules || other.rules,
            endpoint_rules: self.endpoint_rules || other.endpoint_rules,
            transitions: self.transitions || other.transitions,
            policies: self.policies || other.policies,
        }
    }
}

/// `runtime/veld.ts` with the `rules` and `endpoint_rules` regions kept or dropped. Endpoint rules
/// need the invariant machinery, so `endpoint_rules` keeps the `rules` regions too.
///
/// # Errors
/// The embedded file has markers that do not pair up.
pub fn source_with(rules: bool, endpoint_rules: bool) -> Result<String, GateError> {
    source_gated(Gates {
        rules,
        endpoint_rules,
        ..Gates::default()
    })
}

/// `runtime/veld.ts` with the regions `gates` asks for.
///
/// # Errors
/// The embedded file has markers that do not pair up.
pub fn source_gated(gates: Gates) -> Result<String, GateError> {
    let g = gates.normalised();
    gate_regions(RAW, &|name| match name {
        "rules" => g.rules,
        "endpoint_rules" => g.endpoint_rules,
        "transitions" => g.transitions,
        "policies" => g.policies,
        // The code the transition guard and `allow` share.
        "guards" => g.transitions || g.policies,
        _ => false,
    })
}

/// Adds `runtime/veld.ts` to `files`, and `runtime/rules.ts` when the contract has rules.
pub fn emit(files: &mut FileSet, rules: bool) -> Result<(), Vec<GenDiagnostic>> {
    emit_with(files, rules, false)
}

/// Like [`emit`], with the regions of endpoint rules when `endpoint_rules` is set.
pub fn emit_with(
    files: &mut FileSet,
    rules: bool,
    endpoint_rules: bool,
) -> Result<(), Vec<GenDiagnostic>> {
    emit_gated(
        files,
        Gates {
            rules,
            endpoint_rules,
            ..Gates::default()
        },
    )
}

/// Like [`emit`], with the regions `gates` asks for.
pub fn emit_gated(files: &mut FileSet, gates: Gates) -> Result<(), Vec<GenDiagnostic>> {
    let gates = gates.normalised();
    let text = source_gated(gates)
        .map_err(|e| vec![GenDiagnostic::new("E0805", format!("runtime/veld.ts: {e}"))])?;
    add_file(files, "runtime/veld.ts", text)?;
    if gates.rules {
        add_file(files, "runtime/rules.ts", RULES_SOURCE.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_markers_pair_up_in_every_mode() {
        assert!(source(false).is_ok());
        assert!(source(true).is_ok());
        assert!(source_with(true, true).is_ok());
        for transitions in [false, true] {
            for policies in [false, true] {
                let gates = Gates {
                    rules: true,
                    endpoint_rules: true,
                    transitions,
                    policies,
                };
                assert!(source_gated(gates).is_ok(), "{gates:?}");
            }
        }
    }

    #[test]
    fn a_contract_without_rules_gets_a_file_with_no_markers() {
        assert!(!SOURCE.is_empty());
        assert!(!SOURCE.contains("// #if"));
        assert!(!SOURCE.contains("// #endif"));
    }

    #[test]
    fn rules_are_a_second_file_and_extra_lines_only_when_used() {
        let mut off = FileSet::new();
        emit(&mut off, false).unwrap();
        assert_eq!(off.len(), 1);
        let mut on = FileSet::new();
        emit(&mut on, true).unwrap();
        assert_eq!(on.len(), 2);
        assert!(on.get("runtime/rules.ts").is_some());
    }

    #[test]
    fn endpoint_rules_add_regions_and_invariants_only_never_see_them() {
        let invariants = source(true).unwrap();
        let endpoint = source_with(true, true).unwrap();
        assert!(!invariants.contains("EndpointRuleSet"));
        // The digest belongs to the invariants: a service that reaches one has it (section 9.7).
        assert!(invariants.contains("onContractMismatch"));
        assert!(!source(false).unwrap().contains("onContractMismatch"));
        assert!(endpoint.contains("EndpointRuleSet"));
        assert!(endpoint.contains("onContractMismatch"));
        // Endpoint rules alone switch the invariant machinery on.
        assert_eq!(source_with(false, true).unwrap(), endpoint);
        let mut files = FileSet::new();
        emit_with(&mut files, false, true).unwrap();
        assert!(files.get("runtime/rules.ts").is_some());
    }

    #[test]
    fn machines_and_policies_add_their_regions_and_endpoint_rules_never_see_them() {
        let endpoint = source_with(true, true).unwrap();
        for word in ["checkAllows", "checkTransition", "refusal", "PolicyVerdict"] {
            assert!(!endpoint.contains(word), "`{word}` leaked into endpoint rules");
        }
        let machine = source_gated(Gates {
            transitions: true,
            ..Gates::default()
        })
        .unwrap();
        assert!(machine.contains("checkTransition") && machine.contains("refusal"));
        assert!(!machine.contains("checkAllows"));
        let policies = source_gated(Gates {
            policies: true,
            ..Gates::default()
        })
        .unwrap();
        assert!(policies.contains("checkAllows") && policies.contains("refusal"));
        assert!(!policies.contains("checkTransition"));
        // Each switches the endpoint machinery on.
        assert!(machine.contains("EndpointRuleSet") && policies.contains("EndpointRuleSet"));
    }
}
