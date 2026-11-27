//! The four features of platform phase P1 together (`spec/targets/java.md` section 17):
//! `tests/toolchain/contract/platform_p1/all_four.veld` compiled as a server (with the test handler,
//! for the JDK server and for Spring) and as a client, with every warning on, and its files
//! snapshotted.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::live::*;
use common::*;
use veld_codegen::{FileSet, Role};
use veld_ir::Ir;

const CONTRACT: &str = "tests/toolchain/contract/platform_p1/all_four.veld";

fn contract() -> Ir {
    compile_path(CONTRACT, "all_four")
}

fn generate(ir: &Ir, role: Role, package: &str, testing: bool, framework: Option<&str>) -> FileSet {
    let mut o = options(package, Some("17"));
    if testing {
        o["testing"] = serde_json::json!(true);
    }
    let all = ir.services.iter().map(|s| s.id).collect();
    run(ir, role, all, vec![], framework, o)
}

#[test]
fn the_four_features_together_are_snapshotted() {
    let ir = contract();
    let server = generate(&ir, Role::Server, "gen.b", true, None);
    for name in ["BookingsHooks", "BookingsHandler", "BookingsTestHandler"] {
        insta::assert_snapshot!(
            format!("all_four_p1_{name}"),
            text(&server, &format!("gen/b/all_four/server/{name}.java"))
        );
    }
    for name in ["Booking", "User", "Refused"] {
        insta::assert_snapshot!(
            format!("all_four_p1_{name}"),
            text(&server, &format!("gen/b/all_four/all_four/{name}.java"))
        );
    }
    let client = generate(&ir, Role::Client, "gen.c", false, None);
    insta::assert_snapshot!(
        "all_four_p1_BookingsClient",
        text(&client, "gen/c/all_four/client/BookingsClient.java")
    );
}

#[test]
fn the_four_features_compile_with_every_warning_on_in_both_roles() {
    if !require_javac("the_four_features_compile") {
        return;
    }
    let ir = contract();
    for (role, package, testing) in [
        (Role::Server, "gen.b", false),
        (Role::Server, "gen.b", true),
        (Role::Client, "gen.c", false),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let sources = write_files(dir.path(), &generate(&ir, role, package, testing, None));
        for release in releases() {
            javac_ok(dir.path(), &sources, release);
        }
    }
}

/// The hooks that the four features need are in the type of the hooks of the service: the caller
/// (limits `by: user`), the audit log and the address (`by: ip`).
#[test]
fn the_hooks_of_the_four_features_are_required_by_type() {
    let server = generate(&contract(), Role::Server, "gen.b", false, None);
    let hooks = text(&server, "gen/b/all_four/server/BookingsHooks.java");
    assert!(
        hooks.contains("public BookingsHooks(BearerAuth bearer, Caller caller, HandlerHooks.AuditHook audit, HandlerHooks.AddressHook address)"),
        "{hooks}"
    );
    assert!(hooks.contains("limiter(Limiter value)"), "{hooks}");
}

/// The four features of P1 next to the four of P2 and an idempotency key (`p1_with_p2.veld`: a
/// deadline, a service level, flags, a timed edge): the regions of the runtime that meet in the router
/// pair up and the output compiles with every warning on, in both roles, with and without the test
/// handler.
#[test]
fn the_features_of_p1_and_p2_compile_together_with_every_warning_on() {
    if !require_javac("the_features_of_p1_and_p2_compile_together") {
        return;
    }
    let ir = compile_path("tests/toolchain/java/platform_p1/p1_with_p2.veld", "mega");
    for (role, package, testing) in [
        (Role::Server, "gen.b", false),
        (Role::Server, "gen.b", true),
        (Role::Client, "gen.c", false),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let sources = write_files(dir.path(), &generate(&ir, role, package, testing, None));
        for release in releases() {
            javac_ok(dir.path(), &sources, release);
        }
    }
}
