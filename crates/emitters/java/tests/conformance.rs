//! The shared generator conformance kit, for every framework of the Java server.
#![allow(clippy::unwrap_used, clippy::panic)]
use veld_codegen::{DynGenerator, GenRequest, check_capabilities};
use veld_emit_conformance::{
    Setup, check_generator_with, compiled_endpoint_rules_contract, compiled_state_machine_contract,
    compiled_transition_contract, ir_with_endpoint_rules, ir_with_rule,
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

/// The rules features Java supports (`spec/targets/java.md` sections 8 to 10).
const SUPPORTED: [Feature; 3] = [
    Feature::RulesInvariants,
    Feature::RulesEndpoint,
    Feature::RulesTransitions,
];

/// Every contract below must pass the gate and generate, in both roles. The shared
/// `check_invariants_and_endpoint_rules_supported` still holds the other generators to refusing
/// transitions, so Java asserts its own support here.
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
            assert!(gate.is_empty(), "{}: the gate reported {gate:?}", role.name());
            if let Err(d) = Java.generate_dyn(&req, &serde_json::Value::Null) {
                panic!("{}: generate failed: {d:?}", role.name());
            }
        }
    }
}

/// Invariants, endpoint rules and state machines are enforced, in both roles and with a hand-built
/// and a compiled contract. Policies are still `E0801` (`spec/rules.md` section 8).
#[test]
fn java_enforces_invariants_endpoint_rules_and_state_machines() {
    assert_generates(&[
        ir_with_rule(),
        ir_with_endpoint_rules(),
        compiled_endpoint_rules_contract(),
        compiled_state_machine_contract().0,
        compiled_transition_contract().0,
    ]);
}

#[test]
fn java_refuses_policies_with_e0801() {
    veld_emit_conformance::check_policies_are_refused(&Java).unwrap();
}
