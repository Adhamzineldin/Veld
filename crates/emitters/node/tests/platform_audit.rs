//! The audit log in the generated TypeScript (`spec/targets/typescript.md` section 5k,
//! `spec/platform.md` section 4).
//!
//! Without `node` or `node_modules` the toolchain tests are skipped unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::toolchain::*;
use common::*;
use serde_json::json;
use veld_codegen::{DynGenerator, GenRequest, Role};
use veld_emit_node::Node;

const CONTRACT: &str = "tests/toolchain/contract/platform_p1/audit.veld";
const GOLDEN_CONTRACT: &str = "tests/wire/audit/contract.veld";
const GOLDEN_CASES: &str = "tests/wire/audit/cases.json";
const REDACTION_CONTRACT: &str = "tests/wire/redaction/contract.veld";
const REDACTION_CASES: &str = "tests/wire/redaction/cases.json";

/// The 17 cases of `tests/wire/audit/cases.json` through the production handler with the stubs of
/// each case; the case of the limiter is not run (the contract has no `@limit`).
#[test]
fn every_case_of_the_audit_goldens_gives_the_event_of_the_reference() {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_as_t(std::fs::read_to_string(repo_root().join(GOLDEN_CONTRACT)).unwrap());
    for (edition, _) in TARGETS {
        let guard = scratch("audit-goldens-");
        let work = guard.path();
        build_services(
            work,
            &tsc,
            &ir,
            &["Bookings", "Public"],
            &["audit_goldens.ts"],
            edition,
            &json!({}),
        );
        std::fs::copy(repo_root().join(GOLDEN_CASES), work.join("cases.json")).unwrap();
        run(
            std::process::Command::new("node")
                .arg("out/audit_goldens.js")
                .current_dir(work),
            "node audit_goldens.js",
        );
    }
}

/// The 11 cases of `tests/wire/redaction/cases.json` through the redaction of the runtime. The
/// contract has no service, so an audited one that publishes its channel is appended: the redaction
/// is only in a server that audits.
#[test]
fn every_case_of_the_redaction_goldens_leaves_out_what_the_reference_leaves_out() {
    let Some(tsc) = tsc_path() else { return };
    let mut text = std::fs::read_to_string(repo_root().join(REDACTION_CONTRACT)).unwrap();
    text.push_str(
        "\n@auth(none)\nservice Guests at \"/guests\" {\n  publishes GuestEvents\n\n  @audited\n  get \"/{id}\" getGuest(id: uuid) -> Guest\n}\n",
    );
    let ir = compile_text(text);
    for (edition, _) in TARGETS {
        let guard = scratch("redaction-goldens-");
        let work = guard.path();
        build_service(work, &tsc, &ir, "Guests", &["audit_redaction.ts"], edition);
        std::fs::copy(repo_root().join(REDACTION_CASES), work.join("cases.json")).unwrap();
        run(
            std::process::Command::new("node")
                .arg("out/audit_redaction.js")
                .current_dir(work),
            "node audit_redaction.js",
        );
    }
}

/// The production handler: the event of a success and of an early answer, of a handler that
/// throws and of a deadline, the one reading of the clock, a failing hook, an endpoint that is not
/// audited, and the settings that change nothing.
fn http_run(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(CONTRACT);
    let guard = scratch(prefix);
    let work = guard.path();
    build_service(work, &tsc, &ir, "Notes", &["audit_http.ts"], edition);
    run(
        std::process::Command::new("node")
            .arg("out/audit_http.js")
            .current_dir(work),
        "node audit_http.js",
    );
}

#[test]
fn the_audit_log_behaves_over_the_production_handler_es2020() {
    http_run("es2020", "audit-http-2020-");
}

#[test]
fn the_audit_log_behaves_over_the_production_handler_es2022() {
    http_run("es2022", "audit-http-2022-");
}

#[test]
fn snapshots_of_a_contract_with_an_audited_endpoint() {
    let ir = compile_path(CONTRACT);
    let server = gen_server_with(&ir, "Notes", &[], &json!({ "testing": true }));
    insta::assert_snapshot!(
        "audit_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!("audit_server_models", text(&server, "models/bookings.ts"));
    insta::assert_snapshot!("audit_server_module", text(&server, "server/notes.ts"));
    insta::assert_snapshot!(
        "audit_test_handler",
        text(&server, "server/notes-test-handler.ts")
    );
    let client = gen_client(&ir);
    insta::assert_snapshot!(
        "audit_client_models",
        text(&client, "bookings/models/bookings.ts")
    );
}

#[test]
fn the_server_requires_the_hook_and_the_client_has_none_of_it() {
    let ir = compile_path(CONTRACT);
    let server = gen_server(&ir, "Notes", &[]);
    let module = text(&server, "server/notes.ts");
    assert!(
        module.contains("audit: (event: veld.AuditEvent) => void | Promise<void>;"),
        "{module}"
    );
    assert!(module.contains("audit: { authenticated: true, principal:"));
    assert!(
        !module.contains("hooks: NotesHooks = {}"),
        "the hooks are required"
    );
    let models = text(&server, "models/bookings.ts");
    assert!(models.contains("pii: true"), "{models}");
    let runtime = text(&server, "runtime/veld.ts");
    for word in [
        "function withAudit(",
        "function redactNode(",
        "class AuditFailure",
    ] {
        assert!(runtime.contains(word), "`{word}` is missing");
    }
    let client = gen_client(&ir);
    for path in client.paths() {
        let body = String::from_utf8_lossy(client.get(path).unwrap_or_default()).into_owned();
        for word in ["withAudit", "redactNode", "AuditEvent", "pii: true"] {
            assert!(!body.contains(word), "client: `{word}` is in {path}");
        }
    }
}

/// An audited `ws` endpoint, and an audited one that streams, are `E0801` at the endpoint, whatever
/// the generator declares (`spec/platform.md` section 9.1).
#[test]
fn an_audited_stream_is_refused_at_the_endpoint() {
    veld_emit_conformance::check_audited_ws_is_refused(&Node).unwrap();
    let ir = compile_text(
        "model Tick {\n  n: int\n}\n\nservice Ticks at \"/ticks\" {\n  @audited\n  get \"/forever\" forever() -> stream Tick\n}\n"
            .to_string(),
    );
    let req = GenRequest {
        ir: &ir,
        role: Role::Server,
        services: ir.services.iter().map(|s| s.id).collect(),
        consumes: vec![],
        framework: None,
    };
    let diags = Node
        .generate_dyn(&req, &named(Role::Server, &json!({})))
        .expect_err("an audited stream must be refused");
    assert!(diags.iter().all(|d| d.code == "E0801"), "{diags:?}");
    assert!(
        diags.iter().any(|d| d.message.contains("forever")),
        "{diags:?}"
    );
}

#[test]
fn a_contract_without_an_audited_endpoint_has_none_of_the_code() {
    let ir = standard_ir();
    let server = gen_server(&ir, "Orders", &["Iam"]);
    let client = gen_client(&ir);
    for (name, files) in [("server", &server), ("client", &client)] {
        for path in files.paths() {
            let body = String::from_utf8_lossy(files.get(path).unwrap_or_default()).into_owned();
            for word in [
                "withAudit",
                "redactNode",
                "AuditEvent",
                "AuditFailure",
                "recordAudit",
                "pii: true",
            ] {
                assert!(!body.contains(word), "{name}: `{word}` is in {path}");
            }
        }
    }
}

/// An audited endpoint next to a version guard, an `@idempotent` endpoint, derived fields, a
/// deadline, a service level and tenancy: the runtime of each combination compiles under
/// `tsc --strict`. Nothing is run.
#[test]
fn the_audit_log_compiles_next_to_the_other_features() {
    let Some(tsc) = tsc_path() else { return };
    let base = std::fs::read_to_string(repo_root().join(CONTRACT)).unwrap();
    let extra = [
        "",
        "\nmodel Memo {\n  note: string\n  shout: string = note\n}\n",
        "\n@auth(bearer)\nservice Keyed at \"/keyed\" {\n  @audited\n  @idempotent\n  post \"/\" keyed() -> Note\n}\n",
        "\n@auth(bearer)\n@audited\nservice Slo at \"/slo\" {\n  @slo(p99: ms(300))\n  get \"/{id}\" slo(id: uuid) -> Note\n}\n",
    ];
    for tail in extra {
        let ir = compile_text(format!("{base}{tail}"));
        let services: Vec<&str> = ir.services.iter().map(|s| s.name.as_str()).collect();
        for (edition, _) in TARGETS {
            let guard = scratch("audit-combo-");
            build_services(guard.path(), &tsc, &ir, &services, &[], edition, &json!({}));
        }
    }
}
