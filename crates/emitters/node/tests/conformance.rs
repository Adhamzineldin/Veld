//! The shared generator conformance kit, for both TypeScript generators. Every fixture is
//! generated, including the streaming ones (SSE, WebSocket, with auth, errors and parameters).
#![allow(clippy::unwrap_used, clippy::panic)]
use veld_codegen::{DynGenerator, GenRequest, check_capabilities};
use veld_emit_conformance::{
    compiled_endpoint_rules_contract, compiled_policy_contract, compiled_principal_read_contract,
    compiled_state_machine_contract, compiled_transition_contract, compiled_unused_policy_contract,
    ir_with_endpoint_rules, ir_with_rule,
};
use veld_emit_node::Node;
use veld_emit_typescript::TypeScript;
use veld_ir::{Feature, Ir};

#[test]
fn node_passes_the_conformance_kit() {
    veld_emit_conformance::check_generator(&Node);
}

#[test]
fn typescript_passes_the_conformance_kit() {
    veld_emit_conformance::check_generator(&TypeScript);
}

/// Both generators enforce every rules feature (`spec/rules.md` section 8): invariants, endpoint
/// rules, state machines and policies. The shared `check_*_are_refused` helpers still hold the
/// Rust generator to refusing, so the support is asserted here, with a check at least as strict.
const SUPPORTED: [Feature; 4] = [
    Feature::RulesInvariants,
    Feature::RulesEndpoint,
    Feature::RulesTransitions,
    Feature::RulesPolicies,
];

/// Every contract must pass the gate and generate, in every role, with every rules feature
/// declared.
fn assert_generates(generator: &dyn DynGenerator, contracts: &[Ir]) {
    for role in generator.roles() {
        let caps = generator.capabilities(*role);
        for f in Feature::RULES {
            assert_eq!(
                caps.supports(f),
                SUPPORTED.contains(&f),
                "{}: `{}`",
                role.name(),
                f.name()
            );
        }
        for ir in contracts {
            let req = GenRequest {
                ir,
                role: *role,
                services: ir.services.iter().map(|s| s.id).collect(),
                consumes: vec![],
                framework: None,
            };
            let gate = check_capabilities(ir, &req, &caps);
            assert!(
                gate.is_empty(),
                "{}: the gate reported {gate:?}",
                role.name()
            );
            if let Err(d) = generator.generate_dyn(&req, &serde_json::Value::Null) {
                panic!("{}: generate failed: {d:?}", role.name());
            }
        }
    }
}

fn every_contract() -> Vec<Ir> {
    vec![
        ir_with_rule(),
        ir_with_endpoint_rules(),
        compiled_endpoint_rules_contract(),
        compiled_state_machine_contract().0,
        compiled_transition_contract().0,
        compiled_policy_contract().0,
        compiled_principal_read_contract().0,
        compiled_unused_policy_contract(),
    ]
}

#[test]
fn typescript_and_node_enforce_every_rules_feature() {
    assert_generates(&Node, &every_contract());
    assert_generates(&TypeScript, &every_contract());
}

/// Events (`spec/events.md`): the Node server generates a service that publishes or subscribes (the
/// Java and Rust servers still refuse it with `E0801`), and the TypeScript client generates the same files with and without events.
#[test]
fn node_generates_events_and_typescript_ignores_them() {
    veld_emit_conformance::check_events_are_generated(&Node).unwrap();
    veld_emit_conformance::check_events_are_ignored_by_clients(&TypeScript).unwrap();
}

/// `@rollout(audit)` (`spec/rules.md` section 17.1): both generators declare `rules_rollout`, so
/// the gate lets an audited invariant, `require` and `ensure` through and `generate` makes files,
/// for the server and the client alike.
#[test]
fn node_and_typescript_generate_audited_rules() {
    veld_emit_conformance::check_rollout_is_generated(&Node).unwrap();
    veld_emit_conformance::check_rollout_is_generated(&TypeScript).unwrap();
}

/// `@version` and `@idempotent` (`spec/rules.md` sections 11.7 to 11.9): until a generator
/// implements them, the Node server and the TypeScript client both refuse a contract that uses
/// either with `E0801`, at the first use, and make no files.
#[test]
fn node_and_typescript_refuse_version_and_idempotency() {
    veld_emit_conformance::check_version_is_refused(&Node).unwrap();
    veld_emit_conformance::check_idempotency_is_refused(&Node).unwrap();
    veld_emit_conformance::check_version_is_refused(&TypeScript).unwrap();
    veld_emit_conformance::check_idempotency_is_refused(&TypeScript).unwrap();
}

/// `rules_timed` is opt-in: Node and TypeScript refuse a timed edge or a timed transition.
#[test]
fn node_and_typescript_refuse_timed_moves() {
    veld_emit_conformance::check_timed_is_refused(&Node).unwrap();
    veld_emit_conformance::check_timed_is_refused(&TypeScript).unwrap();
}

/// `rules_flags` is opt-in: Node and TypeScript refuse a flagged statement.
#[test]
fn node_and_typescript_refuse_flags() {
    veld_emit_conformance::check_flags_are_refused(&Node).unwrap();
    veld_emit_conformance::check_flags_are_refused(&TypeScript).unwrap();
}

/// `timeouts` and `slo` are opt-in: Node and TypeScript refuse a deadline and a service level.
#[test]
fn node_and_typescript_refuse_deadlines_and_service_levels() {
    veld_emit_conformance::check_timeouts_are_refused(&Node).unwrap();
    veld_emit_conformance::check_slo_is_refused(&Node).unwrap();
    veld_emit_conformance::check_timeouts_are_refused(&TypeScript).unwrap();
    veld_emit_conformance::check_slo_is_refused(&TypeScript).unwrap();
}
