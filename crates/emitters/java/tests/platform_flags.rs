//! Flags (`spec/platform.md` section 11, `spec/targets/java.md` section 16.2): the 14 examples of
//! `tests/rules/endpoints/flags.veld.pending` through the generated test handler with
//! `Veld-Test-Flags`, the production handler over the real entry point (on, off, a failing provider,
//! one reading per request, a flagged flow edge, a server without the provider), flagged invariants
//! of a request, a result and an event, the client's functions with and without the answers of a
//! function, the core vectors (`tests/rules/core/flags.json.pending`, in `rules_toolchain.rs`), the
//! snapshots, and that a contract without a flag has none of it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::live::*;
use common::*;
use veld_codegen::{FileSet, Role};
use veld_ir::Ir;

const EXAMPLES: &str = "tests/rules/endpoints/flags.veld.pending";
const CONTRACTS: &str = "tests/toolchain/contract/platform_p2";

fn contract(name: &str) -> Ir {
    compile_path(&format!("{CONTRACTS}/{name}.veld"), name)
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
fn the_files_of_a_flagged_contract_are_snapshotted() {
    let ir = contract("flags");
    let server = generate(&ir, Role::Server, "gen.b", true);
    for name in ["RoomsHooks", "RoomsHandler", "RoomsTestHandler"] {
        let path = format!("gen/b/flags/server/{name}.java");
        insta::assert_snapshot!(format!("flags_{name}"), text(&server, &path));
    }
    insta::assert_snapshot!("flags_Flags", text(&server, "gen/b/runtime/Flags.java"));
    insta::assert_snapshot!(
        "flags_Policies_server",
        text(&server, "gen/b/flags/flags/Policies.java")
    );
    let client = generate(&ir, Role::Client, "gen.c", false);
    insta::assert_snapshot!(
        "flags_RoomsClient",
        text(&client, "gen/c/flags/client/RoomsClient.java")
    );
    insta::assert_snapshot!(
        "flags_Phase_client",
        text(&client, "gen/c/flags/flags/Phase.java")
    );
    let rules = generate(&contract("flags_rules"), Role::Server, "gen.b", false);
    insta::assert_snapshot!(
        "flags_Stay",
        text(&rules, "gen/b/flags_rules/flags_rules/Stay.java")
    );
}

#[test]
fn a_contract_without_a_flag_has_none_of_it() {
    let ir = compile_path("tests/rules/endpoints/policies.veld", "bookings");
    for role in [Role::Server, Role::Client] {
        let files = generate(&ir, role, "gen.b", true);
        assert!(files.paths().all(|p| !p.ends_with("/Flags.java")));
        for (path, bytes) in files.iter() {
            let text = String::from_utf8_lossy(bytes);
            for word in [
                "Flags.",
                "flagSource",
                "hooks.flags",
                "Veld-Test-Flags",
                "invariantOn",
            ] {
                assert!(!text.contains(word), "{path} has `{word}`");
            }
        }
    }
}

#[test]
fn the_flagged_contracts_compile_with_every_warning_on() {
    if !require_javac("the_flagged_contracts_compile") {
        return;
    }
    for name in ["flags", "flags_rules", "flags_events", "all_four"] {
        let ir = contract(name);
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

#[test]
fn every_flag_example_passes_through_the_generated_test_handler_on_the_jdk_server() {
    if !require_javac("every_flag_example_passes_on_the_jdk_server") {
        return;
    }
    let ir = compile_path(EXAMPLES, "flags");
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &generate(&ir, Role::Server, "gen.b", true));
    sources.push(repo_root().join("tests/toolchain/java/platform_p2/FlagsServe.java"));
    javac_ok(work.path(), &sources, "17");
    let server = Running::start(work.path(), "veldtest.FlagsServe", &[]);
    let passed = run_every_example(&ir, &server.url);
    eprintln!("flag examples passed on the JDK test handler: {passed}");
    assert_eq!(passed, 14, "every example of the flag contract");
}

fn run_driver(name: &str, driver: &str, role: Role, package: &str, testing: bool) {
    let ir = contract(name);
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &generate(&ir, role, package, testing));
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

/// On, off, a failing provider for a statement that restricts and one that grants, one reading per
/// request, a flagged flow edge, a server without the provider, the client functions.
#[test]
fn flags_over_the_real_entry_point() {
    if !require_javac("flags_over_the_real_entry_point") {
        return;
    }
    run_driver("flags", "FlagsHttp", Role::Server, "gen.b", false);
}

/// Flagged invariants of a request and of a result, a failing provider, a value with no view.
#[test]
fn flagged_invariants_over_the_real_entry_point() {
    if !require_javac("flagged_invariants_over_the_real_entry_point") {
        return;
    }
    run_driver(
        "flags_rules",
        "FlagsRulesHttp",
        Role::Server,
        "gen.b",
        false,
    );
}

/// A flagged invariant of an event: publisher and consumer, with and without a provider.
#[test]
fn a_flagged_invariant_of_an_event_follows_the_provider_of_the_options() {
    if !require_javac("a_flagged_invariant_of_an_event") {
        return;
    }
    run_driver("flags_events", "FlagsEvents", Role::Server, "gen.b", false);
}
