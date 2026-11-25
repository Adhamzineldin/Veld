//! The shared generator conformance kit, for every framework of the Java server.
#![allow(clippy::unwrap_used)]
use veld_emit_conformance::{Setup, check_generator_with};
use veld_emit_java::Java;
use veld_ir::Feature;

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

/// Flip one feature at a time when the emitter learns to enforce it (`spec/rules.md` section 8).
/// Invariants are enforced; endpoint rules, transitions and policies are still `E0801`.
#[test]
fn java_enforces_invariants_and_declares_no_other_rules_feature() {
    use veld_codegen::{DynGenerator, GenRequest, check_capabilities};
    let ir = veld_emit_conformance::ir_with_rule();
    for role in Java.roles() {
        let caps = Java.capabilities(*role);
        assert!(caps.supports(Feature::RulesInvariants), "{role:?}");
        for f in [
            Feature::RulesEndpoint,
            Feature::RulesTransitions,
            Feature::RulesPolicies,
        ] {
            assert!(!caps.supports(f), "java declares `{}`", f.name());
        }
        let req = GenRequest {
            ir: &ir,
            role: *role,
            services: ir.services.iter().map(|s| s.id).collect(),
            consumes: vec![],
            framework: None,
        };
        assert!(check_capabilities(&ir, &req, &caps).is_empty());
        Java.generate_dyn(&req, &serde_json::Value::Null)
            .unwrap_or_else(|d| {
                panic!("{role:?}: a contract with an invariant must generate: {d:?}")
            });
    }
}

#[test]
fn java_refuses_endpoint_rules_with_e0801() {
    veld_emit_conformance::check_endpoint_rules_are_refused(&Java).unwrap();
}

#[test]
fn java_refuses_a_compiled_contract_with_endpoint_rules_with_e0801() {
    veld_emit_conformance::check_compiled_endpoint_rules_are_refused(&Java).unwrap();
}
