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

fn rules_dir() -> PathBuf {
    repo_root().join("tests/rules/core")
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
    for path in sorted(&rules_dir(), "veld") {
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
    for path in sorted(&rules_dir(), "json") {
        for case in load_cases(&std::fs::read_to_string(&path).unwrap()).unwrap() {
            if case.partial {
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
    eprintln!("{} vector cases pass on Java", cases.len());
}

const BOOKINGS: &str = r#"model Booking {
  start: datetime
  end: datetime
  rule endAfterStart: end > start
    message "A booking must end after it starts"
}

service Bookings at "/bookings" {
  post "/" create(body: Booking) -> 201 Booking
  get "/bad" bad() -> Booking
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
