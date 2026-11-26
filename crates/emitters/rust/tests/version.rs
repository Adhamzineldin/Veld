//! `@version` and `@idempotent` in generated Rust (`spec/rules.md` sections 11.7 to 11.12,
//! `spec/targets/rust.md` section 14): what is generated for a small contract with both
//! annotations, what a contract without them does not get, that the crates build and pass
//! clippy, the production handlers over HTTP (`tests/toolchain/rust/version-harness`) and every
//! example of `tests/rules/endpoints/version.veld.pending` through the generated test handler.
//!
//! Without `cargo` the toolchain test is skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::*;
use serde_json::json;
use veld_codegen::{DynGenerator, FileSet, GenRequest, Role};
use veld_emit_rust::Rust;
use veld_ir::Ir;

const VERSION: &str = "tests/rules/endpoints/version.veld.pending";

/// A guard, an `ETag`, a key and nothing else.
pub const SMALL: &str = r#"
model Note {
  id: uuid
  text: string
  @version
  rev: long
}
model NewNote { text: string }

service Notes at "/notes" {
  put "/{id}" edit(id: uuid, body: NewNote) -> Note {
    fact note: Note = id
  }
  get "/{id}" one(id: uuid) -> Note
  @idempotent
  post "/" make(body: NewNote) -> 201 Note
}
"#;

fn small_ir() -> Ir {
    compile_text(SMALL, "keeper")
}

fn server_of(ir: &Ir, options: serde_json::Value) -> FileSet {
    let mut o = json!({ "crate-name": "keeper-server" });
    o.as_object_mut()
        .unwrap()
        .extend(options.as_object().cloned().unwrap_or_default());
    run(
        ir,
        Role::Server,
        vec![service_id(ir, "Notes")],
        vec![],
        None,
        o,
    )
}

#[test]
fn snapshots_of_a_contract_with_both_annotations() {
    let ir = small_ir();
    let server = server_of(&ir, json!({ "testing": true }));
    insta::assert_snapshot!(
        "keeper_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!("keeper_server", text(&server, "src/keeper/notes_server.rs"));
    insta::assert_snapshot!(
        "keeper_test_handler",
        text(&server, "src/keeper/notes_test_server.rs")
    );
    insta::assert_snapshot!("keeper_server_mod", text(&server, "src/veld/mod.rs"));
    let client = gen_client(&ir, "keeper-client", None);
    insta::assert_snapshot!(
        "keeper_client_file_list",
        client.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!("keeper_client", text(&client, "src/keeper/notes_client.rs"));
    insta::assert_snapshot!("keeper_client_mod", text(&client, "src/veld/mod.rs"));
}

#[test]
fn the_runtime_of_the_annotations_is_there_only_when_they_are() {
    let ir = small_ir();
    let server = server_of(&ir, json!({}));
    for path in ["src/veld/conditional.rs", "src/veld/idempotency.rs"] {
        assert!(server.get(path).is_some(), "{path}");
    }
    // A client has the `If-Match` type and no store.
    let client = gen_client(&ir, "keeper-client", None);
    assert!(client.get("src/veld/conditional.rs").is_some());
    assert!(client.get("src/veld/idempotency.rs").is_none());
    // No gate marker is left in any file.
    for files in [&server, &client] {
        for (path, bytes) in files.iter() {
            let body = String::from_utf8_lossy(bytes);
            assert!(
                !body.contains("// @if") && !body.contains("// #if") && !body.contains("// @end"),
                "{path} has a gate marker"
            );
        }
    }
    // A contract without the annotations has none of it, in any role: the existing snapshots hold
    // its bytes, and this holds the words.
    let ir = standard_ir();
    let server = gen_server(&ir, "Orders", &["Iam"], "orders-server");
    let client = gen_client(&ir, "orders-client", None);
    for files in [&server, &client] {
        for (path, bytes) in files.iter() {
            let body = String::from_utf8_lossy(bytes);
            for word in [
                "Conditional",
                "IfMatch",
                "IdempotencyStore",
                "veld::idempotency",
                "etag",
                "If-Match",
                "if_match",
            ] {
                assert!(!body.contains(word), "{path}: {word}");
            }
            assert!(
                !path.ends_with("conditional.rs") && !path.ends_with("idempotency.rs"),
                "{path}"
            );
        }
    }
}

#[test]
fn the_handler_gets_the_version_and_the_client_the_header() {
    let ir = small_ir();
    let server = server_of(&ir, json!({}));
    let body = text(&server, "src/keeper/notes_server.rs");
    assert!(body.contains(
        "fn edit(&self, request: EditRequest, facts: EditFacts, version: Option<i64>, ctx:"
    ));
    assert!(body.contains("// version note.rev"));
    // The guard sits before the first `require`, and `validate = false` does not move it.
    let off = server_of(&ir, json!({ "validate": false }));
    let body = text(&off, "src/keeper/notes_server.rs");
    assert!(body.find("// version note.rev").unwrap() < body.find("if enforce").unwrap());
    // `one` has no guard: no version, no facts.
    assert!(
        text(&server, "src/keeper/notes_server.rs")
            .contains("fn one(&self, request: OneRequest, ctx:")
    );
    let client = gen_client(&ir, "keeper-client", None);
    let body = text(&client, "src/keeper/notes_client.rs");
    assert!(body.contains("pub if_match: Option<veld::conditional::IfMatch<i64>>,"));
    assert!(body.contains("pub idempotency_key: Option<String>,"));
    assert!(body.contains("header: Some(\"If-Match\"),"));
    assert!(body.contains("header: Some(\"Idempotency-Key\"),"));
}

#[test]
fn the_hooks_require_the_store_and_the_caller_where_they_are_needed() {
    let ir = small_ir();
    let server = server_of(&ir, json!({}));
    let body = text(&server, "src/keeper/notes_server.rs");
    // No default: a server without a store does not build.
    assert!(body.contains("type IdempotencyStore: veld::idempotency::IdempotencyStore;"));
    assert!(body.contains("fn idempotency_store(&self) -> &Self::IdempotencyStore;"));
    assert!(
        !body.contains("fn idempotency_caller"),
        "the endpoint has no auth"
    );
    assert!(!body.contains("NoHooks"), "the hooks are not optional");
    assert!(!body.contains("pub fn plain"), "the hooks are not optional");
    // With auth, the caller is required too, and the scope of the key says so.
    let ir = compile_text(
        "model B { id: uuid }\nservice S at \"/s\" {\n  @auth(bearer)\n  @idempotent\n  post \"/\" make(body: B) -> 201 B\n}\n",
        "authed",
    );
    let server = run(
        &ir,
        Role::Server,
        vec![service_id(&ir, "S")],
        vec![],
        None,
        json!({ "crate-name": "authed-server" }),
    );
    let body = text(&server, "src/authed/s_server.rs");
    assert!(body.contains("fn idempotency_caller(&self, principal: &veld::server::Principal, ctx: &veld::server::RequestContext) -> String;"));
    assert!(body.contains("Some(veld::server::Idempotent { header: \"Idempotency-Key\", required: false, caller: true })"));
}

#[test]
fn a_parameter_that_the_client_needs_for_the_headers_is_refused() {
    for (name, extra) in [
        (
            "ifMatch",
            "put \"/{id}\" edit(id: uuid, @query ifMatch?: string) -> Note {\n    fact note: Note = id\n  }",
        ),
        (
            "idempotencyKey",
            "@idempotent\n  post \"/\" make(@query idempotencyKey?: string) -> 201 Note",
        ),
    ] {
        let src = format!(
            "model Note {{\n  id: uuid\n  @version\n  rev: long\n}}\nservice S at \"/s\" {{\n  {extra}\n}}\n"
        );
        let ir = compile_text(&src, "clash");
        for role in [Role::Server, Role::Client] {
            let req = GenRequest {
                ir: &ir,
                role,
                services: ir.services.iter().map(|s| s.id).collect(),
                consumes: vec![],
                framework: None,
            };
            let err = Rust
                .generate_dyn(&req, &veld_codegen::with_target_name(&json!({}), "t"))
                .expect_err("a clashing parameter is refused");
            assert!(err.iter().all(|d| d.code == "E0801"), "{name}: {err:?}");
            assert!(err[0].message.contains(name), "{err:?}");
        }
    }
}

fn rooms_server(ir: &Ir) -> FileSet {
    run(
        ir,
        Role::Server,
        vec![service_id(ir, "Rooms"), service_id(ir, "Orders")],
        vec![],
        None,
        json!({ "crate-name": "rooms-server", "testing": true }),
    )
}

/// The arguments of a handler are in the order of the pipeline: facts, `version`, `transition`
/// (`spec/rules.md` section 11.12).
#[test]
fn a_handler_takes_facts_then_version_then_transition() {
    let ir = compile_path(VERSION, "rooms");
    let server = rooms_server(&ir);
    let body = text(&server, "src/rooms/orders_server.rs");
    assert!(
        body.contains("facts: PayOrderFacts, version: Option<i64>, transition: veld::rules::Transition<"),
        "{body}"
    );
    let version = body.find("decode_key(&args, \"version\")").unwrap();
    let transition = body.find("decode_key(&args, \"transition\")").unwrap();
    assert!(version < transition);
}

/// The server (with the test handler) and the client build and pass `clippy -D warnings`; the
/// production handlers behave over HTTP; all 19 examples pass against the test handler.
#[test]
fn versions_build_behave_over_http_and_pass_every_example() {
    if !cargo_or_skip() {
        return;
    }
    let ir = compile_path(VERSION, "rooms");
    let server = rooms_server(&ir);
    let client = gen_client(&ir, "rooms-client", None);
    let ws = harness_workspace("version-ws", &server, &client, "version-harness");
    build_lint_and_run(
        &ws,
        ("rooms-server", "rooms-client", "version-harness"),
        &[],
        "version ok",
    );
    run_every_example(&ir, "version-serve", 19);
}
