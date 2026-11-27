//! Deadlines (`spec/platform.md` section 12, `spec/targets/java.md` section 16.3): the handler's
//! context and the `504` over the real entry point, the idempotency key of a request that hit its
//! deadline, the client's default timeout, the snapshots of what is generated, and that a contract
//! without a `@timeout` has none of it.
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

fn generate(ir: &Ir, role: Role, package: &str, testing: bool) -> FileSet {
    let mut o = options(package, Some("17"));
    if testing {
        o["testing"] = serde_json::json!(true);
    }
    let all = ir.services.iter().map(|s| s.id).collect();
    run(ir, role, all, vec![], None, o)
}

#[test]
fn the_files_of_a_server_with_deadlines_are_snapshotted() {
    let ir = contract_ir();
    let files = generate(&ir, Role::Server, "gen.b", false);
    for name in ["RoomsHooks", "RoomsService", "RoomsHandler"] {
        let path = format!("gen/b/levels/server/{name}.java");
        insta::assert_snapshot!(format!("deadline_{name}"), text(&files, &path));
    }
    for name in ["Deadline", "Router", "RequestContext", "HandlerHooks"] {
        let path = format!("gen/b/runtime/{name}.java");
        insta::assert_snapshot!(format!("deadline_{name}"), text(&files, &path));
    }
    let client = generate(&ir, Role::Client, "gen.c", false);
    insta::assert_snapshot!(
        "deadline_RoomsClient",
        text(&client, "gen/c/levels/client/RoomsClient.java")
    );
}

#[test]
fn a_contract_without_a_deadline_has_none_of_it() {
    let ir = compile_path("tests/rules/endpoints/transitions.veld", "bookings");
    for role in [Role::Server, Role::Client] {
        let files = generate(&ir, role, "gen.b", true);
        assert!(files.paths().all(|p| !p.ends_with("/Deadline.java")));
        for (path, bytes) in files.iter() {
            let text = String::from_utf8_lossy(bytes);
            for word in [
                "Deadline",
                "timeoutMs",
                "timeoutSet",
                "WORKERS",
                "expired()",
            ] {
                assert!(!text.contains(word), "{path} has `{word}`");
            }
        }
    }
}

#[test]
fn the_deadline_contract_compiles_with_every_warning_on() {
    if !require_javac("the_deadline_contract_compiles") {
        return;
    }
    for (role, package, testing) in [
        (Role::Server, "gen.b", false),
        (Role::Server, "gen.b", true),
        (Role::Client, "gen.c", false),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let sources = write_files(
            dir.path(),
            &generate(&contract_ir(), role, package, testing),
        );
        for release in releases() {
            javac_ok(dir.path(), &sources, release);
        }
    }
}

fn run_driver(role: Role, package: &str, driver: &str) {
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &generate(&contract_ir(), role, package, false));
    sources.push(
        repo_root()
            .join("tests/toolchain/java/platform_p2")
            .join(format!("{driver}.java")),
    );
    for release in releases() {
        javac_ok(work.path(), &sources, release);
    }
    let out = java_main(work.path(), &format!("veldtest.{driver}"), &[]);
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains("OK"),
        "{driver} failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// In time, not in time (the `504`, its body, the interrupt, nothing in `onError`), and the key.
#[test]
fn the_deadline_answers_504_and_settles_the_idempotency_key() {
    if !require_javac("the_deadline_answers_504") {
        return;
    }
    run_driver(Role::Server, "gen.b", "DeadlineHttp");
}

/// The deadline is the default timeout of a client call; a timeout the caller set wins.
#[test]
fn the_client_uses_the_deadline_as_its_default_timeout() {
    if !require_javac("the_client_uses_the_deadline") {
        return;
    }
    run_driver(Role::Client, "gen.c", "DeadlineClient");
}
