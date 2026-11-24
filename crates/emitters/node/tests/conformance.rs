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
#[test]
fn typescript_and_node_declare_no_rules_feature() {
    use veld_codegen::DynGenerator;
    for (id, caps) in [
        (
            "node",
            Node.roles()
                .iter()
                .map(|r| Node.capabilities(*r))
                .collect::<Vec<_>>(),
        ),
        (
            "typescript",
            TypeScript
                .roles()
                .iter()
                .map(|r| TypeScript.capabilities(*r))
                .collect::<Vec<_>>(),
        ),
    ] {
        for c in caps {
            for f in veld_ir::Feature::RULES {
                assert!(!c.supports(f), "{id} declares `{}`", f.name());
            }
        }
    }
}

#[test]
fn a_contract_with_a_rule_is_refused_with_e0801() {
    veld_emit_conformance::check_rules_are_refused(&Node).unwrap();
    veld_emit_conformance::check_rules_are_refused(&TypeScript).unwrap();
}
