//! The audit log in generated Rust (`spec/platform.md` section 4, `spec/targets/rust.md` section
//! 18.4): what is generated for `tests/toolchain/contract/platform_p1/audit.veld`, the 17 cases of
//! `tests/wire/audit` through the production handlers (`tests/toolchain/rust/audit-goldens`), the 11
//! cases of `tests/wire/redaction` through `veld::platform::redact`
//! (`tests/toolchain/rust/redaction-goldens`), and the production handler
//! (`tests/toolchain/rust/audit-harness`). `check_audited_ws_is_refused` is in `tests/conformance.rs`.
//!
//! Without `cargo` the toolchain tests are skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::*;
use serde_json::json;
use veld_codegen::{FileSet, Role};
use veld_ir::Ir;

const AUDIT: &str = "tests/toolchain/contract/platform_p1/audit.veld";
const GOLDEN_CONTRACT: &str = "tests/wire/audit/contract.veld";
const GOLDEN_CASES: &str = "tests/wire/audit/cases.json";
const REDACTION_CONTRACT: &str = "tests/wire/redaction/contract.veld";
const REDACTION_CASES: &str = "tests/wire/redaction/cases.json";

fn audit_ir() -> Ir {
    compile_path(AUDIT, "bookings")
}

fn server(ir: &Ir, testing: bool) -> FileSet {
    let all = ir.services.iter().map(|s| s.id).collect();
    run(
        ir,
        Role::Server,
        all,
        vec![],
        None,
        json!({ "crate-name": "bookings-server", "testing": testing }),
    )
}

#[test]
fn the_server_and_the_test_handler_are_snapshotted() {
    let files = server(&audit_ir(), true);
    insta::assert_snapshot!("audit_models", text(&files, "src/bookings/bookings.rs"));
    insta::assert_snapshot!("audit_server", text(&files, "src/bookings/notes_server.rs"));
    insta::assert_snapshot!(
        "audit_test_handler",
        text(&files, "src/bookings/notes_test_server.rs")
    );
    insta::assert_snapshot!(
        "audit_runtime_platform",
        text(&files, "src/veld/platform.rs")
    );
    insta::assert_snapshot!("audit_runtime_server", text(&files, "src/veld/server.rs"));
}

/// What is generated: the required hook (a server without it does not compile, and has no
/// `NoHooks`), the spec of each audited endpoint with its own errors, the principal for the event,
/// the facts noted after they are loaded, the marks of the schemas, and the test handler records
/// nothing (it does not forward the hook).
#[test]
fn the_hook_is_required_and_the_trail_is_generated() {
    let files = server(&audit_ir(), true);
    let notes = text(&files, "src/bookings/notes_server.rs");
    for word in [
        "fn audit<'a>(&'a self, event: &'a veld::platform::AuditEvent) -> impl std::future::Future<Output = Result<(), veld::Error>> + Send + 'a;",
        "fn audit_spec(&self, endpoint: usize) -> Option<veld::platform::AuditSpec>",
        "0 => Some(veld::platform::AuditSpec { errors: &[] }),",
        "_ => None,",
        "fn audit_principal(&self, ctx: &veld::server::RequestContext)",
        "veld::platform::note_fact(ctx, \"note\",",
        "self.hooks.audit(event)",
    ] {
        assert!(notes.contains(word), "{word}\n{notes}");
    }
    assert!(
        !notes.contains("pub struct NoHooks"),
        "a hook with no default has no NoHooks"
    );
    let models = text(&files, "src/bookings/bookings.rs");
    assert!(models.contains("pii: true,") && models.contains("pii: false,"));
    let handler = text(&files, "src/bookings/notes_test_server.rs");
    assert!(
        !handler.contains("audit_spec") && !handler.contains("fn audit"),
        "the test handler records nothing"
    );
    let core = text(&files, "src/veld/server.rs");
    for word in [
        "struct AuditHold<E: Endpoints>",
        "std::thread::panicking()",
        "async fn run_inner(",
        "fn spawn_audit<",
    ] {
        assert!(core.contains(word), "{word}");
    }
    let manifest = text(&files, "Cargo.toml");
    assert!(
        manifest.contains("\"rt\""),
        "the delivery of an owed event needs tokio's `rt`: {manifest}"
    );
}

/// A contract without an audited endpoint has none of the words, in the runtime either.
#[test]
fn a_contract_without_it_has_none_of_the_words() {
    let ir = compile_path("tests/rules/endpoints/policies.veld", "bookings");
    let server = run(
        &ir,
        Role::Server,
        vec![service_id(&ir, "Bookings")],
        vec![],
        None,
        json!({ "crate-name": "bookings-server", "testing": true }),
    );
    for (path, bytes) in server.iter() {
        let body = String::from_utf8_lossy(bytes);
        for word in [
            "AuditEvent",
            "AuditHold",
            "audit_spec",
            "note_fact",
            "note_parameters",
            "veld::platform::redact",
            "run_inner",
            "pii:",
        ] {
            assert!(!body.contains(word), "{path}: {word}");
        }
    }
}

/// A service with an audited endpoint has no hook that a contract without one lacks: `@audited` on
/// a service reaches its endpoints that do not stream (`Public` of the goldens).
#[test]
fn an_audited_service_has_the_hook_too() {
    let ir = compile_path(GOLDEN_CONTRACT, "bookings");
    let files = run(
        &ir,
        Role::Server,
        vec![service_id(&ir, "Public")],
        vec![],
        None,
        json!({ "crate-name": "bookings-server" }),
    );
    let public = text(&files, "src/bookings/public_server.rs");
    assert!(
        public.contains("fn audit<'a>(&'a self, event: &'a veld::platform::AuditEvent)"),
        "{public}"
    );
    assert!(
        public.contains("0 => Some(veld::platform::AuditSpec"),
        "{public}"
    );
}

/// The 17 cases of `tests/wire/audit` through the production handlers of the goldens' contract
/// with `@limit` on `payBooking` (the operation signature does not change), with recording hooks:
/// the canonical bytes of the event, the status and the error.
#[test]
fn the_goldens_hold_through_the_generated_server() {
    if !cargo_or_skip() {
        return;
    }
    let mut contract = std::fs::read_to_string(repo_root().join(GOLDEN_CONTRACT)).unwrap();
    let before = "  @audited\n  post \"/{id}/pay\" payBooking";
    assert!(contract.contains(before));
    contract = contract.replacen(
        before,
        "  @audited\n  @limit(10, per: minute, by: user)\n  post \"/{id}/pay\" payBooking",
        1,
    );
    let ir = compile_text(&contract, "bookings");
    let server = run(
        &ir,
        Role::Server,
        ir.services.iter().map(|s| s.id).collect(),
        vec![],
        None,
        json!({ "crate-name": "bookings-server" }),
    );
    let client = gen_client(&ir, "bookings-client", None);
    let ws = harness_workspace("audit-goldens-ws", &server, &client, "audit-goldens");
    let cases = repo_root().join(GOLDEN_CASES);
    build_lint_and_run(
        &ws,
        ("bookings-server", "bookings-client", "audit-goldens"),
        &[cases.to_str().unwrap()],
        "audit goldens ok: 17",
    );
}

/// The 11 cases of `tests/wire/redaction`: the redaction of the audit log, written once.
#[test]
fn the_redaction_goldens_hold() {
    if !cargo_or_skip() {
        return;
    }
    let mut contract = std::fs::read_to_string(repo_root().join(REDACTION_CONTRACT)).unwrap();
    contract.push_str(
        "\n@auth(bearer)\nservice Probe at \"/probe\" {\n  publishes GuestEvents\n  @audited\n  get \"/address\" address() -> Address\n  @audited\n  get \"/guest\" guest() -> Guest\n  @audited\n  get \"/staff\" staff() -> Staff\n  @audited\n  get \"/order\" order() -> Order | Rejected\n}\n",
    );
    let ir = compile_text(&contract, "guests");
    let server = run(
        &ir,
        Role::Server,
        vec![service_id(&ir, "Probe")],
        vec![],
        None,
        json!({ "crate-name": "guests-server" }),
    );
    let client = gen_client(&ir, "guests-client", None);
    let ws = harness_workspace(
        "redaction-goldens-ws",
        &server,
        &client,
        "redaction-goldens",
    );
    let cases = repo_root().join(REDACTION_CASES);
    build_lint_and_run(
        &ws,
        ("guests-server", "guests-client", "redaction-goldens"),
        &[cases.to_str().unwrap()],
        "redaction goldens ok: 11",
    );
}

/// The production handler: a success, a body, a fact and the one reading of the clock, a handler
/// that fails and one that panics, a deadline, a `401`, an endpoint that is not audited, a hook that
/// fails, an event owed to a request that was dropped; also with `validate = false`.
#[test]
fn the_production_handler_records_every_answer() {
    if !cargo_or_skip() {
        return;
    }
    let ir = audit_ir();
    let server = server(&ir, false);
    let client = gen_client(&ir, "bookings-client", None);
    let ws = harness_workspace("audit-http-ws", &server, &client, "audit-harness");
    build_lint_and_run(
        &ws,
        ("bookings-server", "bookings-client", "audit-harness"),
        &[],
        "audit http ok",
    );
}
