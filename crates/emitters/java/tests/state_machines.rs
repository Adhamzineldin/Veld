//! State machines of endpoints (`spec/rules.md` section 10, `spec/targets/java.md` section 10):
//! what is generated for `tests/rules/endpoints/transitions.veld.pending`, that it compiles under
//! `javac -Xlint:all -Werror`, and that every example of the contract passes through the
//! generated test handler on the JDK server and on Spring.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::live::*;
use common::*;
use veld_codegen::{FileSet, Role};
use veld_ir::Ir;

pub const TRANSITIONS: &str = "tests/rules/endpoints/transitions.veld.pending";

fn transitions_ir() -> Ir {
    compile_path(TRANSITIONS, "bookings")
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
fn the_server_files_of_the_transition_contract_are_snapshotted() {
    let ir = transitions_ir();
    let files = generate(&ir, Role::Server, None, true);
    let paths: Vec<&str> = files.paths().filter(|p| !p.contains("/runtime/")).collect();
    insta::assert_snapshot!("machine_server_file_list", paths.join("\n"));
    for name in [
        "BookingsHandler",
        "BookingsTestHandler",
        "BookingsService",
        "PayBookingFacts",
    ] {
        let path = format!("gen/b/bookings/server/{name}.java");
        insta::assert_snapshot!(format!("machine_{name}"), text(&files, &path));
    }
    insta::assert_snapshot!(
        "machine_Transition",
        text(&files, "gen/b/runtime/Transition.java")
    );
}

#[test]
fn the_helpers_of_the_state_machine_are_in_the_client_and_the_server_output() {
    let ir = transitions_ir();
    for role in [Role::Client, Role::Server] {
        let files = generate(&ir, role, None, false);
        let status = text(&files, "gen/b/bookings/bookings/Status.java");
        insta::assert_snapshot!(format!("machine_Status_{}", role.name()), status);
    }
}

#[test]
fn an_enum_without_flows_has_no_helpers() {
    let ir = standard_ir();
    let files = gen_client(&ir, "gen.c");
    for (path, bytes) in files.iter().filter(|(p, _)| p.contains("/catalog/")) {
        let text = String::from_utf8_lossy(bytes);
        assert!(
            !text.contains("nextStates") && !text.contains("isFinal"),
            "{path} has state machine helpers"
        );
    }
}

#[test]
fn no_state_machine_runtime_without_a_transition() {
    let ir = standard_ir();
    let files = gen_server(&ir, "Orders", &["Iam"], "gen.a", None);
    assert!(files.paths().all(|p| !p.ends_with("/runtime/Transition.java")));
    for (path, bytes) in files.iter() {
        let text = String::from_utf8_lossy(bytes);
        assert!(!text.contains("InvalidTransition"), "{path}");
    }
}

#[test]
fn the_transition_contract_compiles_with_every_warning_on() {
    if !require_javac("the_transition_contract_compiles") {
        return;
    }
    let ir = transitions_ir();
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

#[test]
fn every_example_passes_through_the_generated_test_handler_on_the_jdk_server() {
    if !require_javac("every_transition_example_passes_on_the_jdk_server") {
        return;
    }
    let ir = transitions_ir();
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &generate(&ir, Role::Server, None, true));
    sources.push(repo_root().join("tests/toolchain/java/rules/MachineServe.java"));
    javac_ok(work.path(), &sources, "17");
    let server = Running::start(work.path(), "veldtest.MachineServe", &[]);
    let passed = run_every_example(&ir, &server.url);
    eprintln!("transition examples passed on the JDK test handler: {passed}");
    assert_eq!(passed, 14, "every example of the transition contract");
}

#[test]
fn every_example_passes_through_the_generated_test_handler_on_spring() {
    let name = "every_transition_example_passes_on_spring";
    if !require_javac(name) || !spring_available(name) {
        return;
    }
    let ir = transitions_ir();
    let files = generate(&ir, Role::Server, Some("spring"), true);
    let app = start_spring(
        "spring-machine",
        &files,
        &["rules/spring-machine/App.java"],
        "/__veld/test",
        &[],
    );
    let passed = run_every_example(&ir, &format!("http://127.0.0.1:{}", app.port));
    eprintln!("transition examples passed on the Spring test handler: {passed}");
    assert_eq!(passed, 14, "every example of the transition contract");
}

/// Compiles the production server of the contract and one driver and runs it.
fn run_driver(driver: &str, class: &str) {
    let ir = transitions_ir();
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &generate(&ir, Role::Server, None, false));
    sources.push(repo_root().join("tests/toolchain/java/rules").join(driver));
    javac_ok(work.path(), &sources, "17");
    let out = java_main(work.path(), class, &[]);
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains("OK"),
        "{class} failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn the_production_handler_guards_hands_over_the_transition_and_checks_the_target() {
    if !require_javac("the_production_handler_guards_transitions") {
        return;
    }
    run_driver("MachineHttp.java", "veldtest.MachineHttp");
}
