//! The shared rule vectors (`spec/rules.md` section 15, `tests/rules/`) against generated TypeScript.
//!
//! For every `tests/rules/**/*.veld` the contract is compiled, the TypeScript client is generated,
//! and a checked-in driver (`tests/toolchain/ts/rules_vectors.ts`) is compiled with it under
//! `tsc --strict`. The driver decodes each case's `value` from wire form, runs the check in the
//! case's mode (full, request or partial) and prints one JSON line per case. Those lines are
//! compared with the expected violations by `veld_rules::vectors::compare_produced`.
//!
//! Without `node` or `node_modules` the test is skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::*;
use serde::Deserialize;
use serde_json::json;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_project::output::{WriteOptions, write};
use veld_rules::vectors::{Case, Produced, compare_produced, load_cases};

/// A service that makes the contract generatable: its package's models are all emitted.
const PROBE: &str = "service Probe at \"/probe\" {\n  get \"/ping\" ping() -> string\n}\n";

/// One vector group: `tests/rules/<group>/<name>.veld` with its cases.
struct Group {
    name: String,
    source: String,
    cases: Vec<Case>,
    cases_path: PathBuf,
}

fn rules_dir() -> PathBuf {
    repo_root().join("tests/rules")
}

/// Every `<group>/<name>.veld` that has a `<name>.json` next to it, in a fixed order.
fn load_groups() -> Vec<Group> {
    let mut groups = Vec::new();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(rules_dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "veld"))
            .collect();
        files.sort();
        for veld in files {
            let cases_path = veld.with_extension("json");
            let text = std::fs::read_to_string(&cases_path)
                .unwrap_or_else(|e| panic!("{}: {e}", cases_path.display()));
            let stem = veld.file_stem().unwrap().to_string_lossy().into_owned();
            let group = dir.file_name().unwrap().to_string_lossy().into_owned();
            groups.push(Group {
                name: format!("{group}_{stem}"),
                source: std::fs::read_to_string(&veld).unwrap(),
                cases: load_cases(&text)
                    .unwrap_or_else(|e| panic!("{}: {e}", cases_path.display())),
                cases_path,
            });
        }
    }
    groups
}

/// Compiles a vector contract as package `t`, with the probe service.
fn compile_group(group: &Group) -> Ir {
    let files = vec![
        SourceFile {
            path: "vectors.veld".into(),
            text: group.source.clone(),
        },
        SourceFile {
            path: "probe.veld".into(),
            text: PROBE.into(),
        },
    ];
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
        "{}: the contract failed to compile:\n{}",
        group.name,
        out.render()
    );
    out.ir.unwrap_or_else(|| unreachable!())
}

fn toolchain_dir() -> PathBuf {
    repo_root().join("tests/toolchain/ts")
}

fn locate_tsc() -> Result<PathBuf, String> {
    let node_ok = Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !node_ok {
        return Err("`node` is not on PATH".into());
    }
    let tsc = toolchain_dir().join("node_modules/typescript/bin/tsc");
    if !tsc.exists() {
        return Err(format!("{} is missing", tsc.display()));
    }
    Ok(tsc)
}

/// One line of the driver's output.
#[derive(Deserialize)]
struct Line {
    name: String,
    #[serde(default)]
    violations: Vec<Produced>,
    #[serde(default)]
    error: Option<String>,
}

fn run_driver(work: &Path, group: &Group) -> Vec<Line> {
    let out = Command::new("node")
        .arg(format!("out/rules_vectors_{}.js", group.name))
        .arg(&group.cases_path)
        .current_dir(work)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}: the driver failed\n{}",
        group.name,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str::<Line>(l).unwrap_or_else(|e| panic!("{l}: {e}")))
        .collect()
}

fn vectors_pass(tsc_path: &Path, edition: &str, target: &str) {
    let groups = load_groups();
    assert!(!groups.is_empty(), "no vectors under tests/rules");
    std::fs::create_dir_all(toolchain_dir().join(".work")).unwrap();
    let guard = tempfile::Builder::new()
        .prefix("rules-")
        .tempdir_in(toolchain_dir().join(".work"))
        .unwrap();
    let work = guard.path();
    let driver = std::fs::read_to_string(toolchain_dir().join("rules_vectors.ts")).unwrap();
    for group in &groups {
        let files = gen_client_with(&compile_group(group), &typescript_option(edition));
        let dir = format!("gen_{}", group.name);
        write(&work.join(&dir), &files, &WriteOptions::default()).unwrap();
        let text = driver.replace("./gen/", &format!("./{dir}/"));
        std::fs::write(work.join(format!("rules_vectors_{}.ts", group.name)), text).unwrap();
    }
    std::fs::write(
        work.join("tsconfig.json"),
        serde_json::to_string_pretty(&json!({
            "compilerOptions": {
                "strict": true, "target": target, "lib": [target], "types": ["node"],
                "module": "commonjs", "moduleResolution": "node10", "esModuleInterop": true,
                "skipLibCheck": true, "outDir": "out", "rootDir": "."
            },
            "include": ["*.ts", "gen_*/**/*.ts"]
        }))
        .unwrap(),
    )
    .unwrap();
    let tsc = Command::new("node")
        .arg(tsc_path)
        .args(["-p", "tsconfig.json"])
        .current_dir(work)
        .output()
        .unwrap();
    assert!(
        tsc.status.success(),
        "tsc failed:\n{}{}",
        String::from_utf8_lossy(&tsc.stdout),
        String::from_utf8_lossy(&tsc.stderr)
    );
    let (mut passed, mut failures) = (0usize, Vec::new());
    for group in &groups {
        let lines = run_driver(work, group);
        assert_eq!(lines.len(), group.cases.len(), "{}", group.name);
        for (case, line) in group.cases.iter().zip(&lines) {
            assert_eq!(case.name, line.name, "{}: lines out of order", group.name);
            if let Some(e) = &line.error {
                failures.push(format!(
                    "{} / {}: the driver threw {e}",
                    group.name, case.name
                ));
            } else if let Err(m) = compare_produced(case, &line.violations) {
                failures.push(format!("{} / {m}", group.name));
            } else {
                passed += 1;
            }
        }
    }
    eprintln!("rule vectors ({edition}): {passed} passed");
    assert!(
        failures.is_empty(),
        "{} failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn every_rule_vector_gives_the_reference_result_in_typescript() {
    let tsc_path = match locate_tsc() {
        Ok(p) => p,
        Err(why) => {
            assert!(
                std::env::var("VELD_REQUIRE_TOOLCHAINS").as_deref() != Ok("1"),
                "toolchain required but unavailable: {why}"
            );
            eprintln!("SKIPPED rule vectors: {why}");
            return;
        }
    };
    for (edition, target) in TARGETS {
        vectors_pass(&tsc_path, edition, target);
    }
}
