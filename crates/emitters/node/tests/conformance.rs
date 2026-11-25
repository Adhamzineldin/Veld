//! The shared generator conformance kit, for both TypeScript generators. Every fixture is
//! generated, including the streaming ones (SSE, WebSocket, with auth, errors and parameters).
#![allow(clippy::unwrap_used)]
use veld_emit_node::Node;
use veld_emit_typescript::TypeScript;

#[test]
fn node_passes_the_conformance_kit() {
    veld_emit_conformance::check_generator(&Node);
}

#[test]
fn typescript_passes_the_conformance_kit() {
    veld_emit_conformance::check_generator(&TypeScript);
}

/// Flip one feature at a time when an emitter learns to enforce it (`spec/rules.md` section 8).
/// Invariants and endpoint rules are enforced; transitions and policies are refused.
#[test]
fn typescript_and_node_declare_invariants_and_endpoint_rules_only() {
    veld_emit_conformance::check_invariants_and_endpoint_rules_supported(&Node).unwrap();
    veld_emit_conformance::check_invariants_and_endpoint_rules_supported(&TypeScript).unwrap();
}

#[test]
fn typescript_and_node_refuse_state_machines_with_e0801() {
    veld_emit_conformance::check_state_machines_are_refused(&Node).unwrap();
    veld_emit_conformance::check_state_machines_are_refused(&TypeScript).unwrap();
}
