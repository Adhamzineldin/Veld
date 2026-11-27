//! Endpoint rules in the generated TypeScript (`spec/targets/typescript.md` section 5c): snapshots
//! of the server module, the test handler and the client; `tsc --strict` on all of them; an HTTP
//! round trip of the production handler and the client; and every example of
//! `tests/rules/endpoints` sent to the generated test handler.
//!
//! Without `node` or `node_modules` the toolchain tests are skipped unless
//! `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use common::*;
use serde_json::json;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_project::output::{WriteOptions, write};

/// The contract of the endpoint rules vectors. The other contracts of `tests/rules/endpoints`
/// declare a service of the same name with other endpoints; they have their own test handlers
/// and their own tests (`state_machines.rs`, `policies.rs`).
fn endpoint_contracts() -> Vec<PathBuf> {
    vec![repo_root().join("tests/rules/endpoints/bookings.veld")]
}

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

fn testing() -> serde_json::Value {
    json!({ "testing": true })
}

// ---- snapshots and the shape of the output ------------------------------------------------

#[test]
fn snapshots_of_endpoint_rules() {
    let ir = endpoints_ir();
    let server = gen_server_with(&ir, "Bookings", &[], &testing());
    insta::assert_snapshot!(
        "endpoint_rules_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!(
        "endpoint_rules_server_bookings",
        text(&server, "server/bookings.ts")
    );
    insta::assert_snapshot!(
        "endpoint_rules_test_handler",
        text(&server, "server/bookings-test-handler.ts")
    );
    let client = gen_client(&ir);
    insta::assert_snapshot!("endpoint_rules_client", text(&client, "bookings/client.ts"));
}

#[test]
fn the_production_entry_point_does_not_change_with_testing() {
    let ir = endpoints_ir();
    let off = gen_server(&ir, "Bookings", &[]);
    let on = gen_server_with(&ir, "Bookings", &[], &testing());
    assert!(off.get("server/bookings-test-handler.ts").is_none());
    assert!(on.get("server/bookings-test-handler.ts").is_some());
    for (path, bytes) in off.iter() {
        assert_eq!(
            Some(bytes),
            on.get(path),
            "{path} changed when `testing` was switched on"
        );
    }
    assert_eq!(off.len() + 1, on.len(), "only the test handler is new");
    // Nothing exports the test handler, and nothing in the production module reads a test header.
    assert!(!text(&on, "index.ts").contains("test-handler"));
    assert!(!text(&on, "server/bookings.ts").contains("veld-test"));
    assert!(
        !text(&on, "runtime/veld.ts")
            .to_lowercase()
            .contains("veld-test")
    );
}

#[test]
fn the_test_handler_says_at_its_top_that_it_must_never_be_mounted_in_production() {
    let ir = endpoints_ir();
    let on = gen_server_with(&ir, "Bookings", &[], &testing());
    let file = text(&on, "server/bookings-test-handler.ts");
    let top: String = file.lines().take(8).collect::<Vec<_>>().join("\n");
    assert!(top.contains("NEVER MOUNT IT IN PRODUCTION"), "{top}");
    assert!(file.contains("/__veld/test"));
    assert!(
        file.contains("Never mount it in production"),
        "reported on creation"
    );
}

#[test]
fn a_contract_with_invariants_only_gets_no_endpoint_machinery_but_the_digest() {
    let ir = rules_ir();
    let server = gen_server_with(&ir, "Bookings", &[], &testing());
    assert!(
        !server.paths().any(|p| p.contains("test-handler")),
        "no endpoint rules, no test handler"
    );
    let runtime = text(&server, "runtime/veld.ts");
    for word in ["EndpointRuleSet", "EarlyResponse", "reportEndpointRules"] {
        assert!(
            !runtime.contains(word),
            "`{word}` leaked into an invariants-only runtime"
        );
    }
    // The rules digest is not an endpoint rule: a service that reaches an invariant has it (section 9.7).
    for word in ["onContractMismatch", "veld-rules", "withDigest"] {
        assert!(
            runtime.contains(word),
            "`{word}` is missing from the runtime"
        );
    }
    let client = gen_client(&ir);
    assert!(text(&client, "bookings/client.ts").contains("digest: '"));
    assert!(text(&server, "server/bookings.ts").contains("digest: '"));
}

#[test]
fn the_endpoint_rules_regions_come_with_endpoint_rules() {
    let ir = endpoints_ir();
    let client = gen_client(&ir);
    let server = gen_server(&ir, "Bookings", &[]);
    assert_eq!(
        text(&server, "runtime/veld.ts"),
        text(&client, "runtime/veld.ts"),
        "one runtime in both roles"
    );
    assert!(text(&client, "runtime/veld.ts").contains("EndpointRuleSet"));
    assert!(text(&client, "bookings/client.ts").contains("digest:"));
}

#[test]
fn the_testing_option_must_be_a_boolean() {
    let ir = endpoints_ir();
    let all = vec![service_id(&ir, "Bookings")];
    let req = veld_codegen::GenRequest {
        ir: &ir,
        role: veld_codegen::Role::Server,
        services: all,
        consumes: vec![],
        framework: None,
    };
    let bad = veld_codegen::DynGenerator::generate_dyn(
        &veld_emit_node::Node,
        &req,
        &json!({ "testing": "yes" }),
    );
    assert!(bad.is_err());
}

// ---- toolchain ------------------------------------------------------------------------------

fn toolchain_dir() -> PathBuf {
    repo_root().join("tests/toolchain/ts")
}

fn tsc_path() -> Option<PathBuf> {
    let node_ok = Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    let tsc = toolchain_dir().join("node_modules/typescript/bin/tsc");
    if node_ok && tsc.exists() {
        return Some(tsc);
    }
    assert!(
        std::env::var("VELD_REQUIRE_TOOLCHAINS").as_deref() != Ok("1"),
        "toolchain required but unavailable"
    );
    eprintln!("SKIPPED endpoint rules toolchain test");
    None
}

fn run(cmd: &mut Command, what: &str) {
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "{what} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn write_config(work: &Path, name: &str, value: &serde_json::Value) {
    std::fs::write(
        work.join(name),
        serde_json::to_string_pretty(value).unwrap(),
    )
    .unwrap();
}

/// Generates the server (with the test handler) and the client, and compiles the drivers.
fn build(work: &Path, tsc: &Path, edition: &str, target: &str) {
    let ir = endpoints_ir();
    let opts = WriteOptions::default();
    let option = json!({ "typescript": edition, "testing": true });
    write(
        &work.join("server"),
        &gen_server_with(&ir, "Bookings", &[], &option),
        &opts,
    )
    .unwrap();
    write(
        &work.join("client"),
        &gen_client_with(&ir, &typescript_option(edition)),
        &opts,
    )
    .unwrap();
    for driver in ["endpoint_rules_http.ts", "endpoint_rules_serve.ts"] {
        std::fs::copy(toolchain_dir().join(driver), work.join(driver)).unwrap();
    }
    // 1. Each generated package compiles on its own under `--strict`, with no types packages.
    write_config(
        work,
        "tsconfig.generated.json",
        &json!({
            "compilerOptions": {
                "strict": true, "target": target, "lib": [target], "types": [],
                "module": "commonjs", "moduleResolution": "node10", "noEmit": true,
                "skipLibCheck": false, "forceConsistentCasingInFileNames": true
            },
            "include": ["server/**/*.ts", "client/**/*.ts"]
        }),
    );
    run(
        Command::new("node")
            .arg(tsc)
            .args(["-p", "tsconfig.generated.json"])
            .current_dir(work),
        "tsc generated",
    );
    // 2. The drivers compile with the Node types.
    write_config(
        work,
        "tsconfig.json",
        &json!({
            "compilerOptions": {
                "strict": true, "target": target, "lib": [target, "dom"], "types": ["node"],
                "module": "commonjs", "moduleResolution": "node10", "esModuleInterop": true,
                "skipLibCheck": true, "outDir": "out", "rootDir": "."
            },
            "include": ["*.ts", "server/**/*.ts", "client/**/*.ts"]
        }),
    );
    run(
        Command::new("node")
            .arg(tsc)
            .args(["-p", "tsconfig.json"])
            .current_dir(work),
        "tsc drivers",
    );
}

#[test]
fn endpoint_rules_compile_strict_and_behave_over_http() {
    let Some(tsc) = tsc_path() else { return };
    for (edition, target) in TARGETS {
        std::fs::create_dir_all(toolchain_dir().join(".work")).unwrap();
        let guard = tempfile::Builder::new()
            .prefix("endpoint-rules-")
            .tempdir_in(toolchain_dir().join(".work"))
            .unwrap();
        let work = guard.path();
        build(work, &tsc, edition, target);
        run(
            Command::new("node")
                .arg("out/endpoint_rules_http.js")
                .current_dir(work),
            "node endpoint_rules_http.js",
        );
        run_examples(work, &endpoints_ir_all());
    }
}

// ---- the examples are the vectors -------------------------------------------------------------

/// The contracts of `endpoint_contracts`, compiled.
fn endpoints_ir_all() -> Vec<Ir> {
    endpoint_contracts()
        .into_iter()
        .map(|p| compile_text(std::fs::read_to_string(p).unwrap()))
        .collect()
}

/// A running test handler; killed when dropped.
struct Live {
    tree: veld_test_support::ProcessTree,
    url: String,
}

impl Live {
    fn start(work: &Path) -> Live {
        let mut cmd = Command::new("node");
        cmd.arg("out/endpoint_rules_serve.js")
            .current_dir(work)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut tree =
            veld_test_support::ProcessTree::spawn(&mut cmd).expect("cannot start the server");
        let mut line = String::new();
        BufReader::new(tree.child_mut().stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let port = line
            .trim()
            .strip_prefix("PORT=")
            .unwrap_or_else(|| panic!("the server did not report a port: {line:?}"));
        Live {
            url: format!("http://127.0.0.1:{port}"),
            tree,
        }
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        self.tree.kill_tree();
    }
}

/// Sends every example of every contract to the generated test handler, through the shared live
/// runner (`veld-testing`), in dry-run mode.
fn run_examples(work: &Path, contracts: &[Ir]) {
    let server = Live::start(work);
    let live = veld_testing::Live::connect(&server.url, true).unwrap();
    let (mut passed, mut failures) = (0, Vec::new());
    for ir in contracts {
        for ex in &ir.examples {
            match live.run_example(ir, ex) {
                Ok(report) if report.passed() => passed += 1,
                Ok(report) => failures.push(format!("\"{}\": {report}", ex.name)),
                Err(e) => failures.push(format!("\"{}\": {e}", ex.name)),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} examples failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    eprintln!("endpoint examples passed through the generated test handler: {passed}");
    assert_eq!(
        passed, 20,
        "every example of tests/rules/endpoints/bookings.veld"
    );
}

// ---- fuzzing the generated test handler ---------------------------------------------------------

/// Generated requests, valid and invalid, boundaries first, go to the generated test handler in
/// dry-run mode: its answer must be the one the reference evaluator works out, in status, error,
/// payload and violations. A failure here is a real disagreement between this server and the
/// reference evaluator; the message carries the seed and the request as an `example` block.
#[test]
fn fuzzing_the_generated_test_handler_agrees_with_the_reference_evaluator() {
    let Some(tsc) = tsc_path() else { return };
    let (edition, target) = TARGETS[0];
    std::fs::create_dir_all(toolchain_dir().join(".work")).unwrap();
    let guard = tempfile::Builder::new()
        .prefix("endpoint-fuzz-")
        .tempdir_in(toolchain_dir().join(".work"))
        .unwrap();
    let work = guard.path();
    build(work, &tsc, edition, target);
    let server = Live::start(work);
    let live = veld_testing::Live::connect(&server.url, true).unwrap();
    let ir = endpoints_ir();
    let opts = veld_testing::FuzzOptions {
        count: 400,
        seed: 20_260_101,
    };
    let report = veld_testing::fuzz(&ir, &live, &opts).unwrap();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for e in &report.endpoints {
        assert!(e.skipped.is_none(), "{}: {:?}", e.endpoint, e.skipped);
        if let Some(m) = &e.mismatch {
            let diffs: Vec<String> = m.differences.iter().map(ToString::to_string).collect();
            panic!(
                "seed {}: {}.{} request {} disagrees with the reference evaluator\n\
                 expected: {}\nactual:   {}\n{}\n{}",
                report.seed,
                e.service,
                e.endpoint,
                m.index,
                m.expected,
                m.actual,
                diffs.join("\n"),
                m.example.as_deref().unwrap_or("(no example)")
            );
        }
        assert_eq!(e.sent, 400, "{}", e.endpoint);
        for (kind, n) in &e.outcomes {
            *seen.entry(kind.clone()).or_default() += n;
        }
    }
    eprintln!(
        "fuzz of the generated test handler, seed {}: {seen:?}",
        report.seed
    );
    assert_eq!(report.stopped, None, "the run stopped early");
    assert!(report.one_sided().is_empty(), "{:?}", report.one_sided());
    // A green run proves nothing unless the fuzz reached the rules. `Internal` cannot be reached
    // without a handler result, and a dry run has none.
    for kind in [
        "ok",
        "ValidationError",
        "NotFound",
        "RoomFull",
        "RoomClosed",
        "AmountMismatch",
    ] {
        assert!(
            seen.get(kind).copied().unwrap_or(0) > 0,
            "no {kind} in {seen:?}"
        );
    }
}
