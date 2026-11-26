//! The shared generator conformance kit, for every framework of the Java server.
#![allow(clippy::unwrap_used, clippy::panic)]
use veld_codegen::{DynGenerator, GenRequest, check_capabilities};
use veld_emit_conformance::{
    Setup, check_generator_with, compiled_endpoint_rules_contract, compiled_policy_contract,
    compiled_principal_read_contract, compiled_state_machine_contract,
    compiled_transition_contract, compiled_unused_policy_contract, ir_with_endpoint_rules,
    ir_with_rule,
};
use veld_emit_java::Java;
use veld_ir::{Feature, Ir};

/// The default framework (`jdk`) has no WebSocket server: `ws` endpoints are `E0801`. Everything
/// else, including the client role, generates.
#[test]
fn java_passes_the_conformance_kit() {
    check_generator_with(
        &Java,
        Setup {
            framework: None,
            rejected: &[Feature::StreamingWs],
        },
    );
}

/// The explicit `jdk` framework behaves like the default.
#[test]
fn the_jdk_framework_rejects_websockets() {
    check_generator_with(
        &Java,
        Setup {
            framework: Some("jdk"),
            rejected: &[Feature::StreamingWs],
        },
    );
}

/// Spring Boot serves every feature, WebSocket included.
#[test]
fn spring_passes_the_conformance_kit_with_everything() {
    check_generator_with(
        &Java,
        Setup {
            framework: Some("spring"),
            rejected: &[],
        },
    );
}

/// The rules features Java supports (`spec/targets/java.md` sections 8 to 11): all of them.
const SUPPORTED: [Feature; 4] = [
    Feature::RulesInvariants,
    Feature::RulesEndpoint,
    Feature::RulesTransitions,
    Feature::RulesPolicies,
];

/// Every contract below must pass the gate and generate, in both roles. The shared
/// `check_invariants_and_endpoint_rules_supported` still holds the other generators to refusing
/// transitions and policies, so Java asserts its own support here.
fn assert_generates(contracts: &[Ir]) {
    for role in Java.roles() {
        let caps = Java.capabilities(*role);
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
            if let Err(d) = Java.generate_dyn(&req, &serde_json::Value::Null) {
                panic!("{}: generate failed: {d:?}", role.name());
            }
        }
    }
}

/// Every rules feature is enforced, in both roles and with a hand-built
/// and a compiled contract: state machines, a policy, a read of the principal and a policy no
/// endpoint uses.
#[test]
fn java_enforces_every_rules_feature() {
    assert_generates(&[
        ir_with_rule(),
        ir_with_endpoint_rules(),
        compiled_endpoint_rules_contract(),
        compiled_state_machine_contract().0,
        compiled_transition_contract().0,
        compiled_policy_contract().0,
        compiled_principal_read_contract().0,
        compiled_unused_policy_contract(),
    ]);
}

/// Events (`spec/events.md`): the server generates a service that publishes or subscribes, and
/// the client generates the same files with and without events.
#[test]
fn java_server_generates_events_and_the_client_ignores_them() {
    veld_emit_conformance::check_events_are_generated(&Java).unwrap();
    veld_emit_conformance::check_events_are_ignored_by_clients(&Java).unwrap();
}

/// `@rollout(audit)` (`spec/rules.md` section 17.1): Java declares `rules_rollout` as a server and
/// as a client, so an audited invariant, `require` and `ensure` pass the gate and generate.
#[test]
fn java_generates_audited_rules() {
    veld_emit_conformance::check_rollout_is_generated(&Java).unwrap();
}

/// `@version` and `@idempotent` (`spec/rules.md` sections 11.7 to 11.9): until Java implements
/// them, it refuses a contract that uses either with `E0801`, as a server and as a client.
#[test]
fn java_refuses_version_and_idempotency() {
    veld_emit_conformance::check_version_is_refused(&Java).unwrap();
    veld_emit_conformance::check_idempotency_is_refused(&Java).unwrap();
}

/// Sagas (`spec/platform.md` sections 17 and 18): until Java declares `sagas`, a contract with a
/// `saga post` endpoint is refused with `E0801` at the endpoint, in every role.
#[test]
fn java_refuses_sagas() {
    veld_emit_conformance::check_sagas_are_refused(&Java).unwrap();
}
