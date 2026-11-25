//! Helpers that compile generated Java, start it (JDK or Spring) and talk to it over a raw socket.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use veld_codegen::FileSet;

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
    let mut request = format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n");
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
        .current_dir(&work);
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
