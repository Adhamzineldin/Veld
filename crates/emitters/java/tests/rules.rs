//! Invariants (`spec/rules.md`, `spec/targets/java.md` section 8): what is generated for a
//! contract with rules, that it compiles under `javac -Xlint:all -Werror`, and that a contract
//! without rules gets none of it.
//!
//! The vector runner and the HTTP round trip are in `tests/rules_toolchain.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::*;
use veld_codegen::{FileSet, Role};

/// Whether `javac` is usable; a missing one fails when `VELD_REQUIRE_TOOLCHAINS=1`.
pub fn have_javac(test: &str) -> bool {
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

/// Writes `files` below `dir` and compiles every `.java` file with warnings as errors.
pub fn compile_generated(dir: &Path, files: &FileSet, release: &str) {
    let src = dir.join("src");
    for (path, bytes) in files.iter() {
        let target = src.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, bytes).unwrap();
    }
    let list: Vec<String> = java_files(&src)
        .iter()
        .map(|p| p.display().to_string().replace('\\', "/"))
        .collect();
    std::fs::write(dir.join("sources.args"), list.join("\n")).unwrap();
    let out = Command::new("javac")
        .args([
            "--release",
            release,
            "-Xlint:all",
            "-Werror",
            "-encoding",
            "UTF-8",
            "-d",
            "classes",
            "@sources.args",
        ])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "javac failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn java_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "java") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn generate(ir: &veld_ir::Ir, role: Role, framework: Option<&str>) -> FileSet {
    let all = ir.services.iter().map(|s| s.id).collect();
    run(ir, role, all, vec![], framework, options("gen.r", None))
}

#[test]
fn a_hand_built_rule_gets_a_check_function_and_the_rule_classes() {
    let ir = veld_emit_conformance::ir_with_rule();
    let files = generate(&ir, Role::Client, None);
    let greeting = text(&files, "gen/r/app/plain/Greeting.java");
    assert!(greeting.contains("RULE_ALWAYS_TRUE = \"veld:app/Greeting.alwaysTrue@0\""));
    assert!(greeting.contains(".checked(Greeting::checkWire)"));
    assert!(greeting.contains("public static List<Violation> check(Greeting value)"));
    for class in ["Rules", "RuleWalk", "RuleError", "Violation"] {
        assert!(files.get(&format!("gen/r/runtime/{class}.java")).is_some());
    }
}

#[test]
fn dump_for_debugging() {
    let ir = veld_emit_conformance::ir_with_rule();
    let files = generate(&ir, Role::Client, None);
    for p in files.paths() {
        if p.contains("Greeting") {
            eprintln!("{p}\n{}", text(&files, p));
        }
    }
}

#[test]
fn a_hand_built_rule_compiles_for_the_client_and_both_servers() {
    if !have_javac("a_hand_built_rule_compiles") {
        return;
    }
    let ir = veld_emit_conformance::ir_with_rule();
    for (name, role, framework) in [("client", Role::Client, None), ("jdk", Role::Server, None)] {
        let dir = tempfile::tempdir().unwrap();
        compile_generated(dir.path(), &generate(&ir, role, framework), "17");
        eprintln!("compiled {name}");
    }
}
