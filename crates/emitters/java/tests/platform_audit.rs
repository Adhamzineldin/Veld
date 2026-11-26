//! The audit log (`spec/platform.md` section 4, `spec/targets/java.md` section 17.4): the 11 cases of
//! `tests/wire/redaction` through the redaction of the runtime, the production handler over the real
//! entry point (the events of a success, a body, a fact, a handler that throws, a deadline and the
//! early answers, a failing hook), the 17 cases of `tests/wire/audit` through the generated server
//! (with the limiter of the case: they need `limits`, section 17.5), the snapshots, and that a
//! contract without an audited endpoint has none of it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::live::*;
use common::*;
use veld_codegen::{DynGenerator, FileSet, GenRequest, Role};
use veld_emit_java::Java;
use veld_ir::Ir;

const CONTRACT: &str = "tests/toolchain/contract/platform_p1/audit.veld";
const GOLDEN_CONTRACT: &str = "tests/wire/audit/contract.veld";
const REDACTION_CONTRACT: &str = "tests/wire/redaction/contract.veld";
const REDACTION_CASES: &str = "tests/wire/redaction/cases.json";
const DRIVERS: &str = "tests/toolchain/java/platform_p1";

/// The contract of the redaction goldens has no service: an audited one that publishes its channel
/// is appended, because the redaction is only in a server that audits.
const REDACTION_SERVICE: &str = "
@auth(none)
service Guests at \"/guests\" {
  publishes GuestEvents

  @audited
  get \"/{id}\" getGuest(id: uuid) -> Guest
}
";

fn contract() -> Ir {
    compile_path(CONTRACT, "audit")
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

/// Compiles the generated files with `driver`, runs it with `cases` (a file copied next to the
/// working directory as `cases.json`) and checks that it printed `OK`.
fn run_driver(files: &FileSet, name: &str, cases: Option<&str>, args: &[&str]) {
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), files);
    sources.push(driver(name));
    for release in releases() {
        javac_ok(work.path(), &sources, release);
    }
    if let Some(path) = cases {
        std::fs::copy(repo_root().join(path), work.path().join("cases.json")).unwrap();
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
fn the_files_of_a_contract_with_an_audited_endpoint_are_snapshotted() {
    let ir = contract();
    let server = generate(&ir, Role::Server, "gen.b", true);
    for (name, path) in [
        ("NotesHooks", "gen/b/audit/server/NotesHooks.java"),
        ("NotesHandler", "gen/b/audit/server/NotesHandler.java"),
        (
            "NotesTestHandler",
            "gen/b/audit/server/NotesTestHandler.java",
        ),
        ("Note", "gen/b/audit/audit/Note.java"),
        ("User", "gen/b/audit/audit/User.java"),
    ] {
        insta::assert_snapshot!(format!("audit_{name}"), text(&server, path));
    }
    for name in ["Platform", "Redact", "AuditEvent", "AuditFailure"] {
        insta::assert_snapshot!(
            format!("audit_{name}"),
            text(&server, &format!("gen/b/runtime/{name}.java"))
        );
    }
}

#[test]
fn the_hooks_require_the_audit_hook_by_type_and_a_client_has_none_of_it() {
    let ir = contract();
    let server = generate(&ir, Role::Server, "gen.b", false);
    let hooks = text(&server, "gen/b/audit/server/NotesHooks.java");
    assert!(
        hooks.contains("public NotesHooks(BearerAuth bearer, HandlerHooks.AuditHook audit)"),
        "{hooks}"
    );
    assert!(
        !server.paths().any(|p| p.ends_with("NotesHandler.java")
            && text(&server, p).contains("create(NotesService impl)")),
        "a server with an audited endpoint needs hooks"
    );
    let client = generate(&ir, Role::Client, "gen.c", false);
    for (path, bytes) in client.iter() {
        let text = String::from_utf8_lossy(bytes);
        for word in ["Platform", "Redact", "AuditEvent", "pii("] {
            assert!(!text.contains(word), "client: {path} has `{word}`");
        }
    }
}

#[test]
fn a_contract_without_an_audited_endpoint_has_none_of_it() {
    let ir = compile_path("tests/rules/endpoints/policies.veld", "bookings");
    for role in [Role::Server, Role::Client] {
        let files = generate(&ir, role, "gen.b", true);
        for name in ["AuditEvent", "AuditFailure", "Redact"] {
            assert!(
                files
                    .paths()
                    .all(|p| !p.ends_with(&format!("/{name}.java"))),
                "{name}"
            );
        }
        for (path, bytes) in files.iter() {
            let text = String::from_utf8_lossy(bytes);
            for word in ["AuditHook", "AuditEvent", "Platform.audited", "pii("] {
                assert!(!text.contains(word), "{path} has `{word}`");
            }
        }
    }
}

/// An audited `ws` endpoint, and an audited one that streams, are `E0801` at the endpoint, whatever
/// the generator declares (`spec/platform.md` section 9.1).
#[test]
fn an_audited_stream_is_refused_at_the_endpoint() {
    veld_emit_conformance::check_audited_ws_is_refused(&Java).unwrap();
    let ir = compile_source(
        "model Tick {\n  n: int\n}\n\nservice Ticks at \"/ticks\" {\n  @audited\n  get \"/forever\" forever() -> stream Tick\n}\n"
            .to_string(),
        "ticks",
        "an audited stream",
    );
    let req = GenRequest {
        ir: &ir,
        role: Role::Server,
        services: ir.services.iter().map(|s| s.id).collect(),
        consumes: vec![],
        framework: None,
    };
    let diags = Java
        .generate_dyn(&req, &options("gen.b", Some("17")))
        .expect_err("an audited stream must be refused");
    assert!(diags.iter().all(|d| d.code == "E0801"), "{diags:?}");
    assert!(
        diags.iter().any(|d| d.message.contains("forever")),
        "{diags:?}"
    );
}

#[test]
fn the_audited_contracts_compile_with_every_warning_on() {
    if !require_javac("the_audited_contracts_compile") {
        return;
    }
    let guests = {
        let mut text = std::fs::read_to_string(repo_root().join(REDACTION_CONTRACT)).unwrap();
        text.push_str(REDACTION_SERVICE);
        compile_source(text, "guests", REDACTION_CONTRACT)
    };
    let bookings = compile_path(GOLDEN_CONTRACT, "bookings");
    for ir in [contract(), guests, bookings] {
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

/// The 11 cases of `tests/wire/redaction/cases.json` through the redaction of the runtime.
#[test]
fn every_case_of_the_redaction_goldens_leaves_out_what_the_reference_leaves_out() {
    if !require_javac("every_case_of_the_redaction_goldens") {
        return;
    }
    let mut text = std::fs::read_to_string(repo_root().join(REDACTION_CONTRACT)).unwrap();
    text.push_str(REDACTION_SERVICE);
    let ir = compile_source(text, "guests", REDACTION_CONTRACT);
    run_driver(
        &generate(&ir, Role::Server, "gen.b", false),
        "AuditRedaction",
        Some(REDACTION_CASES),
        &[],
    );
}

/// The event of a success and of an early answer, of a handler that throws and of a deadline, the
/// one reading of the clock, a failing hook, an endpoint that is not audited and a missing hook.
#[test]
fn the_audit_log_behaves_over_the_production_handler() {
    if !require_javac("the_audit_log_behaves_over_the_production_handler") {
        return;
    }
    run_driver(
        &generate(&contract(), Role::Server, "gen.b", false),
        "AuditHttp",
        None,
        &[],
    );
}

/// `validate = false` changes nothing about the audit log: it is not an input check.
#[test]
fn the_audit_log_does_not_depend_on_request_validation() {
    if !require_javac("the_audit_log_does_not_depend_on_request_validation") {
        return;
    }
    let ir = contract();
    run_driver(
        &gen_unvalidated(&ir, Role::Server, "gen.b", Some("17")),
        "AuditHttp",
        None,
        &["unvalidated"],
    );
}
