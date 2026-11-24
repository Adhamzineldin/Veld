//! The reference evaluator for `spec/rules.md`.
//!
//! It will evaluate the typed expressions of `veld_ir::Rule` with the exact value semantics of
//! the spec, and the shared test vectors in `tests/rules/` will run against it. Generated code
//! never links to this crate; it is the oracle that the generated checks are compared with.
//! There is no logic here yet.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

/// The IR version whose rules this crate evaluates.
pub const IR_VERSION: &str = veld_ir::IR_VERSION;

#[cfg(test)]
mod tests {
    #[test]
    fn the_crate_links_against_the_ir_it_will_evaluate() {
        assert_eq!(super::IR_VERSION, "1.1");
        let rules: Vec<veld_ir::Rule> = serde_json::from_str("[]").unwrap();
        assert!(rules.is_empty());
    }
}
