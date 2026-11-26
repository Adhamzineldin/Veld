//! Timed moves in generated Rust (`spec/platform.md` section 10, `spec/targets/rust.md` section 17):
//! what is generated for `tests/toolchain/contract/platform_p2/timed.veld`, that the crates build and
//! pass clippy, the due check and the due-moves runner over the production handler
//! (`tests/toolchain/rust/timed-harness`), and the 8 examples of
//! `tests/rules/endpoints/timed.veld.pending` through the generated test handler.
//!
//! Without `cargo` the toolchain test is skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::*;
use serde_json::json;
use veld_codegen::{FileSet, Role};
use veld_ir::Ir;

const TIMED: &str = "tests/toolchain/contract/platform_p2/timed.veld";
const EXAMPLES: &str = "tests/rules/endpoints/timed.veld.pending";

fn timed_ir() -> Ir {
    compile_path(TIMED, "bookings")
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
fn the_server_snapshots() {
    let ir = timed_ir();
    let server = server_with(&ir, json!({}));
    insta::assert_snapshot!(
        "timed_server",
        text(&server, "src/bookings/bookings_server.rs")
    );
    insta::assert_snapshot!("timed_models", text(&server, "src/bookings/bookings.rs"));
    let client = gen_client(&ir, "bookings-client", None);
    insta::assert_snapshot!(
        "timed_client_models",
        text(&client, "src/bookings/bookings.rs")
    );
}

#[test]
fn the_runner_the_guard_and_the_helper_are_generated() {
    let ir = timed_ir();
    let server = server_with(&ir, json!({}));
    let body = text(&server, "src/bookings/bookings_server.rs");
    for word in [
        "pub trait BookingsDueLoaders",
        "pub struct BookingsDueOptions<D>",
        "pub async fn run_bookings_due_moves<E: veld::server::Endpoints, D: BookingsDueLoaders>",
        "fn timer_principal<'a>(&'a self)",
        "if !vr::due(now, &fact_booking.pending_since, 86400000_i64)",
        "violates rule transition: not due",
        "ctx.timer_now().unwrap_or_else(|| self.hooks.clock(ctx))",
    ] {
        assert!(body.contains(word), "{word}\n{body}");
    }
    let models = text(&server, "src/bookings/bookings.rs");
    assert!(models.contains("pub const fn timed_moves() -> &'static [(Status, Status, i64)]"));
    // The client has the helper and nothing of the runner or the due check.
    let client = gen_client(&ir, "bookings-client", None);
    assert!(text(&client, "src/bookings/bookings.rs").contains("pub const fn timed_moves()"));
    for (path, bytes) in client.iter() {
        let body = String::from_utf8_lossy(bytes);
        assert!(
            !body.contains("DueLoaders")
                && !body.contains("vr::due")
                && !body.contains("TimerCall"),
            "{path}"
        );
    }
}

/// A contract without a timed edge has none of the words of the feature, in the runtime either.
#[test]
fn a_contract_without_timed_edges_has_none_of_the_words() {
    let ir = compile_path("tests/rules/endpoints/transitions.veld", "bookings");
    let server = server_with(&ir, json!({}));
    for (path, bytes) in server.iter() {
        let body = String::from_utf8_lossy(bytes);
        for word in [
            "timed_moves",
            "TimerCall",
            "DueLoaders",
            "vr::due",
            "timer_principal",
            "timer_now",
            "DueReport",
        ] {
            assert!(!body.contains(word), "{path}: {word}");
        }
    }
}

/// Builds the crates, lints them, runs the production handler over HTTP and then every example
/// through the test handler.
#[test]
fn timed_moves_build_behave_over_http_and_pass_every_example() {
    if !cargo_or_skip() {
        return;
    }
    let ir = timed_ir();
    let server = server_with(&ir, json!({}));
    let client = gen_client(&ir, "bookings-client", None);
    let ws = harness_workspace("timed-ws", &server, &client, "timed-harness");
    build_lint_and_run(
        &ws,
        ("bookings-server", "bookings-client", "timed-harness"),
        &[],
        "timed ok",
    );
    let ir = compile_path(EXAMPLES, "bookings");
    let server = server_with(&ir, json!({ "testing": true }));
    let client = gen_client(&ir, "bookings-client", None);
    let ws = harness_workspace("timed-examples-ws", &server, &client, "timed-examples");
    run_ok(
        &mut cargo(&ws, &["build", "--workspace", "--bins"]),
        "cargo build",
    );
    run_ok(
        &mut cargo(
            &ws,
            &[
                "clippy",
                "--all-targets",
                "-p",
                "bookings-server",
                "--",
                "-D",
                "warnings",
            ],
        ),
        "cargo clippy",
    );
    run_every_example(&ir, "timed-serve", 8);
}
