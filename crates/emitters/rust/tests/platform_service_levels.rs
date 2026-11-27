//! Deadlines and service levels in generated Rust (`spec/platform.md` sections 12 and 13,
//! `spec/targets/rust.md` sections 17.3 and 17.4): what is generated for
//! `tests/toolchain/contract/platform_p2/service_levels.veld`, the production handler over HTTP
//! (`tests/toolchain/rust/levels-harness`: the programs `deadline` and `slo`), and the four features
//! together (`all_four.veld`).
//!
//! Without `cargo` the toolchain tests are skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::*;
use serde_json::json;
use veld_codegen::{FileSet, Role};
use veld_ir::Ir;

const LEVELS: &str = "tests/toolchain/contract/platform_p2/service_levels.veld";
const ALL_FOUR: &str = "tests/toolchain/contract/platform_p2/all_four.veld";

fn levels_ir() -> Ir {
    compile_path(LEVELS, "rooms")
}

fn server_with(ir: &Ir, options: serde_json::Value) -> FileSet {
    let mut o = json!({ "crate-name": "rooms-server" });
    o.as_object_mut()
        .unwrap()
        .extend(options.as_object().cloned().unwrap_or_default());
    run(
        ir,
        Role::Server,
        vec![service_id(ir, "Rooms")],
        vec![],
        None,
        o,
    )
}

#[test]
fn the_server_and_client_snapshots() {
    let ir = levels_ir();
    let server = server_with(&ir, json!({}));
    insta::assert_snapshot!("levels_server", text(&server, "src/rooms/rooms_server.rs"));
    insta::assert_snapshot!("levels_runtime_server", text(&server, "src/veld/server.rs"));
    let client = gen_client(&ir, "rooms-client", None);
    insta::assert_snapshot!("levels_client", text(&client, "src/rooms/rooms_client.rs"));
    insta::assert_snapshot!("levels_runtime_client", text(&client, "src/veld/client.rs"));
}

#[test]
fn the_deadline_and_the_metrics_hook_are_generated() {
    let ir = levels_ir();
    let server = server_with(&ir, json!({}));
    let body = text(&server, "src/rooms/rooms_server.rs");
    for word in [
        "timeout_ms: Some(200),",
        "timeout_ms: None,",
        "service: \"rooms/Rooms\",",
        "deadline: \"veld:",
        "fn metrics(&self, _metric: &veld::server::Metric) {}",
        "self.hooks.metrics(metric);",
    ] {
        assert!(body.contains(word), "{word}\n{body}");
    }
    let core = text(&server, "src/veld/server.rs");
    for word in [
        "tokio::time::timeout_at(at, fut)",
        "pub deadline: Option<std::time::SystemTime>",
        "pub fn expired(&self) -> bool",
        "fn deadline_exceeded(&self, ep: &EndpointSpec, timeout_ms: u64) -> Response",
        "\"DeadlineExceeded\", 504",
        "pub struct Metric {",
        "async fn measured(",
        "catch_unwind",
    ] {
        assert!(core.contains(word), "{word}");
    }
    let manifest = text(&server, "Cargo.toml");
    assert!(manifest.contains("\"time\""), "{manifest}");
    // The client has the deadline as its default timeout and nothing of the server's.
    let client = gen_client(&ir, "rooms-client", None);
    let client_core = text(&client, "src/veld/client.rs");
    assert!(
        client_core.contains("Some(ms) if !self.options.timeout_set => Duration::from_millis(ms)")
    );
    assert!(!client_core.contains("Metric") && !client_core.contains("timeout_at"));
}

/// A contract without a deadline and without a service level has none of the words of the features,
/// in the runtime either: it generates what it generated before.
#[test]
fn a_contract_without_them_has_none_of_the_words() {
    let ir = compile_path("tests/rules/endpoints/transitions.veld", "bookings");
    let server = run(
        &ir,
        Role::Server,
        vec![service_id(&ir, "Bookings")],
        vec![],
        None,
        json!({ "crate-name": "bookings-server" }),
    );
    let client = gen_client(&ir, "bookings-client", None);
    for files in [&server, &client] {
        for (path, bytes) in files.iter() {
            let body = String::from_utf8_lossy(bytes);
            for word in [
                "timeout_ms",
                "timeout_set",
                "deadline_exceeded",
                "DeadlineExceeded",
                "timeout_at",
                "Metric",
                "measured(",
                "veld_service",
                "fn metrics",
                "within(",
            ] {
                assert!(!body.contains(word), "{path}: {word}");
            }
        }
    }
}

/// The crates build and pass clippy, and the programs `deadline` and `slo` pass over HTTP.
#[test]
fn deadlines_and_service_levels_build_and_behave_over_http() {
    if !cargo_or_skip() {
        return;
    }
    let ir = levels_ir();
    let server = server_with(&ir, json!({}));
    let client = gen_client(&ir, "rooms-client", None);
    let ws = harness_workspace("levels-ws", &server, &client, "levels-harness");
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
                "rooms-server",
                "-p",
                "rooms-client",
                "--",
                "-D",
                "warnings",
            ],
        ),
        "cargo clippy",
    );
    for (bin, ok) in [("deadline", "deadline ok"), ("slo", "slo ok")] {
        let out = run_ok(
            &mut cargo(&ws, &["run", "-q", "-p", "levels-harness", "--bin", bin]),
            bin,
        );
        assert!(out.contains(ok), "{out}");
    }
}

// ---- the four together --------------------------------------------------------------------------------------

/// `all_four.veld`: a timed edge that is also flagged, a flagged `require` and invariant, a deadline, a
/// service level and an idempotency key generate, in both roles, and the crates build and pass
/// clippy; the server file is snapshotted.
#[test]
fn the_four_features_together_generate_and_build() {
    let ir = compile_path(ALL_FOUR, "rooms");
    let server = server_with(&ir, json!({ "testing": true }));
    insta::assert_snapshot!(
        "all_four_server",
        text(&server, "src/rooms/rooms_server.rs")
    );
    insta::assert_snapshot!(
        "all_four_test_handler",
        text(&server, "src/rooms/rooms_test_server.rs")
    );
    let client = gen_client(&ir, "rooms-client", None);
    insta::assert_snapshot!(
        "all_four_client",
        text(&client, "src/rooms/rooms_client.rs")
    );
    if !cargo_or_skip() {
        return;
    }
    let ws = work_dir().join("all-four-ws");
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    write_files(&ws.join("server"), &server);
    write_files(&ws.join("client"), &client);
    std::fs::write(
        ws.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"server\", \"client\"]\n",
    )
    .unwrap();
    let lock = repo_root().join("tests/toolchain/rust/Cargo.lock");
    if lock.exists() {
        std::fs::copy(&lock, ws.join("Cargo.lock")).unwrap();
    }
    run_ok(
        &mut cargo(
            &ws,
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        ),
        "cargo clippy",
    );
}
