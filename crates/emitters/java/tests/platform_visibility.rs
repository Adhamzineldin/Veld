//! Field visibility (`spec/platform.md` section 2, `spec/targets/java.md` section 17.2): the 14
//! examples of `tests/rules/endpoints/visibility.veld.pending` through the generated test handler,
//! the 23 goldens of `tests/wire/visibility` through the runtime, the production handler over the
//! real entry point (also generated with `validate = false`), the snapshots, and that a contract
//! without a `@visible` field has none of it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::live::*;
use common::*;
use veld_codegen::{FileSet, Role};
use veld_ir::Ir;

const EXAMPLES: &str = "tests/rules/endpoints/visibility.veld.pending";
const CONTRACT: &str = "tests/toolchain/contract/platform_p1/visibility.veld";
const GOLDEN_CONTRACT: &str = "tests/wire/visibility/contract.veld";
const GOLDEN_CASES: &str = "tests/wire/visibility/cases.json";
const DRIVERS: &str = "tests/toolchain/java/platform_p1";

/// The golden contract has no service (a server needs one): one that returns every declaration the
/// cases name is appended.
const GOLDEN_SERVICE: &str = "
@auth(bearer)
service Guests at \"/guests\" {
  get \"/{id}\" getGuest(id: uuid) -> Guest
  get \"/vip\" getVip() -> Vip
  get \"/order\" getOrder() -> Order
  post \"/reject\" reject() -> 204 | Rejected
}
";

fn contract() -> Ir {
    compile_path(CONTRACT, "visibility")
}

fn generate(ir: &Ir, role: Role, package: &str, testing: bool) -> FileSet {
    let mut o = options(package, Some("17"));
    if testing {
        o["testing"] = serde_json::json!(true);
    }
    let all = ir.services.iter().map(|s| s.id).collect();
    run(ir, role, all, vec![], None, o)
}

fn generate_unvalidated(ir: &Ir) -> FileSet {
    gen_unvalidated(ir, Role::Server, "gen.b", Some("17"))
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
fn the_files_of_a_contract_with_visible_fields_are_snapshotted() {
    let ir = contract();
    let server = generate(&ir, Role::Server, "gen.b", true);
    for (name, path) in [
        ("DeskHooks", "gen/b/visibility/server/DeskHooks.java"),
        ("DeskHandler", "gen/b/visibility/server/DeskHandler.java"),
        (
            "DeskTestHandler",
            "gen/b/visibility/server/DeskTestHandler.java",
        ),
        ("Booking", "gen/b/visibility/visibility/Booking.java"),
        ("Guest", "gen/b/visibility/visibility/Guest.java"),
        ("Refused", "gen/b/visibility/visibility/Refused.java"),
    ] {
        insta::assert_snapshot!(format!("visibility_{name}"), text(&server, path));
    }
    insta::assert_snapshot!(
        "visibility_Platform",
        text(&server, "gen/b/runtime/Platform.java")
    );
    // A client has nothing to enforce: its models are the models of before.
    let client = generate(&ir, Role::Client, "gen.c", false);
    insta::assert_snapshot!(
        "visibility_client_Booking",
        text(&client, "gen/c/visibility/visibility/Booking.java")
    );
}

#[test]
fn a_contract_without_a_visible_field_has_none_of_it() {
    let ir = compile_path("tests/rules/endpoints/policies.veld", "bookings");
    for role in [Role::Server, Role::Client] {
        let files = generate(&ir, role, "gen.b", true);
        assert!(files.paths().all(|p| !p.ends_with("/Platform.java")));
        for (path, bytes) in files.iter() {
            let text = String::from_utf8_lossy(bytes);
            for word in ["Platform", "showWire", "visibleTo", "clockReading"] {
                assert!(!text.contains(word), "{path} has `{word}`");
            }
        }
    }
}

#[test]
fn a_client_gets_the_models_of_before() {
    let ir = contract();
    let client = generate(&ir, Role::Client, "gen.c", false);
    for (path, bytes) in client.iter() {
        let text = String::from_utf8_lossy(bytes);
        for word in ["Platform", "showWire", "visibleTo"] {
            assert!(!text.contains(word), "client: {path} has `{word}`");
        }
    }
}

#[test]
fn the_visible_contracts_compile_with_every_warning_on() {
    if !require_javac("the_visible_contracts_compile") {
        return;
    }
    let examples = compile_path(EXAMPLES, "visibility");
    for ir in [contract(), examples] {
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
fn every_visibility_example_passes_through_the_generated_test_handler_on_the_jdk_server() {
    if !require_javac("every_visibility_example_passes_on_the_jdk_server") {
        return;
    }
    let ir = compile_path(EXAMPLES, "visibility");
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &generate(&ir, Role::Server, "gen.b", true));
    sources.push(driver("VisibilityServe"));
    javac_ok(work.path(), &sources, "17");
    let server = Running::start(work.path(), "veldtest.VisibilityServe", &[]);
    let passed = run_every_example(&ir, &server.url);
    eprintln!("visibility examples passed on the JDK test handler: {passed}");
    assert_eq!(passed, 14, "every example of the visibility contract");
}

/// The 23 cases of `tests/wire/visibility/cases.json`: the value after visibility, canonical, and
/// the paths of what was dropped, from the function of the runtime.
#[test]
fn every_golden_of_visibility_gives_the_bytes_of_the_reference() {
    if !require_javac("every_golden_of_visibility") {
        return;
    }
    let mut text = std::fs::read_to_string(repo_root().join(GOLDEN_CONTRACT)).unwrap();
    text.push_str(GOLDEN_SERVICE);
    let ir = compile_source(text, "guests", GOLDEN_CONTRACT);
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &generate(&ir, Role::Server, "gen.b", false));
    sources.push(driver("VisibilityGoldens"));
    for release in releases() {
        javac_ok(work.path(), &sources, release);
    }
    std::fs::copy(
        repo_root().join(GOLDEN_CASES),
        work.path().join("cases.json"),
    )
    .unwrap();
    let out = java_main(work.path(), "veldtest.VisibilityGoldens", &[]);
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains("OK"),
        "VisibilityGoldens failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A field hidden for one caller and present for another, in a nested model, a list, a page, a
/// declared error and a stream item; an expression that cannot be evaluated; the clock to the
/// millisecond; an `ensure` that still sees the field; an endpoint with no caller; output `off`.
#[test]
fn visibility_over_the_real_entry_point() {
    if !require_javac("visibility_over_the_real_entry_point") {
        return;
    }
    run_driver(
        &generate(&contract(), Role::Server, "gen.b", false),
        "VisibilityHttp",
        &[],
    );
}

/// `validate = false` changes nothing about visibility: it is not an input check.
#[test]
fn visibility_does_not_depend_on_request_validation() {
    if !require_javac("visibility_does_not_depend_on_request_validation") {
        return;
    }
    run_driver(
        &generate_unvalidated(&contract()),
        "VisibilityHttp",
        &["unvalidated"],
    );
}
