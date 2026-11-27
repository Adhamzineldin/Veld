//! Rate limits in generated Rust (`spec/platform.md` section 5, `spec/targets/rust.md` section
//! 18.5): what is generated for `tests/toolchain/contract/platform_p1/limits.veld` (server and
//! client), the goldens of `tests/wire/limits` (the windows through the generated memory limiter
//! with a clock the program controls, the keys through the production handler of each service),
//! the production handler and the client (`tests/toolchain/rust/limits-harness`).
//!
//! Without `cargo` the toolchain tests are skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::*;
use serde_json::json;
use veld_codegen::{FileSet, Role};
use veld_ir::Ir;

const LIMITS: &str = "tests/toolchain/contract/platform_p1/limits.veld";
const GOLDEN_CONTRACT: &str = "tests/wire/limits/contract.veld";
const GOLDEN_CASES: &str = "tests/wire/limits/cases.json";

fn limits_ir() -> Ir {
    compile_path(LIMITS, "bookings")
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
fn the_server_and_the_client_are_snapshotted() {
    let ir = limits_ir();
    let files = server(&ir, true);
    insta::assert_snapshot!(
        "limits_rooms_server",
        text(&files, "src/bookings/rooms_server.rs")
    );
    insta::assert_snapshot!(
        "limits_desks_server",
        text(&files, "src/bookings/desks_server.rs")
    );
    insta::assert_snapshot!(
        "limits_public_server",
        text(&files, "src/bookings/public_server.rs")
    );
    insta::assert_snapshot!(
        "limits_runtime_platform",
        text(&files, "src/veld/platform.rs")
    );
    let client = gen_client(&ir, "bookings-client", None);
    insta::assert_snapshot!(
        "limits_rooms_client",
        text(&client, "src/bookings/rooms_client.rs")
    );
    insta::assert_snapshot!("limits_runtime_client", text(&client, "src/veld/client.rs"));
}

/// What is generated: the limit of each endpoint (its own, a service's, none), the hooks (the
/// limiter is optional, `address` and `caller` are required, the caller text of an idempotency key is
/// shared), and what a client has.
#[test]
fn the_limits_and_the_hooks_are_generated() {
    let files = server(&limits_ir(), false);
    let rooms = text(&files, "src/bookings/rooms_server.rs");
    for word in [
        "0 => Some(veld::platform::LimitSpec { count: 2, window_seconds: 60, by: veld::platform::LimitBy::User }),",
        "1 => Some(veld::platform::LimitSpec { count: 3, window_seconds: 3600, by: veld::platform::LimitBy::Tenant }),",
        "by: veld::platform::LimitBy::Ip }),",
        "fn limiter(&self) -> Option<&dyn veld::platform::Limiter> {\n        None\n    }",
        "fn address(&self, ctx: &veld::server::RequestContext) -> Result<String, veld::Error>;",
        "Some(p) => Ok(self.hooks.idempotency_caller(p, ctx)),",
        "self.hooks.address(ctx)",
    ] {
        assert!(rooms.contains(word), "{word}\n{rooms}");
    }
    assert!(
        !rooms.contains("fn caller(&self"),
        "the caller text of the idempotency key serves the limit"
    );
    let desks = text(&files, "src/bookings/desks_server.rs");
    assert!(
        desks.contains("fn caller(&self, principal: &crate::bookings::bookings::User, ctx: &veld::server::RequestContext) -> Result<String, veld::Error>;"),
        "a service with no idempotent endpoint has a caller hook of its own\n{desks}"
    );
    let public = text(&files, "src/bookings/public_server.rs");
    assert!(
        public.contains("count: 2, window_seconds: 1,"),
        "the limit of a service reaches its endpoints\n{public}"
    );
    let core = text(&files, "src/veld/server.rs");
    for word in [
        "async fn limited(&self, ep: &EndpointSpec, ctx: &RequestContext) -> Option<Response>",
        "\"retry-after\"",
        "MemoryLimiter::new()",
        "counted in memory",
    ] {
        assert!(core.contains(word), "{word}");
    }
    let client = gen_client(&limits_ir(), "bookings-client", None);
    let rooms = text(&client, "src/bookings/rooms_client.rs");
    assert!(
        rooms.contains("retry_on_limit: true,") && rooms.contains("retry_on_limit: false,"),
        "{rooms}"
    );
    let core = text(&client, "src/veld/client.rs");
    for word in [
        "pub fn retry_on_limit(mut self, on: bool) -> Self",
        "pub fn sleeper<F, Fut>",
        "async fn send_limited(",
        "fn retry_after(body: &[u8])",
    ] {
        assert!(core.contains(word), "{word}");
    }
    assert!(
        !core.contains("MemoryLimiter") && !core.contains("limit_spec"),
        "a client has nothing of the server's"
    );
}

/// A contract without a limit has none of the words, in the runtime either: it generates what it
/// generated before (the client has `Transport::sleep` only with streams).
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
                "LimitSpec",
                "limit_spec",
                "MemoryLimiter",
                "retry_on_limit",
                "send_limited",
                "sleeper",
                "fn sleep(",
                "retry-after",
            ] {
                assert!(!body.contains(word), "{path}: {word}");
            }
        }
    }
}

/// The goldens of `tests/wire/limits`: the 10 windows through the generated memory limiter with a
/// clock the program controls, and the 7 keys through the production handlers.
#[test]
fn the_goldens_hold_through_the_generated_limiter_and_handlers() {
    if !cargo_or_skip() {
        return;
    }
    let ir = compile_path(GOLDEN_CONTRACT, "bookings");
    let server = run(
        &ir,
        Role::Server,
        ir.services.iter().map(|s| s.id).collect(),
        vec![],
        None,
        json!({ "crate-name": "bookings-server" }),
    );
    let client = gen_client(&ir, "bookings-client", None);
    let ws = harness_workspace("limits-goldens-ws", &server, &client, "limits-goldens");
    let cases = repo_root().join(GOLDEN_CASES);
    build_lint_and_run(
        &ws,
        ("bookings-server", "bookings-client", "limits-goldens"),
        &[cases.to_str().unwrap()],
        "limits goldens ok: 10 windows, 7 keys",
    );
}

/// The production handler (the last allowed call, the first refused one with its body and header,
/// the next window, `user`, `tenant`, `ip`, a service's limit, the audit of a refused call, a failing
/// limiter and failing hooks, a server with no limiter) and the client (one retry of a `get` and of
/// an `@idempotent` call with the same key, none for another `post`, `retry_on_limit(false)`, a second
/// `429`, a `429` without `retryAfter`, a wait that does not fit).
#[test]
fn the_production_handler_and_the_client_behave() {
    if !cargo_or_skip() {
        return;
    }
    let ir = limits_ir();
    let server = server(&ir, false);
    let client = gen_client(&ir, "bookings-client", None);
    let ws = harness_workspace("limits-http-ws", &server, &client, "limits-harness");
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
    for (bin, ok) in [
        ("limits-http", "limits http ok"),
        ("limits-client", "limits client ok"),
    ] {
        let out = run_ok(
            &mut cargo(&ws, &["run", "-q", "-p", "limits-harness", "--bin", bin]),
            bin,
        );
        assert!(out.contains(ok), "{out}");
    }
}

/// The `ureq` client has the same wait (it blocks the thread, as it does everywhere): the crate
/// builds and passes clippy.
#[test]
fn the_ureq_client_waits_too() {
    if !cargo_or_skip() {
        return;
    }
    let ir = limits_ir();
    let client = gen_client(&ir, "bookings-client", Some("ureq"));
    let text = text(&client, "src/veld/transport_ureq.rs");
    assert!(
        text.contains("async fn sleep(&self, d: Duration)")
            && text.contains("std::thread::sleep(d);"),
        "{text}"
    );
}
