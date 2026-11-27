//! Tenancy in the generated TypeScript (`spec/targets/typescript.md` section 5k,
//! `spec/platform.md` section 3).
//!
//! Without `node` or `node_modules` the toolchain tests are skipped unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::toolchain::*;
use common::*;
use serde_json::json;

const EXAMPLES: &str = "tests/rules/endpoints/tenancy.veld.pending";
const CONTRACT: &str = "tests/toolchain/contract/platform_p1/tenancy.veld";
const EVENTS: &str = "tests/wire/events/tenant.veld";

/// The 17 examples of the pending file through the generated test handler.
fn examples_run(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(EXAMPLES);
    let guard = scratch(prefix);
    let work = guard.path();
    build_service(work, &tsc, &ir, "Bookings", &["tenancy_serve.ts"], edition);
    let server = Live::start(work, "tenancy_serve.js");
    let passed = run_examples(&server.url, &ir);
    assert_eq!(ir.examples.len(), 17, "the pending file has 17 examples");
    assert_eq!(passed, 17, "every example ran, none was skipped");
}

#[test]
fn the_tenancy_examples_compile_strict_and_pass_es2020() {
    examples_run("es2020", "tenancy-2020-");
}

#[test]
fn the_tenancy_examples_compile_strict_and_pass_es2022() {
    examples_run("es2022", "tenancy-2022-");
}

/// The production handler: a fact of another tenant, a value of another tenant in a result, a
/// list, a page, an error and a stream item, a caller without a tenant, a request that lacks or
/// carries the tenant member, and the settings that change nothing.
fn http_run(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(CONTRACT);
    let guard = scratch(prefix);
    let work = guard.path();
    build_service(work, &tsc, &ir, "Tenants", &["tenancy_http.ts"], edition);
    run(
        std::process::Command::new("node")
            .arg("out/tenancy_http.js")
            .current_dir(work),
        "node tenancy_http.js",
    );
}

#[test]
fn tenancy_behaves_over_the_production_handler_es2020() {
    http_run("es2020", "tenancy-http-2020-");
}

#[test]
fn tenancy_behaves_over_the_production_handler_es2022() {
    http_run("es2022", "tenancy-http-2022-");
}

#[test]
fn snapshots_of_a_contract_with_a_tenant() {
    let ir = compile_path(CONTRACT);
    let server = gen_server_with(&ir, "Tenants", &[], &json!({ "testing": true }));
    insta::assert_snapshot!(
        "tenancy_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!("tenancy_server_models", text(&server, "models/bookings.ts"));
    insta::assert_snapshot!("tenancy_server_module", text(&server, "server/tenants.ts"));
    insta::assert_snapshot!(
        "tenancy_test_handler",
        text(&server, "server/tenants-test-handler.ts")
    );
    let client = gen_client(&ir);
    insta::assert_snapshot!(
        "tenancy_client_models",
        text(&client, "bookings/models/bookings.ts")
    );
    let events = compile_path(EVENTS);
    let published = gen_server(&events, "Bookings", &[]);
    insta::assert_snapshot!(
        "tenancy_events_module",
        text(&published, "server/bookings-events.ts")
    );
}

#[test]
fn the_server_checks_and_the_client_only_has_its_types_right() {
    let ir = compile_path(CONTRACT);
    let server = gen_server(&ir, "Tenants", &[]);
    let models = text(&server, "models/bookings.ts");
    assert!(models.contains("tn: { n: 'tenantId',"), "{models}");
    assert!(models.contains("st: true"), "a request may lack the member");
    assert!(models.contains("readonly tenantId: string;"));
    let module = text(&server, "server/tenants.ts");
    assert!(module.contains("tenant: { n: 'tenantId', k: 'uuid' },"));
    assert!(module.contains("schema: { k: 'ref', to: () => Booking.schema },"));
    assert!(text(&server, "runtime/veld.ts").contains("function tenantNode("));
    let client = gen_client(&ir);
    let client_models = text(&client, "bookings/models/bookings.ts");
    assert!(client_models.contains("st: true"));
    assert!(client_models.contains("readonly tenantId: string;"));
    for path in client.paths() {
        let body = String::from_utf8_lossy(client.get(path).unwrap_or_default()).into_owned();
        for word in ["tenantNode", "checkResultTenant", "tn: {", "veldtenant"] {
            assert!(!body.contains(word), "client: `{word}` is in {path}");
        }
    }
}

#[test]
fn an_event_with_a_tenant_names_it_and_one_without_keeps_its_bytes() {
    let ir = compile_path(EVENTS);
    let server = gen_server(&ir, "Bookings", &[]);
    let module = text(&server, "server/bookings-events.ts");
    for word in [
        "tenant: { wire: 'tenantId', kind: 'uuid' },",
        "tenant: { wire: 'account', kind: 'string' },",
        "tenant: { wire: 'shard', kind: 'long' },",
    ] {
        assert!(module.contains(word), "`{word}` is missing");
    }
    let runtime = text(&server, "runtime/events.ts");
    assert!(runtime.contains("veldtenant") && runtime.contains("function tenantTextOf("));
    // A contract with events and no tenant has none of it.
    let plain = compile_path("tests/wire/events/edges.veld");
    let services: Vec<String> = plain.services.iter().map(|s| s.name.clone()).collect();
    let names: Vec<&str> = services.iter().map(String::as_str).collect();
    let files = gen_servers_with(&plain, &names, &[], &json!({}));
    assert!(!text(&files, "runtime/events.ts").contains("veldtenant"));
}

#[test]
fn a_contract_without_a_tenant_has_none_of_the_code() {
    let ir = standard_ir();
    let server = gen_server(&ir, "Orders", &["Iam"]);
    let client = gen_client(&ir);
    for (name, files) in [("server", &server), ("client", &client)] {
        for path in files.paths() {
            let body = String::from_utf8_lossy(files.get(path).unwrap_or_default()).into_owned();
            for word in [
                "tenantNode",
                "checkResultTenant",
                "TenantField",
                "veldtenant",
                "tenantTextOf",
                "st: true",
            ] {
                assert!(!body.contains(word), "{name}: `{word}` is in {path}");
            }
        }
    }
}

/// Tenancy next to a version guard, an `@idempotent` endpoint, derived fields, a deadline and a
/// service level: the runtime of each combination compiles under `tsc --strict`. Nothing is run.
#[test]
fn tenancy_compiles_next_to_the_other_features() {
    let Some(tsc) = tsc_path() else { return };
    let base = std::fs::read_to_string(repo_root().join(CONTRACT)).unwrap();
    let extra = [
        "",
        "\nmodel Memo {\n  note: string\n  shout: string = note\n}\n",
        "\n@auth(bearer)\nservice Slow at \"/slow\" {\n  @timeout(seconds(2))\n  @slo(p99: ms(300))\n  get \"/{id}\" slow(id: uuid) -> Booking\n}\n",
        "\n@auth(bearer)\nservice Keyed at \"/keyed\" {\n  @idempotent\n  post \"/\" keyed() -> Booking\n}\n",
    ];
    for tail in extra {
        let ir = compile_text(format!("{base}{tail}"));
        let services: Vec<&str> = ir.services.iter().map(|s| s.name.as_str()).collect();
        for (edition, _) in TARGETS {
            let guard = scratch("tenancy-combo-");
            build_services(guard.path(), &tsc, &ir, &services, &[], edition, &json!({}));
        }
    }
}
