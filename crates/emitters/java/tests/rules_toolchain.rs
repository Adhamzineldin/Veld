//! The shared rule vectors (`tests/rules/core`) run against generated Java, and the HTTP round
//! trip of invariants on the JDK server. Needs a JDK; `VELD_REQUIRE_TOOLCHAINS=1` makes a missing
//! one a failure.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use common::*;
use veld_codegen::Role;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_emit_java::tree::Tree;
use veld_emit_java::util::type_name;
use veld_ir::{DeclKind, Ir, PackageKind};
use veld_rules::vectors::{Produced, compare_produced, load_cases};

/// The shared vectors. Every target runs the same files.
fn vector_dirs() -> Vec<PathBuf> {
    vec![repo_root().join("tests/rules/core")]
}

fn vector_files(ext: &str) -> Vec<PathBuf> {
    vector_dirs().iter().flat_map(|d| sorted(d, ext)).collect()
}

fn javac_ok() -> bool {
    Command::new("javac")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn require_javac(test: &str) -> bool {
    if javac_ok() {
        return true;
    }
    assert!(
        std::env::var("VELD_REQUIRE_TOOLCHAINS").as_deref() != Ok("1"),
        "toolchain required but `javac` is not available"
    );
    eprintln!("SKIPPED {test}: `javac` is not on PATH");
    false
}

fn sorted(dir: &std::path::Path, ext: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .collect();
    v.sort();
    v
}

/// Every vector contract in one package, plus a service that makes every model reachable.
fn vector_ir() -> Ir {
    let mut files = Vec::new();
    let mut uses = String::new();
    let mut probes = String::from("service Probe at \"/probe\" {\n");
    let mut n = 0;
    for path in vector_files("veld") {
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .replace("\r\n", "\n");
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        let mut names = Vec::new();
        for line in text.lines() {
            let mut words = line.split_whitespace();
            if let (Some("model" | "union"), Some(name)) = (words.next(), words.next()) {
                n += 1;
                names.push(name.to_string());
                probes.push_str(&format!("  post \"/p{n}\" probe{n}(body: {name}) -> 204\n"));
            }
        }
        if !names.is_empty() {
            uses.push_str(&format!("use {stem}.{{{}}}\n", names.join(", ")));
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        files.push(SourceFile { path: name, text });
    }
    probes.push_str("}\n");
    files.push(SourceFile {
        path: "zz_probe.veld".into(),
        text: format!("{uses}\n{probes}"),
    });
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files,
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    assert!(
        out.ir.is_some(),
        "vectors do not compile:\n{}",
        out.render()
    );
    out.ir.unwrap()
}

fn all_services(ir: &Ir) -> Vec<veld_ir::ServiceId> {
    ir.services.iter().map(|s| s.id).collect()
}

#[test]
fn the_shared_vectors_pass_on_generated_java() {
    if !require_javac("the_shared_vectors_pass_on_generated_java") {
        return;
    }
    let ir = vector_ir();
    let files = run(
        &ir,
        Role::Client,
        all_services(&ir),
        vec![],
        None,
        options("gen.v", None),
    );
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path();
    let mut sources = Vec::new();
    for (path, bytes) in files.iter() {
        let target = work.join("src").join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, bytes).unwrap();
        if path.ends_with(".java") {
            sources.push(target);
        }
    }
    let driver = repo_root().join("tests/toolchain/java/rules/RuleVectors.java");
    sources.push(driver);
    let list: Vec<String> = sources
        .iter()
        .map(|p| p.display().to_string().replace('\\', "/"))
        .collect();
    std::fs::write(work.join("sources.args"), list.join("\n")).unwrap();
    let out = Command::new("javac")
        .args([
            "--release",
            "17",
            "-Xlint:all",
            "-Werror",
            "-encoding",
            "UTF-8",
        ])
        .args(["-d", "classes", "@sources.args"])
        .current_dir(work)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "javac failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // The cases of every vector file, partial ones left out (TypeScript only).
    let mut cases = Vec::new();
    let mut lines = String::new();
    let (mut total, mut partial) = (0, 0);
    for path in vector_files("json") {
        for case in load_cases(&std::fs::read_to_string(&path).unwrap()).unwrap() {
            total += 1;
            if case.partial {
                partial += 1;
                continue;
            }
            lines.push_str(
                &serde_json::json!({
                    "id": cases.len(), "type": case.ty, "value": case.value, "request": case.request
                })
                .to_string(),
            );
            lines.push('\n');
            cases.push(case);
        }
    }
    std::fs::write(work.join("cases.jsonl"), lines).unwrap();
    let tree = Tree::new(&ir, "gen.v", "gen.v.runtime", all_services(&ir));
    let mut types = String::new();
    for id in tree.decls() {
        let d = ir.decl(*id);
        if matches!(d.kind, DeclKind::Model(_) | DeclKind::Union(_)) {
            types.push_str(&format!(
                "{}\t{}.{}\n",
                d.name,
                tree.decl_package(d),
                type_name(d)
            ));
        }
    }
    std::fs::write(work.join("types.tsv"), types).unwrap();

    let out = Command::new("java")
        .args([
            "-cp",
            "classes",
            "veldtest.RuleVectors",
            "cases.jsonl",
            "types.tsv",
        ])
        .current_dir(work)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "driver failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    for case in &cases {
        if let Err(m) = veld_rules::vectors::run_case(&ir, case) {
            panic!("the reference evaluator disagrees with the vector: {m}");
        }
    }
    let mut failures = Vec::new();
    let mut seen = 0;
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let case = &cases[v["id"].as_u64().unwrap() as usize];
        seen += 1;
        if let Some(e) = v.get("error") {
            failures.push(format!("{}: driver error {e}", case.name));
            continue;
        }
        let produced: Vec<Produced> = serde_json::from_value(v["violations"].clone()).unwrap();
        if let Err(m) = compare_produced(case, &produced) {
            failures.push(m.to_string());
        }
    }
    assert_eq!(
        seen,
        cases.len(),
        "the driver answered {seen} of {} cases",
        cases.len()
    );
    assert!(
        failures.is_empty(),
        "{} of {} cases failed:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
    eprintln!(
        "vectors: {total} cases, {partial} partial (TypeScript only) skipped, {} run on Java and pass",
        cases.len()
    );
}

const BOOKINGS: &str = r#"model Booking {
  start: datetime
  end: datetime
  rule endAfterStart: end > start
    message "A booking must end after it starts"
}

model Window {
  lo: int
  hi: int
  rule ordered: lo <= hi
}

error Overbooked 409 {
  seats: int
  limit: int
  rule within: seats <= limit
}

service Bookings at "/bookings" {
  post "/" create(body: Booking) -> 201 Booking
  get "/bad" bad() -> Booking
  get "/over/ok" overOk() -> 204 | Overbooked
  get "/over/bad" overBad() -> 204 | Overbooked
  get "/search" search(query: Window) -> 204
}
"#;

fn bookings_ir() -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "bk".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "bk.veld".into(),
                text: BOOKINGS.into(),
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    assert!(
        out.ir.is_some(),
        "bookings do not compile:\n{}",
        out.render()
    );
    out.ir.unwrap()
}

fn write_java(work: &std::path::Path, files: &veld_codegen::FileSet, sources: &mut Vec<PathBuf>) {
    for (path, bytes) in files.iter() {
        let target = work.join("src").join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, bytes).unwrap();
        if path.ends_with(".java") {
            sources.push(target);
        }
    }
}

#[test]
fn invariants_round_trip_over_http_on_the_jdk_server() {
    if !require_javac("invariants_round_trip_over_http_on_the_jdk_server") {
        return;
    }
    let ir = bookings_ir();
    let work = tempfile::tempdir().unwrap();
    let mut sources = Vec::new();
    for (role, base) in [(Role::Server, "gen.rs"), (Role::Client, "gen.rc")] {
        let files = run(
            &ir,
            role,
            all_services(&ir),
            vec![],
            None,
            options(base, None),
        );
        write_java(work.path(), &files, &mut sources);
    }
    let java = repo_root().join("tests/toolchain/java");
    sources.push(java.join("Check.java"));
    sources.push(java.join("rules/RuleRoundtrip.java"));
    let list: Vec<String> = sources
        .iter()
        .map(|p| p.display().to_string().replace('\\', "/"))
        .collect();
    std::fs::write(work.path().join("sources.args"), list.join("\n")).unwrap();
    let out = Command::new("javac")
        .args([
            "--release",
            "17",
            "-Xlint:all",
            "-Werror",
            "-encoding",
            "UTF-8",
        ])
        .args(["-d", "classes", "@sources.args"])
        .current_dir(work.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "javac failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let out = Command::new("java")
        .args(["-cp", "classes", "veldtest.RuleRoundtrip"])
        .current_dir(work.path())
        .output()
        .unwrap();
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains("OK"),
        "round trip failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// One HTTP/1.1 exchange on a raw socket: the status and the raw text of the response.
fn raw_http(port: u16, method: &str, path: &str, body: &str) -> Option<(u16, String)> {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).ok()?;
    s.set_read_timeout(Some(std::time::Duration::from_secs(20)))
        .ok()?;
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    s.write_all(request.as_bytes()).ok()?;
    let mut raw = String::new();
    s.read_to_string(&mut raw).ok()?;
    let status = raw.split_whitespace().nth(1)?.parse().ok()?;
    Some((status, raw))
}

/// The generated Spring adapter in a real Spring Boot 3 application, for a contract with rules:
/// step 5 (`422` with `violations`, for a body and for a `query` object) and step 8 (`500`).
/// Needs the Maven Wrapper and Spring from the network or `~/.m2`, like the toolchain test of the
/// same name.
#[test]
fn invariants_are_enforced_through_the_spring_adapter() {
    // The same gate as `spring_adapter_serves_the_round_trip`: Spring already in `~/.m2`, or a
    // route to Maven Central so the Maven Wrapper can fetch it (a fresh CI machine).
    let have_maven = {
        use std::net::{TcpStream, ToSocketAddrs};
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
        let cached = home.is_some_and(|h| {
            PathBuf::from(h)
                .join(".m2/repository/org/springframework/boot/spring-boot-starter-web")
                .is_dir()
        });
        cached
            || "repo.maven.apache.org:443"
                .to_socket_addrs()
                .ok()
                .and_then(|mut a| a.next())
                .is_some_and(|addr| {
                    TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(10)).is_ok()
                })
    };
    if !require_javac("invariants_are_enforced_through_the_spring_adapter") {
        return;
    }
    if !have_maven {
        assert!(
            std::env::var("VELD_REQUIRE_TOOLCHAINS").as_deref() != Ok("1"),
            "toolchain required but Spring is not in ~/.m2 and Maven Central is not reachable"
        );
        eprintln!(
            "SKIPPED the Spring rules test: no Spring in ~/.m2 and no route to Maven Central"
        );
        return;
    }
    let ir = bookings_ir();
    let java = repo_root().join("tests/toolchain/java");
    let work = java.join(".work/spring-rules");
    let _ = std::fs::remove_dir_all(&work);
    let sources = work.join("src/main/java");
    let files = run(
        &ir,
        Role::Server,
        all_services(&ir),
        vec![],
        Some("spring"),
        options("gen.rs", Some("17")),
    );
    for (path, bytes) in files.iter().filter(|(p, _)| p.ends_with(".java")) {
        let target = sources.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, bytes).unwrap();
    }
    std::fs::create_dir_all(sources.join("veldtest")).unwrap();
    std::fs::copy(
        java.join("rules/spring/App.java"),
        sources.join("veldtest/App.java"),
    )
    .unwrap();
    for name in ["pom.xml", "mvnw", "mvnw.cmd"] {
        std::fs::copy(java.join("spring").join(name), work.join(name)).unwrap();
    }
    copy_dir(&java.join("spring/.mvn"), &work.join(".mvn"));
    mvnw_package(&work, "17");

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
        .current_dir(&work);
    let mut server =
        veld_test_support::ProcessTree::spawn_logged(&mut app, &work.join("app.log")).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    while raw_http(port, "GET", "/bookings/over/ok", "").is_none() {
        assert!(
            server.child_mut().try_wait().unwrap().is_none(),
            "the application exited"
        );
        assert!(
            std::time::Instant::now() < deadline,
            "the application never answered"
        );
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    let bad = r#"{"start":"2026-01-02T00:00:00Z","end":"2026-01-01T00:00:00Z"}"#;
    let (status, text) = raw_http(port, "POST", "/bookings", bad).unwrap();
    assert_eq!(status, 422, "{text}");
    assert!(
        text.contains("\"violations\":[{\"rule\":\"veld:bk/Booking.endAfterStart@"),
        "{text}"
    );
    assert!(text.contains("\"code\":\"rule\""), "{text}");
    let good = r#"{"start":"2026-01-01T00:00:00Z","end":"2026-01-02T00:00:00Z"}"#;
    assert_eq!(raw_http(port, "POST", "/bookings", good).unwrap().0, 201);
    let (status, text) = raw_http(port, "GET", "/bookings/search?lo=5&hi=1", "").unwrap();
    assert_eq!(status, 422, "{text}");
    assert!(text.contains("\"path\":\"query\""), "{text}");
    assert_eq!(
        raw_http(port, "GET", "/bookings/search?lo=1&hi=5", "")
            .unwrap()
            .0,
        204
    );
    assert_eq!(raw_http(port, "GET", "/bookings/bad", "").unwrap().0, 500);
    assert_eq!(
        raw_http(port, "GET", "/bookings/over/bad", "").unwrap().0,
        500
    );
    assert_eq!(
        raw_http(port, "GET", "/bookings/over/ok", "").unwrap().0,
        409
    );
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
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
