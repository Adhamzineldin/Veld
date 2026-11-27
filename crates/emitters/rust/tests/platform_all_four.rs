//! The four features of platform phase P1 together in generated Rust (`spec/targets/rust.md`
//! section 18): `tests/toolchain/contract/platform_p1/all_four.veld` generates in both roles, with
//! the test handler, the crates build and pass `clippy -D warnings`, and the production handler
//! (`tests/toolchain/rust/all-four-harness`) shows what each feature does next to the others.
//!
//! Without `cargo` the toolchain tests are skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::*;
use serde_json::json;
use veld_codegen::Role;

const ALL_FOUR: &str = "tests/toolchain/contract/platform_p1/all_four.veld";

#[test]
fn the_four_features_together_are_snapshotted() {
    let ir = compile_path(ALL_FOUR, "bookings");
    let server = run(
        &ir,
        Role::Server,
        ir.services.iter().map(|s| s.id).collect(),
        vec![],
        None,
        json!({ "crate-name": "bookings-server", "testing": true }),
    );
    insta::assert_snapshot!("all_four_models", text(&server, "src/bookings/bookings.rs"));
    insta::assert_snapshot!(
        "all_four_server",
        text(&server, "src/bookings/bookings_server.rs")
    );
    insta::assert_snapshot!(
        "all_four_test_handler",
        text(&server, "src/bookings/bookings_test_server.rs")
    );
    insta::assert_snapshot!(
        "all_four_runtime_platform",
        text(&server, "src/veld/platform.rs")
    );
    insta::assert_snapshot!(
        "all_four_runtime_server",
        text(&server, "src/veld/server.rs")
    );
    let client = gen_client(&ir, "bookings-client", None);
    insta::assert_snapshot!(
        "all_four_client",
        text(&client, "src/bookings/bookings_client.rs")
    );
}

/// The four features generate, in both roles, with a test handler; the test handler has neither the
/// audit hook nor a limit (it is not a request of a caller), but it filters and compares tenants.
#[test]
fn the_test_handler_has_no_limits_and_no_audit() {
    let ir = compile_path(ALL_FOUR, "bookings");
    let server = run(
        &ir,
        Role::Server,
        ir.services.iter().map(|s| s.id).collect(),
        vec![],
        None,
        json!({ "crate-name": "bookings-server", "testing": true }),
    );
    let handler = text(&server, "src/bookings/bookings_test_server.rs");
    for word in [
        "limit_spec",
        "limiter",
        "audit_spec",
        "fn audit",
        "limit_caller",
        "limit_address",
    ] {
        assert!(!handler.contains(word), "{word}");
    }
    assert!(
        handler.contains("fn request_clock(&self, ctx: &RequestContext)"),
        "the clock of the test handler is Veld-Test-Now"
    );
}

/// The crates build and pass clippy, and the production handler passes the program.
#[test]
fn the_four_features_build_and_behave_together() {
    if !cargo_or_skip() {
        return;
    }
    let ir = compile_path(ALL_FOUR, "bookings");
    let server = run(
        &ir,
        Role::Server,
        ir.services.iter().map(|s| s.id).collect(),
        vec![],
        None,
        json!({ "crate-name": "bookings-server", "testing": true }),
    );
    let client = gen_client(&ir, "bookings-client", None);
    let ws = harness_workspace("all-four-ws", &server, &client, "all-four-harness");
    build_lint_and_run(
        &ws,
        ("bookings-server", "bookings-client", "all-four-harness"),
        &[],
        "all four ok",
    );
}
