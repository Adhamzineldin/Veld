//! Field visibility in generated Rust (`spec/platform.md` section 2, `spec/targets/rust.md`
//! section 18.2): what is generated for `tests/toolchain/contract/platform_p1/visibility.veld`, the
//! goldens of `tests/wire/visibility` through `veld::platform::show_value`, the 14 examples of
//! `tests/rules/endpoints/visibility.veld.pending` through the generated test handler, and the
//! production handler (`tests/toolchain/rust/visibility-harness`).
//!
//! Without `cargo` the toolchain tests are skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::*;
use serde_json::json;
use veld_codegen::{FileSet, Role};
use veld_ir::Ir;

const VISIBILITY: &str = "tests/toolchain/contract/platform_p1/visibility.veld";
const EXAMPLES: &str = "tests/rules/endpoints/visibility.veld.pending";
const GOLDEN_CONTRACT: &str = "tests/wire/visibility/contract.veld";
const GOLDEN_CASES: &str = "tests/wire/visibility/cases.json";

fn visibility_ir() -> Ir {
    compile_path(VISIBILITY, "guests")
}

fn server(ir: &Ir, testing: bool) -> FileSet {
    let all = ir.services.iter().map(|s| s.id).collect();
    run(
        ir,
        Role::Server,
        all,
        vec![],
        None,
        json!({ "crate-name": "guests-server", "testing": testing }),
    )
}

#[test]
fn the_server_and_the_test_handler_are_snapshotted() {
    let ir = visibility_ir();
    let files = server(&ir, true);
    insta::assert_snapshot!("visibility_models", text(&files, "src/guests/guests.rs"));
    insta::assert_snapshot!(
        "visibility_server",
        text(&files, "src/guests/desk_server.rs")
    );
    insta::assert_snapshot!(
        "visibility_test_handler",
        text(&files, "src/guests/desk_test_server.rs")
    );
    insta::assert_snapshot!(
        "visibility_runtime_platform",
        text(&files, "src/veld/platform.rs")
    );
    insta::assert_snapshot!(
        "visibility_runtime_server",
        text(&files, "src/veld/server.rs")
    );
    // A client gets nothing of it: the field is optional in its types already.
    let client = gen_client(&ir, "guests-client", None);
    insta::assert_snapshot!(
        "visibility_client_models",
        text(&client, "src/guests/guests.rs")
    );
}

/// What is generated: the `Visible` impl of each declaration with a `@visible` field, the `show`
/// function of its schema, the typed hooks, the clock hook and the one reading of the clock.
#[test]
fn the_declarations_the_schemas_and_the_hooks_have_the_feature() {
    let files = server(&visibility_ir(), true);
    let models = text(&files, "src/guests/guests.rs");
    for word in [
        "impl veld::platform::Visible for Booking",
        "const FIELDS: &'static [&'static str] = &[\"note\", \"ratio\", \"early\"];",
        "show: Some(veld::platform::show_record::<Booking>),",
        "show: Some(veld::platform::show_record::<Refused>),",
        "show: None,",
        "caller.and_then(|c| c.get::<User>())",
        "None => false,",
    ] {
        assert!(models.contains(word), "{word}\n{models}");
    }
    let desk = text(&files, "src/guests/desk_server.rs");
    for word in [
        "Output = Option<crate::guests::guests::User>",
        "fn clock(&self, _ctx: &veld::server::RequestContext) -> std::time::SystemTime",
        "fn request_clock(&self, ctx: &veld::server::RequestContext) -> std::time::SystemTime",
        "let now = ctx.now_ms();",
    ] {
        assert!(desk.contains(word), "{word}");
    }
    let core = text(&files, "src/veld/server.rs");
    for word in [
        "fn open(&self, ctx: &mut RequestContext)",
        "fn shown(&self, ep: &EndpointSpec, schema: &Schema, wire: &Value, ctx: &RequestContext) -> Value",
        "self.item_text(ep, schema, &ev.data, ctx)",
        "ctx.platform.note_hidden(&shown.hidden);",
    ] {
        assert!(core.contains(word), "{word}");
    }
    assert!(
        !core.contains("fn tenant_refused"),
        "tenancy is not in this contract"
    );
    let platform = text(&files, "src/veld/platform.rs");
    assert!(platform.contains("pub fn show_value(") && !platform.contains("pub struct AuditEvent"));
}

/// A contract without a `@visible` field has none of the words, in the runtime either: it generates
/// what it generated before.
#[test]
fn a_contract_without_it_has_none_of_the_words() {
    let ir = compile_path("tests/rules/endpoints/transitions.veld", "bookings");
    let server = run(
        &ir,
        Role::Server,
        vec![service_id(&ir, "Bookings")],
        vec![],
        None,
        json!({ "crate-name": "bookings-server", "testing": true }),
    );
    let client = gen_client(&ir, "bookings-client", None);
    for files in [&server, &client] {
        assert!(files.get("src/veld/platform.rs").is_none());
        for (path, bytes) in files.iter() {
            let body = String::from_utf8_lossy(bytes);
            for word in [
                "veld::platform",
                "Visible",
                "show_record",
                "request_clock",
                "fn now_ms(",
                "item_text",
                "note_hidden",
            ] {
                assert!(!body.contains(word), "{path}: {word}");
            }
        }
    }
}

/// The goldens' contract with a service appended that returns each of its declarations: a server
/// has the feature only when something it sends can reach a `@visible` field.
fn goldens_contract() -> Ir {
    let mut text = std::fs::read_to_string(repo_root().join(GOLDEN_CONTRACT)).unwrap();
    text.push_str(
        "\n@auth(bearer)\nservice Probe at \"/probe\" {\n  get \"/address\" address() -> Address\n  get \"/guest\" guest() -> Guest\n  get \"/vip\" vip() -> Vip\n  get \"/card\" card() -> Card\n  get \"/wire\" wire() -> Wire\n  get \"/payment\" payment() -> Payment\n  get \"/order\" order() -> Order | Rejected\n}\n",
    );
    compile_text(&text, "guests")
}

/// The 23 cases of `tests/wire/visibility`: the bytes of what is shown and the paths that were
/// dropped, through the schema walk of the generated crate.
#[test]
fn the_goldens_hold_through_the_generated_walk() {
    if !cargo_or_skip() {
        return;
    }
    let ir = goldens_contract();
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
        "visibility-goldens-ws",
        &server,
        &client,
        "visibility-goldens",
    );
    let cases = repo_root().join(GOLDEN_CASES);
    build_lint_and_run(
        &ws,
        ("guests-server", "guests-client", "visibility-goldens"),
        &[cases.to_str().unwrap()],
        "visibility goldens ok: 23",
    );
}

/// The production handler: a field hidden for one caller and present for another, in a list and a
/// page, an expression that fails, the clock, an `ensure`, a declared error, no caller, and the
/// settings that change nothing.
#[test]
fn the_production_handler_shows_each_caller_what_it_may_see() {
    if !cargo_or_skip() {
        return;
    }
    let ir = visibility_ir();
    let server = server(&ir, false);
    let client = gen_client(&ir, "guests-client", None);
    let ws = harness_workspace("visibility-http-ws", &server, &client, "visibility-harness");
    build_lint_and_run(
        &ws,
        ("guests-server", "guests-client", "visibility-harness"),
        &[],
        "visibility http ok",
    );
}

/// The 14 examples through the generated test handlers (one per service: `Bookings` has a
/// principal, `Notices` has no caller); a skipped example is a failure.
#[test]
fn the_examples_pass_through_the_generated_test_handlers() {
    if !cargo_or_skip() {
        return;
    }
    let ir = compile_path(EXAMPLES, "guests");
    let server = run(
        &ir,
        Role::Server,
        ir.services.iter().map(|s| s.id).collect(),
        vec![],
        None,
        json!({ "crate-name": "guests-server", "testing": true }),
    );
    let client = gen_client(&ir, "guests-client", None);
    let ws = harness_workspace(
        "visibility-examples-ws",
        &server,
        &client,
        "visibility-examples",
    );
    run_ok(
        &mut cargo(&ws, &["build", "--workspace", "--bins"]),
        "cargo build",
    );
    run_ok(
        &mut cargo(
            &ws,
            &[
                "clippy",
                "--all-targets",
                "-p",
                "guests-server",
                "-p",
                "guests-client",
                "--",
                "-D",
                "warnings",
            ],
        ),
        "cargo clippy",
    );
    let bookings = Running::start("visibility-serve-bookings");
    let notices = Running::start("visibility-serve-notices");
    let lives = [
        (
            "Bookings",
            veld_testing::Live::connect(&bookings.url, true).unwrap(),
        ),
        (
            "Notices",
            veld_testing::Live::connect(&notices.url, true).unwrap(),
        ),
    ];
    let (mut passed, mut failures) = (0, Vec::new());
    for ex in &ir.examples {
        let service = ex
            .service
            .map(|s| ir.services[s.index()].name.as_str())
            .unwrap_or_default();
        let live = &lives
            .iter()
            .find(|(name, _)| *name == service)
            .expect("a service with a handler")
            .1;
        match live.run_example(&ir, ex) {
            Ok(report) if report.passed() => passed += 1,
            Ok(report) => failures.push(format!("\"{}\": {report}", ex.name)),
            Err(e) => failures.push(format!("\"{}\": {e}", ex.name)),
        }
    }
    assert!(
        failures.is_empty(),
        "{} examples failed or were skipped:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert_eq!(passed, 14, "every example of the contract");
}
