//! Policies and the principal in generated Rust (`spec/rules.md` section 11,
//! `spec/targets/rust.md` section 11): what is generated, that unused policies and unread
//! principals change nothing, the production handler over HTTP
//! (`tests/toolchain/rust/policy-harness`, against `crates/emitters/java/tests/fixtures/
//! guarded.veld`) and every example of `tests/rules/endpoints/policies.veld.pending` through the
//! generated test handler.
//!
//! Without `cargo` the toolchain tests are skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::*;
use serde_json::json;
use veld_codegen::{FileSet, Role};
use veld_ir::Ir;

const POLICIES: &str = "tests/rules/endpoints/policies.veld.pending";
const GUARDED: &str = "crates/emitters/java/tests/fixtures/guarded.veld";

/// The crates of the examples workspace. All scratch workspaces build into one shared target
/// directory, where Cargo names an artifact after the crate's name and its path inside its
/// workspace. Two workspaces that hold a `bookings-client` generated from two different contracts
/// and are built at the same time (the tests of a file run in parallel) overwrite each other's
/// artifact, and the harness of one links the client of the other. So the two workspaces of this
/// file use different crate names, like `edges-server` in `endpoint_rules.rs`.
const EXAMPLES_SERVER: &str = "policies-server";
const EXAMPLES_CLIENT: &str = "policies-client";

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
fn snapshots_of_the_policy_contracts() {
    let ir = compile_path(POLICIES, "bookings");
    let server = server_with(&ir, json!({ "testing": true }));
    insta::assert_snapshot!(
        "policies_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    for (name, path) in [
        (
            "policies_server_bookings",
            "src/bookings/bookings_server.rs",
        ),
        (
            "policies_server_policies",
            "src/bookings/bookings_policies.rs",
        ),
        (
            "policies_test_handler",
            "src/bookings/bookings_test_server.rs",
        ),
    ] {
        insta::assert_snapshot!(name, text(&server, path));
    }
    let client = gen_client(&ir, "bookings-client", None);
    insta::assert_snapshot!(
        "policies_client_policies",
        text(&client, "src/bookings/bookings_policies.rs")
    );
    insta::assert_snapshot!(
        "policies_client",
        text(&client, "src/bookings/bookings_client.rs")
    );
    // The guarded contract: a transition next to the policies, a parameter read by an argument.
    let guarded = compile_path(GUARDED, "bookings");
    let client = gen_client(&guarded, "bookings-client", None);
    insta::assert_snapshot!(
        "guarded_client",
        text(&client, "src/bookings/bookings_client.rs")
    );
}

#[test]
fn the_policy_functions_and_the_typed_principal_have_the_documented_shape() {
    let ir = compile_path(POLICIES, "bookings");
    let server = server_with(&ir, json!({}));
    let policies = text(&server, "src/bookings/bookings_policies.rs");
    assert!(policies.contains("pub fn manage_booking(user: &crate::bookings::bookings::User, booking: &crate::bookings::bookings::Booking) -> bool"));
    assert!(policies.contains("pub fn manage_booking_verdict("));
    assert!(policies.contains("now: i64) -> Result<(), String>"));
    let body = text(&server, "src/bookings/bookings_server.rs");
    assert!(
        body.contains("Option<crate::bookings::bookings::User>"),
        "typed hook"
    );
    assert!(body.contains("veld::server::Principal::new"));
    // `allow` comes after the facts and before the `require`.
    let (fact, allow, require) = (
        body.find("// fact booking").unwrap(),
        body.find("// allow ManageBooking").unwrap(),
        body.find("// require").unwrap(),
    );
    assert!(fact < allow && allow < require);
    let client = gen_client(&ir, "bookings-client", None);
    let c = text(&client, "src/bookings/bookings_client.rs");
    // The facts, then the principal, then the parameters read.
    assert!(c.contains("pub fn can_pay_booking(booking: &crate::bookings::bookings::Booking, user: &crate::bookings::bookings::User) -> bool"), "{c}");
    // The client has the policies and the clock, not the server runtime.
    assert!(client.get("src/bookings/bookings_policies.rs").is_some());
    assert!(!text(&client, "src/veld/rules/mod.rs").contains("pub struct Transition"));
    assert!(text(&client, "src/veld/rules/mod.rs").contains("pub fn system_now_ms"));
}

#[test]
fn a_policy_that_nothing_names_and_a_principal_that_nothing_reads_change_no_bytes() {
    let unused = veld_emit_conformance::compiled_unused_policy_contract();
    assert_eq!(unused.policies.len(), 1);
    let services: Vec<_> = unused.services.iter().map(|s| s.id).collect();
    for role in [Role::Server, Role::Client] {
        let files = run(&unused, role, services.clone(), vec![], None, json!({}));
        for (path, bytes) in files.iter() {
            let body = String::from_utf8_lossy(bytes);
            assert!(!path.contains("policies"), "{path}");
            for word in [
                "system_now_ms",
                "guard_violation",
                "Principal::new",
                "decode_key",
            ] {
                assert!(!body.contains(word), "{path} mentions {word}");
            }
        }
    }
}

#[test]
fn policies_are_not_part_of_a_contract_without_them() {
    let ir = compile_path("tests/rules/endpoints/bookings.veld", "bookings");
    let server = server_with(&ir, json!({}));
    assert!(!server.paths().any(|p| p.contains("policies")));
    let all: String = server
        .iter()
        .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
        .collect();
    assert!(!all.contains("system_now_ms") && !all.contains("vr::refusal"));
}

#[test]
fn validate_false_keeps_the_policies_outside_the_validated_statements() {
    let ir = compile_path(POLICIES, "bookings");
    let server = server_with(&ir, json!({ "validate": false }));
    let body = text(&server, "src/bookings/bookings_server.rs");
    let allow = body.find("// allow ManageBooking").unwrap();
    let enforce = body.find("if enforce").unwrap();
    assert!(allow < enforce);
}

fn build_and_lint(ws: &std::path::Path, server: &str, client: &str) {
    run_ok(
        &mut cargo(ws, &["build", "--workspace", "--bins"]),
        "cargo build",
    );
    let clippy = [
        "clippy",
        "--all-targets",
        "-p",
        server,
        "-p",
        client,
        "--",
        "-D",
        "warnings",
    ];
    run_ok(&mut cargo(ws, &clippy), "cargo clippy");
}

/// The server (with the test handler) and the client build and pass `clippy -D warnings`, and all
/// 10 examples pass against the test handler.
#[test]
fn every_policy_example_passes_through_the_generated_test_handler() {
    if !cargo_or_skip() {
        return;
    }
    let ir = compile_path(POLICIES, "bookings");
    let server = server_with(
        &ir,
        json!({ "crate-name": EXAMPLES_SERVER, "testing": true }),
    );
    let client = gen_client(&ir, EXAMPLES_CLIENT, None);
    let ws = harness_workspace("policy-examples-ws", &server, &client, "policy-examples");
    build_and_lint(&ws, EXAMPLES_SERVER, EXAMPLES_CLIENT);
    run_every_example(&ir, "policy-serve", 10);
}

/// The production handler over HTTP, and `can_<endpoint>` against it.
#[test]
fn policies_behave_over_http_and_can_agrees_with_the_server() {
    if !cargo_or_skip() {
        return;
    }
    let ir = compile_path(GUARDED, "bookings");
    let server = server_with(&ir, json!({}));
    let client = gen_client(&ir, "bookings-client", None);
    let ws = harness_workspace("policy-http-ws", &server, &client, "policy-harness");
    build_lint_and_run(
        &ws,
        ("bookings-server", "bookings-client", "policy-harness"),
        &[],
        "policies ok",
    );
}
