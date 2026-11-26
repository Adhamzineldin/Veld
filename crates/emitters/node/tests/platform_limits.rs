//! Rate limits in the generated TypeScript server and client (`spec/targets/typescript.md` section
//! 5k, `spec/platform.md` section 5).
//!
//! Without `node` or `node_modules` the toolchain tests are skipped unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::toolchain::*;
use common::*;
use serde_json::json;

const CONTRACT: &str = "tests/toolchain/contract/platform_p1/limits.veld";
const WIRE_CONTRACT: &str = "tests/wire/limits/contract.veld";
const WIRE_CASES: &str = "tests/wire/limits/cases.json";

const SERVICES: [&str; 3] = ["Rooms", "Public", "Desks"];

/// The production handler and the generated client: the key, the last allowed call and the first
/// refused one, the next window, `user`, `tenant` and `ip`, a limiter that fails, a server created
/// without the hook it needs, the memory limiter and its notice, the audit of a refused call, the
/// client's single retry, no retry of a `post` that is not idempotent, `retryOnLimit: false`.
fn http_run(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(CONTRACT);
    let guard = scratch(prefix);
    let work = guard.path();
    build_services(
        work,
        &tsc,
        &ir,
        &SERVICES,
        &["limits_http.ts"],
        edition,
        &json!({}),
    );
    run(
        std::process::Command::new("node")
            .arg("out/limits_http.js")
            .current_dir(work),
        "node limits_http.js",
    );
}

#[test]
fn limits_behave_over_the_production_handler_and_the_client_es2020() {
    http_run("es2020", "limits-http-2020-");
}

#[test]
fn limits_behave_over_the_production_handler_and_the_client_es2022() {
    http_run("es2022", "limits-http-2022-");
}

/// The windows and the keys of `tests/wire/limits/cases.json`: the generated memory limiter with a
/// clock the test controls, and the key a server asks about. The contract is the one of the keys.
#[test]
fn every_case_of_the_limit_goldens_gives_the_decisions_and_the_keys_of_the_reference() {
    let Some(tsc) = tsc_path() else { return };
    let text = std::fs::read_to_string(repo_root().join(WIRE_CONTRACT)).unwrap();
    let ir = compile_as_t(text);
    for (edition, _) in TARGETS {
        let guard = scratch("limits-goldens-");
        let work = guard.path();
        build_services(
            work,
            &tsc,
            &ir,
            &["Rooms", "Public"],
            &["limits_goldens.ts"],
            edition,
            &json!({}),
        );
        std::fs::copy(repo_root().join(WIRE_CASES), work.join("cases.json")).unwrap();
        run(
            std::process::Command::new("node")
                .arg("out/limits_goldens.js")
                .current_dir(work),
            "node limits_goldens.js",
        );
    }
}

#[test]
fn snapshots_of_a_contract_with_limits() {
    let ir = compile_path(CONTRACT);
    let server = gen_servers_with(&ir, &SERVICES, &[], &json!({ "testing": true }));
    insta::assert_snapshot!(
        "limits_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!("limits_server_rooms", text(&server, "server/rooms.ts"));
    insta::assert_snapshot!("limits_server_desks", text(&server, "server/desks.ts"));
    insta::assert_snapshot!(
        "limits_test_handler",
        text(&server, "server/rooms-test-handler.ts")
    );
    let client = gen_client(&ir);
    insta::assert_snapshot!("limits_client", text(&client, "bookings/client.ts"));
}

#[test]
fn the_hooks_name_what_each_limit_needs() {
    let ir = compile_path(CONTRACT);
    let server = gen_servers_with(&ir, &SERVICES, &[], &json!({}));
    let rooms = text(&server, "server/rooms.ts");
    assert!(rooms.contains("limiter?: veld.Limiter;"), "{rooms}");
    assert!(rooms.contains("address: (ctx: veld.RequestContext) => string | Promise<string>;"));
    // The caller of the idempotency store is the caller of the limit: no hook of its own.
    assert!(!rooms.contains("  caller: (principal"), "{rooms}");
    assert!(rooms.contains("limit: { count: 2, seconds: 60, by: 'user' },"));
    assert!(rooms.contains("by: 'tenant', tenant: { n: 'tenantId', k: 'uuid' } },"));
    assert!(rooms.contains("tooMany: TooManyRequests.sig"));
    let desks = text(&server, "server/desks.ts");
    assert!(desks.contains("  caller: (principal: unknown, ctx: veld.RequestContext)"));
    assert!(
        !desks.contains("hooks: DesksHooks = {}"),
        "a hook that is required leaves no default"
    );
    // The `Public` service takes no auth, so its hooks have only the address and the limiter.
    let public = text(&server, "server/public.ts");
    assert!(public.contains("address: ") && !public.contains("caller:"));
    let runtime = text(&server, "runtime/veld.ts");
    for word in [
        "class MemoryLimiter",
        "function takeLimit(",
        "function withLimiter(",
    ] {
        assert!(runtime.contains(word), "`{word}` is missing");
    }
}

#[test]
fn the_client_waits_only_where_it_may() {
    let ir = compile_path(CONTRACT);
    let client = gen_client(&ir);
    let module = text(&client, "bookings/client.ts");
    // A `get` and an `@idempotent` post wait; a `post` that is not idempotent does not.
    let retries = module.matches("retryOnLimit: true,").count();
    assert_eq!(
        retries, 5,
        "byUser, byTenant, create, listing and mine:\n{module}"
    );
    let runtime = text(&client, "runtime/veld.ts");
    for word in [
        "sendLimited",
        "retryOnLimit?: boolean;",
        "sleep?: (ms: number)",
    ] {
        assert!(runtime.contains(word), "`{word}` is missing");
    }
    // The client has the wait and not the limiter.
    for path in client.paths() {
        let body = String::from_utf8_lossy(client.get(path).unwrap_or_default()).into_owned();
        for word in ["MemoryLimiter", "takeLimit", "withLimiter"] {
            assert!(!body.contains(word), "client: `{word}` is in {path}");
        }
    }
}

#[test]
fn a_contract_without_limits_has_none_of_the_code() {
    let ir = standard_ir();
    let server = gen_server(&ir, "Orders", &["Iam"]);
    let client = gen_client(&ir);
    for (name, files) in [("server", &server), ("client", &client)] {
        for path in files.paths() {
            let body = String::from_utf8_lossy(files.get(path).unwrap_or_default()).into_owned();
            for word in [
                "MemoryLimiter",
                "takeLimit",
                "sendLimited",
                "retryOnLimit",
                "LimitDecision",
                "tooMany",
            ] {
                assert!(!body.contains(word), "{name}: `{word}` is in {path}");
            }
        }
    }
}

/// Limits next to a version guard, an `@idempotent` endpoint, derived fields, a deadline, a service
/// level, audit, tenancy and visibility, and a client with the rules: each combination compiles
/// under `tsc --strict`. Nothing is run.
#[test]
fn limits_compile_next_to_the_other_features() {
    let Some(tsc) = tsc_path() else { return };
    let base = std::fs::read_to_string(repo_root().join(CONTRACT)).unwrap();
    let extra = [
        "",
        "\nmodel Memo {\n  note: string\n  shout: string = note\n  rule short: len(note) < 100\n}\n",
        "\n@auth(bearer)\nservice Slow at \"/slow\" {\n  @timeout(seconds(2))\n  @slo(p99: ms(300))\n  @limit(5, per: day, by: tenant)\n  get \"/{id}\" slow(id: uuid) -> Room\n}\n",
        "\nmodel Held {\n  id: uuid\n  tenantId: uuid @tenant\n  note?: string @visible(when: user.id == id)\n}\n\n@auth(bearer)\nservice Keeps at \"/held\" {\n  @audited\n  @limit(5, per: day, by: user)\n  get \"/{id}\" held(id: uuid) -> Held\n}\n",
    ];
    for tail in extra {
        let ir = compile_text(format!("{base}{tail}"));
        let services: Vec<&str> = ir.services.iter().map(|s| s.name.as_str()).collect();
        for (edition, _) in TARGETS {
            let guard = scratch("limits-combo-");
            build_services(guard.path(), &tsc, &ir, &services, &[], edition, &json!({}));
        }
    }
}
