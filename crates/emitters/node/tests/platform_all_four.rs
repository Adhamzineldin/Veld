//! The four features of platform phase P1 together (`spec/targets/typescript.md` section 5k):
//! `tests/toolchain/contract/platform_p1/all_four.veld` compiled as a server (with the test
//! handler and the Nest module) and as a client, and its files snapshotted.
//!
//! Without `node` or `node_modules` the toolchain tests are skipped unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::toolchain::*;
use common::*;
use serde_json::json;

const CONTRACT: &str = "tests/toolchain/contract/platform_p1/all_four.veld";

#[test]
fn the_four_features_compile_strict_in_both_editions() {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(CONTRACT);
    for (edition, _) in TARGETS {
        let guard = scratch("all-four-");
        build_service(guard.path(), &tsc, &ir, "Bookings", &[], edition);
    }
}

/// The Nest module has the hooks that the features require, and compiles with the decorator flags.
#[test]
fn the_nest_server_of_the_four_features_compiles_with_the_decorator_flags() {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(CONTRACT);
    let server = gen_server_fw(&ir, "Bookings", &[], &serde_json::Value::Null, Some("nest"));
    let module = text(&server, "server/nest/bookings.ts");
    assert!(
        module.contains("hooks: BookingsHooks;"),
        "the hooks are required: {module}"
    );
    let guard = scratch("all-four-nest-");
    let work = guard.path();
    veld_project::output::write(
        &work.join("server"),
        &server,
        &veld_project::output::WriteOptions::default(),
    )
    .unwrap();
    std::fs::write(
        work.join("tsconfig.json"),
        serde_json::to_string_pretty(&json!({
            "compilerOptions": {
                "strict": true, "target": "ES2020", "lib": ["ES2020"], "types": ["node"],
                "module": "commonjs", "moduleResolution": "node10", "noEmit": true,
                "experimentalDecorators": true, "emitDecoratorMetadata": true,
                "skipLibCheck": true
            },
            "include": ["server/**/*.ts"]
        }))
        .unwrap(),
    )
    .unwrap();
    run(
        std::process::Command::new("node")
            .arg(tsc)
            .args(["-p", "tsconfig.json"])
            .current_dir(work),
        "tsc nest",
    );
}

#[test]
fn snapshots_of_a_contract_with_the_four_features() {
    let ir = compile_path(CONTRACT);
    let server = gen_server_with(&ir, "Bookings", &[], &json!({ "testing": true }));
    insta::assert_snapshot!(
        "all_four_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!(
        "all_four_server_models",
        text(&server, "models/bookings.ts")
    );
    insta::assert_snapshot!(
        "all_four_server_module",
        text(&server, "server/bookings.ts")
    );
    insta::assert_snapshot!(
        "all_four_test_handler",
        text(&server, "server/bookings-test-handler.ts")
    );
    let client = gen_client(&ir);
    insta::assert_snapshot!("all_four_client", text(&client, "bookings/client.ts"));
    insta::assert_snapshot!(
        "all_four_client_models",
        text(&client, "bookings/models/bookings.ts")
    );
}

/// What the server asks of the application, in one place.
#[test]
fn the_hooks_of_the_four_features_are_required_by_type() {
    let ir = compile_path(CONTRACT);
    let server = gen_server(&ir, "Bookings", &[]);
    let module = text(&server, "server/bookings.ts");
    for word in [
        "audit: (event: veld.AuditEvent) => void | Promise<void>;",
        "limiter?: veld.Limiter;",
        "address: (ctx: veld.RequestContext) => string | Promise<string>;",
        "caller: (principal: unknown, ctx: veld.RequestContext) => string | Promise<string>;",
        "caller: 'veld:bookings/User@",
        "visible: true,",
        "tenant: { n: 'tenantId', k: 'uuid' },",
        "audit: { authenticated: true,",
        "limit: { count: 10, seconds: 60, by: 'user' },",
    ] {
        assert!(module.contains(word), "`{word}` is missing:\n{module}");
    }
    assert!(!module.contains("hooks: BookingsHooks = {}"));
    // The one runtime of every role: the client has the wait and nothing else of the four.
    let client = gen_client(&ir);
    for path in client.paths() {
        let body = String::from_utf8_lossy(client.get(path).unwrap_or_default()).into_owned();
        for word in [
            "showNode",
            "tenantNode",
            "withAudit",
            "redactNode",
            "MemoryLimiter",
        ] {
            assert!(!body.contains(word), "client: `{word}` is in {path}");
        }
    }
}
