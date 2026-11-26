//! Timed moves (`spec/platform.md` section 10, `spec/targets/java.md` section 16): the 8 examples of
//! `tests/rules/endpoints/timed.veld.pending` through the generated test handler, the due check and
//! the due-moves runner over the real entry point, `timedMoves()` in both roles, the snapshots of
//! what is generated, and that a contract without a timed edge has none of it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::live::*;
use common::*;
use veld_codegen::{FileSet, Role};
use veld_ir::Ir;

const EXAMPLES: &str = "tests/rules/endpoints/timed.veld.pending";
const CONTRACT: &str = "tests/toolchain/contract/platform_p2/timed.veld";

fn examples_ir() -> Ir {
    compile_path(EXAMPLES, "bookings")
}

fn contract_ir() -> Ir {
    compile_path(CONTRACT, "timed")
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
fn the_files_of_the_timed_contract_are_snapshotted() {
    let ir = contract_ir();
    let files = generate(&ir, Role::Server, None, true);
    for name in [
        "BookingsDueLoaders",
        "BookingsDueMoves",
        "BookingsHooks",
        "BookingsHandler",
    ] {
        let path = format!("gen/b/timed/server/{name}.java");
        insta::assert_snapshot!(format!("timed_{name}"), text(&files, &path));
    }
    insta::assert_snapshot!(
        "timed_DueMoves",
        text(&files, "gen/b/runtime/DueMoves.java")
    );
    for role in [Role::Client, Role::Server] {
        let files = generate(&ir, role, None, false);
        let status = text(&files, "gen/b/timed/timed/Status.java");
        insta::assert_snapshot!(format!("timed_Status_{}", role.name()), status);
    }
}

#[test]
fn timed_moves_are_in_both_roles_and_the_runner_only_in_a_server() {
    let ir = contract_ir();
    for role in [Role::Client, Role::Server] {
        let files = generate(&ir, role, None, false);
        let status = text(&files, "gen/b/timed/timed/Status.java");
        assert!(status.contains("public static List<TimedMove> timedMoves()"));
        assert!(status.contains("new TimedMove(PENDING, CANCELLED, 86400000L)"));
        let runner = files.paths().any(|p| p.ends_with("/DueMoves.java"));
        assert_eq!(runner, role == Role::Server, "{role:?}");
    }
}

#[test]
fn a_contract_without_a_timed_edge_has_none_of_it() {
    let ir = compile_path("tests/rules/endpoints/transitions.veld", "bookings");
    let files = generate(&ir, Role::Server, None, true);
    for (path, bytes) in files.iter() {
        let text = String::from_utf8_lossy(bytes);
        assert!(
            !path.contains("DueMoves") && !path.contains("DueLoaders"),
            "{path}"
        );
        for word in [
            "timedMoves",
            "transitionDue",
            "DueMoves",
            "TimedMove",
            "timerPrincipal",
        ] {
            assert!(!text.contains(word), "{path} has `{word}`");
        }
    }
}

#[test]
fn the_timed_contract_compiles_with_every_warning_on() {
    if !require_javac("the_timed_contract_compiles") {
        return;
    }
    for ir in [examples_ir(), contract_ir()] {
        for (role, testing) in [
            (Role::Server, false),
            (Role::Server, true),
            (Role::Client, false),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let sources = write_files(dir.path(), &generate(&ir, role, None, testing));
            for release in releases() {
                javac_ok(dir.path(), &sources, release);
            }
        }
    }
}

#[test]
fn every_timed_example_passes_through_the_generated_test_handler_on_the_jdk_server() {
    if !require_javac("every_timed_example_passes_on_the_jdk_server") {
        return;
    }
    let ir = examples_ir();
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &generate(&ir, Role::Server, None, true));
    sources.push(repo_root().join("tests/toolchain/java/platform_p2/TimedServe.java"));
    javac_ok(work.path(), &sources, "17");
    let server = Running::start(work.path(), "veldtest.TimedServe", &[]);
    let passed = run_every_example(&ir, &server.url);
    eprintln!("timed examples passed on the JDK test handler: {passed}");
    assert_eq!(passed, 8, "every example of the timed contract");
}

/// The boundary of the due check, `timedMoves()` and the runner, over the production handler.
#[test]
fn the_due_check_and_the_runner_over_the_real_entry_point() {
    if !require_javac("the_due_check_and_the_runner") {
        return;
    }
    let ir = contract_ir();
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &generate(&ir, Role::Server, None, false));
    sources.push(repo_root().join("tests/toolchain/java/platform_p2/TimedHttp.java"));
    for release in releases() {
        javac_ok(work.path(), &sources, release);
    }
    let out = java_main(work.path(), "veldtest.TimedHttp", &[]);
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains("OK"),
        "TimedHttp failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
