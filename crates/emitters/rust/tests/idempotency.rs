//! `@idempotent` in generated Rust (`spec/rules.md` sections 11.8 to 11.12,
//! `spec/targets/rust.md` section 14): what is generated for
//! `tests/rules/endpoints/idempotency.veld.pending`, that the crates build and pass clippy, the
//! production handler over HTTP (`tests/toolchain/rust/idempotency-harness`: the key, the replay,
//! the conflicts, the scope, a store that fails, the fingerprint goldens, the client) and every
//! example of the contract through the generated test handler.
//!
//! Without `cargo` the toolchain test is skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::*;
use serde_json::json;
use veld_codegen::{FileSet, Role};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};

const IDEMPOTENCY: &str = "tests/rules/endpoints/idempotency.veld.pending";

fn contract() -> Ir {
    compile_path(IDEMPOTENCY, "bookings")
}

fn server_with(ir: &Ir, options: serde_json::Value) -> FileSet {
    let mut o = json!({ "crate-name": "bookings-server" });
    o.as_object_mut()
        .unwrap()
        .extend(options.as_object().cloned().unwrap_or_default());
    run(
        ir,
        Role::Server,
        vec![service_id(ir, "Bookings")],
        vec![],
        None,
        o,
    )
}

#[test]
fn snapshots_of_the_idempotent_contract() {
    let ir = contract();
    let server = server_with(&ir, json!({ "testing": true }));
    insta::assert_snapshot!(
        "idempotency_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!(
        "idempotency_server",
        text(&server, "src/bookings/bookings_server.rs")
    );
    insta::assert_snapshot!(
        "idempotency_test_handler",
        text(&server, "src/bookings/bookings_test_server.rs")
    );
    insta::assert_snapshot!(
        "idempotency_runtime",
        text(&server, "src/veld/idempotency.rs")
    );
    let client = gen_client(&ir, "bookings-client", None);
    insta::assert_snapshot!(
        "idempotency_client",
        text(&client, "src/bookings/bookings_client.rs")
    );
}

#[test]
fn each_endpoint_says_what_it_owes_and_the_hooks_say_how_a_caller_is_told() {
    let ir = contract();
    let server = server_with(&ir, json!({}));
    let body = text(&server, "src/bookings/bookings_server.rs");
    // `book`, `pay`, `replace`, `cancel`, `bookMine`, `saveRoom`: each has its header and flag.
    for member in [
        "Idempotent { header: \"Idempotency-Key\", required: false, caller: false }",
        "Idempotent { header: \"Idempotency-Key\", required: true, caller: false }",
        "Idempotent { header: \"X-Request-Id\", required: false, caller: false }",
        "Idempotent { header: \"Idempotency-Token\", required: true, caller: true }",
    ] {
        assert!(body.contains(member), "{member}");
    }
    // `saveRoom` also answers with a model that has a version: the `ETag` and the guard.
    assert!(body.contains("guard: true, etag: Some(\"revision\")"));
    // The caller hook is there because `bookMine` has auth, and it has no default.
    assert!(body.contains("fn idempotency_caller(&self, principal: &veld::server::Principal"));
    assert!(body.contains("fn idempotency_store(&self) -> &Self::IdempotencyStore;"));
    // A server hands the store to the core through its dispatch: three forwards, no default store.
    for forward in [
        "fn idem_begin<",
        "fn idem_complete<",
        "fn idem_release<",
        "fn idem_caller(",
    ] {
        assert!(body.contains(forward), "{forward}");
    }
    // The test handler has a store of its own and a caller text of its own.
    let test = text(
        &server_with(&ir, json!({ "testing": true })),
        "src/bookings/bookings_test_server.rs",
    )
    .to_string();
    assert!(test.contains("veld::idempotency::seeded_begin("));
    assert!(test.contains("veld-test-idempotency"));
    assert!(test.contains("\"caller\".to_string()"));
}

#[test]
fn the_idempotency_runtime_is_the_same_bytes_in_every_server() {
    let ir = contract();
    let a = server_with(&ir, json!({}));
    let b = server_with(&ir, json!({ "testing": true }));
    assert_eq!(
        text(&a, "src/veld/idempotency.rs"),
        text(&b, "src/veld/idempotency.rs")
    );
    assert!(text(&a, "src/veld/idempotency.rs").contains("pub fn sha256(data: &[u8]) -> [u8; 32]"));
    // SHA-256 is written out: the generated crate depends on nothing new for it.
    let cargo = text(&a, "Cargo.toml");
    assert!(
        !cargo.contains("sha2") && !cargo.contains("sha-2"),
        "{cargo}"
    );
}

/// The server (with the test handler) and the client build and pass `clippy -D warnings`; the
/// production handler behaves over HTTP; all 21 examples pass against the test handler.
#[test]
fn idempotency_builds_behaves_over_http_and_passes_every_example() {
    if !cargo_or_skip() {
        return;
    }
    let ir = contract();
    let server = server_with(&ir, json!({ "testing": true }));
    let client = gen_client(&ir, "bookings-client", None);
    let ws = harness_workspace("idempotency-ws", &server, &client, "idempotency-harness");
    build_lint_and_run(
        &ws,
        ("bookings-server", "bookings-client", "idempotency-harness"),
        &[],
        "idempotency ok",
    );
    run_every_example(&ir, "idempotency-serve", 21);
}

/// A service that is idempotent and has no endpoint rule still gets a test handler: it runs the
/// pipeline with no steps of its own and no clock hook (the production hooks have none).
#[test]
fn an_idempotent_service_without_rules_has_a_test_handler() {
    let ir = compile_text(
        "model B { id: uuid }\nservice S at \"/s\" {\n  @idempotent\n  post \"/\" make(body: B) -> 201 B\n}\n",
        "plain",
    );
    let server = run(
        &ir,
        Role::Server,
        vec![service_id(&ir, "S")],
        vec![],
        None,
        json!({ "crate-name": "plain-server", "testing": true }),
    );
    let test = text(&server, "src/plain/s_test_server.rs");
    assert!(test.contains("let prepared = Prepared::none();"));
    assert!(test.contains("veld::rules::clock_ms(std::time::SystemTime::now())"));
    assert!(!test.contains("hooks.clock"));
    // Without the option there is no test handler and no rules runtime.
    let plain = run(
        &ir,
        Role::Server,
        vec![service_id(&ir, "S")],
        vec![],
        None,
        json!({ "crate-name": "plain-server" }),
    );
    assert!(plain.get("src/plain/s_test_server.rs").is_none());
    assert!(plain.get("src/veld/rules/mod.rs").is_none());
}

/// The contract of the fingerprint goldens, compiled as the file `t.veld` of the package
/// `bookings` (`tests/wire/idempotency/README.md`).
fn fingerprints_ir() -> Ir {
    let text =
        std::fs::read_to_string(repo_root().join("tests/wire/idempotency/fingerprints.veld"))
            .unwrap();
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "bookings".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "t.veld".into(),
                text,
            }],
            base_url: None,
        }],
        consumes: std::collections::BTreeMap::new(),
    });
    assert!(out.ir.is_some(), "{}", out.render());
    out.ir.unwrap()
}

/// The 115 cases of `tests/wire/idempotency/fingerprints.json` through the generated production
/// server (`tests/toolchain/rust/fingerprint-harness`): the fingerprint the server gives to
/// `IdempotencyStore::begin` is the one the reference printed, a `rejected` case is answered with
/// its status and never reaches the store, and every `same_as` and `differs_from` holds. The
/// uuid, datetime and `json` cases (`*_uuid_upper`, `*_datetime_offset`, `*_datetime_millis_zero`,
/// `body_datetime_millis_25`, `body_any_null`) are the ones that failed before the fingerprint was
/// taken from the canonical form of the request; `holder_*` states that a computed derived value
/// is hashed.
#[test]
fn the_fingerprint_goldens_pass_through_the_generated_server() {
    if !cargo_or_skip() {
        return;
    }
    let ir = fingerprints_ir();
    let server = run(
        &ir,
        Role::Server,
        vec![service_id(&ir, "Fingerprints")],
        vec![],
        None,
        json!({ "crate-name": "bookings-server" }),
    );
    let ws = work_dir().join("fingerprints-ws");
    let _ = std::fs::remove_dir_all(&ws);
    write_files(&ws.join("server"), &server);
    copy_dir(
        &repo_root().join("tests/toolchain/rust/fingerprint-harness"),
        &ws.join("fingerprint-harness"),
    );
    std::fs::write(
        ws.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"server\", \"fingerprint-harness\"]\n",
    )
    .unwrap();
    let lock = repo_root().join("tests/toolchain/rust/Cargo.lock");
    if lock.exists() {
        std::fs::copy(&lock, ws.join("Cargo.lock")).unwrap();
    }
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
                "--",
                "-D",
                "warnings",
            ],
        ),
        "cargo clippy",
    );
    let goldens = repo_root().join("tests/wire/idempotency/fingerprints.json");
    let out = run_ok(
        &mut cargo(
            &ws,
            &[
                "run",
                "-q",
                "-p",
                "fingerprint-harness",
                "--",
                &goldens.to_string_lossy(),
            ],
        ),
        "the fingerprint harness",
    );
    assert!(out.contains("fingerprints ok 115"), "{out}");
}
