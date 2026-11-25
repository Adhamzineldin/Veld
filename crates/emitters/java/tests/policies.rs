//! Policies and the principal (`spec/rules.md` section 11, `spec/targets/java.md` section 11): what
//! is generated for `tests/rules/endpoints/policies.veld.pending`, that it compiles under
//! `javac -Xlint:all -Werror`, and that every example of the contract passes through the generated
//! test handler on the JDK server and on Spring.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::live::*;
use common::*;
use veld_codegen::{FileSet, Role};
use veld_ir::Ir;

pub const POLICIES: &str = "tests/rules/endpoints/policies.veld.pending";
/// A contract that has the policies of the examples and a state machine on one endpoint, for the
/// order of the checks over HTTP.
pub const GUARDED: &str = "crates/emitters/java/tests/fixtures/guarded.veld";

fn policies_ir() -> Ir {
    compile_path(POLICIES, "bookings")
}

fn guarded_ir() -> Ir {
    compile_path(GUARDED, "bookings")
}

fn generate(ir: &Ir, role: Role, framework: Option<&str>, testing: bool) -> FileSet {
    generate_in(ir, role, "gen.b", framework, testing)
}

fn generate_in(
    ir: &Ir,
    role: Role,
    package: &str,
    framework: Option<&str>,
    testing: bool,
) -> FileSet {
    let mut o = options(package, Some("17"));
    if testing {
        o["testing"] = serde_json::json!(true);
    }
    let all = ir.services.iter().map(|s| s.id).collect();
    run(ir, role, all, vec![], framework, o)
}

#[test]
fn the_server_files_of_the_policy_contract_are_snapshotted() {
    let ir = policies_ir();
    let files = generate(&ir, Role::Server, None, true);
    let paths: Vec<&str> = files.paths().filter(|p| !p.contains("/runtime/")).collect();
    insta::assert_snapshot!("policies_server_file_list", paths.join("\n"));
    for (name, path) in [
        ("Policies", "gen/b/bookings/bookings/Policies.java"),
        (
            "BookingsHandler",
            "gen/b/bookings/server/BookingsHandler.java",
        ),
        ("BookingsHooks", "gen/b/bookings/server/BookingsHooks.java"),
        (
            "BookingsTestHandler",
            "gen/b/bookings/server/BookingsTestHandler.java",
        ),
        ("Policy", "gen/b/runtime/Policy.java"),
    ] {
        insta::assert_snapshot!(format!("policies_{name}"), text(&files, path));
    }
}

#[test]
fn the_client_has_the_policy_methods_and_can_helpers() {
    let ir = policies_ir();
    let files = generate(&ir, Role::Client, None, false);
    let paths: Vec<&str> = files.paths().filter(|p| !p.contains("/runtime/")).collect();
    insta::assert_snapshot!("policies_client_file_list", paths.join("\n"));
    insta::assert_snapshot!(
        "policies_client_Policies",
        text(&files, "gen/b/bookings/bookings/Policies.java")
    );
    insta::assert_snapshot!(
        "policies_BookingsClient",
        text(&files, "gen/b/bookings/client/BookingsClient.java")
    );
}

#[test]
fn the_guarded_contract_has_the_helpers_of_both_features() {
    let ir = guarded_ir();
    let files = generate_in(&ir, Role::Client, "gen.c", None, false);
    insta::assert_snapshot!(
        "guarded_BookingsClient",
        text(&files, "gen/c/bookings/client/BookingsClient.java")
    );
    let server = generate(&ir, Role::Server, None, false);
    insta::assert_snapshot!(
        "guarded_BookingsHandler",
        text(&server, "gen/b/bookings/server/BookingsHandler.java")
    );
}

#[test]
fn no_policy_code_without_policies() {
    let ir = standard_ir();
    let files = gen_server(&ir, "Orders", &["Iam"], "gen.a", None);
    assert!(files.paths().all(|p| !p.ends_with("/runtime/Policy.java")));
    assert!(files.paths().all(|p| !p.ends_with("/Policies.java")));
    for (path, bytes) in files.iter() {
        let text = String::from_utf8_lossy(bytes);
        assert!(
            !text.contains("Forbidden") && !text.contains("run.allow"),
            "{path}"
        );
    }
}

#[test]
fn the_policy_contract_compiles_with_every_warning_on() {
    if !require_javac("the_policy_contract_compiles") {
        return;
    }
    for ir in [policies_ir(), guarded_ir()] {
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
fn every_example_passes_through_the_generated_test_handler_on_the_jdk_server() {
    if !require_javac("every_policy_example_passes_on_the_jdk_server") {
        return;
    }
    let ir = policies_ir();
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &generate(&ir, Role::Server, None, true));
    sources.push(repo_root().join("tests/toolchain/java/rules/PolicyServe.java"));
    javac_ok(work.path(), &sources, "17");
    let server = Running::start(work.path(), "veldtest.PolicyServe", &[]);
    let passed = run_every_example(&ir, &server.url);
    eprintln!("policy examples passed on the JDK test handler: {passed}");
    assert_eq!(passed, 10, "every example of the policy contract");
}

#[test]
fn every_example_passes_through_the_generated_test_handler_on_spring() {
    let name = "every_policy_example_passes_on_spring";
    if !require_javac(name) || !spring_available(name) {
        return;
    }
    let ir = policies_ir();
    let files = generate(&ir, Role::Server, Some("spring"), true);
    let app = start_spring(
        "spring-policies",
        &files,
        &["rules/spring-policies/App.java"],
        "/__veld/test",
        &[],
    );
    let passed = run_every_example(&ir, &format!("http://127.0.0.1:{}", app.port));
    eprintln!("policy examples passed on the Spring test handler: {passed}");
    assert_eq!(passed, 10, "every example of the policy contract");
}

/// Compiles the production server and the client of the guarded contract and one driver, and
/// runs it.
#[test]
fn the_production_handler_denies_in_order_and_the_client_agrees() {
    if !require_javac("the_production_handler_denies_in_order") {
        return;
    }
    let ir = guarded_ir();
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &generate(&ir, Role::Server, None, false));
    sources.extend(write_files(
        work.path(),
        &generate_in(&ir, Role::Client, "gen.c", None, false),
    ));
    sources.push(repo_root().join("tests/toolchain/java/rules/PolicyHttp.java"));
    javac_ok(work.path(), &sources, "17");
    let out = java_main(work.path(), "veldtest.PolicyHttp", &[]);
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains("OK"),
        "PolicyHttp failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
