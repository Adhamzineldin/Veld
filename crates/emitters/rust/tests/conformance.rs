//! The shared checks every generator must pass: capability coverage, signatures, determinism.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use veld_codegen::{DynGenerator, GenRequest, Role};
use veld_emit_conformance::{FIXTURES, check_generator, check_signatures, compile};
use veld_emit_rust::Rust;

#[test]
fn passes_the_shared_conformance_suite() {
    check_generator(&Rust);
}

/// Every `sig` in the IR (declarations and operations) must appear verbatim in the output.
#[test]
fn every_generated_sig_equals_the_ir_sig() {
    let plain = FIXTURES.iter().find(|f| f.feature.is_none()).unwrap();
    let ir = compile(plain.name, plain.source);
    let services: Vec<_> = ir.services.iter().map(|s| s.id).collect();
    for role in Rust.roles() {
        let req = GenRequest {
            ir: &ir,
            role: *role,
            services: services.clone(),
            consumes: vec![],
            framework: None,
        };
        let files = Rust.generate_dyn(&req, &serde_json::Value::Null).unwrap();
        check_signatures(&ir, &services, &files).unwrap();
    }
}

#[test]
fn output_is_deterministic() {
    let plain = FIXTURES.iter().find(|f| f.feature.is_none()).unwrap();
    let ir = compile(plain.name, plain.source);
    let services: Vec<_> = ir.services.iter().map(|s| s.id).collect();
    let req = GenRequest {
        ir: &ir,
        role: Rust.roles()[0],
        services,
        consumes: vec![],
        framework: None,
    };
    let a = Rust.generate_dyn(&req, &serde_json::Value::Null).unwrap();
    let b = Rust.generate_dyn(&req, &serde_json::Value::Null).unwrap();
    assert_eq!(a, b);
}

#[test]
fn a_capability_that_is_not_declared_is_reported_not_silently_ignored() {
    for fixture in FIXTURES.iter().filter(|f| f.feature.is_some()) {
        let ir = compile(fixture.name, fixture.source);
        let services: Vec<_> = ir.services.iter().map(|s| s.id).collect();
        let role = Rust.roles()[0];
        let req = GenRequest {
            ir: &ir,
            role,
            services,
            consumes: vec![],
            framework: None,
        };
        let caps = Rust.capabilities(role);
        let feature = fixture.feature.unwrap();
        let reported = veld_codegen::check_capabilities(&ir, &req, &caps)
            .iter()
            .any(|d| d.code == "E0801");
        assert_eq!(reported, !caps.supports(feature), "{}", fixture.name);
    }
}

/// The `axum` server serves every feature, WebSocket included.
#[test]
fn the_axum_server_passes_the_conformance_kit_with_everything() {
    veld_emit_conformance::check_generator_with(
        &Rust,
        veld_emit_conformance::Setup {
            framework: Some("axum"),
            rejected: &[],
        },
    );
}

fn client_request<'a>(ir: &'a veld_ir::Ir, framework: Option<&str>) -> GenRequest<'a> {
    GenRequest {
        ir,
        role: Role::Client,
        services: ir.services.iter().map(|s| s.id).collect(),
        consumes: vec![],
        framework: framework.map(str::to_string),
    }
}

/// The blocking `ureq` transport has no WebSocket client: a contract with `ws` endpoints is
/// `E0801`, and every other fixture generates.
#[test]
fn the_ureq_client_rejects_websockets_with_e0801_and_serves_the_rest() {
    for fixture in FIXTURES {
        let ir = compile(fixture.name, fixture.source);
        let req = client_request(&ir, Some("ureq"));
        let uses_ws = ir
            .features_used(&req.services)
            .contains(&veld_ir::Feature::StreamingWs);
        match Rust.generate_dyn(&req, &serde_json::Value::Null) {
            Ok(files) => {
                assert!(!uses_ws, "{}: a ws contract must be refused", fixture.name);
                check_signatures(&ir, &req.services, &files).unwrap();
            }
            Err(d) => {
                assert!(uses_ws, "{}: unexpected failure {d:?}", fixture.name);
                assert!(d.iter().all(|d| d.code == "E0801"), "{d:?}");
            }
        }
    }
}

#[test]
fn unknown_frameworks_and_options_are_reported() {
    let plain = FIXTURES.iter().find(|f| f.feature.is_none()).unwrap();
    let ir = compile(plain.name, plain.source);
    let mut req = client_request(&ir, Some("hyper"));
    let err = Rust
        .generate_dyn(&req, &serde_json::Value::Null)
        .unwrap_err();
    assert_eq!(err[0].code, "E0803");
    assert!(
        err[0].message.contains("reqwest, ureq"),
        "{}",
        err[0].message
    );
    req.framework = None;
    let err = Rust
        .generate_dyn(&req, &serde_json::json!({ "bogus": true }))
        .unwrap_err();
    assert_eq!(err[0].code, "E0802");
    let err = Rust
        .generate_dyn(&req, &serde_json::json!({ "crate-name": "1 bad" }))
        .unwrap_err();
    assert_eq!(err[0].code, "E0802");
    // A server takes `axum` only, and no `actix`.
    let server = GenRequest {
        role: Role::Server,
        framework: Some("actix".into()),
        ..client_request(&ir, None)
    };
    let err = Rust
        .generate_dyn(&server, &serde_json::Value::Null)
        .unwrap_err();
    assert_eq!(err[0].code, "E0803");
    assert!(err[0].message.contains("axum"), "{}", err[0].message);
}

/// Invariants are enforced; the endpoint, state-machine and policy rules are still refused
/// (`spec/rules.md` section 8).
#[test]
fn rust_supports_invariants_and_still_refuses_the_other_rules_features() {
    use veld_ir::Feature;
    for role in Rust.roles() {
        let caps = Rust.capabilities(*role);
        assert!(caps.supports(Feature::RulesInvariants));
        for f in [
            Feature::RulesEndpoint,
            Feature::RulesTransitions,
            Feature::RulesPolicies,
        ] {
            assert!(!caps.supports(f), "rust declares `{}`", f.name());
        }
    }
    // A contract whose only rule is an invariant generates, for both roles.
    let ir = veld_emit_conformance::ir_with_rule();
    for role in Rust.roles() {
        let req = GenRequest {
            ir: &ir,
            role: *role,
            services: ir.services.iter().map(|s| s.id).collect(),
            consumes: vec![],
            framework: None,
        };
        let files = Rust.generate_dyn(&req, &serde_json::Value::Null).unwrap();
        assert!(files.contains("src/veld/rules/mod.rs"));
        for (path, bytes) in files.iter() {
            let text = String::from_utf8_lossy(bytes);
            assert!(
                !text.contains("// #if") && !text.contains("// #endif"),
                "{path}"
            );
        }
    }
}

#[test]
fn rust_refuses_endpoint_rules_with_e0801() {
    veld_emit_conformance::check_endpoint_rules_are_refused(&Rust).unwrap();
}
