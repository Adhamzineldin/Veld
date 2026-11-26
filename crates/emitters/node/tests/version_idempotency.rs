//! `@version` and `@idempotent` in the generated TypeScript (`spec/targets/typescript.md` section
//! 5g, `spec/rules.md` sections 11.7 to 11.12): snapshots of the server module, the test handler and
//! the client; what a contract without the annotations does not get; `tsc --strict` on all of it,
//! for both ECMAScript editions; every example of the two vector files sent to the generated test
//! handler; the production handler over HTTP; the fingerprint goldens of the reference evaluator;
//! and `validate = false`.
//!
//! Without `node` or `node_modules` the toolchain tests are skipped unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::toolchain::*;
use common::*;
use serde_json::json;
use veld_codegen::{DynGenerator, GenRequest, Role};
use veld_ir::Ir;

const VERSION: &str = "tests/rules/endpoints/version.veld.pending";
const IDEMPOTENCY: &str = "tests/rules/endpoints/idempotency.veld.pending";
const SHELVES: &str = "tests/toolchain/contract/version_idempotency/shelves.veld";

/// The contract of the reference evaluator's fingerprint goldens
/// (`crates/veld-rules/tests/version_idempotency.rs`), with an endpoint that has no parameter.
const FINGERPRINTED: &str = r#"
model Body {
  n: int
  note: string = "none"
  tag?: string
}

service F at "/f" {
  @idempotent
  put "/{id}" make(id: uuid, body: Body, @query dry?: bool) -> Body

  @idempotent
  post "/ping" ping() -> 201 Body

  @idempotent
  post "/stamp" stamp(@query instant: datetime) -> 201 Body
}
"#;

fn testing() -> serde_json::Value {
    json!({ "testing": true })
}

fn version_ir() -> Ir {
    compile_path(VERSION)
}

fn idempotency_ir() -> Ir {
    compile_path(IDEMPOTENCY)
}

// --- what is generated -------------------------------------------------------------------------

#[test]
fn snapshots_of_a_contract_with_both_annotations() {
    let ir = compile_path(SHELVES);
    let server = gen_server_with(&ir, "Shelves", &[], &testing());
    insta::assert_snapshot!(
        "version_idempotency_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!(
        "version_idempotency_server",
        text(&server, "server/shelves.ts")
    );
    insta::assert_snapshot!(
        "version_idempotency_test_handler",
        text(&server, "server/shelves-test-handler.ts")
    );
    let client = gen_client(&ir);
    insta::assert_snapshot!(
        "version_idempotency_client",
        text(&client, "bookings/client.ts")
    );
}

#[test]
fn the_server_module_and_the_client_have_the_names_of_the_spec() {
    let ir = compile_path(SHELVES);
    let server = gen_server_with(&ir, "Shelves", &[], &testing());
    let module = text(&server, "server/shelves.ts");
    // The handler gets the checked version, typed like the version field, optional.
    assert!(module.contains("RenameRequest & RenameFacts & RenameVersion"));
    assert!(module.contains("version?: string;"));
    assert!(module.contains("not an atomic guarantee"));
    assert!(module.contains("answer `PreconditionFailed` yourself"));
    // The hooks require the store; there is no auth, so no caller hook.
    assert!(module.contains("idempotency: { store: veld.IdempotencyStore };"));
    // The endpoint descriptions carry the guard, the ETag and the key.
    assert!(module.contains("ifMatch: true,"));
    assert!(module.contains("etag: 'revision',"));
    assert!(
        module.contains(
            "idempotency: { header: 'Idempotency-Key', required: false, caller: false },"
        )
    );
    assert!(
        module.contains("idempotency: { header: 'X-Request-Id', required: true, caller: false },")
    );
    assert!(module.contains("version: {"));
    // The std signatures name `Conflict`, so the runtime can answer 409.
    assert!(module.contains("conflict: Conflict.sig"));
    let runtime = text(&server, "runtime/veld.ts");
    for word in [
        "export interface IdempotencyStore",
        "export class MemoryIdempotencyStore",
        "export interface StoredAnswer",
        "function checkVersion",
        "function parseIfMatch",
        "function headerIssues",
        "veld-idempotent-replay",
    ] {
        assert!(
            runtime.contains(word),
            "`{word}` is missing from the runtime"
        );
    }
    // Plain code only: no platform API, so the same file runs on Node, Deno, Bun and the edge.
    for word in ["crypto", "require(", "Buffer.from"] {
        assert!(!runtime.contains(word), "`{word}` in the runtime");
    }
    let client = gen_client(&ir);
    let client_module = text(&client, "bookings/client.ts");
    // `ifMatch` is a version typed like the field, or `'*'`; the key is optional, and required when the annotation says so.
    assert!(client_module.contains("ifMatch?: string | '*';"));
    assert!(client_module.contains("idempotencyKey?: string;"));
    assert!(client_module.contains("idempotencyKey: string;"));
    assert!(client_module.contains("sent in the `X-Request-Id` header"));
    // An endpoint without the annotations has neither.
    let get = client_module
        .split("export interface GetShelfRequest")
        .nth(1)
        .and_then(|s| s.split("const shelvesGetShelfOp").next())
        .unwrap();
    assert!(
        !get.contains("ifMatch") && !get.contains("idempotencyKey"),
        "{get}"
    );
}

#[test]
fn a_contract_without_the_annotations_has_none_of_the_code() {
    for ir in [
        standard_ir(),
        rules_ir(),
        compile_path("tests/rules/endpoints/transitions.veld"),
    ] {
        let services: Vec<&str> = ir.services.iter().map(|s| s.name.as_str()).collect();
        let client = gen_client(&ir);
        let server = gen_server_with(&ir, services[0], &[], &testing());
        for fs in [&client, &server] {
            let runtime = text(fs, "runtime/veld.ts");
            for word in [
                "parseIfMatch",
                "withEtag",
                "checkVersion",
                "headerIssues",
                "headerRefusal",
                "IdempotencyStore",
                "MemoryIdempotencyStore",
                "sha256Hex",
                "runCore",
                "ifMatchText",
            ] {
                assert!(!runtime.contains(word), "`{word}` leaked into the runtime");
            }
        }
        for path in server.paths().filter(|p| p.ends_with(".ts")) {
            let file = text(&server, path);
            assert!(!file.contains("idempotency"), "{path}");
        }
        for path in client.paths().filter(|p| p.ends_with("client.ts")) {
            let file = text(&client, path);
            assert!(
                !file.contains("ifMatch") && !file.contains("idempotencyKey"),
                "{path}"
            );
        }
    }
}

#[test]
fn a_contract_with_an_etag_only_gets_the_etag_and_no_guard() {
    let ir = compile_text(
        "model Room {\n  id: uuid\n  @version\n  revision: int\n}\n\nservice Rooms at \"/rooms\" {\n  get \"/{id}\" getRoom(id: uuid) -> Room | NotFound\n}\n"
            .to_string(),
    );
    let server = gen_server(&ir, "Rooms", &[]);
    let runtime = text(&server, "runtime/veld.ts");
    assert!(runtime.contains("function withEtag"));
    assert!(!runtime.contains("checkVersion") && !runtime.contains("IdempotencyStore"));
    assert!(text(&server, "server/rooms.ts").contains("etag: 'revision',"));
    let client = gen_client(&ir);
    assert!(!text(&client, "bookings/client.ts").contains("ifMatch"));
}

fn refused_by(
    generator: &dyn DynGenerator,
    ir: &Ir,
    role: Role,
) -> Vec<veld_codegen::GenDiagnostic> {
    let req = GenRequest {
        ir,
        role,
        services: ir.services.iter().map(|s| s.id).collect(),
        consumes: vec![],
        framework: None,
    };
    generator
        .generate_dyn(&req, &common::named(role, &serde_json::Value::Null))
        .unwrap_err()
}

/// `spec/rules.md` section 11.12: a declared parameter with the name of a member the client adds.
#[test]
fn a_parameter_called_like_a_member_the_client_adds_is_refused_with_e0801() {
    let source = std::fs::read_to_string(repo_root().join(SHELVES)).unwrap();
    for (clash, name) in [
        ("@header(\"X-Tag\") ifMatch?: string", "ifMatch"),
        (
            "@header(\"X-Key\") idempotencyKey?: string",
            "idempotencyKey",
        ),
    ] {
        let clashing = source.replace(
            "rename(id: uuid, body: Rename)",
            &format!("rename(id: uuid, body: Rename, {clash})"),
        );
        assert_ne!(source, clashing);
        let ir = compile_text(clashing);
        for (generator, role) in [
            (&veld_emit_node::Node as &dyn DynGenerator, Role::Server),
            (
                &veld_emit_typescript::TypeScript as &dyn DynGenerator,
                Role::Client,
            ),
        ] {
            let err = refused_by(generator, &ir, role);
            assert!(err.iter().all(|d| d.code == "E0801"), "{err:?}");
            assert!(err[0].message.contains(name), "{err:?}");
            assert!(err[0].origin.is_some(), "{err:?}");
        }
    }
}

#[test]
fn the_idempotent_test_handler_builds_one_store_per_request() {
    let ir = compile_path(SHELVES);
    let server = gen_server_with(&ir, "Shelves", &[], &testing());
    let handler = text(&server, "server/shelves-test-handler.ts");
    assert!(handler.contains("veld-test-idempotency"));
    assert!(handler.contains("class TestStore implements veld.IdempotencyStore"));
    assert!(handler.contains("'veld-test-principal'"));
    assert!(handler.contains("'caller'"));
    assert!(handler.contains("'facts' | 'clock' | 'auth' | 'middleware' | 'idempotency'"));
    // A contract with endpoint rules and no `@idempotent` gets the handler it always had.
    let plain = gen_server_with(
        &compile_path("tests/rules/endpoints/transitions.veld"),
        "Bookings",
        &[],
        &testing(),
    );
    assert!(!text(&plain, "server/bookings-test-handler.ts").contains("TestStore"));
}

#[test]
fn a_service_with_only_idempotent_endpoints_gets_a_test_handler() {
    let ir = compile_text(
        "model Booking {\n  id: uuid\n  guests: int\n}\n\nservice Bookings at \"/bookings\" {\n  @idempotent\n  post \"/\" book(body: Booking) -> 201 Booking\n}\n"
            .to_string(),
    );
    let server = gen_server_with(&ir, "Bookings", &[], &testing());
    assert!(server.get("server/bookings-test-handler.ts").is_some());
    let module = text(&server, "server/bookings.ts");
    assert!(module.contains("export function bookingsEndpoints(impl: BookingsService)"));
    // The hooks are required, so the factory has no default for them.
    assert!(module.contains("hooks: BookingsHooks)"));
    assert!(!module.contains("hooks: BookingsHooks = {}"));
    // The runtime has what the test handler stops a dry run with, without any endpoint rules.
    assert!(text(&server, "runtime/veld.ts").contains("export class EarlyResponse"));
}

// --- tsc --strict, the examples, and HTTP ------------------------------------------------------

fn version_examples_run(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = version_ir();
    let guard = scratch(prefix);
    let work = guard.path();
    build_services(
        work,
        &tsc,
        &ir,
        &["Rooms", "Orders"],
        &["version_serve.ts", "version_http.ts"],
        edition,
        &json!({}),
    );
    {
        let server = Live::start(work, "version_serve.js");
        let passed = run_examples(&server.url, &ir);
        assert_eq!(ir.examples.len(), 19);
        assert_eq!(
            passed,
            ir.examples.len(),
            "every example ran, none was skipped"
        );
    }
    run(
        std::process::Command::new("node")
            .arg("out/version_http.js")
            .current_dir(work),
        "node version_http.js",
    );
}

#[test]
fn version_compiles_strict_and_passes_every_example_es2020() {
    version_examples_run("es2020", "version-2020-");
}

#[test]
fn version_compiles_strict_and_passes_every_example_es2022() {
    version_examples_run("es2022", "version-2022-");
}

fn idempotency_examples_run(edition: &str, prefix: &str) {
    let Some(tsc) = tsc_path() else { return };
    let ir = idempotency_ir();
    let guard = scratch(prefix);
    let work = guard.path();
    build_service(
        work,
        &tsc,
        &ir,
        "Bookings",
        &["idempotency_serve.ts", "idempotency_http.ts"],
        edition,
    );
    {
        let server = Live::start(work, "idempotency_serve.js");
        let passed = run_examples(&server.url, &ir);
        assert_eq!(ir.examples.len(), 21);
        assert_eq!(
            passed,
            ir.examples.len(),
            "every example ran, none was skipped"
        );
    }
    run(
        std::process::Command::new("node")
            .arg("out/idempotency_http.js")
            .current_dir(work),
        "node idempotency_http.js",
    );
}

#[test]
fn idempotency_compiles_strict_and_passes_every_example_es2020() {
    idempotency_examples_run("es2020", "idempotency-2020-");
}

#[test]
fn idempotency_compiles_strict_and_passes_every_example_es2022() {
    idempotency_examples_run("es2022", "idempotency-2022-");
}

/// The generated runtime computes the fingerprints of the reference evaluator.
#[test]
fn the_fingerprints_are_the_ones_of_the_reference_evaluator() {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_text(FINGERPRINTED.to_string());
    let guard = scratch("fingerprint-");
    let work = guard.path();
    build_service(
        work,
        &tsc,
        &ir,
        "F",
        &["idempotency_fingerprint.ts"],
        "es2020",
    );
    run(
        std::process::Command::new("node")
            .arg("out/idempotency_fingerprint.js")
            .current_dir(work),
        "node idempotency_fingerprint.js",
    );
}

/// `validate = false` switches off neither the guard nor the syntax of `If-Match` and of the key.
#[test]
fn validate_false_keeps_the_guard_and_the_key() {
    let Some(tsc) = tsc_path() else { return };
    let ir = idempotency_ir();
    let guard = scratch("idempotency-no-validate-");
    let work = guard.path();
    build_services(
        work,
        &tsc,
        &ir,
        &["Bookings"],
        &["idempotency_no_validate.ts"],
        "es2020",
        &json!({ "validate": false }),
    );
    run(
        std::process::Command::new("node")
            .arg("out/idempotency_no_validate.js")
            .current_dir(work),
        "node idempotency_no_validate.js",
    );
}

/// The Nest server of a service with an `@idempotent` endpoint: its module needs the hooks, and it
/// compiles with the decorator flags.
#[test]
fn the_nest_server_with_an_idempotency_hook_compiles_with_the_decorator_flags() {
    let Some(tsc) = tsc_path() else { return };
    let ir = idempotency_ir();
    let server = gen_server_fw(&ir, "Bookings", &[], &serde_json::Value::Null, Some("nest"));
    let module = text(&server, "server/nest/bookings.ts");
    assert!(
        module.contains("hooks: BookingsHooks;"),
        "the hooks are required: {module}"
    );
    let guard = scratch("idempotency-nest-");
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

/// The client of a contract with a required key and no other parameter has no default request, in
/// the query options and in the React helpers too.
#[test]
fn a_required_key_is_required_in_every_helper_of_the_client() {
    let ir = compile_text(
        "model Job {\n  id: uuid\n}\n\nservice Jobs at \"/jobs\" {\n  @idempotent(required: true)\n  post \"/start\" start() -> 201 Job\n\n  @idempotent\n  post \"/stop\" stop() -> 201 Job\n}\n"
            .to_string(),
    );
    let client = gen_client_fw(&ir, &json!({ "query-options": true }), Some("react"));
    let queries = text(&client, "bookings/queries.ts");
    assert!(
        !queries.contains("mutationFn: (req: StartRequest = {})"),
        "{queries}"
    );
    assert!(
        queries.contains("mutationFn: (req: StartRequest)"),
        "{queries}"
    );
    assert!(
        queries.contains("mutationFn: (req: StopRequest = {})"),
        "{queries}"
    );
    let module = text(&client, "bookings/client.ts");
    assert!(
        module.contains("start(req: StartRequest, init?"),
        "{module}"
    );
    assert!(
        module.contains("stop(req: StopRequest = {}, init?"),
        "{module}"
    );
}
