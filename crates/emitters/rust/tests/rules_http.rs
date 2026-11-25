//! The bookings contract (`tests/toolchain/contract/rules/bookings_rust.veld`): a snapshot of the
//! generated crates, and invariants over HTTP with the generated server and client
//! (`tests/toolchain/rust/rules-harness`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::*;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};

fn work_dir() -> PathBuf {
    std::env::var("VELD_RUST_WORK")
        .map_or_else(|_| repo_root().join("tests/toolchain/rust/.work"), PathBuf::from)
}

fn target_dir() -> PathBuf {
    std::env::var("VELD_RUST_TARGET").map_or_else(|_| work_dir().join("target"), PathBuf::from)
}

fn have_cargo() -> bool {
    Command::new("cargo")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn cargo(ws: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new("cargo");
    cmd.args(args)
        .current_dir(ws)
        .env("CARGO_TARGET_DIR", target_dir())
        .env("CARGO_TERM_COLOR", "never");
    cmd
}

fn run_ok(cmd: &mut Command, what: &str) -> String {
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "{what} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
            .lines()
            .take(150)
            .collect::<Vec<_>>()
            .join("\n")
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn bookings_ir() -> Ir {
    let path = repo_root().join("tests/toolchain/contract/rules/bookings_rust.veld");
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "bookings".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "bookings.veld".into(),
                text: std::fs::read_to_string(path).unwrap(),
            }],
            base_url: None,
        }],
        consumes: std::collections::BTreeMap::new(),
    });
    assert!(out.ir.is_some(), "{}", out.render());
    out.ir.unwrap()
}

#[test]
fn the_bookings_contract_snapshots() {
    let ir = bookings_ir();
    let server = gen_server(&ir, "Bookings", &[], "bookings-server");
    for path in [
        "src/bookings/bookings.rs",
        "src/bookings/bookings_server.rs",
        "src/veld/mod.rs",
        "src/lib.rs",
    ] {
        insta::assert_snapshot!(
            format!("bookings_server_{}", path.replace('/', "__")),
            text(&server, path)
        );
    }
    let client = gen_client(&ir, "bookings-client", None);
    insta::assert_snapshot!(
        "bookings_client_file_list",
        client.paths().collect::<Vec<_>>().join("\n")
    );
    for (path, bytes) in server.iter().chain(client.iter()) {
        let body = String::from_utf8_lossy(bytes);
        assert!(
            !body.contains("// #if") && !body.contains("// #endif"),
            "{path}"
        );
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if src.is_dir() {
            copy_dir(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
}

/// A generated server rejects a bad body with the 422 problem document; the client with the local
/// check off decodes the same violations, with it on fails locally and sends nothing; an output
/// violation is a 500 in `strict` and passes with a hook event in `audit`.
#[test]
fn rules_over_http() {
    if !have_cargo() {
        assert!(
            std::env::var("VELD_REQUIRE_TOOLCHAINS").as_deref() != Ok("1"),
            "toolchain required but `cargo` is unavailable"
        );
        eprintln!("SKIPPED: `cargo` is not on PATH");
        return;
    }
    let ir = bookings_ir();
    let ws = work_dir().join("rules-http-ws");
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    write_files(&ws.join("server"), &gen_server(&ir, "Bookings", &[], "bookings-server"));
    write_files(&ws.join("client"), &gen_client(&ir, "bookings-client", None));
    copy_dir(
        &repo_root().join("tests/toolchain/rust/rules-harness"),
        &ws.join("rules-harness"),
    );
    std::fs::write(
        ws.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"server\", \"client\", \"rules-harness\"]\n",
    )
    .unwrap();
    let lock = repo_root().join("tests/toolchain/rust/Cargo.lock");
    if lock.exists() {
        std::fs::copy(&lock, ws.join("Cargo.lock")).unwrap();
    }
    run_ok(&mut cargo(&ws, &["build", "--workspace", "--bins"]), "cargo build");
    let clippy = [
        "clippy", "--all-targets", "-p", "bookings-server", "-p", "bookings-client", "--", "-D", "warnings",
    ];
    run_ok(&mut cargo(&ws, &clippy), "cargo clippy");
    let out = run_ok(&mut cargo(&ws, &["run", "-q", "-p", "rules-harness"]), "rules round trip");
    assert!(out.contains("rules roundtrip ok"), "{out}");
}
