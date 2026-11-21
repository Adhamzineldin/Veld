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
//! The Spring adapter is exercised by a real Spring Boot 3 application built with the Maven Wrapper
//! (`tests/toolchain/java/spring`); that test is skipped when neither the network nor a populated
//! `~/.m2` can provide Spring, unless `VELD_REQUIRE_TOOLCHAINS=1`.
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

const CHANGED_ERROR: (&str, &str) = (
    "error OutOfStock 409 {\n  sku: string\n}",
    "error OutOfStock 409 {\n  sku: string\n  warehouse: string\n}",
);

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
    Command::new(program)
        .arg(arg)
        .output()
        .is_ok_and(|o| o.status.success())
}

fn run(cmd: &mut Command, what: &str) -> String {
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("{what}: cannot start: {e}"));
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
    std::fs::write(
        dir.join("sigs.json"),
        json!({ "a": sig_map(&a), "c": sig_map(&c), "d": sig_map(&d) }).to_string(),
    )
    .unwrap();
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
                && !skip
                    .iter()
                    .any(|s| path.file_name().is_some_and(|n| n == *s))
            {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// `javac --release <release> -Xlint:all -Werror` over `sources`; any warning fails the build.
fn javac(
    work: &Path,
    release: &str,
    out: &str,
    classpath: Option<&str>,
    extra: &[&str],
    sources: &[PathBuf],
) {
    let list: Vec<String> = sources
        .iter()
        .map(|p| p.display().to_string().replace('\\', "/"))
        .collect();
    let argfile = work.join(format!("{out}.args"));
    std::fs::write(&argfile, list.join("\n")).unwrap();
    let mut cmd = Command::new("javac");
    cmd.args([
        "--release",
        release,
        "-Xlint:all",
        "-Werror",
        "-encoding",
        "UTF-8",
        "-d",
        out,
    ]);
    if let Some(cp) = classpath {
        cmd.args(["-cp", cp]);
    }
    cmd.args(extra)
        .arg(format!("@{}.args", out))
        .current_dir(work);
    run(&mut cmd, &format!("javac ({out})"));
}

struct Java {
    work: PathBuf,
    /// The `javac --release` and `java` option the code was generated and compiled for.
    release: &'static str,
}

/// Generates every Java target for `release` and compiles them with the test programs.
fn build_java(release: &'static str) -> Result<Java, Unavailable> {
    if !succeeds("javac", "--version") || !succeeds("java", "--version") {
        return Err(Unavailable::Missing(
            "`javac`/`java` are not on PATH".into(),
        ));
    }
    let work = java_dir().join(format!(".work/run-{release}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let a = standard_ir();
    let c = compile_contract("orders", &|s| s.replace(CHANGED_ERROR.0, CHANGED_ERROR.1));
    let d = compile_contract("warehouse", &|s| s.to_string());
    let opts = WriteOptions::default();
    let put = |dir: &str, files: &FileSet| write(&work.join(dir), files, &opts).unwrap();
    put(
        "gen_a",
        &gen_server_for(&a, "Orders", &["Iam"], "gen.a", None, Some(release)),
    );
    // The Spring flavour of the same server: built by Maven in `build_spring_app`, never by plain javac.
    put(
        "gen_spring",
        &gen_server_for(
            &a,
            "Orders",
            &["Iam"],
            "gen.a",
            Some("spring"),
            Some(release),
        ),
    );
    put(
        "gen_i",
        &gen_server_for(&a, "Iam", &[], "gen.i", None, Some(release)),
    );
    put("gen_ca", &gen_client_for(&a, "gen.ca", Some(release)));
    put("gen_cb", &gen_client_for(&a, "gen.cb", Some(release)));
    put("gen_cc", &gen_client_for(&c, "gen.cc", Some(release)));
    put("gen_cd", &gen_client_for(&d, "gen.cd", Some(release)));
    write_sigs(&work);

    // 1. Generated code alone: zero warnings.
    let generated: Vec<PathBuf> = ["gen_a", "gen_i", "gen_ca", "gen_cb", "gen_cc", "gen_cd"]
        .iter()
        .flat_map(|d| java_files(&work.join(d), &[]))
        .collect();
    javac(&work, release, "classes_gen", None, &[], &generated);

    // 2. The test programs against the compiled generated code.
    std::fs::create_dir_all(work.join("tests")).unwrap();
    for f in java_files(&java_dir(), &[]).iter().filter(|p| {
        !p.starts_with(java_dir().join(".work")) && !p.starts_with(java_dir().join("spring"))
    }) {
        std::fs::copy(f, work.join("tests").join(f.file_name().unwrap())).unwrap();
    }
    javac(
        &work,
        release,
        "classes_test",
        Some("classes_gen"),
        &[],
        &java_files(&work.join("tests"), &[]),
    );
    Ok(Java { work, release })
}

/// The built toolchain for `release`, one cell per entry of [`RELEASES`].
fn java(release: &str) -> Result<&'static Java, Unavailable> {
    static JAVA: [OnceLock<Result<Java, Unavailable>>; RELEASES.len()] =
        [const { OnceLock::new() }; RELEASES.len()];
    let Some(i) = RELEASES.iter().position(|r| *r == release) else {
        return Err(Unavailable::Failed(format!(
            "unknown Java release {release}"
        )));
    };
    let release = RELEASES[i];
    match JAVA[i].get_or_init(|| {
        std::panic::catch_unwind(|| build_java(release)).unwrap_or_else(|e| {
            Err(Unavailable::Failed(
                e.downcast_ref::<String>()
                    .cloned()
                    .unwrap_or_else(|| "build panicked".into()),
            ))
        })
    }) {
        Ok(j) => Ok(j),
        Err(e) => Err(e.clone()),
    }
}

/// Runs `body` once per Java release of the matrix, skipping when the toolchain is missing.
fn each_java(test: &str, body: impl Fn(&'static Java)) {
    for release in releases() {
        if let Some(j) = need_java(test, release) {
            body(j);
        }
    }
}

/// The Java toolchain, or `None` (skipped) when it is missing and not required.
fn need_java(test: &str, release: &str) -> Option<&'static Java> {
    match java(release) {
        Ok(j) => Some(j),
        Err(why) => skip(test, &why),
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

/// The class path of the compiled generated code and test programs.
fn run_classpath() -> String {
    format!(
        "classes_gen{}classes_test",
        if cfg!(windows) { ';' } else { ':' }
    )
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
    // `build_java` compiles with `-Xlint:all -Werror` for every release; reaching here means it did.
    each_java("generated_java_compiles_without_warnings", |_| {});
}

#[test]
fn java_round_trip() {
    each_java("java_round_trip", |j| {
        let out = run_java(j, "veldtest.Roundtrip", &[]);
        assert!(out.contains("roundtrip ok"), "java {}", j.release);
    });
}

#[test]
fn cross_package_equality() {
    each_java("cross_package_equality", |j| {
        assert!(run_java(j, "veldtest.Equality", &[]).contains("equality ok"));
    });
}

#[test]
fn wire_goldens() {
    each_java("wire_goldens", |j| {
        assert!(run_java(j, "veldtest.WireCheck", &[]).contains("wire ok"));
    });
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

fn build_ts() -> Result<Ts, Unavailable> {
    if !succeeds("node", "--version") {
        return Err(Unavailable::Missing("`node` is not on PATH".into()));
    }
    let tsc = ts_dir().join("node_modules/typescript/bin/tsc");
    if !tsc.exists() {
        return Err(Unavailable::Missing(format!(
            "{} is missing; run `npm ci` in tests/toolchain/ts",
            tsc.display()
        )));
    }
    let work = ts_dir().join(".work/java-interop");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let a = standard_ir();
    let opts = WriteOptions::default();
    write(
        &work.join("server"),
        &gen_ts(&Node, &a, Role::Server, &["Orders"], &["Iam"]),
        &opts,
    )
    .unwrap();
    write(
        &work.join("copy_a"),
        &gen_ts(&TypeScript, &a, Role::Client, &["Iam", "Orders"], &[]),
        &opts,
    )
    .unwrap();
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
    std::fs::write(
        work.join("tsconfig.json"),
        serde_json::to_string_pretty(&config).unwrap(),
    )
    .unwrap();
    run(
        Command::new("node")
            .arg(&tsc)
            .args(["-p", "tsconfig.json"])
            .current_dir(&work),
        "tsc",
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

/// A server running in another process; killed when dropped.
struct Server {
    tree: veld_test_support::ProcessTree,
    url: String,
}

impl Server {
    /// Starts `cmd`, waits for its `PORT=<n>` line and keeps stdin open (closing it stops the server).
    fn start(mut cmd: Command) -> Server {
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut tree =
            veld_test_support::ProcessTree::spawn(&mut cmd).expect("cannot start server");
        drain_stderr(tree.child_mut());
        let mut line = String::new();
        BufReader::new(tree.child_mut().stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let port = line
            .trim()
            .strip_prefix("PORT=")
            .unwrap_or_else(|| panic!("server did not report a port: {line:?}"));
        Server {
            url: format!("http://127.0.0.1:{port}"),
            tree,
        }
    }
}

/// Forwards the child's stderr from a thread, so the child never holds the test's own stderr.
fn drain_stderr(child: &mut Child) {
    if let Some(err) = child.stderr.take() {
        std::thread::spawn(move || {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                eprintln!("{line}");
            }
        });
    }
}

fn both(test: &str, release: &str) -> Option<(&'static Java, &'static Ts)> {
    let j = need_java(test, release)?;
    match ts() {
        Ok(t) => Some((j, t)),
        Err(why) => skip(test, &why),
    }
}

#[test]
fn typescript_client_calls_the_java_server() {
    for release in releases() {
        typescript_client_calls_the_java_server_on(release);
    }
}

fn typescript_client_calls_the_java_server_on(release: &str) {
    let Some((j, t)) = both("typescript_client_calls_the_java_server", release) else {
        return;
    };
    let mut cmd = Command::new("java");
    cmd.args(["-cp", &run_classpath(), "veldtest.Serve"])
        .current_dir(&j.work);
    let server = Server::start(cmd);
    let out = run(
        Command::new("node")
            .args(["out/crossclient.js", &server.url])
            .current_dir(&t.work),
        "crossclient.js",
    );
    assert!(out.contains("ts client against java server ok"));
}

#[test]
fn java_client_calls_the_node_server() {
    for release in releases() {
        java_client_calls_the_node_server_on(release);
    }
}

fn java_client_calls_the_node_server_on(release: &str) {
    let Some((j, t)) = both("java_client_calls_the_node_server", release) else {
        return;
    };
    let mut cmd = Command::new("node");
    cmd.arg("out/serve.js").current_dir(&t.work);
    let server = Server::start(cmd);
    let out = run_java(j, "veldtest.CrossClient", &[&server.url, "node"]);
    assert!(out.contains("cross client ok"));
}

/// Endpoints, fields, models and enum variants named with every hazardous identifier must still
/// compile under `javac -Xlint:all -Werror`, on the server and the client.
#[test]
fn hazardous_identifiers_compile_without_warnings() {
    if !succeeds("javac", "--version") {
        let why = Unavailable::Missing("`javac` is not on PATH".into());
        let _: Option<()> = skip("hazardous_identifiers_compile_without_warnings", &why);
        return;
    }
    for release in releases() {
        hazards_compile_on(release);
    }
}

fn hazards_compile_on(release: &str) {
    let work = java_dir().join(format!(".work/hazards-{release}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let ir = hazards_ir();
    let opts = WriteOptions::default();
    let put = |dir: &str, files: &FileSet| write(&work.join(dir), files, &opts).unwrap();
    put(
        "srv",
        &gen_server_for(&ir, "Weird", &[], "hz.srv", None, Some(release)),
    );
    put("cli", &gen_client_for(&ir, "hz.cli", Some(release)));
    let sources: Vec<PathBuf> = ["srv", "cli"]
        .iter()
        .flat_map(|d| java_files(&work.join(d), &[]))
        .collect();
    javac(&work, release, "classes", None, &[], &sources);
}

/// The generated Java client and the generated TypeScript client call the same Node servers over
/// HTTP and decode the same declared errors: they must agree on signature, operation and
/// canonical payload, and the other service's identical error must differ in the operation only.
#[test]
fn java_and_typescript_clients_decode_errors_identically() {
    let name = "java_and_typescript_clients_decode_errors_identically";
    if !succeeds("javac", "--version") {
        let why = Unavailable::Missing("`javac` is not on PATH".into());
        let _: Option<()> = skip(name, &why);
        return;
    }
    if !succeeds("node", "--version") || !ts_dir().join("node_modules/typescript/bin/tsc").exists()
    {
        let why = Unavailable::Missing("node or the TypeScript toolchain is missing".into());
        let _: Option<()> = skip(name, &why);
        return;
    }
    let work = ts_dir().join(".work/eq-http");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let ir = equality_ir();
    let opts = WriteOptions::default();
    let put = |dir: &str, files: &FileSet| write(&work.join(dir), files, &opts).unwrap();
    put(
        "srv_alpha",
        &gen_ts(&Node, &ir, Role::Server, &["Alpha"], &[]),
    );
    put(
        "srv_beta",
        &gen_ts(&Node, &ir, Role::Server, &["Beta"], &[]),
    );
    put(
        "cli_1",
        &gen_ts(&TypeScript, &ir, Role::Client, &["Alpha", "Beta"], &[]),
    );
    put("java_cli", &gen_client(&ir, "eqj"));
    for script in ["eq_serve.ts", "eq_decode.ts"] {
        std::fs::copy(ts_dir().join(script), work.join(script)).unwrap();
    }
    std::fs::copy(
        repo_root().join("tests/toolchain/java_eq/EqDecode.java"),
        work.join("EqDecode.java"),
    )
    .unwrap();
    std::fs::write(
        work.join("tsconfig.json"),
        serde_json::to_string_pretty(&json!({
            "compilerOptions": {
                "strict": true, "target": "ES2022", "lib": ["ES2022"], "types": ["node"],
                "module": "commonjs", "moduleResolution": "node10", "esModuleInterop": true,
                "skipLibCheck": true, "outDir": "out", "rootDir": "."
            },
            "include": ["*.ts", "srv_alpha/**/*.ts", "srv_beta/**/*.ts", "cli_1/**/*.ts"]
        }))
        .unwrap(),
    )
    .unwrap();
    run(
        Command::new("node")
            .arg(ts_dir().join("node_modules/typescript/bin/tsc"))
            .args(["-p", "tsconfig.json"])
            .current_dir(&work),
        "tsc",
    );
    let mut sources = java_files(&work.join("java_cli"), &[]);
    sources.push(work.join("EqDecode.java"));
    javac(&work, RELEASES[0], "classes", None, &[], &sources);

    let serve = |which: &str| {
        let mut cmd = Command::new("node");
        cmd.args(["out/eq_serve.js", which]).current_dir(&work);
        Server::start(cmd)
    };
    let (alpha, beta) = (serve("alpha"), serve("beta"));
    let ts_out = run(
        Command::new("node")
            .args(["out/eq_decode.js", &alpha.url, &beta.url])
            .current_dir(&work),
        "eq_decode.js",
    );
    let java_out = run(
        Command::new("java")
            .args(["-cp", "classes", "veldtest.EqDecode", &alpha.url, &beta.url])
            .current_dir(&work),
        "EqDecode",
    );
    // `println` ends lines with CRLF on Windows.
    let normalize = |s: &str| s.lines().collect::<Vec<_>>().join("\n");
    let (java_out, ts_out) = (normalize(&java_out), normalize(&ts_out));
    assert_eq!(java_out, ts_out, "Java and TypeScript decode differently");

    let rows: Vec<Vec<&str>> = ts_out.lines().map(|l| l.split('\t').collect()).collect();
    assert_eq!(rows.len(), 4, "{ts_out}");
    let find = |svc: &str, id: &str| {
        rows.iter()
            .find(|r| r[0] == svc && r[1] == id)
            .unwrap_or_else(|| panic!("no row for {svc} {id}"))
    };
    let (ax, ay, bx) = (find("Alpha", "x"), find("Alpha", "y"), find("Beta", "x"));
    assert_eq!(ax[2], "Gone", "decoded into the declared error class");
    assert_eq!(ax[3], bx[3], "same error declaration, same signature");
    assert_ne!(ax[4], bx[4], "the other service has a different operation");
    assert!(ax[4].contains("/Alpha.fetch@") && bx[4].contains("/Beta.fetch@"));
    assert_ne!(
        ax[5], ay[5],
        "different payload has different canonical bytes"
    );
    let (ax_canon, bx_canon) = (ax[5], bx[5]);
    assert_eq!(ax_canon, bx_canon, "same payload from either service");
}

// ---------------------------------------------------------------------------
// Spring Boot 3
// ---------------------------------------------------------------------------

fn spring_project() -> PathBuf {
    java_dir().join("spring")
}

/// Whether Maven can get Spring: a populated `~/.m2`, or a network route to Maven Central.
fn maven_reachable() -> Result<(), String> {
    use std::net::{TcpStream, ToSocketAddrs};
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from);
    if home.is_some_and(|h| {
        h.join(".m2/repository/org/springframework/boot/spring-boot-starter-web")
            .is_dir()
    }) {
        return Ok(());
    }
    let reachable = "repo.maven.apache.org:443"
        .to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
        .is_some_and(|addr| {
            TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(10)).is_ok()
        });
    if reachable {
        Ok(())
    } else {
        Err("no network route to Maven Central and no Spring in ~/.m2".into())
    }
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let dest = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_tree(&path, &dest);
        } else {
            std::fs::copy(&path, &dest).unwrap();
        }
    }
}

/// Copies the generated sources, the Maven project and the test application into a work
/// directory and runs `mvnw package`. Returns the path of the executable jar.
fn build_spring_app(j: &Java) -> PathBuf {
    let work = java_dir().join(format!(".work/spring-{}", j.release));
    let _ = std::fs::remove_dir_all(&work);
    let sources = work.join("src/main/java");
    std::fs::create_dir_all(sources.join("veldtest")).unwrap();
    let generated = j.work.join("gen_spring");
    for file in java_files(&generated, &[]) {
        let rel = file.strip_prefix(&generated).unwrap();
        let dest = sources.join(rel);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::copy(&file, dest).unwrap();
    }
    let project = spring_project();
    for name in ["pom.xml", "mvnw", "mvnw.cmd"] {
        std::fs::copy(project.join(name), work.join(name)).unwrap();
    }
    copy_tree(&project.join(".mvn"), &work.join(".mvn"));
    copy_tree(
        &project.join("src/main/java/veldtest"),
        &sources.join("veldtest"),
    );
    for shared in ["ServerImpl.java", "Check.java"] {
        std::fs::copy(
            java_dir().join(shared),
            sources.join("veldtest").join(shared),
        )
        .unwrap();
    }
    let mut mvn = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/c", r".\mvnw.cmd"]);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("./mvnw");
        c
    };
    mvn.args(["-q", "-B", "package", "-DskipTests"])
        .current_dir(&work);
    run(&mut mvn, "mvnw package");
    work.join("target/app.jar")
}

/// The port in Spring's `Tomcat started on port 55983 (http) ...` log line.
fn started_port(line: &str) -> Option<u16> {
    let rest = line.split("Tomcat started on port ").nth(1)?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Whether `GET /__probe` answers 200 (the application is serving, not just listening).
fn probe_answers(port: u16) -> bool {
    use std::io::{Read, Write};
    let Ok(mut stream) = std::net::TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
    let request = "GET /__probe HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    let mut reply = String::new();
    stream.write_all(request.as_bytes()).is_ok()
        && stream.read_to_string(&mut reply).is_ok()
        && reply.starts_with("HTTP/1.1 200")
}

/// Starts the jar and waits until it really serves. The server picks its own port (`0`, so no
/// port can be taken between choosing and binding it) and reports it in its log; readiness is an
/// answered request, and an exit or a missing port within the timeout fails the test.
fn start_spring(jar: &Path) -> Server {
    let mut cmd = Command::new("java");
    cmd.arg("-jar")
        .arg(jar)
        .args(["--server.port=0", "--server.address=127.0.0.1"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut tree = veld_test_support::ProcessTree::spawn(&mut cmd)
        .expect("cannot start the Spring application");
    drain_stderr(tree.child_mut());
    let stdout = tree.child_mut().stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            eprintln!("{line}");
            if let Some(port) = started_port(&line) {
                let _ = tx.send(port);
            }
        }
    });
    let mut server = Server {
        tree,
        url: String::new(),
    };
    let port = rx
        .recv_timeout(std::time::Duration::from_secs(120))
        .unwrap_or_else(|e| panic!("the Spring application never reported its port: {e}"));
    server.url = format!("http://127.0.0.1:{port}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !probe_answers(port) {
        assert!(
            server.tree.child_mut().try_wait().unwrap().is_none(),
            "the Spring application exited before it served"
        );
        assert!(
            std::time::Instant::now() < deadline,
            "the Spring application did not answer /__probe within 60s"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    server
}

/// The generated Spring adapter in a real Spring Boot 3 application (Maven Wrapper, starter-web
/// only): the same behaviours as the JDK adapter round trip, through the Java client, and the
/// generated TypeScript client against the same running server.
#[test]
fn spring_adapter_serves_the_round_trip() {
    let name = "spring_adapter_serves_the_round_trip";
    let Some(j) = need_java(name, "17") else {
        return;
    };
    if let Err(why) = maven_reachable() {
        let _: Option<()> = skip(name, &Unavailable::Missing(why));
        return;
    }
    let jar = build_spring_app(j);
    let server = start_spring(&jar);
    let out = run_java(j, "veldtest.SpringCrossClient", &[&server.url]);
    assert!(out.contains("spring client ok"));
    drop(server);
    // The TypeScript scenario counts on a fresh server, so it gets a second instance of the jar.
    if let Some((_, t)) = both(name, "17") {
        let server = start_spring(&jar);
        let out = run(
            Command::new("node")
                .args(["out/crossclient.js", &server.url])
                .current_dir(&t.work),
            "crossclient.js",
        );
        assert!(out.contains("ts client against java server ok"));
    }
}

// ---------------------------------------------------------------------------
// Import hygiene
// ---------------------------------------------------------------------------

/// Whether `word` occurs in `text` as a whole identifier.
fn mentions(text: &str, word: &str) -> bool {
    let ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    text.match_indices(word).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + word.len()..].chars().next();
        !before.is_some_and(ident) && !after.is_some_and(ident)
    })
}

/// The imports of `source` that its body never mentions. `javac` does not report these.
fn unused_imports(source: &str) -> Vec<String> {
    let (imports, body): (Vec<&str>, Vec<&str>) = source
        .lines()
        .partition(|l| l.trim_start().starts_with("import "));
    let body = body.join("\n");
    imports
        .iter()
        .filter_map(|l| l.trim().strip_prefix("import ")?.strip_suffix(';'))
        .filter(|fqn| {
            let simple = fqn.rsplit('.').next().unwrap_or(fqn);
            simple != "*" && !mentions(&body, simple)
        })
        .map(str::to_string)
        .collect()
}

#[test]
fn the_import_scan_finds_unused_imports() {
    let source = "package a;\nimport b.Used;\nimport b.Unused;\nclass A { Used u; }\n";
    assert_eq!(unused_imports(source), ["b.Unused"]);
    assert!(mentions("x.GetMapping(\"/\")", "GetMapping"));
    assert!(!mentions("MyGetMapping", "GetMapping"));
}

#[test]
fn generated_java_has_no_unused_imports() {
    each_java("generated_java_has_no_unused_imports", |j| {
        let mut problems = Vec::new();
        for dir in [
            "gen_a",
            "gen_spring",
            "gen_i",
            "gen_ca",
            "gen_cb",
            "gen_cc",
            "gen_cd",
        ] {
            for file in java_files(&j.work.join(dir), &[]) {
                let source = std::fs::read_to_string(&file).unwrap();
                for fqn in unused_imports(&source) {
                    problems.push(format!("{}: unused import {fqn}", file.display()));
                }
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    });
}
