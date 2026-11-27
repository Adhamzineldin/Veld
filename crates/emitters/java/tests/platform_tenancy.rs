//! Tenancy (`spec/platform.md` section 3, `spec/targets/java.md` section 17.3): the 17 examples of
//! `tests/rules/endpoints/tenancy.veld.pending` through the generated test handler, the production
//! handler over the real entry point (a fact of another tenant, an optional one, a result, a list,
//! a page, a declared error, a stream item, a caller without a tenant, a request that lacks or
//! carries the member), the client, the idempotency fingerprint, the `veldtenant` goldens (they run
//! with the other envelope goldens in `events.rs`), the snapshots, and that a contract without a
//! `@tenant` field has none of it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::live::*;
use common::*;
use veld_codegen::{FileSet, Role};
use veld_ir::Ir;

const EXAMPLES: &str = "tests/rules/endpoints/tenancy.veld.pending";
const CONTRACT: &str = "tests/toolchain/contract/platform_p1/tenancy.veld";
const KEY_CONTRACT: &str = "tests/toolchain/java/platform_p1/tenancy_key.veld";
const EVENTS_CONTRACT: &str = "tests/wire/events/tenant.veld";
const DRIVERS: &str = "tests/toolchain/java/platform_p1";

fn contract() -> Ir {
    compile_path(CONTRACT, "tenancy")
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

fn run_driver(files: &FileSet, name: &str, args: &[&str]) {
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), files);
    sources.push(driver(name));
    for release in releases() {
        javac_ok(work.path(), &sources, release);
    }
    let out = java_main(work.path(), &format!("veldtest.{name}"), args);
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains("OK"),
        "{name} failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn the_files_of_a_tenant_scoped_contract_are_snapshotted() {
    let ir = contract();
    let server = generate(&ir, Role::Server, "gen.b", true);
    for (name, path) in [
        ("TenantsHooks", "gen/b/tenancy/server/TenantsHooks.java"),
        ("TenantsHandler", "gen/b/tenancy/server/TenantsHandler.java"),
        (
            "TenantsTestHandler",
            "gen/b/tenancy/server/TenantsTestHandler.java",
        ),
        ("Booking", "gen/b/tenancy/tenancy/Booking.java"),
        ("User", "gen/b/tenancy/tenancy/User.java"),
        ("Dup", "gen/b/tenancy/tenancy/Dup.java"),
    ] {
        insta::assert_snapshot!(format!("tenancy_{name}"), text(&server, path));
    }
    // A client only has its types right: the field is not something a request sends.
    let client = generate(&ir, Role::Client, "gen.c", false);
    insta::assert_snapshot!(
        "tenancy_client_Booking",
        text(&client, "gen/c/tenancy/tenancy/Booking.java")
    );
    insta::assert_snapshot!(
        "tenancy_client_Schema",
        text(&client, "gen/c/runtime/Schema.java")
    );
}

#[test]
fn a_contract_without_a_tenant_field_has_none_of_it() {
    let ir = compile_path("tests/rules/endpoints/policies.veld", "bookings");
    for role in [Role::Server, Role::Client] {
        let files = generate(&ir, role, "gen.b", true);
        for (path, bytes) in files.iter() {
            let text = String::from_utf8_lossy(bytes);
            for word in [
                "tenantWire",
                "TenantKind",
                "veldtenant",
                "tenant_fields",
                "f.tenant()",
            ] {
                assert!(!text.contains(word), "{path} has `{word}`");
            }
        }
    }
}

#[test]
fn the_tenant_contracts_compile_with_every_warning_on() {
    if !require_javac("the_tenant_contracts_compile") {
        return;
    }
    let examples = compile_path(EXAMPLES, "tenancy");
    let key = compile_path(KEY_CONTRACT, "tenantkey");
    let events = compile_path(EVENTS_CONTRACT, "bookings");
    for ir in [contract(), examples, key, events] {
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
fn every_tenancy_example_passes_through_the_generated_test_handler_on_the_jdk_server() {
    if !require_javac("every_tenancy_example_passes_on_the_jdk_server") {
        return;
    }
    let ir = compile_path(EXAMPLES, "tenancy");
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &generate(&ir, Role::Server, "gen.b", true));
    sources.push(driver("TenancyServe"));
    javac_ok(work.path(), &sources, "17");
    let server = Running::start(work.path(), "veldtest.TenancyServe", &[]);
    let passed = run_every_example(&ir, &server.url);
    eprintln!("tenancy examples passed on the JDK test handler: {passed}");
    assert_eq!(passed, 17, "every example of the tenancy contract");
}

/// A fact of another tenant is `404`, an optional one absent, a value of another tenant in a
/// result, a list, a page, a declared error and a stream item is never sent (`500`, one event for
/// each value), a caller without a tenant owns nothing, a request may lack the member or carry it.
#[test]
fn tenancy_over_the_real_entry_point() {
    if !require_javac("tenancy_over_the_real_entry_point") {
        return;
    }
    run_driver(
        &generate(&contract(), Role::Server, "gen.b", false),
        "TenancyHttp",
        &[],
    );
}

/// The check does not depend on request validation.
#[test]
fn tenancy_does_not_depend_on_request_validation() {
    if !require_javac("tenancy_does_not_depend_on_request_validation") {
        return;
    }
    let ir = contract();
    run_driver(
        &gen_unvalidated(&ir, Role::Server, "gen.b", Some("17")),
        "TenancyHttp",
        &["unvalidated"],
    );
}

#[test]
fn the_client_leaves_the_tenant_to_the_server() {
    if !require_javac("the_client_leaves_the_tenant_to_the_server") {
        return;
    }
    run_driver(
        &generate(&contract(), Role::Client, "gen.c", false),
        "TenancyClient",
        &[],
    );
}

/// The fingerprint of an idempotency key leaves the tenant out.
#[test]
fn the_idempotency_fingerprint_leaves_the_tenant_out() {
    if !require_javac("the_idempotency_fingerprint_leaves_the_tenant_out") {
        return;
    }
    let ir = compile_path(KEY_CONTRACT, "tenantkey");
    run_driver(
        &generate(&ir, Role::Server, "gen.b", false),
        "TenancyFingerprint",
        &[],
    );
}

/// An event with a `@tenant` field carries `veldtenant`; an event without one keeps its bytes.
#[test]
fn only_an_event_with_a_tenant_carries_the_attribute() {
    let ir = compile_path(EVENTS_CONTRACT, "bookings");
    let server = generate(&ir, Role::Server, "gen.b", false);
    let events = text(&server, "gen/b/runtime/Events.java");
    assert!(events.contains("veldtenant"), "{events}");
    let plain = compile_path("tests/rules/endpoints/policies.veld", "bookings");
    let server = generate(&plain, Role::Server, "gen.b", false);
    for (path, bytes) in server.iter() {
        assert!(
            !String::from_utf8_lossy(bytes).contains("veldtenant"),
            "{path}"
        );
    }
}
