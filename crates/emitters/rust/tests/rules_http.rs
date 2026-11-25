//! The bookings contract (`tests/toolchain/contract/rules/bookings_rust.veld`): a snapshot of the
//! generated crates, and invariants over HTTP with the generated server and client
//! (`tests/toolchain/rust/rules-harness`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::*;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};

fn bookings_ir() -> Ir {
    let path = repo_root().join("tests/toolchain/contract/rules/bookings_rust.veld");
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "bookings".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "bookings.veld".into(),
                text: std::fs::read_to_string(path).unwrap(),
            }],
            base_url: None,
        }],
        consumes: std::collections::BTreeMap::new(),
    });
    assert!(out.ir.is_some(), "{}", out.render());
    out.ir.unwrap()
}

#[test]
fn the_bookings_contract_snapshots() {
    let ir = bookings_ir();
    let server = gen_server(&ir, "Bookings", &[], "bookings-server");
    for path in [
        "src/bookings/bookings.rs",
        "src/bookings/bookings_server.rs",
        "src/veld/mod.rs",
        "src/lib.rs",
    ] {
        insta::assert_snapshot!(
            format!("bookings_server_{}", path.replace('/', "__")),
            text(&server, path)
        );
    }
    let client = gen_client(&ir, "bookings-client", None);
    insta::assert_snapshot!(
        "bookings_client_file_list",
        client.paths().collect::<Vec<_>>().join("\n")
    );
    for (path, bytes) in server.iter().chain(client.iter()) {
        let body = String::from_utf8_lossy(bytes);
        assert!(
            !body.contains("// #if") && !body.contains("// #endif"),
            "{path}"
        );
    }
}

/// A generated server rejects a bad body with the 422 problem document; the client with the local
/// check off decodes the same violations, with it on fails locally (once a response has shown the
/// rules digest) and sends nothing; an output violation is a 500 in `strict` and passes with a hook
/// event in `audit`; a server generated with `validate = false` accepts a rule-violating body and
/// says so once.
#[test]
fn rules_over_http() {
    if !cargo_or_skip() {
        return;
    }
    let ir = bookings_ir();
    // The server is generated with the target option `validate = false`; the harness's hooks
    // switch validation back on for the normal scenarios, and the `serve-off` child keeps the
    // option's default.
    let server = run(
        &ir,
        veld_codegen::Role::Server,
        vec![service_id(&ir, "Bookings")],
        vec![],
        None,
        serde_json::json!({ "crate-name": "bookings-server", "validate": false }),
    );
    let client = gen_client(&ir, "bookings-client", None);
    let ws = harness_workspace("rules-http-ws", &server, &client, "rules-harness");
    build_lint_and_run(
        &ws,
        ("bookings-server", "bookings-client", "rules-harness"),
        &[],
        "rules roundtrip ok",
    );
}
