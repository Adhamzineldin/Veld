//! State machines in the generated TypeScript (`spec/targets/typescript.md` section 5d): snapshots
//! of the helpers, the server module and the test handler; `tsc --strict` on all of them; every
//! example of `tests/rules/endpoints/transitions.veld` sent to the generated test handler;
//! and the production handler over HTTP.
//!
//! Without `node` or `node_modules` the toolchain tests are skipped unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::toolchain::*;
use common::*;
use serde_json::json;
use veld_ir::Ir;

const CONTRACT: &str = "tests/rules/endpoints/transitions.veld";

fn machine_ir() -> Ir {
    compile_path(CONTRACT)
}

fn testing() -> serde_json::Value {
    json!({ "testing": true })
}

#[test]
fn snapshots_of_state_machines() {
    let ir = machine_ir();
    let server = gen_server_with(&ir, "Bookings", &[], &testing());
    insta::assert_snapshot!(
        "machine_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!("machine_server", text(&server, "server/bookings.ts"));
    insta::assert_snapshot!("machine_models", text(&server, "models/bookings.ts"));
    insta::assert_snapshot!(
        "machine_test_handler",
        text(&server, "server/bookings-test-handler.ts")
    );
    let client = gen_client(&ir);
    insta::assert_snapshot!("machine_client", text(&client, "bookings/client.ts"));
    insta::assert_snapshot!(
        "machine_client_models",
        text(&client, "bookings/models/bookings.ts")
    );
}

#[test]
fn the_helpers_are_in_both_roles_and_an_enum_without_flows_has_none() {
    let ir = machine_ir();
    let server = gen_server(&ir, "Bookings", &[]);
    let client = gen_client(&ir);
    for models in [
        text(&server, "models/bookings.ts"),
        text(&client, "bookings/models/bookings.ts"),
    ] {
        assert!(models.contains("nextStates(state: Status)"), "{models}");
        assert!(models.contains("isFinal(state: Status)"), "{models}");
    }
    let plain = rules_ir();
    let models = text(&gen_client(&plain), "bookings/models/bookings.ts").to_string();
    assert!(!models.contains("nextStates"));
}

#[test]
fn a_contract_without_a_transition_gets_no_machine_code() {
    let ir = rules_ir();
    let server = gen_server(&ir, "Bookings", &[]);
    let runtime = text(&server, "runtime/veld.ts");
    for word in ["checkTransition", "describeTarget", "refusal", "Transition"] {
        assert!(!runtime.contains(word), "`{word}` leaked into the runtime");
    }
}

#[test]
fn the_handler_gets_the_transition_and_the_runtime_has_the_guard() {
    let ir = machine_ir();
    let server = gen_server(&ir, "Bookings", &[]);
    let module = text(&server, "server/bookings.ts");
    assert!(module.contains("PayBookingRequest & PayBookingFacts & PayBookingTransition"));
    assert!(module.contains("not an atomic guarantee"));
    assert!(text(&server, "runtime/veld.ts").contains("function checkTransition"));
}

fn examples_run(edition: &str, target_dir: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = machine_ir();
    let guard = scratch(target_dir);
    let work = guard.path();
    build(
        work,
        &tsc,
        &ir,
        &["state_machines_serve.ts", "state_machines_http.ts"],
        edition,
    );
    {
        let server = Live::start(work, "state_machines_serve.js");
        let passed = run_examples(&server.url, &ir);
        assert_eq!(passed, 14);
        assert_eq!(ir.examples.len(), 14, "every example ran, none was skipped");
    }
    run(
        std::process::Command::new("node")
            .arg("out/state_machines_http.js")
            .current_dir(work),
        "node state_machines_http.js",
    );
}

#[test]
fn state_machines_compile_strict_and_pass_every_example_es2020() {
    examples_run("es2020", "machine-2020-");
}

#[test]
fn state_machines_compile_strict_and_pass_every_example_es2022() {
    examples_run("es2022", "machine-2022-");
}

/// The generated test handler against the reference evaluator on generated requests: the state of
/// the booking is put in every state of the machine, so the guard refuses each illegal one and
/// lets every source through.
#[test]
fn fuzzing_the_state_machines_agrees_with_the_reference_evaluator() {
    let Some(tsc) = tsc_path() else { return };
    let ir = machine_ir();
    let guard = scratch("machine-fuzz-");
    let work = guard.path();
    build(work, &tsc, &ir, &["state_machines_serve.ts"], "es2020");
    let server = Live::start(work, "state_machines_serve.js");
    let tally = fuzz_agrees(&server.url, &ir, FUZZ_COUNT, FUZZ_SEED);
    for endpoint in [
        "Bookings.payBooking",
        "Bookings.cancelBooking",
        "Bookings.checkIn",
    ] {
        assert!(outcome_count(&tally, endpoint, "ok") > 10, "{endpoint}");
        let refused = outcome_count(&tally, endpoint, "InvalidTransition");
        assert!(refused > 10, "{endpoint}");
    }
}

#[test]
fn a_variant_called_like_a_helper_is_refused_with_e0801() {
    let source = std::fs::read_to_string(repo_root().join(CONTRACT)).unwrap();
    let clashing = source.replace("pending, paid,", "pending, isFinal, paid,");
    assert_ne!(source, clashing);
    // The compiler accepts it (a variant may have any name); the generator refuses.
    let ir = compile_text(clashing);
    let services: Vec<_> = ir.services.iter().map(|s| s.id).collect();
    let req = veld_codegen::GenRequest {
        ir: &ir,
        role: veld_codegen::Role::Client,
        services,
        consumes: vec![],
        framework: None,
    };
    let err = veld_codegen::DynGenerator::generate_dyn(
        &veld_emit_typescript::TypeScript,
        &req,
        &veld_codegen::with_target_name(&serde_json::Value::Null, "web"),
    )
    .unwrap_err();
    assert!(err.iter().all(|d| d.code == "E0801"), "{err:?}");
    assert!(err[0].message.contains("isFinal"), "{err:?}");
}
