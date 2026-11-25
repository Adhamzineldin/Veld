//! State machines of endpoints in generated Rust (`spec/rules.md` section 10,
//! `spec/targets/rust.md` section 11): what is generated for
//! `tests/rules/endpoints/transitions.veld`, that the crates build and pass clippy, the
//! production handler over HTTP (`tests/toolchain/rust/machine-harness`) and every example of the
//! contract through the generated test handler.
//!
//! Without `cargo` the toolchain test is skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::*;
use serde_json::json;
use veld_codegen::{FileSet, Role};
use veld_ir::Ir;

const TRANSITIONS: &str = "tests/rules/endpoints/transitions.veld";

fn transitions_ir() -> Ir {
    compile_path(TRANSITIONS, "bookings")
}

fn server_with(ir: &Ir, options: serde_json::Value) -> FileSet {
    let mut o = json!({ "crate-name": "bookings-server" });
    o.as_object_mut()
        .unwrap()
        .extend(options.as_object().cloned().unwrap_or_default());
    run(
        ir,
        Role::Server,
        vec![service_id(ir, "Bookings")],
        vec![],
        None,
        o,
    )
}

#[test]
fn snapshots_of_the_state_machine_contract() {
    let ir = transitions_ir();
    let server = server_with(&ir, json!({ "testing": true }));
    insta::assert_snapshot!(
        "machine_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!(
        "machine_server_bookings",
        text(&server, "src/bookings/bookings_server.rs")
    );
    insta::assert_snapshot!(
        "machine_test_handler",
        text(&server, "src/bookings/bookings_test_server.rs")
    );
    insta::assert_snapshot!(
        "machine_server_models",
        text(&server, "src/bookings/bookings.rs")
    );
    let client = gen_client(&ir, "bookings-client", None);
    insta::assert_snapshot!(
        "machine_client_models",
        text(&client, "src/bookings/bookings.rs")
    );
    insta::assert_snapshot!(
        "machine_client",
        text(&client, "src/bookings/bookings_client.rs")
    );
}

#[test]
fn the_helpers_are_in_the_client_and_the_server_output() {
    let ir = transitions_ir();
    let server = server_with(&ir, json!({}));
    let client = gen_client(&ir, "bookings-client", None);
    for files in [&server, &client] {
        let models = text(files, "src/bookings/bookings.rs");
        assert!(models.contains("pub const fn next_states(self) -> &'static [Status]"));
        assert!(models.contains("pub const fn is_final(self) -> bool"));
        assert!(models.contains("Status::Pending => &[Status::Paid, Status::Cancelled],"));
    }
    // Only the server enforces: the client gets `can_<endpoint>`, not the guard or the runtime.
    let client_rt = text(&client, "src/veld/mod.rs");
    assert!(!client_rt.contains("pub mod rules;"), "{client_rt}");
    assert!(text(&client, "src/bookings/bookings_client.rs").contains("pub fn can_pay_booking("));
    assert!(!server.paths().any(|p| p.contains("policies")));
}

#[test]
fn an_enum_without_flows_has_no_helpers_and_a_contract_without_machines_no_guard_runtime() {
    let ir = standard_ir();
    let files = gen_client(&ir, "orders-client", None);
    for (path, bytes) in files.iter() {
        let body = String::from_utf8_lossy(bytes);
        assert!(
            !body.contains("next_states") && !body.contains("is_final"),
            "{path} has state machine helpers"
        );
    }
    let ir = endpoints_contract();
    let server = server_with(&ir, json!({}));
    for (path, bytes) in server.iter() {
        let body = String::from_utf8_lossy(bytes);
        for word in [
            "Transition",
            "InvalidTransition",
            "decode_key",
            "guard_violation",
        ] {
            assert!(
                !body.contains(word) || path.ends_with("prelude.rs"),
                "{path}: {word}"
            );
        }
    }
}

fn endpoints_contract() -> Ir {
    compile_path("tests/rules/endpoints/bookings.veld", "bookings")
}

#[test]
fn validate_false_keeps_the_guard_outside_the_validated_statements() {
    let ir = transitions_ir();
    let server = server_with(&ir, json!({ "validate": false }));
    let body = text(&server, "src/bookings/bookings_server.rs");
    let guard = body.find("let transition_from").unwrap();
    let enforce = body.find("if enforce").unwrap();
    assert!(
        guard < enforce,
        "the guard comes before the `require` statements"
    );
}

/// The server (with the test handler) and the client build and pass `clippy -D warnings`; the
/// production handler behaves over HTTP; all 14 examples pass against the test handler.
#[test]
fn state_machines_build_behave_over_http_and_pass_every_example() {
    if !cargo_or_skip() {
        return;
    }
    let ir = transitions_ir();
    let server = server_with(&ir, json!({ "testing": true }));
    let client = gen_client(&ir, "bookings-client", None);
    let ws = harness_workspace("machine-ws", &server, &client, "machine-harness");
    build_lint_and_run(
        &ws,
        ("bookings-server", "bookings-client", "machine-harness"),
        &[],
        "state machines ok",
    );
    run_every_example(&ir, "machine-serve", 14);
}
