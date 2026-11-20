//! The shared generator conformance kit.
use veld_emit_java::Java;

#[test]
fn java_passes_the_conformance_kit() {
    veld_emit_conformance::check_generator(&Java);
}
