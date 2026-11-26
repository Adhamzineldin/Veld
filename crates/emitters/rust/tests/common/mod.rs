//! Shared helpers: compile the realistic contracts and run the Rust generator.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use veld_codegen::{DynGenerator, FileSet, GenRequest, Role, check_capabilities};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_emit_rust::Rust;
use veld_ir::{Ir, PackageKind, ServiceId};

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn read_dir(dir: &str, files: &[&str], patch: &dyn Fn(&str) -> String) -> Vec<SourceFile> {
    files
        .iter()
        .map(|f| {
            let path = repo_root()
                .join("tests/toolchain/contract")
                .join(dir)
                .join(f);
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            SourceFile {
                path: (*f).to_string(),
                text: patch(&text),
            }
        })
        .collect()
}

fn compiled(input: &CompileInput, what: &str) -> Ir {
    let out = compile(input);
    assert!(
        out.ir.is_some(),
        "{what} failed to compile:\n{}",
        out.render()
    );
    out.ir.unwrap_or_else(|| unreachable!())
}

/// Compiles `text` as the one file of the package `package`.
pub fn compile_text(text: &str, package: &str) -> Ir {
    compiled(
        &CompileInput {
            packages: vec![PackageInput {
                name: package.into(),
                kind: PackageKind::Service,
                files: vec![SourceFile {
                    path: format!("{package}.veld"),
                    text: text.to_string(),
                }],
                base_url: None,
            }],
            consumes: BTreeMap::new(),
        },
        package,
    )
}

/// The orders contract: `orders` consumes `iam`. `orders_pkg` renames the orders package; `patch`
/// edits its sources.
pub fn compile_contract(orders_pkg: &str, patch: &dyn Fn(&str) -> String) -> Ir {
    let identity = |s: &str| s.to_string();
    compiled(
        &CompileInput {
            packages: vec![
                PackageInput {
                    name: "iam".into(),
                    kind: PackageKind::Service,
                    files: read_dir("iam", &["users.veld"], &identity),
                    base_url: Some("http://iam:3001".into()),
                },
                PackageInput {
                    name: orders_pkg.into(),
                    kind: PackageKind::Service,
                    files: read_dir("orders", &["catalog.veld", "orders.veld"], patch),
                    base_url: None,
                },
            ],
            consumes: BTreeMap::from([(orders_pkg.to_string(), vec!["iam".to_string()])]),
        },
        "contract",
    )
}

pub fn standard_ir() -> Ir {
    compile_contract("orders", &|s| s.to_string())
}

fn single(name: &str, dir: &str, file: &str) -> Ir {
    let path = repo_root()
        .join("tests/toolchain/contract")
        .join(dir)
        .join(file);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    compiled(
        &CompileInput {
            packages: vec![PackageInput {
                name: name.into(),
                kind: PackageKind::Service,
                files: vec![SourceFile {
                    path: file.into(),
                    text,
                }],
                base_url: None,
            }],
            consumes: BTreeMap::new(),
        },
        dir,
    )
}

/// The hazard fixture: identifiers that are dangerous in several languages.
pub fn hazards_ir() -> Ir {
    single("weird", "hazards", "hazards.veld")
}

/// Two models with identical fields in one contract (`tests/toolchain/contract/equality`).
pub fn equality_ir() -> Ir {
    single("shop", "equality", "shop.veld")
}

/// The streaming fixture: SSE and WebSocket endpoints.
pub fn streaming_ir() -> Ir {
    single("live", "streaming", "live.veld")
}

/// The streaming fixture with the WebSocket endpoints cut out (what a generator without
/// WebSocket support can serve).
pub fn streaming_sse_ir() -> Ir {
    let path = repo_root().join("tests/toolchain/contract/streaming/live.veld");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut out: Vec<&str> = Vec::new();
    let mut docs: Vec<&str> = Vec::new();
    for line in text.lines() {
        let t = line.trim_start();
        let indented = line.len() != t.len();
        if indented && t.starts_with("///") {
            docs.push(line);
        } else if indented && t.starts_with("ws ") {
            docs.clear();
        } else {
            out.append(&mut docs);
            out.push(line);
        }
    }
    compiled(
        &CompileInput {
            packages: vec![PackageInput {
                name: "live".into(),
                kind: PackageKind::Service,
                files: vec![SourceFile {
                    path: "live.veld".into(),
                    text: out.join("\n"),
                }],
                base_url: None,
            }],
            consumes: BTreeMap::new(),
        },
        "streaming (sse only)",
    )
}

/// The shared bookings contract of the rules tests (`tests/toolchain/contract/rules`).
pub fn rules_ir() -> Ir {
    single("bookings", "rules", "bookings.veld")
}

/// Streams and errors with invariants (`tests/toolchain/contract/rules_live`).
pub fn rules_live_ir() -> Ir {
    single("live", "rules_live", "live.veld")
}

pub fn greeter_ir() -> Ir {
    single("greeter", "validate", "greeter.veld")
}

pub fn service_id(ir: &Ir, name: &str) -> ServiceId {
    ir.services
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("no service {name}"))
        .id
}

pub fn run(
    ir: &Ir,
    role: Role,
    services: Vec<ServiceId>,
    consumes: Vec<ServiceId>,
    framework: Option<&str>,
    options: serde_json::Value,
) -> FileSet {
    let req = GenRequest {
        ir,
        role,
        services,
        consumes,
        framework: framework.map(str::to_string),
    };
    let caps = Rust.capabilities(role);
    let diags = check_capabilities(ir, &req, &caps);
    assert!(diags.is_empty(), "capability errors: {diags:?}");
    // The target is named after its crate, so `orders-server` has the runtime config
    // `OrdersServerRuntime` (`runtime_config::OrdersServerRuntime`).
    let target = options["crate-name"]
        .as_str()
        .unwrap_or(role.name())
        .to_string();
    let options = veld_codegen::with_target_name(&options, &target);
    Rust.generate_dyn(&req, &options)
        .unwrap_or_else(|d| panic!("generation failed: {d:?}"))
}

fn named(name: &str) -> serde_json::Value {
    serde_json::json!({ "crate-name": name })
}

/// A client crate for every service of the IR.
pub fn gen_client(ir: &Ir, name: &str, framework: Option<&str>) -> FileSet {
    let all = ir.services.iter().map(|s| s.id).collect();
    run(ir, Role::Client, all, vec![], framework, named(name))
}

/// A server crate for `service`, with clients for `consumes`.
pub fn gen_server(ir: &Ir, service: &str, consumes: &[&str], name: &str) -> FileSet {
    let consumed = consumes.iter().map(|n| service_id(ir, n)).collect();
    run(
        ir,
        Role::Server,
        vec![service_id(ir, service)],
        consumed,
        None,
        named(name),
    )
}

/// A server crate for several services of one package.
pub fn gen_server_services(ir: &Ir, services: &[&str], name: &str) -> FileSet {
    run(
        ir,
        Role::Server,
        services.iter().map(|n| service_id(ir, n)).collect(),
        vec![],
        None,
        named(name),
    )
}

pub fn text<'a>(files: &'a FileSet, path: &str) -> &'a str {
    std::str::from_utf8(
        files
            .get(path)
            .unwrap_or_else(|| panic!("missing generated file {path}")),
    )
    .unwrap_or_default()
}

/// Writes a file set below `dir` (replacing what was there).
pub fn write_files(dir: &Path, files: &FileSet) {
    let _ = std::fs::remove_dir_all(dir);
    for (path, bytes) in files.iter() {
        let target = dir.join(path);
        std::fs::create_dir_all(target.parent().unwrap_or(dir))
            .unwrap_or_else(|e| panic!("{}: {e}", target.display()));
        std::fs::write(&target, bytes).unwrap_or_else(|e| panic!("{}: {e}", target.display()));
    }
}

// ---------------------------------------------------------------------------
// Scratch workspaces of generated crates (the rules tests)
// ---------------------------------------------------------------------------

pub fn work_dir() -> PathBuf {
    std::env::var("VELD_RUST_WORK").map_or_else(
        |_| repo_root().join("tests/toolchain/rust/.work"),
        PathBuf::from,
    )
}

pub fn target_dir() -> PathBuf {
    std::env::var("VELD_RUST_TARGET").map_or_else(|_| work_dir().join("target"), PathBuf::from)
}

pub fn have_cargo() -> bool {
    std::process::Command::new("cargo")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Without `cargo` a toolchain test is skipped, unless `VELD_REQUIRE_TOOLCHAINS=1`.
pub fn cargo_or_skip() -> bool {
    if have_cargo() {
        return true;
    }
    assert!(
        std::env::var("VELD_REQUIRE_TOOLCHAINS").as_deref() != Ok("1"),
        "toolchain required but `cargo` is unavailable"
    );
    eprintln!("SKIPPED: `cargo` is not on PATH");
    false
}

pub fn cargo(ws: &Path, args: &[&str]) -> std::process::Command {
    let mut cmd = std::process::Command::new("cargo");
    cmd.args(args)
        .current_dir(ws)
        .env("CARGO_TARGET_DIR", target_dir())
        .env("CARGO_TERM_COLOR", "never");
    cmd
}

pub fn run_ok(cmd: &mut std::process::Command, what: &str) -> String {
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "{what} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
            .lines()
            .take(150)
            .collect::<Vec<_>>()
            .join("\n")
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

pub fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if src.is_dir() {
            copy_dir(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
}

/// A workspace of the generated `server` and `client` crates and a harness crate (a directory of
/// `tests/toolchain/rust`) at `name` below the scratch directory, with the pinned lock file.
pub fn harness_workspace(name: &str, server: &FileSet, client: &FileSet, harness: &str) -> PathBuf {
    let ws = work_dir().join(name);
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    write_files(&ws.join("server"), server);
    write_files(&ws.join("client"), client);
    copy_dir(
        &repo_root().join("tests/toolchain/rust").join(harness),
        &ws.join(harness),
    );
    std::fs::write(
        ws.join("Cargo.toml"),
        format!(
            "[workspace]\nresolver = \"2\"\nmembers = [\"server\", \"client\", \"{harness}\"]\n"
        ),
    )
    .unwrap();
    let lock = repo_root().join("tests/toolchain/rust/Cargo.lock");
    if lock.exists() {
        std::fs::copy(&lock, ws.join("Cargo.lock")).unwrap();
    }
    ws
}

/// Builds the workspace, runs clippy on the generated crates and runs the harness, which must
/// print `ok_line`. `args` are passed to it.
pub fn build_lint_and_run(
    ws: &Path,
    (server, client, harness): (&str, &str, &str),
    args: &[&str],
    ok_line: &str,
) {
    run_ok(
        &mut cargo(ws, &["build", "--workspace", "--bins"]),
        "cargo build",
    );
    let clippy = [
        "clippy",
        "--all-targets",
        "-p",
        server,
        "-p",
        client,
        "--",
        "-D",
        "warnings",
    ];
    run_ok(&mut cargo(ws, &clippy), "cargo clippy");
    let mut run = vec!["run", "-q", "-p", harness, "--"];
    run.extend_from_slice(args);
    let out = run_ok(&mut cargo(ws, &run), "the harness");
    assert!(out.contains(ok_line), "{out}");
}

// ---------------------------------------------------------------------------
// Contracts by path, and the examples through a generated test handler
// ---------------------------------------------------------------------------

/// Compiles the one file at `path` (below the repository root) as the package `package`.
pub fn compile_path(path: &str, package: &str) -> Ir {
    let text =
        std::fs::read_to_string(repo_root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
    compiled(
        &CompileInput {
            packages: vec![PackageInput {
                name: package.into(),
                kind: PackageKind::Service,
                files: vec![SourceFile {
                    path: format!("{package}.veld"),
                    text,
                }],
                base_url: None,
            }],
            consumes: BTreeMap::new(),
        },
        path,
    )
}

/// A running test handler (a harness binary that prints `PORT=<n>`); killed when dropped.
pub struct Running {
    child: std::process::Child,
    pub url: String,
}

impl Running {
    pub fn start(binary: &str) -> Running {
        use std::io::{BufRead, BufReader};
        use std::process::{Command, Stdio};
        let exe = if cfg!(windows) {
            format!("{binary}.exe")
        } else {
            binary.to_string()
        };
        let mut child = Command::new(target_dir().join("debug").join(exe))
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

/// Sends every example of the contract to the test handler of the binary `binary`, in dry-run
/// mode, through the shared live runner. Every example must run and pass: an example the handler
/// skips (because its probe does not name the service) is a failure.
pub fn run_every_example(ir: &Ir, binary: &str, expected: usize) {
    let server = Running::start(binary);
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
        "{} examples failed or were skipped:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert_eq!(passed, expected, "every example of the contract");
    eprintln!("examples passed through the generated test handler: {passed}");
}

/// Requests per endpoint and the seed of the fuzz runs of the three contracts of
/// `tests/rules/endpoints`. Fixed, so a failure repeats.
pub const FUZZ_COUNT: usize = 300;
pub const FUZZ_SEED: u64 = 20_260_201;

/// The tally of one fuzz run: the outcomes of every endpoint, by `Service.endpoint`.
pub type Tally = std::collections::BTreeMap<String, std::collections::BTreeMap<String, usize>>;

/// Fuzzes the test handler at `url` (`veld test --live <url> --fuzz <count> --seed <seed>`) and
/// fails on the first thing that makes a green run mean nothing: a run that did not finish, an
/// endpoint that was skipped, a disagreement with the reference evaluator (printed with its
/// seed, both answers and the request as an `example` block) and an endpoint whose every request
/// ended the same way.
pub fn fuzz_agrees(url: &str, ir: &Ir, count: usize, seed: u64) -> Tally {
    let live = veld_testing::Live::connect(url, true).unwrap();
    let report = veld_testing::fuzz(ir, &live, &veld_testing::FuzzOptions { count, seed }).unwrap();
    let mut tally = Tally::new();
    for e in &report.endpoints {
        if let Some(m) = &e.mismatch {
            let diffs: Vec<String> = m.differences.iter().map(ToString::to_string).collect();
            panic!(
                "seed {seed}: {}.{} request {} disagrees with the reference evaluator\n\
                 expected: {}\nactual:   {}\n{}\n{}",
                e.service,
                e.endpoint,
                m.index,
                m.expected,
                m.actual,
                diffs.join("\n"),
                m.example.as_deref().unwrap_or("(no example)")
            );
        }
        assert_eq!(e.sent, count, "{}.{}", e.service, e.endpoint);
        eprintln!(
            "fuzz seed {seed} {}.{}: {} sent ({} boundary): {:?}",
            e.service, e.endpoint, e.sent, e.boundaries, e.outcomes
        );
        tally.insert(format!("{}.{}", e.service, e.endpoint), e.outcomes.clone());
    }
    assert_eq!(report.stopped, None, "the run stopped early");
    assert!(report.skipped().is_empty(), "{:?}", report.skipped());
    assert!(report.one_sided().is_empty(), "{:?}", report.one_sided());
    tally
}

/// Starts the test handler `binary` and fuzzes it (see [`fuzz_agrees`]).
pub fn fuzz_every_endpoint(ir: &Ir, binary: &str) -> Tally {
    let server = Running::start(binary);
    fuzz_agrees(&server.url, ir, FUZZ_COUNT, FUZZ_SEED)
}

/// The `message` of every `Forbidden` the reference evaluator gives for the requests that
/// [`fuzz_agrees`] sends to `endpoint` (the same seed gives the same requests). The fuzz compared
/// each of these with the server's answer, so a message in this tally was compared.
pub fn denial_messages(
    ir: &Ir,
    endpoint: &str,
    count: usize,
    seed: u64,
) -> std::collections::BTreeMap<String, usize> {
    let service = ir
        .services
        .iter()
        .find(|s| s.endpoints.iter().any(|e| e.name == endpoint))
        .unwrap();
    let ep = service
        .endpoints
        .iter()
        .find(|e| e.name == endpoint)
        .unwrap();
    let pipeline = veld_rules::Pipeline::new(ir, service, endpoint).unwrap();
    let mut seen = std::collections::BTreeMap::new();
    for req in veld_testing::generate(ir, service, ep, seed, count)
        .unwrap()
        .requests
    {
        if let veld_rules::Outcome::Error(a) = pipeline.run(&req).unwrap()
            && a.name == "Forbidden"
        {
            let message = a.data["message"].as_str().unwrap().to_owned();
            *seen.entry(message).or_default() += 1;
        }
    }
    seen
}

/// How many requests of `endpoint` ended in `outcome`.
pub fn outcome_count(tally: &Tally, endpoint: &str, outcome: &str) -> usize {
    tally
        .get(endpoint)
        .and_then(|t| t.get(outcome))
        .copied()
        .unwrap_or(0)
}
