//! Service levels (`spec/platform.md` section 13, `spec/targets/java.md` section 16.4): the metrics
//! hook of a server with an `@slo` over the real entry point, the snapshots of what is generated, and
//! that a server without an `@slo` has no hook in its type and no trace of it in its runtime.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::live::*;
use common::*;
use veld_codegen::{FileSet, Role};
use veld_ir::Ir;

const CONTRACT: &str = "tests/toolchain/contract/platform_p2/service_levels.veld";

fn contract_ir() -> Ir {
    compile_path(CONTRACT, "levels")
}

fn generate(ir: &Ir, role: Role, framework: Option<&str>, testing: bool) -> FileSet {
    let mut o = options("gen.b", Some("17"));
    if testing {
        o["testing"] = serde_json::json!(true);
    }
    let all = ir.services.iter().map(|s| s.id).collect();
    run(ir, role, all, vec![], framework, o)
}

#[test]
fn the_files_of_a_server_with_service_levels_are_snapshotted() {
    let ir = contract_ir();
    let files = generate(&ir, Role::Server, None, false);
    insta::assert_snapshot!(
        "slo_RoomsHooks",
        text(&files, "gen/b/levels/server/RoomsHooks.java")
    );
    insta::assert_snapshot!("slo_Metric", text(&files, "gen/b/runtime/Metric.java"));
    insta::assert_snapshot!(
        "slo_HandlerHooks",
        text(&files, "gen/b/runtime/HandlerHooks.java")
    );
}

#[test]
fn a_server_without_a_service_level_has_no_metrics_hook() {
    let ir = compile_path("tests/rules/endpoints/transitions.veld", "bookings");
    let files = generate(&ir, Role::Server, None, true);
    assert!(files.paths().all(|p| !p.ends_with("/Metric.java")));
    for (path, bytes) in files.iter() {
        let text = String::from_utf8_lossy(bytes);
        for word in [
            "MetricsHook",
            "Metric(",
            "http.server.request.duration",
            "measure(",
        ] {
            assert!(!text.contains(word), "{path} has `{word}`");
        }
    }
}

#[test]
fn a_client_has_no_metrics_hook() {
    let files = generate(&contract_ir(), Role::Client, None, false);
    assert!(files.paths().all(|p| !p.ends_with("/Metric.java")));
}

#[test]
fn the_service_level_contract_compiles_with_every_warning_on() {
    if !require_javac("the_service_level_contract_compiles") {
        return;
    }
    for (role, testing) in [
        (Role::Server, false),
        (Role::Server, true),
        (Role::Client, false),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let sources = write_files(dir.path(), &generate(&contract_ir(), role, None, testing));
        for release in releases() {
            javac_ok(dir.path(), &sources, release);
        }
    }
}

/// A `200`, a declared error, a `500`, every endpoint, no unmatched route, a failing hook.
#[test]
fn the_metrics_hook_is_called_once_per_matched_request() {
    if !require_javac("the_metrics_hook_is_called") {
        return;
    }
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(
        work.path(),
        &generate(&contract_ir(), Role::Server, None, false),
    );
    sources.push(repo_root().join("tests/toolchain/java/platform_p2/SloHttp.java"));
    for release in releases() {
        javac_ok(work.path(), &sources, release);
    }
    let out = java_main(work.path(), "veldtest.SloHttp", &[]);
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains("OK"),
        "SloHttp failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
