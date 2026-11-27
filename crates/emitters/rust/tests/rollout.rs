//! `@rollout(audit)` in generated Rust (`spec/rules.md` section 17.1): snapshots, a contract without
//! an audited rule keeping its bytes, the generated crates built and linted, an HTTP and SSE round
//! trip that reads what reaches `on_rule_violation` (`tests/toolchain/rust/rollout-harness`), and
//! every example of `tests/rules/endpoints/rollout*.veld` sent to the generated test handler
//! (`tests/toolchain/rust/rollout-serve-harness`, `veld test --live`).
//!
//! Without `cargo` the toolchain tests are skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;

use common::*;
use serde_json::json;
use veld_codegen::{FileSet, Role};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};

const ENDPOINTS: &str = "tests/rules/endpoints/rollout.veld";
const ERROR: &str = "tests/rules/endpoints/rollout_error.veld";
const CONTRACT: &str = "tests/toolchain/contract/rollout/rollout.veld";

/// Compiles `text` as the package `package`.
fn compile_text(text: &str, package: &str) -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: package.into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: format!("{package}.veld"),
                text: text.to_owned(),
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    assert!(
        out.ir.is_some(),
        "the contract failed to compile:\n{}",
        out.render()
    );
    out.ir.unwrap_or_else(|| unreachable!())
}

fn source(rel: &str) -> String {
    std::fs::read_to_string(repo_root().join(rel)).unwrap()
}

/// The contract of the round trip, with its SSE and WebSocket endpoints: the generated crate is built
/// and linted with them, so the audited paths of a message and of an item are compiled.
fn contract_ir() -> Ir {
    compile_text(&source(CONTRACT), "audit")
}

/// `rollout.veld` and `rollout_error.veld` as one package: their declarations do not clash.
fn served_ir() -> Ir {
    let both = format!("{}\n{}", source(ENDPOINTS), source(ERROR));
    compile_text(&both, "rollout")
}

fn endpoints_ir() -> Ir {
    compile_path(ENDPOINTS, "rollout")
}

fn without_rollout() -> Ir {
    let plain: String = source(ENDPOINTS)
        .lines()
        .filter(|l| !l.trim_start().starts_with("@rollout(audit)"))
        .collect::<Vec<_>>()
        .join("\n");
    compile_text(&plain, "plain")
}

fn server(ir: &Ir, name: &str, testing: bool) -> FileSet {
    let all = ir.services.iter().map(|s| s.id).collect();
    run(
        ir,
        Role::Server,
        all,
        vec![],
        None,
        json!({ "crate-name": name, "testing": testing }),
    )
}

fn client(ir: &Ir, name: &str) -> FileSet {
    gen_client(ir, name, None)
}

// ---- snapshots and the shape of the output ------------------------------------------------

#[test]
fn snapshot_of_the_server_files_of_an_audited_contract() {
    let files = server(&endpoints_ir(), "rollout-server", true);
    let paths: Vec<&str> = files
        .paths()
        .filter(|p| !p.starts_with("src/veld/"))
        .collect();
    insta::assert_snapshot!("rollout_server_file_list", paths.join("\n"));
}

#[test]
fn snapshot_of_the_server_module_of_an_audited_contract() {
    let files = server(&endpoints_ir(), "rollout-server", true);
    insta::assert_snapshot!(
        "rollout_parties_server",
        text(&files, "src/rollout/parties_server.rs")
    );
}

#[test]
fn snapshot_of_the_test_handler_of_an_audited_contract() {
    let files = server(&endpoints_ir(), "rollout-server", true);
    insta::assert_snapshot!(
        "rollout_parties_test_server",
        text(&files, "src/rollout/parties_test_server.rs")
    );
}

#[test]
fn snapshot_of_the_models_of_an_audited_contract() {
    let files = client(&endpoints_ir(), "rollout-client");
    insta::assert_snapshot!("rollout_models", text(&files, "src/rollout/rollout.rs"));
}

#[test]
fn an_audited_rule_adds_the_rollout_regions_and_a_contract_without_one_does_not() {
    let audited = server(&endpoints_ir(), "rollout-server", true);
    let core = text(&audited, "src/veld/server.rs");
    for word in [
        "AuditLog",
        "audit_output",
        "stamp_audited",
        "ensure_audited",
    ] {
        assert!(core.contains(word), "`{word}` is missing");
    }
    assert!(text(&audited, "src/veld/rules/mod.rs").contains("pub fn audit_wire"));
    let plain = server(&without_rollout(), "plain-server", true);
    for (path, bytes) in plain.iter().filter(|(p, _)| p.ends_with(".rs")) {
        let body = String::from_utf8_lossy(bytes);
        for word in [
            "AuditLog",
            "audit_output",
            "audit_wire",
            "stamp_audited",
            "ensure_audited",
            "test_audited",
            "ctx.audited",
            "vr::audit(",
            "veld-test-audited",
        ] {
            assert!(!body.contains(word), "`{word}` leaked into {path}");
        }
    }
}

// ---- the generated crates -----------------------------------------------------------------------

/// The server and the client of the round-trip contract build and pass `clippy -D warnings`; the
/// production handler reports audited rules to the hook and never rejects on them.
#[test]
fn audited_rules_report_and_never_reject_over_http_and_sse() {
    if !cargo_or_skip() {
        return;
    }
    let ir = contract_ir();
    let ws = harness_workspace(
        "rollout-ws",
        &server(&ir, "audit-server", false),
        &client(&ir, "audit-client"),
        "rollout-harness",
    );
    build_lint_and_run(
        &ws,
        ("audit-server", "audit-client", "rollout-harness"),
        &[],
        "rollout ok",
    );
}

/// An audited invariant of a declaration that no endpoint reaches: the crate emits the declaration
/// all the same, so it needs the audit machinery of the runtime, in both roles. The gate does not ask
/// for `rules_rollout` here (nothing reaches the rule), and the generator must not need it either.
#[test]
fn an_audited_model_that_no_endpoint_reaches_is_still_built_in_both_roles() {
    if !cargo_or_skip() {
        return;
    }
    let ir = compile_text(
        "model Unused {\n  n: int\n\n  @rollout(audit)\n  rule small: n < 10\n}\n\nmodel Pong {\n  ok: bool\n}\n\nservice Ping at \"/ping\" {\n  get \"/\" ping() -> Pong\n}\n",
        "unreached",
    );
    let (server, client) = (
        server(&ir, "unreached-server", false),
        client(&ir, "unreached-client"),
    );
    let ws = work_dir().join("rollout-unreached-ws");
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
    run_ok(&mut cargo(&ws, &["build", "--workspace"]), "cargo build");
    let clippy = [
        "clippy",
        "--all-targets",
        "-p",
        "unreached-server",
        "-p",
        "unreached-client",
        "--",
        "-D",
        "warnings",
    ];
    run_ok(&mut cargo(&ws, &clippy), "cargo clippy");
}

/// Every example of the two contracts, sent to the generated test handler.
#[test]
fn every_example_of_the_rollout_contracts_passes_against_the_generated_test_handler() {
    if !cargo_or_skip() {
        return;
    }
    let ir = served_ir();
    let ws = harness_workspace(
        "rollout-serve-ws",
        &server(&ir, "rollout-server", true),
        &client(&ir, "rollout-client"),
        "rollout-serve-harness",
    );
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
                "rollout-server",
                "-p",
                "rollout-client",
                "--",
                "-D",
                "warnings",
            ],
        ),
        "cargo clippy",
    );
    run_every_example(&compile_path(ENDPOINTS, "rollout"), "parties-serve", 11);
    run_every_example(&compile_path(ERROR, "rollout"), "tables-serve", 3);
}
