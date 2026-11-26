//! A crate with Server-Sent Events in which one service has no SSE endpoint
//! (`spec/targets/rust.md`): it must build and pass clippy. Skipped without `cargo` unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::*;

#[test]
fn a_server_crate_with_sse_builds_when_one_service_has_no_sse_endpoint() {
    if !cargo_or_skip() {
        return;
    }
    let ir = compile_path(
        "tests/toolchain/contract/streaming_mixed/mixed.veld",
        "mixed",
    );
    let server = gen_server_services(&ir, &["Feed", "Notes"], "mixed-server");
    let ws = work_dir().join("sse-mixed");
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    write_files(&ws.join("server"), &server);
    std::fs::write(
        ws.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"server\"]\n",
    )
    .unwrap();
    let lock = repo_root().join("tests/toolchain/rust/Cargo.lock");
    if lock.exists() {
        std::fs::copy(&lock, ws.join("Cargo.lock")).unwrap();
    }
    run_ok(&mut cargo(&ws, &["build", "--workspace"]), "cargo build");
    let clippy = ["clippy", "--all-targets", "--", "-D", "warnings"];
    run_ok(&mut cargo(&ws, &clippy), "cargo clippy");
    let _ = std::fs::remove_dir_all(&ws);
}
