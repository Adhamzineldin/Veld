//! The embedded runtime. One real `.ts` file, byte-identical in every role.
//!
//! `runtime/veld.ts` has regions between `// #if <name>` and `// #endif` marker lines
//! (`veld_codegen::gate_regions`). A contract without rules gets the file without them, exactly
//! as it was before rules existed; a contract with invariants also gets the `rules` regions and
//! `runtime/rules.ts` (`spec/targets/typescript.md` section 5b). A contract with endpoint rules
//! gets the `endpoint_rules` regions as well (section 5c); a contract with invariants only never
//! sees them.

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

/// `runtime/veld.ts` with the `rules` and `endpoint_rules` regions kept or dropped. Endpoint rules
/// need the invariant machinery, so `endpoint_rules` keeps the `rules` regions too.
///
/// # Errors
/// The embedded file has markers that do not pair up.
pub fn source_with(rules: bool, endpoint_rules: bool) -> Result<String, GateError> {
    gate_regions(RAW, &|name| match name {
        "rules" => rules || endpoint_rules,
        "endpoint_rules" => endpoint_rules,
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
    let rules = rules || endpoint_rules;
    let text = source_with(rules, endpoint_rules)
        .map_err(|e| vec![GenDiagnostic::new("E0805", format!("runtime/veld.ts: {e}"))])?;
    add_file(files, "runtime/veld.ts", text)?;
    if rules {
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
        assert!(!invariants.contains("onContractMismatch"));
        assert!(endpoint.contains("EndpointRuleSet"));
        assert!(endpoint.contains("onContractMismatch"));
        // Endpoint rules alone switch the invariant machinery on.
        assert_eq!(source_with(false, true).unwrap(), endpoint);
        let mut files = FileSet::new();
        emit_with(&mut files, false, true).unwrap();
        assert!(files.get("runtime/rules.ts").is_some());
    }
}
