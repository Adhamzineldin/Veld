//! Platform phase P4, part 1: no message repeats a value (`spec/rules.md` section 19.3,
//! `spec/targets/typescript.md` section 5l).
//!
//! Without `node` or `node_modules` the toolchain tests are skipped unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::toolchain::*;
use common::*;
use serde_json::json;

const CONTRACT: &str = "tests/toolchain/contract/platform_p4/no_values.veld";
const EVENTS: &str = "tests/toolchain/contract/platform_p4/no_values_events.veld";

/// The marker goes through the generated server and client: validation issues of every code,
/// violations of every kind of rule, the guards of an endpoint, the runtime config and a transport
/// that fails. The driver searches every text for it.
fn server_and_client(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(CONTRACT);
    let guard = scratch(prefix);
    let work = guard.path();
    build_services(
        work,
        &tsc,
        &ir,
        &["Probe"],
        &["messages_no_values.ts"],
        edition,
        &json!({}),
    );
    run(
        std::process::Command::new("node")
            .arg("out/messages_no_values.js")
            .current_dir(work),
        "node messages_no_values.js",
    );
}

#[test]
fn no_message_of_the_server_or_the_client_repeats_a_value_es2020() {
    server_and_client("es2020", "no-values-2020-");
}

#[test]
fn no_message_of_the_server_or_the_client_repeats_a_value_es2022() {
    server_and_client("es2022", "no-values-2022-");
}

fn events(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(EVENTS);
    let guard = scratch(prefix);
    let work = guard.path();
    build_services(
        work,
        &tsc,
        &ir,
        &["Writer", "Reader"],
        &["messages_no_values_events.ts"],
        edition,
        &json!({}),
    );
    run(
        std::process::Command::new("node")
            .arg("out/messages_no_values_events.js")
            .current_dir(work),
        "node messages_no_values_events.js",
    );
}

#[test]
fn no_message_of_the_events_runtime_repeats_a_value_es2020() {
    events("es2020", "no-values-events-2020-");
}

#[test]
fn no_message_of_the_events_runtime_repeats_a_value_es2022() {
    events("es2022", "no-values-events-2022-");
}

/// The places that used to put a value into a message, found by reading every interpolation of
/// the embedded runtimes: each now writes a fixed text with a name or a path.
#[test]
fn the_messages_that_repeated_a_value_are_fixed_texts() {
    let root = repo_root().join("crates/emitters");
    let veld = std::fs::read_to_string(root.join("typescript/runtime/veld.ts")).unwrap();
    // A transport failure names the endpoint's route, never the address with its parameters.
    assert!(veld.contains("`${ep.method} ${ep.path} failed`"));
    assert!(!veld.contains("`${ep.method} ${path} failed`"));
    let rules = std::fs::read_to_string(root.join("typescript/runtime/rules.ts")).unwrap();
    assert!(rules.contains("new TypeError('invalid decimal')"));
    assert!(rules.contains("new TypeError(`invalid ${kind}`)"));
    assert!(!rules.contains("invalid decimal:") && !rules.contains("invalid ${kind}:"));
    let events = std::fs::read_to_string(root.join("node/runtime/events.ts")).unwrap();
    assert!(events.contains("'`datacontenttype` is not `application/json`'"));
    assert!(!events.contains("String(contentType)"));
}
