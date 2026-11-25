//! The shared rule vectors (`spec/rules.md` section 15, `tests/rules/core`) against the generated
//! Node **server**: what reaches `onRuleViolation`.
//!
//! The client run (`rules_vectors.rs`) checks values with `checkValue`. A server also reports, so
//! a generator that never calls the hook for an audited rule would pass every vector there. Here
//! each case goes through the generated handler of a service `Vectors` that has, for every model of
//! the file, an endpoint that takes it (`/v/in/<Type>`, step 5) and one that answers with it
//! (`/v/out/<Type>`, step 8). A driver (`tests/toolchain/ts/rules_vectors_server.ts`) records the
//! events of the hook, and this test compares the `rejected` ones with `expect` and the `audited`
//! ones with `audited` of the case, as sets, and the status of the answer with what `expect` says.
//!
//! Without `node` or `node_modules` the test is skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::toolchain::{toolchain_dir, tsc_path};
use common::*;
use serde::Deserialize;
use serde_json::json;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_project::output::{WriteOptions, write};
use veld_rules::vectors::{Case, Produced, compare_produced, compare_produced_audited, load_cases};

/// One vector group: `tests/rules/core/<name>.veld` with its cases.
struct Group {
    name: String,
    source: String,
    models: Vec<String>,
    cases: Vec<Case>,
    cases_path: PathBuf,
}

fn core_dir() -> PathBuf {
    repo_root().join("tests/rules/core")
}

fn model_names(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in source.lines() {
        let mut words = line.split_whitespace();
        if let (Some("model" | "union"), Some(name)) = (words.next(), words.next()) {
            names.push(name.trim_end_matches('{').to_owned());
        }
    }
    names
}

fn load_groups() -> Vec<Group> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(core_dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "veld"))
        .collect();
    files.sort();
    let mut groups = Vec::new();
    for veld in files {
        let cases_path = veld.with_extension("json");
        let text = std::fs::read_to_string(&cases_path).unwrap();
        let source = std::fs::read_to_string(&veld).unwrap();
        groups.push(Group {
            name: veld.file_stem().unwrap().to_string_lossy().into_owned(),
            models: model_names(&source),
            source,
            cases: load_cases(&text).unwrap(),
            cases_path,
        });
    }
    groups
}

/// The service that gives every model an endpoint that takes it and one that answers with it.
fn service_for(models: &[String]) -> String {
    let mut out = String::from("\nservice Vectors at \"/v\" {\n");
    for m in models {
        out.push_str(&format!("  post \"/in/{m}\" in{m}(body: {m}) -> 204\n"));
        out.push_str(&format!("  get \"/out/{m}\" out{m}() -> {m}\n"));
    }
    out.push_str("}\n");
    out
}

fn compile_group(group: &Group) -> Ir {
    let text = format!("{}{}", group.source, service_for(&group.models));
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "vectors.veld".into(),
                text,
            }],
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

/// One line of the driver's output.
#[derive(Deserialize)]
struct Line {
    name: String,
    #[serde(default)]
    status: u16,
    #[serde(default)]
    violations: Vec<Produced>,
    #[serde(default)]
    audited: Vec<Produced>,
    #[serde(default)]
    error: Option<String>,
}

fn run_driver(work: &Path, group: &Group) -> Vec<Line> {
    let out = Command::new("node")
        .arg("out/rules_vectors_server.js")
        .arg(&group.cases_path)
        .current_dir(work.join(format!("run_{}", group.name)))
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

/// The status of the answer to a case: what `expect` says it must be.
fn wanted_status(case: &Case) -> u16 {
    match (case.request, case.expect.is_empty()) {
        (true, false) => 422,
        (true, true) => 204,
        (false, false) => 500,
        (false, true) => 200,
    }
}

/// Compiles the generated servers and the driver of every group, one directory per group, so the
/// models of one file never meet those of another.
fn build_all(tsc_path: &Path, groups: &[Group], edition: &str, target: &str, work: &Path) {
    let driver = std::fs::read_to_string(toolchain_dir().join("rules_vectors_server.ts")).unwrap();
    for group in groups {
        let dir = work.join(format!("run_{}", group.name));
        let files = gen_server_with(
            &compile_group(group),
            "Vectors",
            &[],
            &typescript_option(edition),
        );
        write(&dir.join("gen"), &files, &WriteOptions::default()).unwrap();
        std::fs::write(dir.join("rules_vectors_server.ts"), &driver).unwrap();
        std::fs::write(
            dir.join("tsconfig.json"),
            serde_json::to_string_pretty(&json!({
                "compilerOptions": {
                    "strict": true, "target": target, "lib": [target], "types": ["node"],
                    "module": "commonjs", "moduleResolution": "node10", "esModuleInterop": true,
                    "skipLibCheck": true, "outDir": "out", "rootDir": "."
                },
                "include": ["*.ts", "gen/**/*.ts"]
            }))
            .unwrap(),
        )
        .unwrap();
        let tsc = Command::new("node")
            .arg(tsc_path)
            .args(["-p", "tsconfig.json"])
            .current_dir(&dir)
            .output()
            .unwrap();
        assert!(
            tsc.status.success(),
            "{}: tsc failed:\n{}{}",
            group.name,
            String::from_utf8_lossy(&tsc.stdout),
            String::from_utf8_lossy(&tsc.stderr)
        );
    }
}

fn judge(group: &Group, line: &Line, case: &Case) -> Result<(), String> {
    if let Some(e) = &line.error {
        return Err(format!("the driver threw {e}"));
    }
    compare_produced(case, &line.violations).map_err(|m| m.to_string())?;
    compare_produced_audited(case, &line.audited).map_err(|m| m.to_string())?;
    if line.status != wanted_status(case) {
        return Err(format!(
            "case `{}` of {}: the answer is HTTP {}, wanted {}",
            case.name,
            group.name,
            line.status,
            wanted_status(case)
        ));
    }
    Ok(())
}

fn vectors_pass(tsc_path: &Path, edition: &str, target: &str) {
    let groups = load_groups();
    assert!(!groups.is_empty(), "no vectors under tests/rules/core");
    std::fs::create_dir_all(toolchain_dir().join(".work")).unwrap();
    let guard = tempfile::Builder::new()
        .prefix("rules-server-")
        .tempdir_in(toolchain_dir().join(".work"))
        .unwrap();
    let work = guard.path();
    build_all(tsc_path, &groups, edition, target, work);
    let (mut passed, mut failures, mut audited_cases) = (0usize, Vec::new(), 0usize);
    for group in &groups {
        // The partial check is a client's: the driver leaves those cases out.
        let cases: Vec<&Case> = group.cases.iter().filter(|c| !c.partial).collect();
        let lines = run_driver(work, group);
        assert_eq!(lines.len(), cases.len(), "{}", group.name);
        for (case, line) in cases.iter().zip(&lines) {
            assert_eq!(case.name, line.name, "{}: lines out of order", group.name);
            audited_cases += usize::from(!case.audited.is_empty());
            match judge(group, line, case) {
                Ok(()) => passed += 1,
                Err(m) => failures.push(format!("{} / {m}", group.name)),
            }
        }
    }
    eprintln!("server vectors ({edition}): {passed} passed, {audited_cases} with audited reports");
    assert!(
        failures.is_empty(),
        "{} failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(
        audited_cases >= 8,
        "the vectors of audited rules did not run: {audited_cases}"
    );
}

#[test]
fn every_core_vector_gives_the_reference_result_on_the_node_server() {
    let Some(tsc_path) = tsc_path() else {
        return;
    };
    for (edition, target) in TARGETS {
        vectors_pass(&tsc_path, edition, target);
    }
}
