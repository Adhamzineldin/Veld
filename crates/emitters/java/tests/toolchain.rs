//! Behaviour tests against a real JDK (`tests/toolchain/java`) and, for the cross-language tests,
//! the pinned TypeScript toolchain (`tests/toolchain/ts`).
//!
//! The generated Java is compiled with `javac --release 17 -Xlint:all -Werror`, then:
//! the Java round trip (generated server on the JDK adapter, every endpoint through the generated
//! client), cross-package equality, the wire goldens, and both directions of the cross-language
//! check (generated TypeScript client against the Java server, generated Java client against the
//! Node server).
//!
//! Without `javac` (or `node` and `node_modules` for the cross-language tests) a test is skipped
//! with a message, unless `VELD_REQUIRE_TOOLCHAINS=1`, in which case it fails.
//! The Spring adapter is compiled only when `VELD_SPRING_CLASSPATH` is set.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;

use common::*;
use serde_json::{Map, Value, json};
use veld_codegen::{DynGenerator, FileSet, GenRequest, Role};
use veld_emit_node::Node;
use veld_emit_typescript::TypeScript;
use veld_ir::Ir;
use veld_project::output::{WriteOptions, write};

const CHANGED_ERROR: (&str, &str) =
    ("error OutOfStock 409 {\n  sku: string\n}", "error OutOfStock 409 {\n  sku: string\n  warehouse: string\n}");

/// Why a toolchain is not usable: missing (skip) or a real failure (panic).
#[derive(Clone)]
enum Unavailable {
    Missing(String),
    Failed(String),
}

fn java_dir() -> PathBuf {
    repo_root().join("tests/toolchain/java")
}

fn ts_dir() -> PathBuf {
    repo_root().join("tests/toolchain/ts")
}

fn succeeds(program: &str, arg: &str) -> bool {
    Command::new(program).arg(arg).output().is_ok_and(|o| o.status.success())
}

fn run(cmd: &mut Command, what: &str) -> String {
    let out = cmd.output().unwrap_or_else(|e| panic!("{what}: cannot start: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    print!("{stdout}");
    assert!(
        out.status.success(),
        "{what} failed ({})\nstdout:\n{stdout}\nstderr:\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    stdout
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

fn write_sigs(dir: &Path) {
    let a = standard_ir();
    let c = compile_contract("orders", &|s| s.replace(CHANGED_ERROR.0, CHANGED_ERROR.1));
    let d = compile_contract("warehouse", &|s| s.to_string());
    std::fs::write(dir.join("sigs.json"), json!({ "a": sig_map(&a), "c": sig_map(&c), "d": sig_map(&d) }).to_string()).unwrap();
}

fn java_files(dir: &Path, skip: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "java")
                && !skip.iter().any(|s| path.file_name().is_some_and(|n| n == *s))
            {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// `javac --release 17 -Xlint:all -Werror` over `sources`; any warning fails the build.
fn javac(work: &Path, out: &str, classpath: Option<&str>, extra: &[&str], sources: &[PathBuf]) {
    let list: Vec<String> = sources.iter().map(|p| p.display().to_string().replace('\\', "/")).collect();
    let argfile = work.join(format!("{out}.args"));
    std::fs::write(&argfile, list.join("\n")).unwrap();
    let mut cmd = Command::new("javac");
    cmd.args(["--release", "17", "-Xlint:all", "-Werror", "-encoding", "UTF-8", "-d", out]);
    if let Some(cp) = classpath {
        cmd.args(["-cp", cp]);
    }
    cmd.args(extra).arg(format!("@{}.args", out)).current_dir(work);
    run(&mut cmd, &format!("javac ({out})"));
}

struct Java {
    work: PathBuf,
}

/// Generates every Java target and compiles them together with the test programs.
fn build_java() -> Result<Java, Unavailable> {
    if !succeeds("javac", "--version") || !succeeds("java", "--version") {
        return Err(Unavailable::Missing("`javac`/`java` are not on PATH".into()));
    }
    let work = java_dir().join(".work/run");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let a = standard_ir();
    let c = compile_contract("orders", &|s| s.replace(CHANGED_ERROR.0, CHANGED_ERROR.1));
    let d = compile_contract("warehouse", &|s| s.to_string());
    let opts = WriteOptions::default();
    let put = |dir: &str, files: &FileSet| write(&work.join(dir), files, &opts).unwrap();
    put("gen_a", &gen_server(&a, "Orders", &["Iam"], "gen.a", Some("spring")));
    put("gen_i", &gen_server(&a, "Iam", &[], "gen.i", None));
    put("gen_ca", &gen_client(&a, "gen.ca"));
    put("gen_cb", &gen_client(&a, "gen.cb"));
    put("gen_cc", &gen_client(&c, "gen.cc"));
    put("gen_cd", &gen_client(&d, "gen.cd"));
    write_sigs(&work);

    // 1. Generated code alone: zero warnings (the Spring adapter needs Spring on the classpath).
    let generated: Vec<PathBuf> = ["gen_a", "gen_i", "gen_ca", "gen_cb", "gen_cc", "gen_cd"]
        .iter()
        .flat_map(|d| java_files(&work.join(d), &["SpringAdapter.java"]))
        .collect();
    javac(&work, "classes_gen", None, &[], &generated);

    // 2. The test programs against the compiled generated code.
    std::fs::create_dir_all(work.join("tests")).unwrap();
    for f in java_files(&java_dir(), &[]).iter().filter(|p| !p.starts_with(java_dir().join(".work"))) {
        std::fs::copy(f, work.join("tests").join(f.file_name().unwrap())).unwrap();
    }
    javac(&work, "classes_test", Some("classes_gen"), &[], &java_files(&work.join("tests"), &[]));
    Ok(Java { work })
}

fn java() -> Result<&'static Java, Unavailable> {
    static JAVA: OnceLock<Result<Java, Unavailable>> = OnceLock::new();
    match JAVA.get_or_init(|| {
        std::panic::catch_unwind(build_java).unwrap_or_else(|e| {
            Err(Unavailable::Failed(e.downcast_ref::<String>().cloned().unwrap_or_else(|| "build panicked".into())))
        })
    }) {
        Ok(j) => Ok(j),
        Err(e) => Err(e.clone()),
    }
}

/// The Java toolchain, or `None` (skipped) when it is missing and not required.
fn need_java(test: &str) -> Option<&'static Java> {
    match java() {
        Ok(j) => Some(j),
        Err(why) => skip(test, &why),
    }
}

fn skip<T>(test: &str, why: &Unavailable) -> Option<T> {
    match why {
        Unavailable::Failed(msg) => panic!("{msg}"),
        Unavailable::Missing(msg) => {
            assert!(std::env::var("VELD_REQUIRE_TOOLCHAINS").as_deref() != Ok("1"), "toolchain required but unavailable: {msg}");
            eprintln!("SKIPPED {test}: {msg}");
            None
        }
    }
}

/// The class path of the compiled generated code and test programs.
fn run_classpath() -> String {
    format!("classes_gen{}classes_test", if cfg!(windows) { ';' } else { ':' })
}

fn run_java(j: &Java, class: &str, args: &[&str]) -> String {
    let mut cmd = Command::new("java");
    cmd.args(["-cp", &run_classpath()])
        .arg(class)
        .args(args)
        .env("WIRE_DIR", repo_root().join("tests/wire"))
        .current_dir(&j.work);
    run(&mut cmd, class)
}

#[test]
fn generated_java_compiles_without_warnings() {
    // `build_java` compiles with `-Xlint:all -Werror`; reaching here means it did.
    let _ = need_java("generated_java_compiles_without_warnings");
}

#[test]
fn java_round_trip() {
    if let Some(j) = need_java("java_round_trip") {
        let out = run_java(j, "veldtest.Roundtrip", &[]);
        assert!(out.contains("roundtrip ok"));
    }
}

#[test]
fn cross_package_equality() {
    if let Some(j) = need_java("cross_package_equality") {
        assert!(run_java(j, "veldtest.Equality", &[]).contains("equality ok"));
    }
}

#[test]
fn wire_goldens() {
    if let Some(j) = need_java("wire_goldens") {
        assert!(run_java(j, "veldtest.WireCheck", &[]).contains("wire ok"));
    }
}

#[test]
fn spring_adapter_compiles_with_spring_on_the_classpath() {
    let Ok(spring) = std::env::var("VELD_SPRING_CLASSPATH") else {
        eprintln!("SKIPPED spring_adapter_compiles_with_spring_on_the_classpath: VELD_SPRING_CLASSPATH is not set");
        return;
    };
    let Some(j) = need_java("spring_adapter") else { return };
    let source = j.work.join("gen_a/gen/a/adapters/SpringAdapter.java");
    let cp = format!("classes_gen{}{spring}", if cfg!(windows) { ';' } else { ':' });
    javac(&j.work, "classes_spring", Some(&cp), &[], &[source]);
}

// ---------------------------------------------------------------------------
// Cross-language
// ---------------------------------------------------------------------------

struct Ts {
    work: PathBuf,
}

fn gen_ts(generator: &dyn DynGenerator, ir: &Ir, role: Role, services: &[&str], consumes: &[&str]) -> FileSet {
    let ids = |names: &[&str]| names.iter().map(|n| service_id(ir, n)).collect::<Vec<_>>();
    let req = GenRequest { ir, role, services: ids(services), consumes: ids(consumes), framework: None };
    generator.generate_dyn(&req, &Value::Null).unwrap_or_else(|d| panic!("generation failed: {d:?}"))
}

fn build_ts() -> Result<Ts, Unavailable> {
    if !succeeds("node", "--version") {
        return Err(Unavailable::Missing("`node` is not on PATH".into()));
    }
    let tsc = ts_dir().join("node_modules/typescript/bin/tsc");
    if !tsc.exists() {
        return Err(Unavailable::Missing(format!("{} is missing; run `npm ci` in tests/toolchain/ts", tsc.display())));
    }
    let work = ts_dir().join(".work/java-interop");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let a = standard_ir();
    let opts = WriteOptions::default();
    write(&work.join("server"), &gen_ts(&Node, &a, Role::Server, &["Orders"], &["Iam"]), &opts).unwrap();
    write(&work.join("copy_a"), &gen_ts(&TypeScript, &a, Role::Client, &["Iam", "Orders"], &[]), &opts).unwrap();
    write_sigs(&work);
    for script in ["serve.ts", "crossclient.ts"] {
        std::fs::copy(ts_dir().join(script), work.join(script)).unwrap();
    }
    let config = json!({
        "compilerOptions": {
            "strict": true, "target": "ES2022", "lib": ["ES2022"], "types": ["node"],
            "module": "commonjs", "moduleResolution": "node10", "esModuleInterop": true,
            "skipLibCheck": true, "outDir": "out", "rootDir": "."
        },
        "include": ["*.ts", "server/**/*.ts", "copy_a/**/*.ts"]
    });
    std::fs::write(work.join("tsconfig.json"), serde_json::to_string_pretty(&config).unwrap()).unwrap();
    run(Command::new("node").arg(&tsc).args(["-p", "tsconfig.json"]).current_dir(&work), "tsc");
    Ok(Ts { work })
}

fn ts() -> Result<&'static Ts, Unavailable> {
    static TS: OnceLock<Result<Ts, Unavailable>> = OnceLock::new();
    match TS.get_or_init(|| {
        std::panic::catch_unwind(build_ts).unwrap_or_else(|e| {
            Err(Unavailable::Failed(e.downcast_ref::<String>().cloned().unwrap_or_else(|| "build panicked".into())))
        })
    }) {
        Ok(t) => Ok(t),
        Err(e) => Err(e.clone()),
    }
}

/// A server running in another process; killed when dropped.
struct Server {
    child: Child,
    url: String,
}

impl Server {
    /// Starts `cmd`, waits for its `PORT=<n>` line and keeps stdin open (closing it stops the server).
    fn start(mut cmd: Command) -> Server {
        let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).spawn().expect("cannot start server");
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
        let port = line.trim().strip_prefix("PORT=").unwrap_or_else(|| panic!("server did not report a port: {line:?}"));
        Server { url: format!("http://127.0.0.1:{port}"), child }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn both(test: &str) -> Option<(&'static Java, &'static Ts)> {
    let j = need_java(test)?;
    match ts() {
        Ok(t) => Some((j, t)),
        Err(why) => skip(test, &why),
    }
}

#[test]
fn typescript_client_calls_the_java_server() {
    let Some((j, t)) = both("typescript_client_calls_the_java_server") else { return };
    let mut cmd = Command::new("java");
    cmd.args(["-cp", &run_classpath(), "veldtest.Serve"]).current_dir(&j.work);
    let server = Server::start(cmd);
    let out = run(Command::new("node").args(["out/crossclient.js", &server.url]).current_dir(&t.work), "crossclient.js");
    assert!(out.contains("ts client against java server ok"));
}

#[test]
fn java_client_calls_the_node_server() {
    let Some((j, t)) = both("java_client_calls_the_node_server") else { return };
    let mut cmd = Command::new("node");
    cmd.arg("out/serve.js").current_dir(&t.work);
    let server = Server::start(cmd);
    let out = run_java(j, "veldtest.CrossClient", &[&server.url, "node"]);
    assert!(out.contains("cross client ok"));
}
