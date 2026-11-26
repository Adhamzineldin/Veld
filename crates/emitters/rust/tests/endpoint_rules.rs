//! Endpoint rules in generated Rust (`spec/targets/rust.md` section 10.2): snapshots of the server
//! module and the test handler, the shape of the output, the generated crates built and linted, the
//! production handler over HTTP (`tests/toolchain/rust/endpoint-harness`) and every example of
//! `tests/rules/endpoints` sent to the generated test handler.
//!
//! Without `cargo` the toolchain test is skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use common::*;
use serde_json::json;
use veld_codegen::{DynGenerator, FileSet, GenRequest, Role};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_emit_rust::Rust;
use veld_ir::{Ir, PackageKind};

fn compile_text(text: String) -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "bookings".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "bookings.veld".into(),
                text,
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    assert!(
        out.ir.is_some(),
        "the contract failed to compile:\n{}",
        out.render()
    );
    out.ir.unwrap_or_else(|| unreachable!())
}

/// `tests/rules/endpoints/bookings.veld`: the examples are the vectors.
fn endpoints_ir() -> Ir {
    let path = repo_root().join("tests/rules/endpoints/bookings.veld");
    compile_text(std::fs::read_to_string(path).unwrap())
}

fn server_with(ir: &Ir, options: serde_json::Value) -> FileSet {
    let services = vec![service_id(ir, "Bookings")];
    run(
        ir,
        Role::Server,
        services,
        vec![],
        None,
        json!({ "crate-name": "bookings-server" })
            .as_object()
            .cloned()
            .map(|mut base| {
                base.extend(options.as_object().cloned().unwrap_or_default());
                serde_json::Value::Object(base)
            })
            .unwrap_or_default(),
    )
}

fn testing() -> serde_json::Value {
    json!({ "testing": true })
}

// ---- snapshots and the shape of the output ------------------------------------------------

#[test]
fn snapshots_of_endpoint_rules() {
    let ir = endpoints_ir();
    let server = server_with(&ir, testing());
    insta::assert_snapshot!(
        "endpoint_rules_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!(
        "endpoint_rules_server_bookings",
        text(&server, "src/bookings/bookings_server.rs")
    );
    insta::assert_snapshot!(
        "endpoint_rules_test_handler",
        text(&server, "src/bookings/bookings_test_server.rs")
    );
    let client = gen_client(&ir, "bookings-client", None);
    insta::assert_snapshot!(
        "endpoint_rules_client",
        text(&client, "src/bookings/bookings_client.rs")
    );
}

#[test]
fn the_production_entry_point_does_not_change_with_testing() {
    let ir = endpoints_ir();
    let off = server_with(&ir, json!({}));
    let on = server_with(&ir, testing());
    assert!(off.get("src/bookings/bookings_test_server.rs").is_none());
    assert!(on.get("src/bookings/bookings_test_server.rs").is_some());
    for (path, bytes) in off.iter() {
        if path == "src/bookings/mod.rs" {
            continue;
        }
        assert_eq!(
            Some(bytes),
            on.get(path),
            "{path} changed when `testing` was switched on"
        );
    }
    assert_eq!(off.len() + 1, on.len(), "only the test handler is new");
    // The one line that registers the new module is the only other difference.
    let (a, b) = (
        text(&off, "src/bookings/mod.rs"),
        text(&on, "src/bookings/mod.rs"),
    );
    let extra: Vec<&str> = b.lines().filter(|l| !a.lines().any(|x| x == *l)).collect();
    assert_eq!(extra, ["pub mod bookings_test_server;"]);
    assert_eq!(a.lines().count() + 1, b.lines().count());
    // Nothing in the production module or the runtime reads a test header.
    for (path, bytes) in on.iter() {
        if path == "src/bookings/bookings_test_server.rs" {
            continue;
        }
        let body = String::from_utf8_lossy(bytes).to_lowercase();
        assert!(!body.contains("veld-test"), "{path} mentions a test header");
    }
}

#[test]
fn the_test_handler_says_at_its_top_that_it_must_never_be_mounted_in_production() {
    let ir = endpoints_ir();
    let on = server_with(&ir, testing());
    let file = text(&on, "src/bookings/bookings_test_server.rs");
    let top: String = file.lines().take(12).collect::<Vec<_>>().join("\n");
    assert!(top.contains("NEVER MOUNT IT IN PRODUCTION"), "{top}");
    assert!(file.contains("/__veld/test"));
    assert!(
        file.contains("Never mount it in production"),
        "reported on creation"
    );
}

/// `spec/runtime-config.md` section 4: the test handler takes no runtime config, and its output
/// mode is a member of its own hooks.
#[test]
fn the_test_handler_takes_no_runtime_config_and_its_hooks_hold_the_output_mode() {
    let ir = endpoints_ir();
    let on = server_with(&ir, testing());
    let file = text(&on, "src/bookings/bookings_test_server.rs");
    assert!(!file.contains("runtime"), "{file}");
    assert!(!file.contains("ConfigError"), "{file}");
    assert!(
        file.contains("pub trait BookingsTestHooks: BookingsHooks"),
        "{file}"
    );
    assert!(
        file.contains("let output = hooks.rules_output();"),
        "{file}"
    );
    assert!(
        file.contains("pub fn plain<S: BookingsService>(service: S) -> Handler<"),
        "{file}"
    );
    assert!(
        file.contains("impl BookingsTestHooks for NoHooks {}"),
        "{file}"
    );
    // The production entry still takes the config, and its hooks have no mode.
    let prod = text(&on, "src/bookings/bookings_server.rs");
    assert!(
        !prod.contains("fn rules_output(&self) -> veld::rules::RuleOutput {\n        veld"),
        "{prod}"
    );
    assert!(
        prod.contains("runtime: Option<&crate::runtime_config::BookingsServerRuntime>"),
        "{prod}"
    );
}

#[test]
fn a_contract_with_invariants_only_gets_none_of_it() {
    let ir = rules_ir();
    let server = run(
        &ir,
        Role::Server,
        vec![service_id(&ir, "Bookings")],
        vec![],
        None,
        json!({ "crate-name": "bookings-server", "testing": true }),
    );
    assert!(
        !server.paths().any(|p| p.contains("test_server")),
        "no endpoint rules, no test handler"
    );
    let all: String = server
        .iter()
        .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
        .collect();
    for word in [
        "Prepared",
        "Halt",
        "fn prepare",
        "trusted_principal",
        "FactLoaders",
    ] {
        assert!(!all.contains(word), "{word} is for endpoint rules only");
    }
    // The digest is for every contract that reaches an invariant.
    assert!(all.contains("fn digest(&self)"));
    assert!(all.contains("\"veld-rules\""));
}

#[test]
fn the_testing_option_belongs_to_the_server_and_must_be_a_boolean() {
    let ir = endpoints_ir();
    let client = GenRequest {
        ir: &ir,
        role: Role::Client,
        services: vec![service_id(&ir, "Bookings")],
        consumes: vec![],
        framework: None,
    };
    let err = Rust
        .generate_dyn(
            &client,
            &veld_codegen::with_target_name(&json!({ "testing": true }), "t"),
        )
        .unwrap_err();
    assert_eq!(err[0].code, "E0802");
    let server = GenRequest {
        role: Role::Server,
        ..client
    };
    let err = Rust
        .generate_dyn(
            &server,
            &veld_codegen::with_target_name(&json!({ "testing": "yes" }), "t"),
        )
        .unwrap_err();
    assert_eq!(err[0].code, "E0802");
}

#[test]
fn an_endpoint_rule_on_a_ws_endpoint_is_e0801() {
    use veld_ir::{EndpointEnsure, Expr, ExprKind, ExprLit, ExprType, Origin, Scalar, Type};
    let mut ir = streaming_ir();
    let ensure = EndpointEnsure {
        flag: None,
        name: None,
        id: "veld:live/Live.chat.ensure@0".into(),
        expr: Expr {
            t: ExprType::Type(Type::Scalar(Scalar::Bool)),
            e: ExprKind::Lit(ExprLit::Bool(true)),
        },
        message: None,
        reads: vec![],
        docs: None,
        rollout: None,
        origin: Origin {
            file: "live.veld".into(),
            line: 1,
            col: 1,
        },
    };
    let ws = ir
        .services
        .iter_mut()
        .flat_map(|s| s.endpoints.iter_mut())
        .find(|e| e.method == veld_ir::Method::Ws)
        .unwrap();
    ws.rules.ensures.push(ensure);
    let services = vec![service_id(&ir, "Live")];
    let req = GenRequest {
        ir: &ir,
        role: Role::Server,
        services,
        consumes: vec![],
        framework: None,
    };
    let err = Rust
        .generate_dyn(&req, &veld_codegen::with_target_name(&json!({}), "t"))
        .unwrap_err();
    assert!(err.iter().all(|d| d.code == "E0801"), "{err:?}");
    assert!(err[0].message.contains("`ws` endpoint"), "{err:?}");
}

#[test]
fn w0904_is_reported_for_endpoint_rules_with_validation_off() {
    let ir = endpoints_ir();
    let req = GenRequest {
        ir: &ir,
        role: Role::Server,
        services: vec![service_id(&ir, "Bookings")],
        consumes: vec![],
        framework: None,
    };
    let warnings = Rust.warnings_dyn(&req, &json!({ "validate": false }));
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0].code, "W0904");
    assert!(Rust.warnings_dyn(&req, &json!({})).is_empty());
}

// ---- the generated crates ---------------------------------------------------------------------

fn serve_binary() -> std::path::PathBuf {
    let exe = if cfg!(windows) {
        "endpoint-serve.exe"
    } else {
        "endpoint-serve"
    };
    target_dir().join("debug").join(exe)
}

/// A running test handler; killed when dropped.
struct Running {
    child: std::process::Child,
    url: String,
}

impl Running {
    fn start() -> Running {
        let mut child = Command::new(serve_binary())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("cannot start the test handler");
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let port = line
            .trim()
            .strip_prefix("PORT=")
            .unwrap_or_else(|| panic!("the server did not report a port: {line:?}"));
        Running {
            url: format!("http://127.0.0.1:{port}"),
            child,
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Sends every example of the contract to the generated test handler through the shared live
/// runner, in dry-run mode.
fn run_examples(ir: &Ir) {
    let server = Running::start();
    let live = veld_testing::Live::connect(&server.url, true).unwrap();
    let (mut passed, mut failures) = (0, Vec::new());
    for ex in &ir.examples {
        match live.run_example(ir, ex) {
            Ok(report) if report.passed() => passed += 1,
            Ok(report) => failures.push(format!("\"{}\": {report}", ex.name)),
            Err(e) => failures.push(format!("\"{}\": {e}", ex.name)),
        }
    }
    assert!(
        failures.is_empty(),
        "{} examples failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    eprintln!("endpoint examples passed through the generated test handler: {passed}");
    assert_eq!(passed, 20, "every example of tests/rules/endpoints");
}

/// The same handler, fuzzed: generated requests, boundaries first, must get the answer the
/// reference evaluator works out.
fn fuzz_examples(ir: &Ir) {
    let server = Running::start();
    let tally = fuzz_agrees(&server.url, ir, FUZZ_COUNT, FUZZ_SEED);
    let seen = |kind: &str| -> usize { tally.values().filter_map(|t| t.get(kind)).sum() };
    for kind in [
        "ok",
        "ValidationError",
        "NotFound",
        "RoomFull",
        "RoomClosed",
    ] {
        assert!(seen(kind) > 0, "no {kind} in {tally:?}");
    }
}

/// The shapes of block that the bookings contract does not have (`tests/toolchain/contract/
/// rules_edges`): ensure only, require only, a keyless and a scalar fact, a computed key and a
/// computed payload, an optional query parameter in a rule, an SSE endpoint with a fact, an
/// `ensure` on an endpoint with no body, and a service with blocks and no facts, each with the test
/// handler. The crates build and pass `clippy -D warnings`, and the handlers answer.
#[test]
fn the_edge_shapes_of_a_block_build_lint_and_answer() {
    if !cargo_or_skip() {
        return;
    }
    let path = repo_root().join("tests/toolchain/contract/rules_edges/edges.veld");
    let ir = {
        let out = compile(&CompileInput {
            packages: vec![PackageInput {
                name: "edges".into(),
                kind: PackageKind::Service,
                files: vec![SourceFile {
                    path: "edges.veld".into(),
                    text: std::fs::read_to_string(path).unwrap(),
                }],
                base_url: None,
            }],
            consumes: BTreeMap::new(),
        });
        assert!(out.ir.is_some(), "{}", out.render());
        out.ir.unwrap()
    };
    let server = run(
        &ir,
        Role::Server,
        vec![service_id(&ir, "Edges"), service_id(&ir, "Plain")],
        vec![],
        None,
        json!({ "crate-name": "edges-server", "testing": true }),
    );
    let client = gen_client(&ir, "edges-client", None);
    let ws = harness_workspace("endpoint-edges-ws", &server, &client, "edges-harness");
    build_lint_and_run(
        &ws,
        ("edges-server", "edges-client", "edges-harness"),
        &[],
        "edges ok",
    );
}

/// The server (with the test handler) and the client build and pass `clippy -D warnings`; the
/// production handler behaves over HTTP; every example passes against the test handler.
#[test]
fn endpoint_rules_build_behave_over_http_and_pass_every_example() {
    if !cargo_or_skip() {
        return;
    }
    let ir = endpoints_ir();
    let server = server_with(&ir, testing());
    let client = gen_client(&ir, "bookings-client", None);
    let ws = harness_workspace("endpoint-rules-ws", &server, &client, "endpoint-harness");
    build_lint_and_run(
        &ws,
        ("bookings-server", "bookings-client", "endpoint-harness"),
        &[],
        "endpoint rules ok",
    );
    run_examples(&ir);
    fuzz_examples(&ir);
}
