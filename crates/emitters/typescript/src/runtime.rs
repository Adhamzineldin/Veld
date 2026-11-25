//! The embedded runtime. One real `.ts` file, byte-identical in every role.
//!
//! `runtime/veld.ts` has regions between `// #if rules` and `// #endif` marker lines
//! (`veld_codegen::gate_regions`). A contract without rules gets the file without them, exactly
//! as it was before rules existed; a contract with rules also gets the regions and
//! `runtime/rules.ts` (`spec/targets/typescript.md` section 5b).

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
    gate_regions(RAW, &|name| name == "rules" && rules)
}

/// Adds `runtime/veld.ts` to `files`, and `runtime/rules.ts` when the contract has rules.
pub fn emit(files: &mut FileSet, rules: bool) -> Result<(), Vec<GenDiagnostic>> {
    let text = source(rules)
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
    fn the_embedded_markers_pair_up_in_both_modes() {
        assert!(source(false).is_ok());
        assert!(source(true).is_ok());
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
}
