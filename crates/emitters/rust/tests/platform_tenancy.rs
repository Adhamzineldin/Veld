//! Tenancy in generated Rust (`spec/platform.md` section 3, `spec/targets/rust.md` section 18.3):
//! what is generated for `tests/toolchain/contract/platform_p1/tenancy.veld` (server and client),
//! the 17 examples of `tests/rules/endpoints/tenancy.veld.pending` through the generated test
//! handler, and the production handler (`tests/toolchain/rust/tenancy-harness`). The `veldtenant`
//! goldens run with the other envelope goldens in `tests/events.rs`.
//!
//! Without `cargo` the toolchain tests are skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::*;
use serde_json::json;
use veld_codegen::{FileSet, Role};
use veld_ir::Ir;

const TENANCY: &str = "tests/toolchain/contract/platform_p1/tenancy.veld";
const EXAMPLES: &str = "tests/rules/endpoints/tenancy.veld.pending";

fn tenancy_ir() -> Ir {
    compile_path(TENANCY, "bookings")
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
fn the_server_the_client_and_the_test_handler_are_snapshotted() {
    let ir = tenancy_ir();
    let files = server(&ir, true);
    insta::assert_snapshot!("tenancy_models", text(&files, "src/bookings/bookings.rs"));
    insta::assert_snapshot!(
        "tenancy_server",
        text(&files, "src/bookings/tenants_server.rs")
    );
    insta::assert_snapshot!(
        "tenancy_test_handler",
        text(&files, "src/bookings/tenants_test_server.rs")
    );
    insta::assert_snapshot!(
        "tenancy_runtime_platform",
        text(&files, "src/veld/platform.rs")
    );
    let client = gen_client(&ir, "bookings-client", None);
    insta::assert_snapshot!(
        "tenancy_client_models",
        text(&client, "src/bookings/bookings.rs")
    );
    insta::assert_snapshot!(
        "tenancy_runtime_schema",
        text(&client, "src/veld/schema.rs")
    );
}

/// What is generated: the mark of the model, the field flag, the caller's tenant from the typed
/// principal, the check of the facts and the walk of the answers; a client has the field flag and
/// nothing of the server's.
#[test]
fn the_schemas_the_facts_and_the_dispatch_have_the_feature() {
    let ir = tenancy_ir();
    let files = server(&ir, false);
    let models = text(&files, "src/bookings/bookings.rs");
    for word in [
        "tenant: Some(veld::platform::TenantMark { wire: \"tenantId\", id: \"veld:bookings/Booking.tenant@",
        "kind: veld::platform::TenantKind::Uuid }),",
        "tenant: true,",
        "tenant: None,",
    ] {
        assert!(models.contains(word), "{word}");
    }
    let tenants = text(&files, "src/bookings/tenants_server.rs");
    for word in [
        "fn caller_tenant(&self, ctx: &veld::server::RequestContext) -> Option<veld::platform::Tenant>",
        "veld::platform::Tenant::of(veld::platform::TenantKind::Uuid, &principal.tenant_id)",
        "if veld::platform::foreign(",
        ".filter(|v| !veld::platform::foreign(",
    ] {
        assert!(tenants.contains(word), "{word}\n{tenants}");
    }
    let core = text(&files, "src/veld/server.rs");
    for word in [
        "fn tenant_refused(",
        "\"result\"",
        "\"error\"",
        "\"item\"",
        "ctx.tenant = e.caller_tenant(ctx);",
    ] {
        assert!(core.contains(word), "{word}");
    }
    let client = gen_client(&ir, "bookings-client", None);
    assert!(text(&client, "src/bookings/bookings.rs").contains("tenant: true,"));
    for (path, bytes) in client.iter() {
        let body = String::from_utf8_lossy(bytes);
        for word in [
            "TenantMark",
            "veld::platform",
            "tenant_refused",
            "caller_tenant",
        ] {
            assert!(!body.contains(word), "{path}: {word}");
        }
    }
}

/// A contract without a `@tenant` field has none of the words, in the runtime either.
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
    let client = gen_client(&ir, "bookings-client", None);
    for files in [&server, &client] {
        for (path, bytes) in files.iter() {
            let body = String::from_utf8_lossy(bytes);
            for word in [
                "TenantMark",
                "tenant_refused",
                "caller_tenant",
                "veldtenant",
                "Tenant::of",
                "tenant: true",
                "tenant: false",
            ] {
                assert!(!body.contains(word), "{path}: {word}");
            }
        }
    }
}

/// The 17 examples through the generated test handler; a skipped example is a failure.
#[test]
fn the_examples_pass_through_the_generated_test_handler() {
    if !cargo_or_skip() {
        return;
    }
    let ir = compile_path(EXAMPLES, "bookings");
    let server = run(
        &ir,
        Role::Server,
        vec![service_id(&ir, "Bookings")],
        vec![],
        None,
        json!({ "crate-name": "bookings-server", "testing": true }),
    );
    let client = gen_client(&ir, "bookings-client", None);
    let ws = harness_workspace("tenancy-examples-ws", &server, &client, "tenancy-examples");
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
                "bookings-server",
                "-p",
                "bookings-client",
                "--",
                "-D",
                "warnings",
            ],
        ),
        "cargo clippy",
    );
    run_every_example(&ir, "tenancy-serve", 17);
}

/// The production handler: a fact of another tenant is `404`, an optional one absent, a value of
/// another tenant is never sent (a result, a list, a page, a declared error, a stream item), a
/// caller without a tenant, and the member of a request.
#[test]
fn the_production_handler_never_sends_a_value_of_another_tenant() {
    if !cargo_or_skip() {
        return;
    }
    let ir = tenancy_ir();
    let server = server(&ir, false);
    let client = gen_client(&ir, "bookings-client", None);
    let ws = harness_workspace("tenancy-http-ws", &server, &client, "tenancy-harness");
    build_lint_and_run(
        &ws,
        ("bookings-server", "bookings-client", "tenancy-harness"),
        &[],
        "tenancy http ok",
    );
}
