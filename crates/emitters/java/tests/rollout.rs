//! `@rollout(audit)` in the generated Java (`spec/rules.md` section 17.1): snapshots, a contract
//! without an audited rule keeping its bytes, `javac -Xlint:all -Werror`, a round trip over HTTP
//! that reads what reaches `onRuleViolation`, and every example of `tests/rules/endpoints/rollout*.veld`
//! sent to the generated test handler on the JDK server and on Spring (`veld test --live`).
//!
//! The toolchain tests need a JDK (and Spring in `~/.m2` or Maven Central for the Spring one);
//! `VELD_REQUIRE_TOOLCHAINS=1` makes a missing one a failure.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::live::*;
use common::*;
use veld_codegen::{FileSet, Role};
use veld_ir::Ir;

const ENDPOINTS: &str = "tests/rules/endpoints/rollout.veld";
const ERROR: &str = "tests/rules/endpoints/rollout_error.veld";
const CONTRACT: &str = "tests/toolchain/contract/rollout/rollout.veld";

/// The contract of the round trip: Java serves SSE on the JDK server but not WebSockets, so the `ws`
/// endpoint of the shared contract is left out.
fn contract_ir() -> Ir {
    let source = std::fs::read_to_string(repo_root().join(CONTRACT)).unwrap();
    let plain: String = source
        .lines()
        .filter(|l| !l.trim_start().starts_with("ws "))
        .collect::<Vec<_>>()
        .join("\n");
    compile_source(plain, "bk", CONTRACT)
}

fn endpoints_ir() -> Ir {
    compile_path(ENDPOINTS, "bk")
}

/// The same contract with every `@rollout(audit)` line taken out.
fn endpoints_without_rollout() -> Ir {
    let source = std::fs::read_to_string(repo_root().join(ENDPOINTS)).unwrap();
    let plain: String = source
        .lines()
        .filter(|l| !l.trim_start().starts_with("@rollout(audit)"))
        .collect::<Vec<_>>()
        .join("\n");
    compile_source(plain, "bk", ENDPOINTS)
}

fn generate(ir: &Ir, role: Role, framework: Option<&str>, base: &str, testing: bool) -> FileSet {
    let mut o = options(base, Some("17"));
    if testing {
        o["testing"] = serde_json::json!(true);
    }
    let all = ir.services.iter().map(|s| s.id).collect();
    run(ir, role, all, vec![], framework, o)
}

// ---- snapshots and the shape of the output ------------------------------------------------

#[test]
fn snapshot_of_the_server_files_of_an_audited_contract() {
    let files = generate(&endpoints_ir(), Role::Server, None, "gen.p", true);
    let paths: Vec<&str> = files.paths().filter(|p| !p.contains("/runtime/")).collect();
    insta::assert_snapshot!("rollout_server_file_list", paths.join("\n"));
}

#[test]
fn snapshot_of_the_handler_of_an_audited_contract() {
    let files = generate(&endpoints_ir(), Role::Server, None, "gen.p", true);
    insta::assert_snapshot!(
        "rollout_parties_handler",
        text(&files, "gen/p/bk/server/PartiesHandler.java")
    );
}

#[test]
fn snapshot_of_the_test_handler_of_an_audited_contract() {
    let files = generate(&endpoints_ir(), Role::Server, None, "gen.p", true);
    insta::assert_snapshot!(
        "rollout_parties_test_handler",
        text(&files, "gen/p/bk/server/PartiesTestHandler.java")
    );
}

#[test]
fn snapshot_of_a_model_with_an_audited_invariant() {
    let files = generate(&endpoints_ir(), Role::Client, None, "gen.p", false);
    insta::assert_snapshot!(
        "rollout_mixed_model",
        text(&files, "gen/p/bk/bk/Mixed.java")
    );
}

#[test]
fn snapshot_of_the_audit_runtime_class() {
    let files = generate(&endpoints_ir(), Role::Server, None, "gen.p", false);
    insta::assert_snapshot!(
        "rollout_audit_class",
        text(&files, "gen/p/runtime/Audit.java")
    );
}

#[test]
fn an_audited_rule_adds_the_audit_class_and_a_contract_without_one_does_not() {
    let audited = generate(&endpoints_ir(), Role::Server, None, "gen.p", true);
    assert!(audited.get("gen/p/runtime/Audit.java").is_some());
    let rules = text(&audited, "gen/p/runtime/EndpointRules.java");
    assert!(rules.contains("requireAudited") && rules.contains("ensureAudited"));
    assert!(text(&audited, "gen/p/runtime/TestSupport.java").contains("auditedHandler"));
    let plain = generate(
        &endpoints_without_rollout(),
        Role::Server,
        None,
        "gen.p",
        true,
    );
    assert!(plain.get("gen/p/runtime/Audit.java").is_none());
    for (path, bytes) in plain.iter().filter(|(p, _)| p.ends_with(".java")) {
        let text = String::from_utf8_lossy(bytes);
        for word in [
            "Audit.",
            "requireAudited",
            "ensureAudited",
            "auditedHandler",
            "veld-test-audited",
        ] {
            assert!(!text.contains(word), "`{word}` leaked into {path}");
        }
    }
}

// ---- toolchain ------------------------------------------------------------------------------

#[test]
fn the_audited_server_and_client_compile_with_every_warning_on() {
    if !require_javac("the_audited_server_and_client_compile") {
        return;
    }
    let ir = endpoints_ir();
    for release in releases() {
        let work = tempfile::tempdir().unwrap();
        let mut sources = Vec::new();
        for (base, role, testing) in [
            ("gen.s", Role::Server, true),
            ("gen.c", Role::Client, false),
        ] {
            let mut o = options(base, Some(release));
            o["testing"] = serde_json::json!(testing);
            let all = ir.services.iter().map(|s| s.id).collect();
            sources.extend(write_files(
                work.path(),
                &run(&ir, role, all, vec![], None, o),
            ));
        }
        javac_ok(work.path(), &sources, release);
    }
}

/// Compiles the audited server (`gen.r`) and client (`gen.c`) and the given driver, and runs it.
fn run_driver(driver: &str, class: &str, framework: Option<&str>) {
    if !require_javac(class) {
        return;
    }
    let ir = contract_ir();
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(
        work.path(),
        &generate(&ir, Role::Server, framework, "gen.r", false),
    );
    sources.extend(write_files(
        work.path(),
        &generate(&ir, Role::Client, None, "gen.c", false),
    ));
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
fn audited_rules_report_and_never_reject_over_http_and_sse() {
    run_driver("RolloutHttp.java", "veldtest.RolloutHttp", None);
}

/// Sends every example of a contract to its generated test handler on the JDK server.
fn live_examples(path: &str, base: &str, driver: &str, class: &str, expected: usize) {
    let ir = compile_path(path, "bk");
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &generate(&ir, Role::Server, None, base, true));
    sources.push(repo_root().join("tests/toolchain/java/rules").join(driver));
    javac_ok(work.path(), &sources, "17");
    let server = Running::start(work.path(), class, &[]);
    let passed = run_every_example(&ir, &server.url);
    eprintln!("examples of {path} passed on the JDK test handler: {passed}");
    assert_eq!(passed, expected, "every example of {path}");
}

#[test]
fn every_example_of_the_rollout_contracts_passes_on_the_jdk_test_handler() {
    if !require_javac("every_example_of_the_rollout_contracts_passes") {
        return;
    }
    live_examples(
        ENDPOINTS,
        "gen.p",
        "RolloutServe.java",
        "veldtest.RolloutServe",
        11,
    );
    live_examples(
        ERROR,
        "gen.e",
        "RolloutErrorServe.java",
        "veldtest.RolloutErrorServe",
        3,
    );
}

#[test]
fn every_example_of_the_rollout_contract_passes_on_the_spring_test_handler() {
    let name = "every_example_of_the_rollout_contract_passes_on_spring";
    if !require_javac(name) || !spring_available(name) {
        return;
    }
    let ir = endpoints_ir();
    let files = generate(&ir, Role::Server, Some("spring"), "gen.p", true);
    let app = start_spring(
        "spring-rollout",
        &files,
        &["rules/spring-rollout/App.java"],
        "/__veld/test",
        &[],
    );
    let passed = run_every_example(&ir, &format!("http://127.0.0.1:{}", app.port));
    assert_eq!(passed, 11, "every example of {ENDPOINTS}");
}
