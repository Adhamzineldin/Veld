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

/// A crate with a rule carries no unresolved region marker.
#[test]
fn a_crate_with_a_rule_has_no_gate_markers() {
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

/// Every contract of the kit that uses a rules feature: a hand-built invariant and block, and the
/// ones the compiler made for endpoint rules, a state machine, a transition, a policy, a read of
/// the principal and a policy that no endpoint uses.
fn rules_contracts() -> Vec<veld_ir::Ir> {
    vec![
        veld_emit_conformance::ir_with_rule(),
        veld_emit_conformance::ir_with_endpoint_rules(),
        veld_emit_conformance::compiled_endpoint_rules_contract(),
        veld_emit_conformance::compiled_state_machine_contract().0,
        veld_emit_conformance::compiled_transition_contract().0,
        veld_emit_conformance::compiled_policy_contract().0,
        veld_emit_conformance::compiled_principal_read_contract().0,
        veld_emit_conformance::compiled_unused_policy_contract(),
    ]
}

/// Invariants, endpoint rules, state machines and policies are all enforced, in both roles, with a
/// hand-built and a compiled contract: the capabilities declare them, the gate lets them through,
/// generation succeeds, every signature is embedded and no region marker is left in the output.
/// (This is the local, stricter form of the shared `check_*_supported` helpers, which still hold
/// the other generators to refusing the last two.)
#[test]
fn rust_enforces_every_rules_feature() {
    use veld_codegen::check_capabilities;
    for role in Rust.roles() {
        let caps = Rust.capabilities(*role);
        for f in veld_ir::Feature::RULES {
            assert!(
                caps.supports(f),
                "{}: `{}` is not declared",
                role.name(),
                f.name()
            );
        }
        for ir in rules_contracts() {
            let services: Vec<_> = ir.services.iter().map(|s| s.id).collect();
            let req = GenRequest {
                ir: &ir,
                role: *role,
                services: services.clone(),
                consumes: vec![],
                framework: None,
            };
            let gate = check_capabilities(&ir, &req, &caps);
            assert!(
                gate.is_empty(),
                "{}: the gate reported {gate:?}",
                role.name()
            );
            let files = match Rust.generate_dyn(&req, &serde_json::Value::Null) {
                Ok(files) => files,
                Err(d) => panic!("{}: generate failed: {d:?}", role.name()),
            };
            check_signatures(&ir, &services, &files).unwrap();
            for (path, bytes) in files.iter() {
                let text = String::from_utf8_lossy(bytes);
                assert!(
                    !text.contains("// #if") && !text.contains("// #endif"),
                    "{path}"
                );
            }
        }
    }
}

/// A crate with endpoint rules carries no unresolved region marker either.
#[test]
fn a_crate_with_endpoint_rules_has_no_gate_markers() {
    for ir in [
        veld_emit_conformance::ir_with_endpoint_rules(),
        veld_emit_conformance::compiled_endpoint_rules_contract(),
    ] {
        for role in Rust.roles() {
            let req = GenRequest {
                ir: &ir,
                role: *role,
                services: ir.services.iter().map(|s| s.id).collect(),
                consumes: vec![],
                framework: None,
            };
            let files = Rust.generate_dyn(&req, &serde_json::Value::Null).unwrap();
            for (path, bytes) in files.iter() {
                let text = String::from_utf8_lossy(bytes);
                assert!(
                    !text.contains("// #if") && !text.contains("// #endif"),
                    "{path}"
                );
            }
        }
    }
}

/// Events (`spec/events.md`): the server declares `events` and generates a service that publishes
/// or subscribes, and the client generates the same files with and without events.
#[test]
fn rust_server_generates_events_and_the_client_ignores_them() {
    veld_emit_conformance::check_events_are_generated(&Rust).unwrap();
    veld_emit_conformance::check_events_are_ignored_by_clients(&Rust).unwrap();
}

/// `@rollout(audit)` (`spec/rules.md` section 17.1): Rust declares `rules_rollout` as a server and
/// as a client, so an audited invariant, `require` and `ensure` pass the gate and generate.
#[test]
fn rust_generates_audited_rules() {
    veld_emit_conformance::check_rollout_is_generated(&Rust).unwrap();
}

/// The shared checks hold a generator that does not support state machines and policies to
/// `E0801`. Rust supports them, so both checks must now notice that it does.
#[test]
fn rust_supports_state_machines_and_policies_so_the_refusal_checks_fail() {
    let sm = veld_emit_conformance::check_state_machines_are_refused(&Rust);
    assert!(
        sm.is_err_and(|e| e.contains("declares `rules_transitions`")),
        "Rust must declare `rules_transitions`"
    );
    let pol = veld_emit_conformance::check_policies_are_refused(&Rust);
    assert!(
        pol.is_err_and(|e| e.contains("declares `rules_policies`")),
        "Rust must declare `rules_policies`"
    );
}

/// `@version` and `@idempotent` (`spec/rules.md` sections 11.7 to 11.9): until the Rust generator
/// implements them, it refuses a contract that uses either with `E0801`, in every role.
#[test]
fn rust_refuses_version_and_idempotency() {
    veld_emit_conformance::check_version_is_refused(&Rust).unwrap();
    veld_emit_conformance::check_idempotency_is_refused(&Rust).unwrap();
}

#[test]
fn rust_refuses_timed_moves() {
    veld_emit_conformance::check_timed_is_refused(&Rust).unwrap();
}

#[test]
fn rust_refuses_flags() {
    veld_emit_conformance::check_flags_are_refused(&Rust).unwrap();
}
