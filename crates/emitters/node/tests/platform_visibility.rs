//! Field visibility in the generated TypeScript (`spec/targets/typescript.md` section 5k,
//! `spec/platform.md` section 2).
//!
//! Without `node` or `node_modules` the toolchain tests are skipped unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::toolchain::*;
use common::*;
use serde_json::json;

const EXAMPLES: &str = "tests/rules/endpoints/visibility.veld.pending";
const CONTRACT: &str = "tests/toolchain/contract/platform_p1/visibility.veld";
const GOLDEN_CONTRACT: &str = "tests/wire/visibility/contract.veld";
const GOLDEN_CASES: &str = "tests/wire/visibility/cases.json";

/// The 14 examples of the pending file through the generated test handler.
fn examples_run(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(EXAMPLES);
    let guard = scratch(prefix);
    let work = guard.path();
    build_services(
        work,
        &tsc,
        &ir,
        &["Bookings", "Notices"],
        &["visibility_serve.ts"],
        edition,
        &json!({}),
    );
    let server = Live::start(work, "visibility_serve.js");
    let passed = run_examples(&server.url, &ir);
    assert_eq!(ir.examples.len(), 14, "the pending file has 14 examples");
    assert_eq!(passed, 14, "every example ran, none was skipped");
}

#[test]
fn the_visibility_examples_compile_strict_and_pass_es2020() {
    examples_run("es2020", "visibility-2020-");
}

#[test]
fn the_visibility_examples_compile_strict_and_pass_es2022() {
    examples_run("es2022", "visibility-2022-");
}

/// The production handler: a field hidden for one caller and present for another, in a value, a
/// list and a page; an expression that cannot be evaluated; the clock; an `ensure`; an error; a
/// stream item; no caller; and the settings that change nothing.
fn http_run(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(CONTRACT);
    let guard = scratch(prefix);
    let work = guard.path();
    build_services(
        work,
        &tsc,
        &ir,
        &["Desk", "Notices"],
        &["visibility_http.ts"],
        edition,
        &json!({}),
    );
    run(
        std::process::Command::new("node")
            .arg("out/visibility_http.js")
            .current_dir(work),
        "node visibility_http.js",
    );
}

#[test]
fn visibility_behaves_over_the_production_handler_es2020() {
    http_run("es2020", "visibility-http-2020-");
}

#[test]
fn visibility_behaves_over_the_production_handler_es2022() {
    http_run("es2022", "visibility-http-2022-");
}

/// The 23 cases of `tests/wire/visibility/cases.json`, through the runtime's function: the value
/// that is sent, canonical, equals `shown`, and the paths equal `hidden`. The contract of the
/// goldens has no service, and a server needs one: a trivial one is appended.
#[test]
fn every_case_of_the_visibility_goldens_gives_the_bytes_of_the_reference() {
    let Some(tsc) = tsc_path() else { return };
    let mut text = std::fs::read_to_string(repo_root().join(GOLDEN_CONTRACT)).unwrap();
    text.push_str(
        "\n@auth(bearer)\nservice Guests at \"/guests\" {\n  get \"/{id}\" getGuest(id: uuid) -> Guest\n}\n",
    );
    let ir = compile_text(text);
    for (edition, _) in TARGETS {
        let guard = scratch("visibility-goldens-");
        let work = guard.path();
        build_service(
            work,
            &tsc,
            &ir,
            "Guests",
            &["visibility_goldens.ts"],
            edition,
        );
        std::fs::copy(repo_root().join(GOLDEN_CASES), work.join("cases.json")).unwrap();
        run(
            std::process::Command::new("node")
                .arg("out/visibility_goldens.js")
                .current_dir(work),
            "node visibility_goldens.js",
        );
    }
}

#[test]
fn snapshots_of_a_contract_with_visible_fields() {
    let ir = compile_path(CONTRACT);
    let server = gen_servers_with(&ir, &["Desk", "Notices"], &[], &json!({ "testing": true }));
    insta::assert_snapshot!(
        "visibility_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!(
        "visibility_server_models",
        text(&server, "models/bookings.ts")
    );
    insta::assert_snapshot!(
        "visibility_server_errors",
        text(&server, "errors/bookings.ts")
    );
    insta::assert_snapshot!("visibility_server_desk", text(&server, "server/desk.ts"));
    insta::assert_snapshot!(
        "visibility_test_handler",
        text(&server, "server/desk-test-handler.ts")
    );
    let client = gen_client(&ir);
    insta::assert_snapshot!(
        "visibility_client_models",
        text(&client, "bookings/models/bookings.ts")
    );
}

#[test]
fn the_server_has_the_show_functions_and_the_client_has_none_of_it() {
    let ir = compile_path(CONTRACT);
    let server = gen_servers_with(&ir, &["Desk", "Notices"], &[], &json!({}));
    let models = text(&server, "models/bookings.ts");
    for word in [
        "function showBooking(",
        "function showGuest(",
        "show: showBooking,",
        "veld.visibleTo(caller,",
    ] {
        assert!(models.contains(word), "`{word}` is missing");
    }
    let desk = text(&server, "server/desk.ts");
    assert!(
        desk.contains("caller: 'veld:bookings/User@"),
        "the caller of an endpoint with auth"
    );
    assert!(desk.contains("visible: true,"));
    // A service without auth has no caller: its endpoints are described without one.
    let notices = text(&server, "server/notices.ts");
    assert!(!notices.contains("caller:"), "{notices}");
    assert!(text(&server, "runtime/veld.ts").contains("function showNode("));
    let client = gen_client(&ir);
    for path in client.paths() {
        let body = String::from_utf8_lossy(client.get(path).unwrap_or_default()).into_owned();
        for word in ["showNode", "showSent", "visibleTo", "platformOpen", "show:"] {
            assert!(!body.contains(word), "client: `{word}` is in {path}");
        }
    }
}

#[test]
fn a_contract_without_visible_fields_has_none_of_the_code() {
    let ir = standard_ir();
    let server = gen_server(&ir, "Orders", &["Iam"]);
    let client = gen_client(&ir);
    for (name, files) in [("server", &server), ("client", &client)] {
        for path in files.paths() {
            let body = String::from_utf8_lossy(files.get(path).unwrap_or_default()).into_owned();
            for word in [
                "showNode",
                "showSent",
                "visibleTo",
                "platformOpen",
                "PlatformState",
            ] {
                assert!(!body.contains(word), "{name}: `{word}` is in {path}");
            }
        }
    }
}

/// Visibility next to a version guard, an `@idempotent` endpoint, derived fields and a deadline:
/// the runtime of each combination compiles under `tsc --strict` for both editions. Nothing is run.
#[test]
fn visibility_compiles_next_to_the_other_features() {
    let Some(tsc) = tsc_path() else { return };
    let base = std::fs::read_to_string(repo_root().join(CONTRACT)).unwrap();
    let extra = [
        "",
        "\nmodel Memo {\n  note: string\n  shout: string = note\n}\n",
        "\n@auth(bearer)\nservice Slow at \"/slow\" {\n  @timeout(seconds(2))\n  get \"/{id}\" slow(id: uuid) -> Booking\n}\n",
        "\n@auth(bearer)\nservice Keyed at \"/keyed\" {\n  @idempotent\n  post \"/\" keyed() -> Booking\n}\n",
    ];
    for tail in extra {
        let ir = compile_text(format!("{base}{tail}"));
        let services: Vec<&str> = ir.services.iter().map(|s| s.name.as_str()).collect();
        for (edition, _) in TARGETS {
            let guard = scratch("visibility-combo-");
            build_services(guard.path(), &tsc, &ir, &services, &[], edition, &json!({}));
        }
    }
}
