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
#[test]
fn java_declares_no_rules_feature_and_refuses_a_rule_with_e0801() {
    use veld_codegen::DynGenerator;
    for role in Java.roles() {
        let caps = Java.capabilities(*role);
        for f in Feature::RULES {
            assert!(!caps.supports(f), "java declares `{}`", f.name());
        }
    }
    veld_emit_conformance::check_rules_are_refused(&Java).unwrap();
}
