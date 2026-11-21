//! The shared generator conformance kit, for both TypeScript generators. Every fixture is
//! generated, including the streaming ones (SSE, WebSocket, with auth, errors and parameters).
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
