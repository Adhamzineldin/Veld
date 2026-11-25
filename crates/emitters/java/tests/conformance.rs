//! The shared generator conformance kit, for every framework of the Java server.
#![allow(clippy::unwrap_used)]
use veld_emit_conformance::{Setup, check_generator_with};
use veld_codegen::DynGenerator;
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

/// Invariants and endpoint rules are enforced, in both roles and with a hand-built and a compiled
/// contract. Transitions and policies are still `E0801` (`spec/rules.md` section 8).
#[test]
fn java_enforces_invariants_and_endpoint_rules() {
    veld_emit_conformance::check_invariants_and_endpoint_rules_supported(&Java).unwrap();
}

#[test]
fn java_declares_no_transition_or_policy_feature() {
    for role in Java.roles() {
        let caps = Java.capabilities(*role);
        for f in [Feature::RulesTransitions, Feature::RulesPolicies] {
            assert!(!caps.supports(f), "java declares `{}`", f.name());
        }
    }
}
