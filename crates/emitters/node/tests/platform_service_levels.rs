//! Deadlines and service levels in the generated TypeScript (`spec/targets/typescript.md` section
//! 5j, `spec/platform.md` sections 12 and 13).
//!
//! Without `node` or `node_modules` the toolchain tests are skipped unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::toolchain::*;
use common::*;
use serde_json::json;

const CONTRACT: &str = "tests/toolchain/contract/platform_p2/service_levels.veld";
const TIMED: &str = "tests/toolchain/contract/platform_p2/timed.veld";

fn driver_run(driver: &str, edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(CONTRACT);
    let guard = scratch(prefix);
    let work = guard.path();
    build_service(work, &tsc, &ir, "Rooms", &[driver], edition);
    let js = format!("out/{}", driver.replace(".ts", ".js"));
    run(
        std::process::Command::new("node")
            .arg(&js)
            .current_dir(work),
        &format!("node {js}"),
    );
}

#[test]
fn deadlines_behave_over_the_production_handler_and_the_client_es2020() {
    driver_run("deadline_http.ts", "es2020", "deadline-2020-");
}

#[test]
fn deadlines_behave_over_the_production_handler_and_the_client_es2022() {
    driver_run("deadline_http.ts", "es2022", "deadline-2022-");
}

#[test]
fn the_metrics_hook_is_called_for_every_request_es2020() {
    driver_run("slo_http.ts", "es2020", "slo-2020-");
}

#[test]
fn the_metrics_hook_is_called_for_every_request_es2022() {
    driver_run("slo_http.ts", "es2022", "slo-2022-");
}

#[test]
fn snapshots_of_a_contract_with_deadlines_and_service_levels() {
    let ir = compile_path(CONTRACT);
    let server = gen_server_with(&ir, "Rooms", &[], &json!({}));
    insta::assert_snapshot!(
        "service_levels_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!(
        "service_levels_server_module",
        text(&server, "server/rooms.ts")
    );
    let client = gen_client(&ir);
    insta::assert_snapshot!("service_levels_client", text(&client, "rooms/client.ts"));
}

#[test]
fn the_names_of_the_spec_are_there_and_a_server_without_slo_has_no_hook() {
    let ir = compile_path(CONTRACT);
    let server = gen_server(&ir, "Rooms", &[]);
    let module = text(&server, "server/rooms.ts");
    for word in [
        "metrics?: (m: veld.Metric) => void | Promise<void>;",
        "timeoutMs: 200,",
        "service: 'rooms/Rooms',",
        "deadline: DeadlineExceeded.sig",
    ] {
        assert!(module.contains(word), "`{word}` is missing");
    }
    let client = gen_client(&ir);
    let text_of = text(&client, "rooms/client.ts");
    assert!(text_of.contains("timeoutMs: 200,"));
    assert!(
        !text_of.contains("service:"),
        "a client has no service name"
    );
    // A server without an `@slo` has no hook in its type.
    let timed = gen_server(&compile_path(TIMED), "Bookings", &[]);
    assert!(!text(&timed, "server/bookings.ts").contains("metrics"));
    assert!(!text(&timed, "runtime/veld.ts").contains("reportMetric"));
}

#[test]
fn a_contract_without_deadlines_and_service_levels_has_none_of_the_code() {
    let ir = standard_ir();
    let server = gen_server(&ir, "Orders", &["Iam"]);
    let client = gen_client(&ir);
    for (name, files) in [("server", &server), ("client", &client)] {
        for path in files.paths() {
            let body = String::from_utf8_lossy(files.get(path).unwrap_or_default()).into_owned();
            for word in [
                "withDeadline",
                "deadlineAnswer",
                "reportMetric",
                "withMetrics",
                "callLimit",
                "timeoutMs",
            ] {
                assert!(!body.contains(word), "{name}: `{word}` is in {path}");
            }
        }
    }
}

/// A deadline and a service level next to timed moves, idempotency and rules: the runtime of each
/// combination compiles under `tsc --strict` for both editions. Nothing is run.
#[test]
fn deadlines_and_service_levels_compile_next_to_the_other_features() {
    let Some(tsc) = tsc_path() else { return };
    let timed = std::fs::read_to_string(repo_root().join(TIMED)).unwrap();
    let with_timeout = timed.replacen(
        "  @auth(bearer)\n",
        "  @auth(bearer)\n  @timeout(seconds(1))\n  @slo(p99: ms(900))\n",
        1,
    );
    assert_ne!(with_timeout, timed);
    let ir = compile_text(with_timeout);
    for (edition, _) in TARGETS {
        let guard = scratch("levels-combo-");
        build_service(guard.path(), &tsc, &ir, "Bookings", &[], edition);
    }
}
