//! Helpers that compile generated Java, start it (JDK or Spring) and talk to it over a raw socket.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use veld_codegen::FileSet;
use veld_ir::Ir;

use super::repo_root;

/// Whether `javac` is usable; a missing one fails when `VELD_REQUIRE_TOOLCHAINS=1`.
pub fn require_javac(test: &str) -> bool {
    let ok = Command::new("javac")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !ok {
        assert!(
            std::env::var("VELD_REQUIRE_TOOLCHAINS").as_deref() != Ok("1"),
            "toolchain required but `javac` is not available"
        );
        eprintln!("SKIPPED {test}: `javac` is not on PATH");
    }
    ok
}

/// Writes `files` below `work/src` and returns the `.java` paths.
pub fn write_files(work: &Path, files: &FileSet) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    for (path, bytes) in files.iter() {
        let target = work.join("src").join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, bytes).unwrap();
        if path.ends_with(".java") {
            sources.push(target);
        }
    }
    sources
}

/// Compiles `sources` with `-Xlint:all -Werror` into `work/classes` and returns the compiler output.
pub fn javac(work: &Path, sources: &[PathBuf], release: &str) -> Output {
    let list: Vec<String> = sources
        .iter()
        .map(|p| p.display().to_string().replace('\\', "/"))
        .collect();
    std::fs::write(work.join("sources.args"), list.join("\n")).unwrap();
    Command::new("javac")
        .args([
            "--release",
            release,
            "-Xlint:all",
            "-Werror",
            "-encoding",
            "UTF-8",
        ])
        .args(["-d", "classes", "@sources.args"])
        .current_dir(work)
        .output()
        .unwrap()
}

/// Compiles and fails the test on a compiler error.
pub fn javac_ok(work: &Path, sources: &[PathBuf], release: &str) {
    let out = javac(work, sources, release);
    assert!(
        out.status.success(),
        "javac failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Runs a compiled class from `work/classes`.
pub fn java_main(work: &Path, class: &str, args: &[&str]) -> Output {
    Command::new("java")
        .args(["-cp", "classes", class])
        .args(args)
        .current_dir(work)
        .output()
        .unwrap()
}

/// One HTTP/1.1 exchange on a raw socket: the status, the header block (lower case) and the body.
#[derive(Debug, Clone)]
pub struct Raw {
    pub status: u16,
    pub headers: String,
    pub body: String,
}

/// Sends one request and reads the whole response (the connection is closed after it).
pub fn raw_http(
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> Option<Raw> {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).ok()?;
    s.set_read_timeout(Some(std::time::Duration::from_secs(20)))
        .ok()?;
    let mut request =
        format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n");
    for (k, v) in headers {
        request.push_str(&format!("{k}: {v}\r\n"));
    }
    if !headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("content-type"))
    {
        request.push_str("Content-Type: application/json\r\n");
    }
    request.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));
    s.write_all(request.as_bytes()).ok()?;
    let mut bytes = Vec::new();
    s.read_to_end(&mut bytes).ok()?;
    let raw = String::from_utf8_lossy(&bytes).into_owned();
    let status = raw.split_whitespace().nth(1)?.parse().ok()?;
    let (head, rest) = raw.split_once("\r\n\r\n").unwrap_or((&raw, ""));
    let head = head.to_ascii_lowercase();
    let body = if head.contains("transfer-encoding: chunked") {
        dechunk(rest)
    } else {
        rest.to_string()
    };
    Some(Raw {
        status,
        headers: head,
        body,
    })
}

fn dechunk(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some((size, tail)) = rest.split_once("\r\n") {
        let n = usize::from_str_radix(size.trim(), 16).unwrap_or(0);
        if n == 0 || tail.len() < n {
            break;
        }
        out.push_str(&tail[..n]);
        rest = tail.get(n + 2..).unwrap_or("");
    }
    out
}

/// Whether Spring can be built here: in `~/.m2`, or Maven Central is reachable.
pub fn spring_available(test: &str) -> bool {
    use std::net::{TcpStream, ToSocketAddrs};
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
    let cached = home.is_some_and(|h| {
        PathBuf::from(h)
            .join(".m2/repository/org/springframework/boot/spring-boot-starter-web")
            .is_dir()
    });
    let have = cached
        || "repo.maven.apache.org:443"
            .to_socket_addrs()
            .ok()
            .and_then(|mut a| a.next())
            .is_some_and(|addr| {
                TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(10)).is_ok()
            });
    if !have {
        assert!(
            std::env::var("VELD_REQUIRE_TOOLCHAINS").as_deref() != Ok("1"),
            "toolchain required but Spring is not in ~/.m2 and Maven Central is not reachable"
        );
        eprintln!("SKIPPED {test}: no Spring in ~/.m2 and no route to Maven Central");
    }
    have
}

pub fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let dest = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_dir(&path, &dest);
        } else {
            std::fs::copy(&path, &dest).unwrap();
        }
    }
}

/// A running Spring Boot application around generated code.
pub struct SpringApp {
    _server: veld_test_support::ProcessTree,
    pub port: u16,
}

/// Builds the generated `files` and the application sources `app_sources` (copied to
/// `veldtest/`) with the Maven Wrapper, starts the jar and waits until `ready_path` answers.
///
/// `dir` is the name of the work directory below `tests/toolchain/java/.work`.
pub fn start_spring(
    dir: &str,
    files: &FileSet,
    app_sources: &[&str],
    ready_path: &str,
    extra: &[&str],
) -> SpringApp {
    let work = build_spring(dir, files, app_sources);
    run_spring(&work, ready_path, extra)
}

/// Builds the application and returns its work directory (see [`start_spring`]).
pub fn build_spring(dir: &str, files: &FileSet, app_sources: &[&str]) -> PathBuf {
    let java = repo_root().join("tests/toolchain/java");
    let work = java.join(".work").join(dir);
    let _ = std::fs::remove_dir_all(&work);
    let sources = work.join("src/main/java");
    for (path, bytes) in files.iter().filter(|(p, _)| p.ends_with(".java")) {
        let target = sources.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, bytes).unwrap();
    }
    std::fs::create_dir_all(sources.join("veldtest")).unwrap();
    for name in app_sources {
        let from = java.join(name);
        std::fs::copy(
            &from,
            sources.join("veldtest").join(from.file_name().unwrap()),
        )
        .unwrap();
    }
    for name in ["pom.xml", "mvnw", "mvnw.cmd"] {
        std::fs::copy(java.join("spring").join(name), work.join(name)).unwrap();
    }
    copy_dir(&java.join("spring/.mvn"), &work.join(".mvn"));
    let mut mvn = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/c", r".\mvnw.cmd"]);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("./mvnw");
        c
    };
    mvn.args(["-q", "-B", "package", "-DskipTests", "-Djava.version=17"])
        .current_dir(&work);
    let mut build =
        veld_test_support::ProcessTree::spawn_logged(&mut mvn, &work.join("mvnw.log")).unwrap();
    let status = build.child_mut().wait().unwrap();
    assert!(
        status.success(),
        "mvnw package failed:\n{}",
        std::fs::read_to_string(work.join("mvnw.log")).unwrap_or_default()
    );
    work
}

/// Starts the jar built in `work` and waits until `ready_path` answers.
pub fn run_spring(work: &Path, ready_path: &str, extra: &[&str]) -> SpringApp {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut app = Command::new("java");
    app.arg("-jar")
        .arg(work.join("target/app.jar"))
        .arg(format!("--server.port={port}"))
        .arg("--server.address=127.0.0.1")
        .args(extra)
        .current_dir(work);
    let mut server =
        veld_test_support::ProcessTree::spawn_logged(&mut app, &work.join("app.log")).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    while raw_http(port, "GET", ready_path, &[], "").is_none() {
        assert!(
            server.child_mut().try_wait().unwrap().is_none(),
            "the application exited:\n{}",
            std::fs::read_to_string(work.join("app.log")).unwrap_or_default()
        );
        assert!(
            std::time::Instant::now() < deadline,
            "the application never answered"
        );
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    SpringApp {
        _server: server,
        port,
    }
}

impl SpringApp {
    /// The text the application has logged so far.
    pub fn log(dir: &str) -> String {
        let work = repo_root().join("tests/toolchain/java/.work").join(dir);
        std::fs::read_to_string(work.join("app.log")).unwrap_or_default()
    }
}

/// A running Java process that printed `PORT=<n>`; killed when dropped.
#[allow(missing_debug_implementations)]
pub struct Running {
    tree: veld_test_support::ProcessTree,
    pub url: String,
}

impl Running {
    pub fn start(work: &std::path::Path, class: &str, args: &[&str]) -> Running {
        use std::io::{BufRead, BufReader};
        use std::process::{Command, Stdio};
        let mut cmd = Command::new("java");
        cmd.args(["-cp", "classes", class])
            .args(args)
            .current_dir(work)
            .stdin(Stdio::null())
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
        Running {
            tree,
            url: format!("http://127.0.0.1:{port}"),
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.tree.kill_tree();
    }
}

/// Sends every example of the contract through the shared live runner, in dry-run mode, and
/// returns how many passed.
pub fn run_every_example(ir: &Ir, url: &str) -> usize {
    let live = veld_testing::Live::connect(url, true).unwrap();
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
    passed
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
