//! Rate limits (`spec/platform.md` section 5, `spec/targets/java.md` section 17.5): the windows and
//! the keys of `tests/wire/limits` through the generated memory limiter and the production handler,
//! the production handler over the real entry point (the last allowed call, the first refused one
//! with its body and header, the next window, each of `user`, `tenant` and `ip`, a failing limiter
//! and failing hooks, a server created without the hook it needs), the client (the single retry),
//! the snapshots, and that a contract without a limit has none of it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::live::*;
use common::*;
use veld_codegen::{FileSet, Role};
use veld_ir::Ir;

const CONTRACT: &str = "tests/toolchain/contract/platform_p1/limits.veld";
const GOLDEN_CONTRACT: &str = "tests/wire/limits/contract.veld";
const GOLDEN_CASES: &str = "tests/wire/limits/cases.json";
const DRIVERS: &str = "tests/toolchain/java/platform_p1";

fn contract() -> Ir {
    compile_path(CONTRACT, "limits")
}

fn generate(ir: &Ir, role: Role, package: &str, testing: bool) -> FileSet {
    let mut o = options(package, Some("17"));
    if testing {
        o["testing"] = serde_json::json!(true);
    }
    let all = ir.services.iter().map(|s| s.id).collect();
    run(ir, role, all, vec![], None, o)
}

fn driver(name: &str) -> std::path::PathBuf {
    repo_root().join(DRIVERS).join(format!("{name}.java"))
}

fn run_driver(files: &FileSet, name: &str, cases: Option<&str>) {
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), files);
    sources.push(driver(name));
    for release in releases() {
        javac_ok(work.path(), &sources, release);
    }
    if let Some(path) = cases {
        std::fs::copy(repo_root().join(path), work.path().join("cases.json")).unwrap();
    }
    let out = java_main(work.path(), &format!("veldtest.{name}"), &[]);
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains("OK"),
        "{name} failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn the_files_of_a_contract_with_limits_are_snapshotted() {
    let ir = contract();
    let server = generate(&ir, Role::Server, "gen.b", true);
    for (name, path) in [
        ("RoomsHooks", "gen/b/limits/server/RoomsHooks.java"),
        ("RoomsHandler", "gen/b/limits/server/RoomsHandler.java"),
        (
            "RoomsTestHandler",
            "gen/b/limits/server/RoomsTestHandler.java",
        ),
        ("DesksHooks", "gen/b/limits/server/DesksHooks.java"),
        ("PublicHooks", "gen/b/limits/server/PublicHooks.java"),
    ] {
        insta::assert_snapshot!(format!("limits_{name}"), text(&server, path));
    }
    for name in ["Limiter", "MemoryLimiter"] {
        insta::assert_snapshot!(
            format!("limits_{name}"),
            text(&server, &format!("gen/b/runtime/{name}.java"))
        );
    }
    let client = generate(&ir, Role::Client, "gen.c", false);
    insta::assert_snapshot!(
        "limits_client_RoomsClient",
        text(&client, "gen/c/limits/client/RoomsClient.java")
    );
    insta::assert_snapshot!(
        "limits_client_ClientOptions",
        text(&client, "gen/c/runtime/ClientOptions.java")
    );
}

#[test]
fn a_contract_without_a_limit_has_none_of_it() {
    let ir = compile_path("tests/rules/endpoints/policies.veld", "bookings");
    for role in [Role::Server, Role::Client] {
        let files = generate(&ir, role, "gen.b", true);
        for name in ["Limiter", "MemoryLimiter"] {
            assert!(
                files
                    .paths()
                    .all(|p| !p.ends_with(&format!("/{name}.java"))),
                "{name}"
            );
        }
        for (path, bytes) in files.iter() {
            let text = String::from_utf8_lossy(bytes);
            for word in [
                "Limiter",
                "limitCaller",
                "retryOnLimit",
                "AddressHook",
                "Platform.limit",
                "sendLimited",
            ] {
                assert!(!text.contains(word), "{path} has `{word}`");
            }
        }
    }
}

/// The test handler has no limits (spec/platform.md section 9.2): its hooks are a copy that does not
/// carry them, and its generated class says nothing about a limiter.
#[test]
fn the_test_handler_has_no_limits() {
    let server = generate(&contract(), Role::Server, "gen.b", true);
    let handler = text(&server, "gen/b/limits/server/RoomsTestHandler.java");
    assert!(!handler.contains("imiter"), "{handler}");
}

#[test]
fn the_limited_contracts_compile_with_every_warning_on() {
    if !require_javac("the_limited_contracts_compile") {
        return;
    }
    let keys = compile_path(GOLDEN_CONTRACT, "bookings");
    let all_four = compile_path(
        "tests/toolchain/contract/platform_p1/all_four.veld",
        "all_four",
    );
    for ir in [contract(), keys, all_four] {
        for (role, package, testing) in [
            (Role::Server, "gen.b", false),
            (Role::Server, "gen.b", true),
            (Role::Client, "gen.c", false),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let sources = write_files(dir.path(), &generate(&ir, role, package, testing));
            for release in releases() {
                javac_ok(dir.path(), &sources, release);
            }
        }
    }
}

/// The windows of `tests/wire/limits` through the generated memory limiter with a clock the test
/// controls, and the keys through the production handler of each service.
#[test]
fn every_golden_of_the_limits_gives_the_decisions_and_keys_of_the_reference() {
    if !require_javac("every_golden_of_the_limits") {
        return;
    }
    let ir = compile_path(GOLDEN_CONTRACT, "bookings");
    run_driver(
        &generate(&ir, Role::Server, "gen.b", false),
        "LimitsGoldens",
        Some(GOLDEN_CASES),
    );
}

/// The last allowed call, the first refused one with its body and header, the next window, each of
/// `user`, `tenant` and `ip`, a limiter that fails, hooks that fail, a server without the hook it
/// needs, a server without a limiter.
#[test]
fn limits_over_the_real_entry_point() {
    if !require_javac("limits_over_the_real_entry_point") {
        return;
    }
    run_driver(
        &generate(&contract(), Role::Server, "gen.b", false),
        "LimitsHttp",
        None,
    );
}

/// A limit does not depend on request validation.
#[test]
fn limits_do_not_depend_on_request_validation() {
    if !require_javac("limits_do_not_depend_on_request_validation") {
        return;
    }
    run_driver(
        &gen_unvalidated(&contract(), Role::Server, "gen.b", Some("17")),
        "LimitsHttp",
        None,
    );
}

/// One retry of a `get` and of an `@idempotent` call (with the same key), none for another `post`,
/// `retryOnLimit: false`, a second `429`, a `429` without `retryAfter`.
#[test]
fn the_client_waits_and_sends_again_once() {
    if !require_javac("the_client_waits_and_sends_again_once") {
        return;
    }
    run_driver(
        &generate(&contract(), Role::Client, "gen.c", false),
        "LimitsClient",
        None,
    );
}
