//! The shared rule vectors (`spec/rules.md` section 15, `tests/rules/core`) against the generated
//! Java **server**: what reaches `onRuleViolation`.
//!
//! The client run (`rules_toolchain.rs`) checks values through generated code. A server also
//! reports, so a generator that never calls the hook for an audited rule would pass every vector
//! there. Here each case goes through the generated handler of a service `Vectors` that has, for
//! every model of the file, an endpoint that takes it (`/v/in/<Type>`, step 5) and one that answers
//! with it (`/v/out/<Type>`, step 8). A driver (`tests/toolchain/java/rules/RuleVectorsServer.java`)
//! records the events of the hook, and this test compares the `REJECTED` ones with `expect` and the
//! `AUDITED` ones with `audited` of the case, as sets, and the status of the answer with what
//! `expect` says. Every file is its own package: two files may declare a model of the same name.
//!
//! Needs a JDK; `VELD_REQUIRE_TOOLCHAINS=1` makes a missing one a failure.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::live::*;
use common::*;
use veld_codegen::Role;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_emit_java::tree::Tree;
use veld_emit_java::util::type_name;
use veld_ir::{DeclKind, Ir, PackageKind};
use veld_rules::vectors::{Case, Produced, compare_produced, compare_produced_audited, load_cases};

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

/// The contract with the service that gives every model an endpoint that takes it and one that
/// answers with it.
fn compile_group(name: &str, source: &str) -> Ir {
    let mut text = source.replace("\r\n", "\n");
    text.push_str("\nservice Vectors at \"/v\" {\n");
    for m in model_names(source) {
        text.push_str(&format!("  post \"/in/{m}\" in{m}(body: {m}) -> 204\n"));
        text.push_str(&format!("  get \"/out/{m}\" out{m}() -> {m}\n"));
    }
    text.push_str("}\n");
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
        consumes: BTreeMap::new(),
    });
    assert!(
        out.ir.is_some(),
        "{name}: the contract failed to compile:\n{}",
        out.render()
    );
    out.ir.unwrap_or_else(|| unreachable!())
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

fn judge(name: &str, line: &serde_json::Value, case: &Case) -> Result<(), String> {
    if let Some(e) = line.get("error") {
        return Err(format!("the driver threw {e}"));
    }
    let produced = |key: &str| -> Vec<Produced> {
        serde_json::from_value(line[key].clone()).unwrap_or_default()
    };
    compare_produced(case, &produced("violations")).map_err(|m| m.to_string())?;
    compare_produced_audited(case, &produced("audited")).map_err(|m| m.to_string())?;
    let status = line["status"].as_u64().unwrap_or(0);
    if status != u64::from(wanted_status(case)) {
        return Err(format!(
            "case `{}` of {name}: the answer is HTTP {status} {}, wanted {}",
            case.name,
            line["body"].as_str().unwrap_or(""),
            wanted_status(case)
        ));
    }
    Ok(())
}

/// Runs one vector file through the generated server; returns how many cases passed and how many
/// of them have audited reports.
fn run_group(stem: &str) -> (usize, usize) {
    let source = std::fs::read_to_string(core_dir().join(format!("{stem}.veld"))).unwrap();
    let cases: Vec<Case> =
        load_cases(&std::fs::read_to_string(core_dir().join(format!("{stem}.json"))).unwrap())
            .unwrap()
            .into_iter()
            .filter(|c| !c.partial)
            .collect();
    let ir = compile_group(stem, &source);
    let all = ir.services.iter().map(|s| s.id).collect();
    let files = run(
        &ir,
        Role::Server,
        all,
        vec![],
        None,
        options("gen.v", Some("17")),
    );
    let work = tempfile::tempdir().unwrap();
    let mut sources = write_files(work.path(), &files);
    sources.push(repo_root().join("tests/toolchain/java/rules/RuleVectorsServer.java"));
    javac_ok(work.path(), &sources, "17");
    let mut lines = String::new();
    for (id, c) in cases.iter().enumerate() {
        lines.push_str(
            &serde_json::json!({
                "id": id, "type": c.ty, "value": c.value, "request": c.request, "output": c.output
            })
            .to_string(),
        );
        lines.push('\n');
    }
    std::fs::write(work.path().join("cases.jsonl"), lines).unwrap();
    let tree = Tree::new(
        &ir,
        "gen.v",
        "gen.v.runtime",
        ir.services.iter().map(|s| s.id).collect(),
    );
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
    std::fs::write(work.path().join("types.tsv"), types).unwrap();
    let out = std::process::Command::new("java")
        .args([
            "-cp",
            "classes",
            "veldtest.RuleVectorsServer",
            "cases.jsonl",
            "types.tsv",
        ])
        .current_dir(work.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{stem}: the driver failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let answers: Vec<serde_json::Value> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{l}: {e}")))
        .collect();
    assert_eq!(answers.len(), cases.len(), "{stem}: one line per case");
    let (mut passed, mut audited, mut failures) = (0, 0, Vec::new());
    for line in &answers {
        let case = &cases[line["id"].as_u64().unwrap() as usize];
        audited += usize::from(!case.audited.is_empty());
        match judge(stem, line, case) {
            Ok(()) => passed += 1,
            Err(m) => failures.push(format!("{stem} / {m}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    (passed, audited)
}

#[test]
fn every_core_vector_gives_the_reference_result_on_the_java_server() {
    if !require_javac("every_core_vector_gives_the_reference_result_on_the_java_server") {
        return;
    }
    let mut stems: Vec<String> = std::fs::read_dir(core_dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "veld"))
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    stems.sort();
    let (mut passed, mut audited) = (0, 0);
    for stem in &stems {
        let (p, a) = run_group(stem);
        passed += p;
        audited += a;
    }
    eprintln!("server vectors: {passed} passed, {audited} with audited reports");
    assert!(
        audited >= 8,
        "the vectors of audited rules did not run: {audited}"
    );
}
