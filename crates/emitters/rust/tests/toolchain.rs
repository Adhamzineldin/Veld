//! Behaviour tests against the real Rust toolchain (`tests/toolchain/rust`) and, for the
//! cross-language tests, the pinned TypeScript toolchain (`tests/toolchain/ts`).
//!
//! The generated crates are built with `cargo build` and `cargo clippy -- -D warnings` inside one
//! scratch workspace whose `Cargo.lock` is pinned (`tests/toolchain/rust/Cargo.lock`), so every
//! build resolves the same versions. The workspace and its target directory live under
//! `tests/toolchain/rust/.work` (override with `VELD_RUST_WORK`; `VELD_RUST_TARGET` moves only the
//! target directory), so repeated runs only rebuild what changed.
//!
//! Without `cargo` (or `node` and `node_modules` for the cross-language tests) a test is skipped
//! with a message, unless `VELD_REQUIRE_TOOLCHAINS=1`, in which case it fails.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

use common::*;
use serde_json::{Map, Value, json};
use veld_codegen::{DynGenerator, FileSet, GenRequest, Role};
use veld_emit_node::Node;
use veld_emit_typescript::TypeScript;
use veld_ir::Ir;
use veld_test_support::ProcessTree;

/// Why a toolchain is not usable: missing (skip) or a real failure (panic).
#[derive(Clone)]
enum Unavailable {
    Missing(String),
    Failed(String),
}

fn rust_dir() -> PathBuf {
    repo_root().join("tests/toolchain/rust")
}

fn ts_dir() -> PathBuf {
    repo_root().join("tests/toolchain/ts")
}

fn work_dir() -> PathBuf {
    std::env::var("VELD_RUST_WORK").map_or_else(|_| rust_dir().join(".work"), PathBuf::from)
}

fn target_dir() -> PathBuf {
    std::env::var("VELD_RUST_TARGET").map_or_else(|_| work_dir().join("target"), PathBuf::from)
}

fn succeeds(program: &str, arg: &str) -> bool {
    Command::new(program)
        .arg(arg)
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Runs a command to completion; prints its output only when it fails.
fn run(cmd: &mut Command, what: &str) -> String {
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("{what}: cannot start: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "{what} failed ({})\nstdout:\n{stdout}\nstderr:\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    stdout
}

fn cargo(ws: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new("cargo");
    cmd.args(args)
        .current_dir(ws)
        .env("CARGO_TARGET_DIR", target_dir())
        .env("CARGO_TERM_COLOR", "never");
    cmd
}

fn sig_map(ir: &Ir) -> Value {
    let mut m = Map::new();
    for s in &ir.services {
        for ep in &s.endpoints {
            m.insert(format!("{}.{}", s.name, ep.name), json!(ep.sig));
        }
    }
    for d in &ir.decls {
        m.insert(format!("decl:{}", d.qualified), json!(d.sig));
    }
    Value::Object(m)
}

const CHANGED_ERROR: (&str, &str) = (
    "error OutOfStock 409 {\n  sku: string\n}",
    "error OutOfStock 409 {\n  sku: string\n  warehouse: string\n}",
);

fn copy_dir(from: &Path, to: &Path) {
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

/// The generated crates of one run: name to the file set that is written to `<ws>/<dir>`.
struct Crates {
    members: Vec<(&'static str, FileSet)>,
}

impl Crates {
    fn new() -> Self {
        Self {
            members: Vec::new(),
        }
    }

    fn add(&mut self, dir: &'static str, files: FileSet) {
        self.members.push((dir, files));
    }
}

fn write_sigs(dir: &Path) {
    let a = standard_ir();
    let c = compile_contract("orders", &|s| s.replace(CHANGED_ERROR.0, CHANGED_ERROR.1));
    let d = compile_contract("warehouse", &|s| s.to_string());
    std::fs::write(
        dir.join("sigs.json"),
        json!({ "a": sig_map(&a), "c": sig_map(&c), "d": sig_map(&d) }).to_string(),
    )
    .unwrap();
}

struct Workspace {
    ws: PathBuf,
}

/// Generates every crate of the tests into one workspace and builds it.
fn build_workspace() -> Result<Workspace, Unavailable> {
    if !succeeds("cargo", "--version") {
        return Err(Unavailable::Missing("`cargo` is not on PATH".into()));
    }
    let ws = work_dir().join("ws");
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    let a = standard_ir();
    let mut crates = Crates::new();
    crates.add(
        "server",
        gen_server(&a, "Orders", &["Iam"], "orders-server"),
    );
    crates.add("client", gen_client(&a, "orders-client", None));
    crates.add(
        "client_ureq",
        gen_client(&a, "orders-client-ureq", Some("ureq")),
    );
    crates.add("client_b", gen_client(&a, "orders-client-b", None));
    let changed = compile_contract("orders", &|s| s.replace(CHANGED_ERROR.0, CHANGED_ERROR.1));
    crates.add("client_c", gen_client(&changed, "orders-client-c", None));
    let warehouse = compile_contract("warehouse", &|s| s.to_string());
    crates.add("client_d", gen_client(&warehouse, "orders-client-d", None));
    let shop = equality_ir();
    crates.add("shop_a", gen_client(&shop, "shop-a", None));
    crates.add("shop_b", gen_client(&shop, "shop-b", None));
    let greeter = greeter_ir();
    let unvalidated = |role, name: &str| {
        let all = greeter.services.iter().map(|s| s.id).collect();
        common::run(
            &greeter,
            role,
            all,
            vec![],
            None,
            json!({ "crate-name": name, "validate": false }),
        )
    };
    crates.add(
        "greeter_server",
        unvalidated(Role::Server, "greeter-server"),
    );
    crates.add(
        "greeter_client",
        unvalidated(Role::Client, "greeter-client"),
    );
    let live_sse = streaming_ir();
    crates.add(
        "live_server",
        gen_server_services(&live_sse, &["Live", "Vault"], "live-server"),
    );
    crates.add("live_client", gen_client(&live_sse, "live-client", None));
    // Every identifier that is dangerous in some language, as a server and a client.
    let hazards = hazards_ir();
    crates.add(
        "weird_server",
        gen_server_services(&hazards, &["Weird"], "weird-server"),
    );
    crates.add("weird_client", gen_client(&hazards, "weird-client", None));
    let mut members: Vec<String> = Vec::new();
    for (dir, files) in &crates.members {
        write_files(&ws.join(dir), files);
        members.push(format!("\"{dir}\""));
    }
    copy_dir(&rust_dir().join("harness"), &ws.join("harness"));
    members.push("\"harness\"".into());
    std::fs::write(
        ws.join("Cargo.toml"),
        format!(
            "[workspace]\nresolver = \"2\"\nmembers = [{}]\n",
            members.join(", ")
        ),
    )
    .unwrap();
    let lock = rust_dir().join("Cargo.lock");
    if lock.exists() {
        std::fs::copy(&lock, ws.join("Cargo.lock")).unwrap();
    }
    write_sigs(&ws);
    run(
        &mut cargo(&ws, &["build", "--workspace", "--bins"]),
        "cargo build",
    );
    // Zero warnings, clippy clean, in every generated crate.
    let generated: Vec<&str> = vec![
        "orders-server",
        "orders-client",
        "orders-client-ureq",
        "orders-client-b",
        "orders-client-c",
        "orders-client-d",
        "shop-a",
        "shop-b",
        "greeter-server",
        "greeter-client",
        "live-server",
        "live-client",
        "weird-server",
        "weird-client",
    ];
    let mut args = vec!["clippy", "--all-targets"];
    for g in &generated {
        args.extend(["-p", g]);
    }
    args.extend(["--", "-D", "warnings"]);
    run(&mut cargo(&ws, &args), "cargo clippy");
    Ok(Workspace { ws })
}

fn workspace() -> Result<&'static Workspace, Unavailable> {
    static WS: OnceLock<Result<Workspace, Unavailable>> = OnceLock::new();
    static LOCK: Mutex<()> = Mutex::new(());
    let _guard = LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match WS.get_or_init(|| {
        std::panic::catch_unwind(build_workspace).unwrap_or_else(|e| {
            Err(Unavailable::Failed(
                e.downcast_ref::<String>()
                    .cloned()
                    .unwrap_or_else(|| "build panicked".into()),
            ))
        })
    }) {
        Ok(w) => Ok(w),
        Err(e) => Err(e.clone()),
    }
}

fn skip<T>(test: &str, why: &Unavailable) -> Option<T> {
    match why {
        Unavailable::Failed(msg) => panic!("{msg}"),
        Unavailable::Missing(msg) => {
            assert!(
                std::env::var("VELD_REQUIRE_TOOLCHAINS").as_deref() != Ok("1"),
                "toolchain required but unavailable: {msg}"
            );
            eprintln!("SKIPPED {test}: {msg}");
            None
        }
    }
}

fn need_ws(test: &str) -> Option<&'static Workspace> {
    match workspace() {
        Ok(w) => Some(w),
        Err(why) => skip(test, &why),
    }
}

fn bin(w: &Workspace, name: &str) -> PathBuf {
    let exe = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    target_dir()
        .join("debug")
        .join(exe)
        .canonicalize()
        .unwrap_or_else(|_| panic!("binary {name} was not built in {}", w.ws.display()))
}

#[test]
fn generated_crates_build_and_pass_clippy() {
    // Building the workspace is the test: `build_workspace` runs cargo build and clippy -D warnings.
    let _ = need_ws("generated_crates_build_and_pass_clippy");
}

#[test]
fn rust_round_trip() {
    let Some(w) = need_ws("rust_round_trip") else {
        return;
    };
    let out = run(
        Command::new(bin(w, "roundtrip")).env("VELD_SIGS", w.ws.join("sigs.json")),
        "rust round trip",
    );
    assert!(out.contains("roundtrip ok"), "{out}");
}

// ---------------------------------------------------------------------------
// Cross-language
// ---------------------------------------------------------------------------

struct Ts {
    work: PathBuf,
}

fn gen_ts(
    generator: &dyn DynGenerator,
    ir: &Ir,
    role: Role,
    services: &[&str],
    consumes: &[&str],
) -> FileSet {
    let ids = |names: &[&str]| names.iter().map(|n| service_id(ir, n)).collect::<Vec<_>>();
    let req = GenRequest {
        ir,
        role,
        services: ids(services),
        consumes: ids(consumes),
        framework: None,
    };
    generator
        .generate_dyn(&req, &Value::Null)
        .unwrap_or_else(|d| panic!("generation failed: {d:?}"))
}

/// Compiles `scripts` and the generated trees `dirs` with the pinned `tsc`.
fn tsc(work: &Path, scripts: &[&str], dirs: &[&str]) {
    let tsc = ts_dir().join("node_modules/typescript/bin/tsc");
    for script in scripts {
        std::fs::copy(ts_dir().join(script), work.join(script)).unwrap();
    }
    let mut include: Vec<String> = vec!["*.ts".into()];
    include.extend(dirs.iter().map(|d| format!("{d}/**/*.ts")));
    let config = json!({
        "compilerOptions": {
            "strict": true, "target": "ES2022", "lib": ["ES2022"], "types": ["node"],
            "module": "commonjs", "moduleResolution": "node10", "esModuleInterop": true,
            "skipLibCheck": true, "outDir": "out", "rootDir": "."
        },
        "include": include
    });
    std::fs::write(
        work.join("tsconfig.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    run(
        Command::new("node")
            .arg(&tsc)
            .args(["-p", "tsconfig.json"])
            .current_dir(work),
        "tsc",
    );
}

fn require_node() -> Result<(), Unavailable> {
    if !succeeds("node", "--version") {
        return Err(Unavailable::Missing("`node` is not on PATH".into()));
    }
    if !ts_dir().join("node_modules/typescript/bin/tsc").exists() {
        return Err(Unavailable::Missing(format!(
            "{} is missing; run `npm ci` in tests/toolchain/ts",
            ts_dir().join("node_modules/typescript").display()
        )));
    }
    Ok(())
}

fn build_ts() -> Result<Ts, Unavailable> {
    require_node()?;
    let work = ts_dir().join(".work/rust-interop");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let a = standard_ir();
    write_files(
        &work.join("server"),
        &gen_ts(&Node, &a, Role::Server, &["Orders"], &["Iam"]),
    );
    write_files(
        &work.join("copy_a"),
        &gen_ts(&TypeScript, &a, Role::Client, &["Iam", "Orders"], &[]),
    );
    write_sigs(&work);
    tsc(
        &work,
        &["serve.ts", "crossclient.ts"],
        &["server", "copy_a"],
    );
    Ok(Ts { work })
}

fn ts() -> Result<&'static Ts, Unavailable> {
    static TS: OnceLock<Result<Ts, Unavailable>> = OnceLock::new();
    match TS.get_or_init(|| {
        std::panic::catch_unwind(build_ts).unwrap_or_else(|e| {
            Err(Unavailable::Failed(
                e.downcast_ref::<String>()
                    .cloned()
                    .unwrap_or_else(|| "build panicked".into()),
            ))
        })
    }) {
        Ok(t) => Ok(t),
        Err(e) => Err(e.clone()),
    }
}

fn both(test: &str) -> Option<(&'static Workspace, &'static Ts)> {
    let w = need_ws(test)?;
    match ts() {
        Ok(t) => Some((w, t)),
        Err(why) => skip(test, &why),
    }
}

/// Starts `cmd`, waits for its `PORT=<n>` line and keeps stdin open (closing it stops the server).
fn start(mut cmd: Command, log: &Path) -> (ProcessTree, String) {
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(std::fs::File::create(log).unwrap()));
    let mut tree =
        ProcessTree::spawn(&mut cmd).unwrap_or_else(|e| panic!("cannot start server: {e}"));
    let mut line = String::new();
    BufReader::new(tree.child_mut().stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let port = line
        .trim()
        .strip_prefix("PORT=")
        .unwrap_or_else(|| {
            panic!(
                "server did not report a port: {line:?}; log: {}",
                log.display()
            )
        })
        .to_string();
    (tree, format!("http://127.0.0.1:{port}"))
}

#[test]
fn rust_client_calls_the_node_server() {
    let Some((w, t)) = both("rust_client_calls_the_node_server") else {
        return;
    };
    let mut node = Command::new("node");
    node.arg("out/serve.js").current_dir(&t.work);
    let (_server, url) = start(node, &t.work.join("serve.log"));
    let out = run(
        Command::new(bin(w, "scenario"))
            .env("BASE", &url)
            .env("VELD_SIGS", t.work.join("sigs.json")),
        "rust client against node server",
    );
    assert!(out.contains("scenario ok"), "{out}");
}

#[test]
fn typescript_client_calls_the_rust_server() {
    let Some((w, t)) = both("typescript_client_calls_the_rust_server") else {
        return;
    };
    let (_server, url) = start(Command::new(bin(w, "serve")), &w.ws.join("serve.log"));
    let out = run(
        Command::new("node")
            .args(["out/crossclient.js", &url])
            .current_dir(&t.work),
        "crossclient.js",
    );
    assert!(out.contains("ts client against java server ok"), "{out}");
}

// ---------------------------------------------------------------------------
// Identity, wire goldens, validate = false, the ureq client
// ---------------------------------------------------------------------------

fn run_bin(w: &Workspace, name: &str, envs: &[(&str, PathBuf)]) -> String {
    let mut cmd = Command::new(bin(w, name));
    cmd.current_dir(&w.ws);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    run(&mut cmd, name)
}

#[test]
fn cross_package_equality() {
    let Some(w) = need_ws("cross_package_equality") else {
        return;
    };
    let out = run_bin(w, "equality", &[("VELD_SIGS", w.ws.join("sigs.json"))]);
    assert!(out.contains("equality ok"), "{out}");
}

#[test]
fn wire_goldens() {
    let Some(w) = need_ws("wire_goldens") else {
        return;
    };
    let out = run_bin(w, "wire", &[("WIRE_DIR", repo_root().join("tests/wire"))]);
    assert!(out.contains("wire ok"), "{out}");
}

#[test]
fn validate_false_skips_runtime_validation() {
    let Some(w) = need_ws("validate_false_skips_runtime_validation") else {
        return;
    };
    assert!(run_bin(w, "novalidate", &[]).contains("no-validate ok"));
}

#[test]
fn ureq_client_calls_the_rust_server() {
    let Some(w) = need_ws("ureq_client_calls_the_rust_server") else {
        return;
    };
    let (_server, url) = start(Command::new(bin(w, "serve")), &w.ws.join("serve-ureq.log"));
    let out = run(
        Command::new(bin(w, "ureq_client")).env("BASE", &url),
        "ureq client",
    );
    assert!(out.contains("ureq ok"), "{out}");
}

// ---------------------------------------------------------------------------
// Streaming
// ---------------------------------------------------------------------------

/// The generated Node server and TypeScript client of the streaming contract, compiled.
struct TsStream {
    work: PathBuf,
}

fn build_ts_stream() -> Result<TsStream, Unavailable> {
    require_node()?;
    if !ts_dir().join("node_modules/ws/package.json").exists() {
        return Err(Unavailable::Missing(
            "run `npm ci` in tests/toolchain/ts (needs typescript and ws)".into(),
        ));
    }
    let work = ts_dir().join(".work/rust-stream");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let ir = streaming_ir();
    write_files(
        &work.join("server"),
        &gen_ts(&Node, &ir, Role::Server, &["Live", "Vault"], &[]),
    );
    write_files(
        &work.join("client"),
        &gen_ts(&TypeScript, &ir, Role::Client, &["Live", "Vault"], &[]),
    );
    tsc(
        &work,
        &[
            "stream_server.ts",
            "stream_cross.ts",
            "flaky.ts",
            "flaky_serve.ts",
        ],
        &["server", "client"],
    );
    Ok(TsStream { work })
}

fn ts_stream() -> Result<&'static TsStream, Unavailable> {
    static TS: OnceLock<Result<TsStream, Unavailable>> = OnceLock::new();
    match TS.get_or_init(|| {
        std::panic::catch_unwind(build_ts_stream).unwrap_or_else(|e| {
            Err(Unavailable::Failed(
                e.downcast_ref::<String>()
                    .cloned()
                    .unwrap_or_else(|| "build panicked".into()),
            ))
        })
    }) {
        Ok(t) => Ok(t),
        Err(e) => Err(e.clone()),
    }
}

fn both_stream(test: &str) -> Option<(&'static Workspace, &'static TsStream)> {
    let w = need_ws(test)?;
    match ts_stream() {
        Ok(t) => Some((w, t)),
        Err(why) => skip(test, &why),
    }
}

#[test]
fn streaming_round_trip() {
    let Some(w) = need_ws("streaming_round_trip") else {
        return;
    };
    let out = run_bin(w, "stream_roundtrip", &[]);
    assert!(out.contains("stream roundtrip ok"), "{out}");
}

/// `tests/wire/streaming/sse.json` through the Rust runtime's frame writer and `SseParser`.
#[test]
fn sse_wire_golden() {
    let Some(w) = need_ws("sse_wire_golden") else {
        return;
    };
    let out = run_bin(
        w,
        "sse_wire",
        &[("WIRE_DIR", repo_root().join("tests/wire"))],
    );
    assert!(out.contains("sse wire ok"), "{out}");
}

/// The generated Rust client (SSE) against the generated Node server.
#[test]
fn rust_client_streams_from_the_node_server() {
    let Some((w, t)) = both_stream("rust_client_streams_from_the_node_server") else {
        return;
    };
    let mut node = Command::new("node");
    node.arg("out/stream_server.js").current_dir(&t.work);
    let (_server, url) = start(node, &t.work.join("stream-server.log"));
    let out = run(
        Command::new(bin(w, "stream_client")).env("BASE", &url),
        "rust stream client against node server",
    );
    assert!(out.contains("stream client ok"), "{out}");
}

/// The generated TypeScript client against the generated Rust server (SSE).
#[test]
fn typescript_client_streams_from_the_rust_server() {
    let Some((w, t)) = both_stream("typescript_client_streams_from_the_rust_server") else {
        return;
    };
    let (_server, url) = start(
        Command::new(bin(w, "stream_serve")),
        &w.ws.join("stream-serve.log"),
    );
    let out = run(
        Command::new("node")
            .args(["out/stream_cross.js", &url, "full", "spring"])
            .current_dir(&t.work),
        "stream_cross.js",
    );
    assert!(
        out.contains("ts stream client against java server ok"),
        "{out}"
    );
}

/// The Rust client against the flaky Node server, which drops connections on purpose.
#[test]
fn rust_client_reconnects_after_dropped_connections() {
    let Some((w, t)) = both_stream("rust_client_reconnects_after_dropped_connections") else {
        return;
    };
    let mut node = Command::new("node");
    node.arg("out/flaky_serve.js").current_dir(&t.work);
    let (_server, url) = start(node, &t.work.join("flaky-server.log"));
    let out = run(
        Command::new(bin(w, "stream_reconnect")).env("BASE", &url),
        "rust stream reconnect",
    );
    assert!(out.contains("stream reconnect ok"), "{out}");
}
