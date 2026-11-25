//! `@rollout(audit)` in the generated TypeScript server and client (`spec/rules.md` section 17.1):
//! snapshots, a contract without an audited rule keeping its bytes, `tsc --strict`, an HTTP, SSE
//! and WebSocket round trip that reads what reaches `onRuleViolation`, and every example of
//! `tests/rules/endpoints/rollout*.veld` sent to the generated test handler (`veld test --live`).
//!
//! The toolchain tests need `node` and `tests/toolchain/ts/node_modules`; without them they are
//! skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::path::Path;
use std::process::Command;

use common::toolchain::{Live, build_service, compile_path, run_examples, scratch, tsc_path};
use common::*;
use serde_json::{Value, json};
use veld_project::output::{WriteOptions, write};

const ENDPOINTS: &str = "tests/rules/endpoints/rollout.veld";
const ERROR: &str = "tests/rules/endpoints/rollout_error.veld";
const CONTRACT: &str = "tests/toolchain/contract/rollout/rollout.veld";

fn testing() -> Value {
    json!({ "testing": true })
}

// ---- snapshots and the shape of the output ------------------------------------------------

fn parties_server() -> veld_codegen::FileSet {
    gen_server_with(&compile_path(ENDPOINTS), "Parties", &[], &testing())
}

#[test]
fn snapshot_of_the_server_files_of_an_audited_contract() {
    insta::assert_snapshot!(
        "rollout_server_file_list",
        parties_server().paths().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn snapshot_of_the_server_module_of_an_audited_contract() {
    insta::assert_snapshot!(
        "rollout_server_parties",
        text(&parties_server(), "server/parties.ts")
    );
}

#[test]
fn snapshot_of_the_test_handler_of_an_audited_contract() {
    insta::assert_snapshot!(
        "rollout_test_handler",
        text(&parties_server(), "server/parties-test-handler.ts")
    );
}

#[test]
fn snapshot_of_the_audit_sink() {
    insta::assert_snapshot!(
        "rollout_runtime",
        text(&parties_server(), "runtime/rollout.ts")
    );
}

#[test]
fn snapshot_of_the_client_models_of_an_audited_contract() {
    let client = gen_client(&compile_path(ENDPOINTS));
    insta::assert_snapshot!(
        "rollout_client_models",
        text(&client, "bookings/models/bookings.ts")
    );
}

#[test]
fn snapshot_of_the_client_files_of_an_audited_contract() {
    let client = gen_client(&compile_path(ENDPOINTS));
    insta::assert_snapshot!(
        "rollout_client_file_list",
        client.paths().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn an_audited_rule_adds_the_rollout_files_and_a_contract_without_one_does_not() {
    let audited = gen_server(&compile_path(ENDPOINTS), "Parties", &[]);
    assert!(audited.get("runtime/rollout.ts").is_some());
    let runtime = text(&audited, "runtime/veld.ts");
    for word in ["reportAudited", "auditLog", "beginAudit"] {
        assert!(runtime.contains(word), "`{word}` is missing");
    }
    assert!(text(&audited, "server/parties.ts").contains("audited: true"));
    let client = gen_client(&compile_path(ENDPOINTS));
    assert_eq!(
        text(&client, "runtime/veld.ts"),
        runtime,
        "one runtime in both roles"
    );
    let plain = gen_server(&endpoints_without_rollout(), "Parties", &[]);
    assert!(plain.get("runtime/rollout.ts").is_none());
    let runtime = text(&plain, "runtime/veld.ts");
    for word in [
        "reportAudited",
        "auditLog",
        "beginAudit",
        "veldRollout",
        "rollout_endpoint",
    ] {
        assert!(!runtime.contains(word), "`{word}` leaked into the runtime");
    }
    for (path, bytes) in plain.iter() {
        assert!(
            !String::from_utf8_lossy(bytes).contains("veldRollout"),
            "{path}"
        );
    }
}

/// The same contract with every `@rollout(audit)` line taken out.
fn endpoints_without_rollout() -> veld_ir::Ir {
    let source = std::fs::read_to_string(repo_root().join(ENDPOINTS)).unwrap();
    let plain: String = source
        .lines()
        .filter(|l| !l.trim_start().starts_with("@rollout(audit)"))
        .collect::<Vec<_>>()
        .join("\n");
    common::toolchain::compile_text(plain)
}

/// An audited invariant of a declaration that no endpoint reaches: the output has the declaration
/// all the same, and it imports the sink, so the sink is emitted whether or not anything reaches it.
#[test]
fn an_audited_model_that_no_endpoint_reaches_still_gets_the_audit_sink() {
    let ir = common::toolchain::compile_text(
        "model Unused {\n  n: int\n\n  @rollout(audit)\n  rule small: n < 10\n}\n\nmodel Pong {\n  ok: bool\n}\n\nservice Ping at \"/ping\" {\n  get \"/\" ping() -> Pong\n}\n"
            .to_owned(),
    );
    let server = gen_server(&ir, "Ping", &[]);
    assert!(server.get("runtime/rollout.ts").is_some());
    assert!(text(&server, "runtime/veld.ts").contains("beginAudit"));
    let client = gen_client(&ir);
    assert!(client.get("runtime/rollout.ts").is_some());
}

#[test]
fn the_test_handler_answers_with_veld_test_audited_only_for_a_contract_with_an_audited_rule() {
    let audited = gen_server_with(&compile_path(ENDPOINTS), "Parties", &[], &testing());
    let handler = text(&audited, "server/parties-test-handler.ts");
    assert!(handler.contains("veld-test-audited"));
    assert!(handler.contains("slice(0, 100)"));
    let plain = gen_server_with(&endpoints_without_rollout(), "Parties", &[], &testing());
    assert!(!text(&plain, "server/parties-test-handler.ts").contains("audited"));
}

// ---- toolchain ------------------------------------------------------------------------------

fn matrix(body: fn(&Path, &str, &str)) {
    let Some(tsc) = tsc_path() else {
        return;
    };
    for (edition, target) in TARGETS {
        body(&tsc, edition, target);
    }
}

fn write_json(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

fn tsc(tsc_path: &Path, work: &Path, project: &str) {
    common::toolchain::run(
        Command::new("node")
            .arg(tsc_path)
            .args(["-p", project])
            .current_dir(work),
        &format!("tsc -p {project}"),
    );
}

fn node_script(work: &Path, script: &str) -> String {
    let out = Command::new("node")
        .arg(format!("out/{script}.js"))
        .current_dir(work)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "node {script}.js failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn behaviour_config(target: &str) -> Value {
    json!({
        "compilerOptions": {
            "strict": true, "target": target, "lib": [target], "types": ["node"],
            "module": "commonjs", "moduleResolution": "node10", "esModuleInterop": true,
            "skipLibCheck": true, "outDir": "out", "rootDir": "."
        },
        "include": ["*.ts", "server/**/*.ts", "client/**/*.ts"]
    })
}

/// The server, the client and the HTTP driver: audited rules report and never reject, over real
/// HTTP, SSE and WebSocket.
fn http_round_trip(tsc_path: &Path, edition: &str, target: &str) {
    let guard = scratch("rollout-http-");
    let work = guard.path();
    let ir = compile_path(CONTRACT);
    let ts = typescript_option(edition);
    write(
        &work.join("server"),
        &gen_server_with(&ir, "Audit", &[], &ts),
        &WriteOptions::default(),
    )
    .unwrap();
    write(
        &work.join("client"),
        &gen_client_with(&ir, &ts),
        &WriteOptions::default(),
    )
    .unwrap();
    std::fs::copy(
        common::toolchain::toolchain_dir().join("rollout_http.ts"),
        work.join("rollout_http.ts"),
    )
    .unwrap();
    write_json(&work.join("tsconfig.json"), &behaviour_config(target));
    tsc(tsc_path, work, "tsconfig.json");
    assert!(node_script(work, "rollout_http").contains("rollout http ok"));
}

#[test]
fn audited_rules_report_and_never_reject_over_http_sse_and_websocket() {
    matrix(http_round_trip);
}

/// Every example of the contract, sent to its generated test handler.
fn live_examples(tsc_path: &Path, path: &str, service: &str, driver: &str, expected: usize) {
    let guard = scratch("rollout-live-");
    let work = guard.path();
    let ir = compile_path(path);
    build_service(work, tsc_path, &ir, service, &[driver], "es2020");
    let live = Live::start(work, &driver.replace(".ts", ".js"));
    assert_eq!(run_examples(&live.url, &ir), expected);
}

#[test]
fn every_example_of_the_rollout_contracts_passes_against_the_generated_test_handler() {
    let Some(tsc) = tsc_path() else {
        return;
    };
    live_examples(&tsc, ENDPOINTS, "Parties", "rollout_serve.ts", 11);
    live_examples(&tsc, ERROR, "Tables", "rollout_error_serve.ts", 3);
}
