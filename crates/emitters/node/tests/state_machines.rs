//! State machines in the generated TypeScript (`spec/targets/typescript.md` section 5d): snapshots
//! of the helpers, the server module and the test handler; `tsc --strict` on all of them; every
//! example of `tests/rules/endpoints/transitions.veld.pending` sent to the generated test handler;
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

const CONTRACT: &str = "tests/rules/endpoints/transitions.veld.pending";

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
