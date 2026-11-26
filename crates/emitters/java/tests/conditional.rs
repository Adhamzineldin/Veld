//! `@version` and `@idempotent` (`spec/rules.md` sections 11.7 to 11.12, `spec/targets/java.md`
//! section 13): what is generated for the two vector contracts and for a small contract with both
//! annotations, that it compiles under `javac -Xlint:all -Werror` for every role and every
//! combination of the target options, that every example of the vectors passes through the generated
//! test handler on the JDK server and on Spring, and what the production handler and the client do
//! over HTTP.
//!
//! The vector files keep their `.pending` suffix until every generator implements the features.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::live::*;
use common::*;
use veld_codegen::{DynGenerator, FileSet, GenRequest, Role};
use veld_emit_java::Java;
use veld_ir::Ir;

pub const VERSION: &str = "tests/rules/endpoints/version.veld.pending";
pub const IDEMPOTENCY: &str = "tests/rules/endpoints/idempotency.veld.pending";

/// The fingerprints of the reference (`crates/veld-rules`): defaults, optionals, an empty request.
const FINGERPRINTED: &str = r#"
model Body {
  n: int
  note: string = "none"
  tag?: string
}

model Guests { guests: int }

service F at "/f" {
  @idempotent
  put "/{id}" make(id: uuid, body: Body, @query dry?: bool) -> Body

  @idempotent
  post "/guests" guests(body: Guests) -> 201 Guests

  @idempotent
  post "/ping" ping() -> 204
}
"#;

/// A small contract with both annotations on one endpoint and an `ETag` on another.
const SMALL: &str = r#"
model Room {
  id: uuid
  name: string
  @version
  revision: long
}

model Rename { name: string }

service Rooms at "/rooms" {
  @idempotent
  put "/{id}" rename(id: uuid, body: Rename) -> Room | NotFound {
    fact room: Room = id
  }

  get "/{id}" getRoom(id: uuid) -> Room | NotFound
}
"#;

/// Parameters that the client's request would shadow.
const CLASH_IF_MATCH: &str = r#"
model Room {
  id: uuid
  name: string
  @version
  revision: long
}

service Rooms at "/rooms" {
  put "/{id}" rename(id: uuid, @query ifMatch?: string) -> Room {
    fact room: Room = id
  }
}
"#;

const CLASH_KEY: &str = r#"
model Booking { id: uuid }

service Bookings at "/bookings" {
  @idempotent
  post "/" book(@query idempotencyKey?: string) -> 201 Booking
}
"#;

fn version_ir() -> Ir {
    compile_path(VERSION, "vers")
}

fn idempotency_ir() -> Ir {
    compile_path(IDEMPOTENCY, "idemp")
}

fn fingerprint_ir() -> Ir {
    compile_source(FINGERPRINTED.to_string(), "fp", "the fingerprint contract")
}

fn small_ir() -> Ir {
    compile_source(SMALL.to_string(), "small", "the small contract")
}

fn generate(ir: &Ir, role: Role, framework: Option<&str>, testing: bool) -> FileSet {
    let mut o = options("gen.b", Some("17"));
    if testing {
        o["testing"] = serde_json::json!(true);
    }
    let all = ir.services.iter().map(|s| s.id).collect();
    run(ir, role, all, vec![], framework, o)
}

// --- the examples are the vectors --------------------------------------------------------------

#[test]
fn the_vector_contracts_have_the_examples_these_tests_count() {
    assert_eq!(version_ir().examples.len(), 19);
    assert_eq!(idempotency_ir().examples.len(), 21);
}

// --- what is generated -----------------------------------------------------------------------------

#[test]
fn the_small_contract_is_snapshotted_for_the_server_the_client_and_the_test_handler() {
    let ir = small_ir();
    let server = generate(&ir, Role::Server, None, true);
    let names: Vec<&str> = server
        .paths()
        .filter(|p| !p.contains("/runtime/"))
        .collect();
    insta::assert_snapshot!("conditional_server_file_list", names.join("\n"));
    for name in [
        "RoomsHandler",
        "RoomsHooks",
        "RoomsService",
        "RoomsTestHandler",
        "RenameFacts",
    ] {
        let path = format!("gen/b/small/server/{name}.java");
        insta::assert_snapshot!(format!("conditional_{name}"), text(&server, &path));
    }
    for name in [
        "Conditions",
        "IdempotencyStore",
        "MemoryIdempotencyStore",
        "StoredAnswer",
    ] {
        let path = format!("gen/b/runtime/{name}.java");
        insta::assert_snapshot!(format!("conditional_runtime_{name}"), text(&server, &path));
    }
    let client = generate(&ir, Role::Client, None, false);
    let names: Vec<&str> = client
        .paths()
        .filter(|p| !p.contains("/runtime/"))
        .collect();
    insta::assert_snapshot!("conditional_client_file_list", names.join("\n"));
    for name in ["RoomsClient", "RenameRequest", "GetRoomRequest"] {
        let path = format!("gen/b/small/client/{name}.java");
        insta::assert_snapshot!(format!("conditional_client_{name}"), text(&client, &path));
    }
    insta::assert_snapshot!(
        "conditional_client_IfMatch",
        text(&client, "gen/b/runtime/IfMatch.java")
    );
}

/// A contract without the annotations generates the bytes it generated before: the existing
/// snapshots are that test, and this one says it from the inside. Nothing of the two features is in
/// the output, not even a region of the runtime.
#[test]
fn a_contract_without_the_annotations_has_no_trace_of_them() {
    let ir = standard_ir();
    let server = gen_server(&ir, "Orders", &["Iam"], "gen.a", None);
    let client = gen_client(&ir, "gen.c");
    for files in [&server, &client] {
        for path in files.paths() {
            for class in [
                "Conditions",
                "IdempotencyStore",
                "MemoryIdempotencyStore",
                "StoredAnswer",
                "IfMatch",
            ] {
                assert!(
                    !path.ends_with(&format!("/{class}.java")),
                    "{path} exists without the annotations"
                );
            }
        }
        for (path, bytes) in files.iter() {
            let text = String::from_utf8_lossy(bytes);
            for word in [
                "Conditions",
                "IdempotencyStore",
                "idempotency",
                "IfMatch",
                "If-Match",
                "ifMatch",
                "ETag",
                "etag",
                "PreconditionFailed",
            ] {
                assert!(!text.contains(word), "{path} mentions `{word}`");
            }
        }
    }
}

#[test]
fn the_handler_gets_the_version_after_the_facts_and_before_the_transition() {
    let ir = version_ir();
    let server = generate(&ir, Role::Server, None, false);
    let rooms = text(&server, "gen/b/vers/server/RoomsService.java");
    assert!(
        rooms.contains(
            "Room rename(RenameRequest request, RenameFacts facts, Long version, RequestContext ctx)"
        ),
        "a long version:\n{rooms}"
    );
    assert!(
        rooms.contains(
            "Counter tick(TickRequest request, TickFacts facts, Integer version, RequestContext ctx)"
        ),
        "an int version:\n{rooms}"
    );
    assert!(
        rooms.contains("Room getRoom(GetRoomRequest request, RequestContext ctx)"),
        "an endpoint without a guard has no version:\n{rooms}"
    );
    assert!(
        rooms.contains("The check is a guard, not a guarantee"),
        "the doc says what the doc of a transition says"
    );
    let orders = text(&server, "gen/b/vers/server/OrdersService.java");
    assert!(
        orders.contains("Long version, Transition<Status> transition, RequestContext ctx"),
        "the version before the transition:\n{orders}"
    );
    let handler = text(&server, "gen/b/vers/server/OrdersHandler.java");
    let allow = handler.find("run.allow(").unwrap();
    let version = handler.find("run.version(").unwrap();
    let transition = handler.find("run.transition(").unwrap();
    let require = handler.find("run.require(").unwrap();
    assert!(
        allow < version && version < transition && transition < require,
        "policies, version, transition, require:\n{handler}"
    );
}

#[test]
fn the_hooks_of_an_idempotent_service_take_the_store_and_the_caller() {
    let ir = idempotency_ir();
    let server = generate(&ir, Role::Server, None, true);
    let hooks = text(&server, "gen/b/idemp/server/BookingsHooks.java");
    assert!(
        hooks.contains(
            "public BookingsHooks(HandlerHooks.BearerAuth bearer, IdempotencyStore store, HandlerHooks.IdempotencyCaller caller)"
        ),
        "the type requires them:\n{hooks}"
    );
    let handler = text(&server, "gen/b/idemp/server/BookingsHandler.java");
    assert!(
        !handler.contains("create(BookingsService impl, BookingsFactLoaders loaders)"),
        "there is no handler without hooks"
    );
    assert!(handler.contains("static Map<String, Conditions> conditions()"));
    let test = text(&server, "gen/b/idemp/server/BookingsTestHandler.java");
    assert!(
        test.contains("TestSupport.store(BookingsHandler.conditions())"),
        "the test handler brings its own store:\n{test}"
    );
}

#[test]
fn a_service_with_no_authenticated_idempotent_endpoint_needs_no_caller_hook() {
    let ir = fingerprint_ir();
    let server = generate(&ir, Role::Server, None, false);
    let hooks = text(&server, "gen/b/fp/server/FHooks.java");
    assert!(
        hooks.contains("public FHooks(IdempotencyStore store)"),
        "{hooks}"
    );
    assert!(!hooks.contains("IdempotencyCaller"), "{hooks}");
}

#[test]
fn the_client_request_takes_if_match_and_the_key_after_the_declared_parameters() {
    let ir = idempotency_ir();
    let client = generate(&ir, Role::Client, None, false);
    let save = text(&client, "gen/b/idemp/client/SaveRoomRequest.java");
    assert!(
        save.contains(
            "UUID id,\n    Rename body,\n    IfMatch ifMatch,\n    String idempotencyKey) {"
        ),
        "{save}"
    );
    assert!(
        !save.contains("fromWire"),
        "a client never reads a request: {save}"
    );
    let pay = text(&client, "gen/b/idemp/client/PayRequest.java");
    assert!(
        pay.contains("Objects.requireNonNull(idempotencyKey, \"idempotencyKey\");"),
        "a required key is required by the type:\n{pay}"
    );
    let book = text(&client, "gen/b/idemp/client/BookRequest.java");
    assert!(
        !book.contains("requireNonNull"),
        "an optional key may be null:\n{book}"
    );
    let wire = &save[save.find("toWire").unwrap()..];
    assert!(
        !wire.contains("ifMatch") && !wire.contains("idempotencyKey"),
        "neither is a parameter of the wire form"
    );
    let rooms = text(&client, "gen/b/idemp/client/BookingsClient.java");
    assert!(
        rooms.contains("core.call(SAVE_ROOM, request.toWire(), ClientCore.conditional(request.ifMatch(), \"Idempotency-Key\", request.idempotencyKey()))"),
        "{rooms}"
    );
    assert!(
        rooms.contains("ClientCore.conditional(null, \"X-Request-Id\", request.idempotencyKey())"),
        "the header the annotation names:\n{rooms}"
    );
}

#[test]
fn a_parameter_called_like_the_header_the_client_takes_is_refused() {
    for (source, name) in [(CLASH_IF_MATCH, "ifMatch"), (CLASH_KEY, "idempotencyKey")] {
        let ir = compile_source(source.to_string(), "clash", name);
        for role in [Role::Server, Role::Client] {
            let req = GenRequest {
                ir: &ir,
                role,
                services: ir.services.iter().map(|s| s.id).collect(),
                consumes: vec![],
                framework: None,
            };
            let diags = Java
                .generate_dyn(&req, &options("gen.b", Some("17")))
                .unwrap_err();
            assert!(
                diags
                    .iter()
                    .all(|d| d.code == "E0801" && d.message.contains(name)),
                "{name} / {}: {diags:?}",
                role.name()
            );
        }
    }
}

// --- the generated code compiles with every warning on -----------------------------------------

fn conformance_irs() -> Vec<Ir> {
    veld_emit_conformance::compiled_version_contracts()
        .into_iter()
        .chain(veld_emit_conformance::compiled_idempotency_contracts())
        .map(|(ir, _)| ir)
        .collect()
}

/// Every contract that uses either feature, in every role and with the test handler on and off.
/// The four contracts of the conformance kit have no endpoint rules but an `ETag`, a key or a
/// guard: the combinations of the runtime that the vectors do not reach.
#[test]
fn every_contract_with_the_annotations_compiles_with_every_warning_on() {
    if !require_javac("the_conditional_contracts_compile") {
        return;
    }
    let mut irs = vec![version_ir(), idempotency_ir(), fingerprint_ir(), small_ir()];
    irs.extend(conformance_irs());
    for ir in &irs {
        for (role, testing) in [
            (Role::Server, false),
            (Role::Server, true),
            (Role::Client, false),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let sources = write_files(dir.path(), &generate(ir, role, None, testing));
            for release in releases() {
                javac_ok(dir.path(), &sources, release);
            }
        }
    }
}

// --- the vectors through the generated test handler ------------------------------------------------------

fn serve_every_example(ir: &Ir, helpers: &[&str], class: &str) -> usize {
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &generate(ir, Role::Server, None, true));
    for helper in helpers {
        sources.push(repo_root().join("tests/toolchain/java/rules").join(helper));
    }
    javac_ok(work.path(), &sources, "17");
    let server = Running::start(work.path(), class, &[]);
    run_every_example(ir, &server.url)
}

#[test]
fn every_version_example_passes_through_the_generated_test_handler_on_the_jdk_server() {
    if !require_javac("every_version_example_passes_on_the_jdk_server") {
        return;
    }
    let ir = version_ir();
    let passed = serve_every_example(
        &ir,
        &["VersionStubs.java", "VersionServe.java"],
        "veldtest.VersionServe",
    );
    eprintln!("version examples passed on the JDK test handler: {passed}");
    assert_eq!(passed, 19, "every example of the version contract");
}

#[test]
fn every_idempotency_example_passes_through_the_generated_test_handler_on_the_jdk_server() {
    if !require_javac("every_idempotency_example_passes_on_the_jdk_server") {
        return;
    }
    let ir = idempotency_ir();
    let passed = serve_every_example(
        &ir,
        &["IdempotencyStubs.java", "IdempotencyServe.java"],
        "veldtest.IdempotencyServe",
    );
    eprintln!("idempotency examples passed on the JDK test handler: {passed}");
    assert_eq!(passed, 21, "every example of the idempotency contract");
}

fn spring_every_example(ir: &Ir, dir: &str, apps: &[&str]) -> usize {
    let files = generate(ir, Role::Server, Some("spring"), true);
    let app = start_spring(dir, &files, apps, "/__veld/test", &[]);
    run_every_example(ir, &format!("http://127.0.0.1:{}", app.port))
}

#[test]
fn every_version_example_passes_through_the_generated_test_handler_on_spring() {
    let name = "every_version_example_passes_on_spring";
    if !require_javac(name) || !spring_available(name) {
        return;
    }
    let passed = spring_every_example(
        &version_ir(),
        "spring-version",
        &["rules/VersionStubs.java", "rules/spring-version/App.java"],
    );
    eprintln!("version examples passed on the Spring test handler: {passed}");
    assert_eq!(passed, 19, "every example of the version contract");
}

#[test]
fn every_idempotency_example_passes_through_the_generated_test_handler_on_spring() {
    let name = "every_idempotency_example_passes_on_spring";
    if !require_javac(name) || !spring_available(name) {
        return;
    }
    let passed = spring_every_example(
        &idempotency_ir(),
        "spring-idempotency",
        &[
            "rules/IdempotencyStubs.java",
            "rules/spring-idempotency/App.java",
        ],
    );
    eprintln!("idempotency examples passed on the Spring test handler: {passed}");
    assert_eq!(passed, 21, "every example of the idempotency contract");
}

// --- the production handler over HTTP ----------------------------------------------------------------------

/// Compiles the production server of the contract and the drivers and runs the first one.
fn run_driver(ir: &Ir, drivers: &[&str], class: &str) {
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &generate(ir, Role::Server, None, false));
    for driver in drivers {
        sources.push(repo_root().join("tests/toolchain/java/rules").join(driver));
    }
    javac_ok(work.path(), &sources, "17");
    let out = java_main(work.path(), class, &[]);
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains("OK"),
        "{class} failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A tag that matches, a stale tag and its body, `*`, no header, every malformed form, the version
/// the handler receives, the `ETag`, the order of the checks and an endpoint without a guard.
#[test]
fn the_production_handler_guards_versions_and_writes_etags() {
    if !require_javac("the_production_handler_guards_versions") {
        return;
    }
    run_driver(&version_ir(), &["VersionHttp.java"], "veldtest.VersionHttp");
}

/// A new key, a replay, other parameters, a running request, a stored error, a 500, the key itself,
/// two callers, a server without a hook and a store that fails.
#[test]
fn the_production_handler_stores_and_replays_idempotent_answers() {
    if !require_javac("the_production_handler_replays") {
        return;
    }
    run_driver(
        &idempotency_ir(),
        &["IdempotencyHttp.java"],
        "veldtest.IdempotencyHttp",
    );
}

/// The fingerprints of the reference, computed by the generated runtime.
#[test]
fn the_runtime_computes_the_fingerprints_of_the_reference() {
    if !require_javac("the_runtime_computes_the_fingerprints") {
        return;
    }
    run_driver(
        &fingerprint_ir(),
        &["IdempotencyFingerprint.java"],
        "veldtest.IdempotencyFingerprint",
    );
}

/// The headers a call sends for `ifMatch` (a version and `*`) and `idempotencyKey`.
#[test]
fn the_client_sends_if_match_and_the_key() {
    if !require_javac("the_client_sends_if_match_and_the_key") {
        return;
    }
    let ir = idempotency_ir();
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &generate(&ir, Role::Client, None, false));
    sources.push(repo_root().join("tests/toolchain/java/rules/ConditionalClient.java"));
    javac_ok(work.path(), &sources, "17");
    let out = java_main(work.path(), "veldtest.ConditionalClient", &[]);
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains("OK"),
        "ConditionalClient failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
