//! Platform phase P4, part 1: no message repeats a value (`spec/rules.md` section 19.3,
//! `spec/targets/java.md` section 18.1). The marker goes through the generated server, client and
//! events runtime; the messages that used to repeat a value are fixed texts.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::live::require_javac;
use common::p4::{generate, run_driver};
use common::*;
use serde_json::json;
use veld_codegen::Role;

const CONTRACT: &str = "tests/toolchain/contract/platform_p4/no_values.veld";
const EVENTS: &str = "tests/toolchain/contract/platform_p4/no_values_events.veld";

/// A marker string goes through every path of the generated server and client that writes a
/// message, a detail, a title or an exception text, and the driver searches every text for it.
#[test]
fn no_message_of_the_server_or_the_client_repeats_a_value() {
    if !require_javac("no_message_of_the_server_or_the_client_repeats_a_value") {
        return;
    }
    let ir = compile_path(CONTRACT, "bookings");
    let server = generate(&ir, Role::Server, "gen.b", &json!({}));
    let client = generate(&ir, Role::Client, "gen.c", &json!({}));
    run_driver(&[&server, &client], "NoValues", &[], &[]);
}

/// The same for the events runtime: a publisher that refuses, a consumer that rejects.
#[test]
fn no_message_of_the_events_runtime_repeats_a_value() {
    if !require_javac("no_message_of_the_events_runtime_repeats_a_value") {
        return;
    }
    let ir = compile_path(EVENTS, "events");
    let server = generate(&ir, Role::Server, "gen.b", &json!({}));
    run_driver(&[&server], "NoValuesEvents", &[], &[]);
}

/// The places that used to put a value into a message, found by reading every concatenation of the
/// embedded runtime: each now writes a fixed text with a name or a path.
#[test]
fn the_messages_that_repeated_a_value_are_fixed_texts() {
    let root = repo_root().join("crates/emitters/java");
    let core = std::fs::read_to_string(root.join("runtime/ClientCore.java")).unwrap();
    // A transport failure names the endpoint's route, never the address with its parameters.
    assert!(core.contains("ep.method() + \" \" + ep.path() + \" failed\""));
    assert!(!core.contains("getRawPath"));
    assert!(core.contains("\"invalid request URL\""));
    assert!(!core.contains("invalid request URL \""));
    let events = std::fs::read_to_string(root.join("runtime/Events.java")).unwrap();
    assert!(events.contains("`datacontenttype` is not `application/json`"));
    assert!(!events.contains("`datacontenttype` is `\""));
    assert!(!events.contains("String.valueOf(e.getMessage())"));
    let platform = std::fs::read_to_string(root.join("runtime/Platform.java")).unwrap();
    assert!(!platform.contains("could not be built: \""));
    assert!(!platform.contains("hook failed: \""));
    let models = std::fs::read_to_string(root.join("src/models.rs")).unwrap();
    assert!(models.contains("unknown {name}\\\")"));
    assert!(!models.contains("unknown {name}: "));
    assert!(!models.contains("arm: \\\" + "));
}
