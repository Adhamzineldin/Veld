//! Snapshots of the generated crates, plus the structural checks that do not need a toolchain:
//! determinism, a runtime that is byte-identical across roles, only the chosen framework's
//! dependencies, and no leftover gate markers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::*;
use veld_codegen::{FileSet, Role};

fn server() -> FileSet {
    gen_server(&standard_ir(), "Orders", &["Iam"], "orders-server")
}

fn client() -> FileSet {
    gen_client(&standard_ir(), "orders-client", None)
}

fn snapshot(name: &str, files: &FileSet, paths: &[&str]) {
    insta::assert_snapshot!(
        format!("{name}_file_list"),
        files.paths().collect::<Vec<_>>().join("\n")
    );
    for path in paths {
        insta::assert_snapshot!(
            format!("{name}_{}", path.replace('/', "__")),
            text(files, path)
        );
    }
}

#[test]
fn server_snapshots() {
    snapshot(
        "server",
        &server(),
        &[
            "Cargo.toml",
            "src/lib.rs",
            "src/orders/mod.rs",
            "src/orders/catalog.rs",
            "src/orders/orders.rs",
            "src/orders/orders_server.rs",
            "src/prelude.rs",
            "src/adapters/mod.rs",
            "src/veld/mod.rs",
        ],
    );
}

#[test]
fn client_snapshots() {
    snapshot(
        "client",
        &client(),
        &[
            "Cargo.toml",
            "src/lib.rs",
            "src/orders/orders_client.rs",
            "src/iam/iam_client.rs",
            "src/veld/mod.rs",
        ],
    );
}

#[test]
fn the_ureq_client_snapshot() {
    let files = gen_client(&standard_ir(), "orders-client-ureq", Some("ureq"));
    snapshot("ureq_client", &files, &["Cargo.toml", "src/veld/mod.rs"]);
    assert!(
        text(&files, "src/orders/orders_client.rs")
            .contains("OrdersClient<veld::transport_ureq::UreqTransport>")
    );
}

#[test]
fn streaming_snapshots() {
    let ir = streaming_ir();
    let server = gen_server_services(&ir, &["Live", "Vault"], "live-server");
    snapshot(
        "streaming_server",
        &server,
        &[
            "Cargo.toml",
            "src/live/live_server.rs",
            "src/live/vault_server.rs",
            "src/veld/mod.rs",
        ],
    );
    let client = gen_client(&ir, "live-client", None);
    snapshot(
        "streaming_client",
        &client,
        &["Cargo.toml", "src/live/live_client.rs"],
    );
}

#[test]
fn validate_false_flips_the_defaults_and_keeps_the_types() {
    let ir = greeter_ir();
    let all = ir.services.iter().map(|s| s.id).collect::<Vec<_>>();
    let off = serde_json::json!({ "crate-name": "g", "validate": false });
    let server = run(&ir, Role::Server, all.clone(), vec![], None, off.clone());
    let client = run(&ir, Role::Client, all, vec![], None, off);
    insta::assert_snapshot!(
        "validate_false_server_hooks",
        text(&server, "src/greeter/greeter_server.rs")
    );
    let source = text(&client, "src/greeter/greeter_client.rs");
    assert!(source.contains("STD, false, options"), "{source}");
    // The types are the same either way.
    let on = gen_client(&ir, "g", None);
    assert_eq!(
        text(&client, "src/greeter/greeter.rs"),
        text(&on, "src/greeter/greeter.rs")
    );
}

#[test]
fn output_is_deterministic() {
    assert_eq!(server(), server());
    assert_eq!(client(), client());
}

/// The runtime is the same bytes in both roles, so two crates of one contract agree on it.
#[test]
fn the_shared_runtime_is_identical_in_both_roles() {
    let (s, c) = (server(), client());
    for path in [
        "src/veld/canonical.rs",
        "src/veld/error.rs",
        "src/veld/page.rs",
        "src/veld/regex.rs",
        "src/veld/schema.rs",
        "src/veld/spec.rs",
        "src/veld/util.rs",
    ] {
        assert_eq!(s.get(path), c.get(path), "{path}");
    }
}

#[test]
fn only_the_chosen_frameworks_are_dependencies() {
    let cargo = |files: &FileSet| text(files, "Cargo.toml").to_string();
    let s = cargo(&server());
    assert!(s.contains("axum = \"0.8\""), "{s}");
    assert!(!s.contains("ureq"), "{s}");
    let c = cargo(&client());
    assert!(
        c.contains("reqwest") && !c.contains("axum") && !c.contains("ureq"),
        "{c}"
    );
    // A contract without streams needs no `futures-core`, one without `ws` no tungstenite.
    assert!(
        !c.contains("futures-core") && !c.contains("tungstenite"),
        "{c}"
    );
    let u = cargo(&gen_client(&standard_ir(), "u", Some("ureq")));
    assert!(
        u.contains("ureq") && !u.contains("reqwest") && !u.contains("tokio"),
        "{u}"
    );
    let ws = cargo(&gen_server_services(
        &streaming_ir(),
        &["Live", "Vault"],
        "w",
    ));
    assert!(
        ws.contains("features = [\"ws\"]") && ws.contains("futures-core"),
        "{ws}"
    );
    let wc = cargo(&gen_client(&streaming_ir(), "w", None));
    assert!(
        wc.contains("tokio-tungstenite") && wc.contains("futures-util"),
        "{wc}"
    );
}

#[test]
fn no_gate_markers_survive_and_every_file_has_the_header() {
    for files in [server(), client(), gen_client(&streaming_ir(), "w", None)] {
        for (path, bytes) in files.iter() {
            let s = String::from_utf8_lossy(bytes);
            assert!(!s.contains("// @if") && !s.contains("// @end"), "{path}");
            if path.ends_with(".rs") {
                assert!(
                    s.starts_with("// Code generated by veld. DO NOT EDIT."),
                    "{path}"
                );
            }
        }
    }
}

#[test]
fn a_server_without_hooks_gets_the_plain_constructor() {
    let ir = streaming_sse_ir();
    let server = gen_server_services(&ir, &["Live", "Vault"], "s");
    let live = text(&server, "src/live/live_server.rs");
    assert!(live.contains("pub fn plain<S: LiveService>"), "{live}");
    let vault = text(&server, "src/live/vault_server.rs");
    assert!(!vault.contains("pub fn plain"), "{vault}");
    assert!(vault.contains("fn auth_bearer<'a>"), "{vault}");
}
