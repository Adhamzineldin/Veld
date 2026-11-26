//! Timed moves in the generated TypeScript (`spec/targets/typescript.md` section 5j,
//! `spec/platform.md` section 10).
//!
//! Without `node` or `node_modules` the toolchain tests are skipped unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::toolchain::*;
use common::*;
use serde_json::json;

const EXAMPLES: &str = "tests/rules/endpoints/timed.veld.pending";
const CONTRACT: &str = "tests/toolchain/contract/platform_p2/timed.veld";

fn examples_run(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(EXAMPLES);
    let guard = scratch(prefix);
    let work = guard.path();
    build_service(work, &tsc, &ir, "Bookings", &["timed_serve.ts"], edition);
    let server = Live::start(work, "timed_serve.js");
    let passed = run_examples(&server.url, &ir);
    assert_eq!(passed, 8);
    assert_eq!(ir.examples.len(), 8, "every example ran, none was skipped");
}

#[test]
fn the_timed_examples_compile_strict_and_pass_es2020() {
    examples_run("es2020", "timed-2020-");
}

#[test]
fn the_timed_examples_compile_strict_and_pass_es2022() {
    examples_run("es2022", "timed-2022-");
}

/// The production handler: the boundary of the due check, `timedMoves()` and the runner.
fn http_run(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(CONTRACT);
    let guard = scratch(prefix);
    let work = guard.path();
    build_service(work, &tsc, &ir, "Bookings", &["timed_http.ts"], edition);
    run(
        std::process::Command::new("node")
            .arg("out/timed_http.js")
            .current_dir(work),
        "node timed_http.js",
    );
}

#[test]
fn timed_moves_behave_over_the_production_handler_es2020() {
    http_run("es2020", "timed-http-2020-");
}

#[test]
fn timed_moves_behave_over_the_production_handler_es2022() {
    http_run("es2022", "timed-http-2022-");
}

#[test]
fn snapshots_of_a_contract_with_a_timed_move() {
    let ir = compile_path(CONTRACT);
    let server = gen_server_with(&ir, "Bookings", &[], &json!({ "testing": true }));
    insta::assert_snapshot!(
        "timed_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!("timed_server_models", text(&server, "models/bookings.ts"));
    insta::assert_snapshot!("timed_server_module", text(&server, "server/bookings.ts"));
    insta::assert_snapshot!(
        "timed_test_handler",
        text(&server, "server/bookings-test-handler.ts")
    );
    let client = gen_client(&ir);
    insta::assert_snapshot!(
        "timed_client_models",
        text(&client, "bookings/models/bookings.ts")
    );
}

#[test]
fn the_server_has_the_runner_and_the_client_has_timed_moves_only() {
    let ir = compile_path(CONTRACT);
    let server = gen_server(&ir, "Bookings", &[]);
    let module = text(&server, "server/bookings.ts");
    for word in [
        "export interface BookingsDueLoaders",
        "export function runBookingsDueMoves",
        "timer?: {",
        "afterMs: 86400000n",
    ] {
        assert!(module.contains(word), "`{word}` is missing");
    }
    assert!(text(&server, "runtime/veld.ts").contains("runDueMoves"));
    let client = gen_client(&ir);
    assert!(text(&client, "bookings/models/bookings.ts").contains("timedMoves()"));
    for path in client.paths() {
        let body = String::from_utf8_lossy(client.get(path).unwrap_or_default()).into_owned();
        for word in ["runDueMoves", "DueLoaders", "checkDue"] {
            assert!(!body.contains(word), "client: `{word}` is in {path}");
        }
    }
}

#[test]
fn a_contract_without_a_timed_edge_has_none_of_the_code() {
    let ir = standard_ir();
    let server = gen_server(&ir, "Orders", &["Iam"]);
    let client = gen_client(&ir);
    for (name, files) in [("server", &server), ("client", &client)] {
        for path in files.paths() {
            let body = String::from_utf8_lossy(files.get(path).unwrap_or_default()).into_owned();
            for word in [
                "runDueMoves",
                "DueLoaders",
                "checkDue",
                "timedMoves",
                "timerCalls",
            ] {
                assert!(!body.contains(word), "{name}: `{word}` is in {path}");
            }
        }
    }
}

/// A timed move next to a version guard, an `@idempotent` endpoint and derived fields: the runtime
/// of each combination compiles under `tsc --strict` for both editions. Nothing is run.
#[test]
fn timed_moves_compile_next_to_the_other_features() {
    let Some(tsc) = tsc_path() else { return };
    let base = std::fs::read_to_string(repo_root().join(EXAMPLES)).unwrap();
    let extra = [
        "",
        "\nmodel Memo {\n  note: string\n  shout: string = note\n}\n",
    ];
    for tail in extra {
        let ir = compile_text(format!("{base}{tail}"));
        for (edition, _) in TARGETS {
            let guard = scratch("timed-combo-");
            build_service(guard.path(), &tsc, &ir, "Bookings", &[], edition);
        }
    }
}
