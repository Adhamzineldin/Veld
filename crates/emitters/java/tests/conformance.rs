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
            if let Err(d) = Java.generate_dyn(
                &req,
                &veld_codegen::with_target_name(&serde_json::Value::Null, "t"),
            ) {
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

/// `@retry`, `@deadLetter` and `@ordered` (`spec/events.md` section 10.8): the Java server declares
/// `events_delivery` and generates a service that uses one; the client keeps its bytes.
#[test]
fn java_server_generates_channel_annotations_and_the_client_ignores_them() {
    veld_emit_conformance::check_events_delivery_is_generated(&Java).unwrap();
    veld_emit_conformance::check_events_delivery_is_ignored_by_clients(&Java).unwrap();
}

/// `@rollout(audit)` (`spec/rules.md` section 17.1): Java declares `rules_rollout` as a server and
/// as a client, so an audited invariant, `require` and `ensure` pass the gate and generate.
#[test]
fn java_generates_audited_rules() {
    veld_emit_conformance::check_rollout_is_generated(&Java).unwrap();
}

/// Derived fields (`spec/rules.md` section 17.2): Java declares `rules_derived` as a server and as a
/// client, so the contracts that use it (a body, a fact, an error) pass the gate and generate.
#[test]
fn java_generates_derived_fields() {
    veld_emit_conformance::check_derived_is_generated(&Java).unwrap();
}

/// `@version` and `@idempotent` (`spec/rules.md` sections 11.7 to 11.12): Java declares
/// `rules_version` and `idempotency` as a server and as a client, so the contracts that use either
/// pass the gate and generate.
#[test]
fn java_generates_version_and_idempotency() {
    veld_emit_conformance::check_version_is_generated(&Java).unwrap();
    veld_emit_conformance::check_idempotency_is_generated(&Java).unwrap();
}

/// Sagas (`spec/platform.md` sections 17 and 18): until Java declares `sagas`, a contract with a
/// `saga post` endpoint is refused with `E0801` at the endpoint, in every role.
#[test]
fn java_refuses_sagas() {
    veld_emit_conformance::check_sagas_are_refused(&Java).unwrap();
}

#[test]
fn java_generates_timed_moves() {
    veld_emit_conformance::check_timed_is_generated(&Java).unwrap();
}

#[test]
fn java_generates_flags() {
    veld_emit_conformance::check_flags_are_generated(&Java).unwrap();
}

#[test]
fn java_generates_deadlines_and_service_levels() {
    veld_emit_conformance::check_timeouts_are_generated(&Java).unwrap();
    veld_emit_conformance::check_slo_is_generated(&Java).unwrap();
}

/// `visibility` is opt-in (`spec/platform.md` section 2): the Java server refuses a `@visible`
/// field; the Java client is not asked.
#[test]
fn java_refuses_visibility_as_a_server() {
    veld_emit_conformance::check_visibility_is_refused(&Java).unwrap();
}

/// `tenancy` is opt-in (`spec/platform.md` section 3): Java refuses a `@tenant` field, as a server
/// and, when an endpoint returns one, as a client.
#[test]
fn java_refuses_tenancy_as_a_server() {
    veld_emit_conformance::check_tenancy_is_refused(&Java).unwrap();
}

/// `audit` is opt-in (`spec/platform.md` section 4): the Java server refuses an `@audited`
/// endpoint.
#[test]
fn java_refuses_audit_as_a_server() {
    veld_emit_conformance::check_audit_is_refused(&Java).unwrap();
}

/// `limits` is opt-in (`spec/platform.md` section 5): Java refuses an endpoint with `@limit`, as
/// a server and as a client.
#[test]
fn java_refuses_limits() {
    veld_emit_conformance::check_limits_are_refused(&Java).unwrap();
}

/// Message catalogs (`spec/platform.md` section 22): until Java implements them, it refuses a
/// member that has catalogs with `E0801`, as a server and as a client.
#[test]
fn java_refuses_message_catalogs() {
    veld_emit_conformance::check_messages_is_refused(&Java).unwrap();
}

/// Retention (`spec/platform.md` section 23): until Java implements it, its server refuses a
/// contract with `@retain` models with `E0801`; its client is not affected.
#[test]
fn java_refuses_retention_in_the_server_only() {
    veld_emit_conformance::check_retention_is_refused(&Java).unwrap();
}

/// `emits` and the outbox (`spec/events.md` section 11): the Java server declares `events_outbox` and
/// generates a service with an emitting `transition`; the client keeps its bytes.
#[test]
fn java_server_generates_the_outbox_and_the_client_ignores_it() {
    veld_emit_conformance::check_events_outbox_is_generated(&Java).unwrap();
    veld_emit_conformance::check_events_outbox_is_ignored_by_clients(&Java).unwrap();
}
