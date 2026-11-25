//! Invariants in generated Rust (`spec/rules.md`, `spec/targets/rust.md` section 10): the shared
//! vectors of `tests/rules/core` run through generated code, and a snapshot of a contract with
//! rules.
//!
//! One scratch workspace holds a generated crate for every vector contract plus a driver binary
//! (`src/bin/vectors.rs`, written by this test from the names in the IR, because Rust has no
//! reflection). The driver reads a case file and prints one JSON line of violations per case; the
//! lines are compared with `veld_rules::vectors::compare_produced`.
//!
//! Without `cargo` the test is skipped, unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::*;
use serde_json::json;
use veld_codegen::{FileSet, Role};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_emit_rust::tree::Tree;
use veld_ir::{DeclKind, Ir, PackageKind};
use veld_rules::vectors::{Case, Produced, compare_produced, load_cases};

const CONTRACTS: [&str; 8] = [
    "decimal", "lists", "logic", "nested", "numbers", "optional", "strings", "time",
];

fn core_dir() -> PathBuf {
    repo_root().join("tests/rules/core")
}

fn work_dir() -> PathBuf {
    std::env::var("VELD_RUST_WORK").map_or_else(
        |_| repo_root().join("tests/toolchain/rust/.work"),
        PathBuf::from,
    )
}

fn target_dir() -> PathBuf {
    std::env::var("VELD_RUST_TARGET").map_or_else(|_| work_dir().join("target"), PathBuf::from)
}

fn have_cargo() -> bool {
    Command::new("cargo")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The contract with a stub service appended, so that the generator has something to serve.
fn vector_ir(name: &str) -> Ir {
    let path = core_dir().join(format!("{name}.veld"));
    let mut text = std::fs::read_to_string(&path).unwrap();
    let first = text
        .lines()
        .find_map(|l| l.strip_prefix("model "))
        .and_then(|l| l.split_whitespace().next())
        .unwrap()
        .to_string();
    text.push_str(&format!(
        "\nservice Probe at \"/probe\" {{\n  get \"/p\" probe() -> {first}\n}}\n"
    ));
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: format!("{name}.veld"),
                text,
            }],
            base_url: None,
        }],
        consumes: std::collections::BTreeMap::new(),
    });
    assert!(out.ir.is_some(), "{name}: {}", out.render());
    out.ir.unwrap()
}

fn crate_ident(name: &str) -> String {
    format!("vec_{name}")
}

/// The driver: one `match` arm per model and error of the contract.
fn driver(ir: &Ir, name: &str) -> String {
    let tree = Tree::new(
        ir,
        "crate",
        "src",
        ir.services.iter().map(|s| s.id).collect(),
        true,
    );
    let krate = crate_ident(name);
    let mut arms = String::new();
    for id in tree.decls() {
        let d = ir.decl(*id);
        if matches!(
            d.kind,
            DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Union(_)
        ) && !d.name.is_empty()
            && tree.module_path(d) != "crate::prelude"
        {
            let path = tree.type_path(*id, "").replacen("crate", &krate, 1);
            arms.push_str(&format!(
                "        {:?} => run::<{path}>(value, request),\n",
                d.name
            ));
        }
    }
    format!(
        r#"use {krate}::veld::{{self, VeldDecl}};
use serde_json::Value;

fn run<T: VeldDecl>(value: &Value, request: bool) -> String {{
    let issues = T::schema().validate(value, "$", request);
    if !issues.is_empty() {{
        return format!("invalid {{}}", issues.len());
    }}
    let mode = if request {{ veld::rules::Mode::Request }} else {{ veld::rules::Mode::Full }};
    let found = veld::rules::check_wire(T::schema(), value, "$", mode);
    let items: Vec<String> = found.iter().map(veld::rules::Violation::to_text).collect();
    format!("[{{}}]", items.join(","))
}}

fn main() {{
    let path = std::env::args().nth(1).expect("a case file");
    let text = std::fs::read_to_string(path).expect("readable");
    let cases: Vec<Value> = serde_json::from_str(&text).expect("json");
    for case in cases {{
        let value = &case["value"];
        let request = case["request"].as_bool().unwrap_or(false);
        let out = match case["type"].as_str().unwrap_or("") {{
{arms}            _ => "{{\"unknown\":0}}".to_string(),
        }};
        println!("{{out}}");
    }}
}}
"#
    )
}

fn crate_files(ir: &Ir, name: &str) -> FileSet {
    let all = ir.services.iter().map(|s| s.id).collect();
    let mut files = run(
        ir,
        Role::Client,
        all,
        vec![],
        Some("ureq"),
        json!({ "crate-name": format!("vec-{name}") }),
    );
    files
        .add(&format!("src/bin/vectors_{name}.rs"), driver(ir, name))
        .unwrap();
    files
}

fn cargo(ws: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new("cargo");
    cmd.args(args)
        .current_dir(ws)
        .env("CARGO_TARGET_DIR", target_dir())
        .env("CARGO_TERM_COLOR", "never");
    cmd
}

fn run_ok(cmd: &mut Command, what: &str) -> String {
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "{what} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
            .lines()
            .filter(|l| !l.contains("collision") && !l.starts_with("  ="))
            .take(120)
            .collect::<Vec<_>>()
            .join("\n")
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn every_vector_passes_through_generated_rust() {
    if !have_cargo() {
        assert!(
            std::env::var("VELD_REQUIRE_TOOLCHAINS").as_deref() != Ok("1"),
            "toolchain required but `cargo` is unavailable"
        );
        eprintln!("SKIPPED: `cargo` is not on PATH");
        return;
    }
    let ws = work_dir().join("rules-ws");
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(&ws).unwrap();
    let mut members = Vec::new();
    for name in CONTRACTS {
        write_files(&ws.join(name), &crate_files(&vector_ir(name), name));
        members.push(format!("\"{name}\""));
    }
    std::fs::write(
        ws.join("Cargo.toml"),
        format!(
            "[workspace]\nresolver = \"2\"\nmembers = [{}]\n",
            members.join(", ")
        ),
    )
    .unwrap();
    let lock = repo_root().join("tests/toolchain/rust/Cargo.lock");
    if lock.exists() {
        std::fs::copy(&lock, ws.join("Cargo.lock")).unwrap();
    }
    run_ok(
        &mut cargo(&ws, &["build", "--workspace", "--bins"]),
        "cargo build",
    );
    run_ok(
        &mut cargo(
            &ws,
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        ),
        "cargo clippy",
    );
    let mut passed = 0;
    for name in CONTRACTS {
        passed += run_vectors(&ws, name);
    }
    eprintln!("{passed} vector cases passed through generated Rust");
    assert!(passed > 90, "only {passed} cases ran");
}

/// Runs the non-partial cases of one contract; returns how many passed.
fn run_vectors(ws: &Path, name: &str) -> usize {
    let text = std::fs::read_to_string(core_dir().join(format!("{name}.json"))).unwrap();
    let cases: Vec<Case> = load_cases(&text)
        .unwrap()
        .into_iter()
        .filter(|c| !c.partial)
        .collect();
    let input: Vec<serde_json::Value> = cases
        .iter()
        .map(|c| json!({ "type": c.ty, "value": c.value, "request": c.request }))
        .collect();
    let file = ws.join(format!("{name}.cases.json"));
    std::fs::write(&file, serde_json::to_string(&input).unwrap()).unwrap();
    let out = run_ok(
        &mut cargo(
            ws,
            &[
                "run",
                "-q",
                "-p",
                &format!("vec-{name}"),
                "--bin",
                &format!("vectors_{name}"),
                "--",
                &file.to_string_lossy(),
            ],
        ),
        &format!("vectors of {name}"),
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), cases.len(), "{name}: one line per case");
    let mut failures = Vec::new();
    for (case, line) in cases.iter().zip(&lines) {
        match serde_json::from_str::<Vec<Produced>>(line) {
            Ok(produced) => {
                if let Err(e) = compare_produced(case, &produced) {
                    failures.push(format!("{name}: {e}"));
                }
            }
            Err(_) => failures.push(format!("{name}: case `{}` produced {line}", case.name)),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    cases.len()
}
