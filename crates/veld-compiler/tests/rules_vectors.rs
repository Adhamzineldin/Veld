//! Every contract under `tests/rules/` compiles without errors, and the `fields` of its test
//! vectors agree with the `reads` the compiler computes (`spec/rules.md` sections 7 and 15).
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/rules")
}

fn veld_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            veld_files(&p, out);
        } else if p.extension().is_some_and(|e| e == "veld") {
            out.push(p);
        }
    }
}

fn compile_one(path: &Path) -> veld_compiler::CompileOutput {
    let text = fs::read_to_string(path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "t.veld".into(),
                text,
            }],
            base_url: None,
        }],
        consumes: Default::default(),
    })
}

#[test]
fn every_rules_contract_compiles_without_errors() {
    let mut files = Vec::new();
    veld_files(&root(), &mut files);
    assert!(files.len() >= 8, "found only {} contracts", files.len());
    for f in files {
        let out = compile_one(&f);
        let errors: Vec<String> = out
            .diagnostics
            .iter()
            .filter(|d| d.is_error())
            .map(|d| d.code.to_string())
            .collect();
        assert!(
            errors.is_empty() && out.ir.is_some(),
            "{} has errors {errors:?}:\n{}",
            f.display(),
            out.render()
        );
    }
}

fn reads_of(ir: &Ir, model: &str, rule: &str) -> Option<Vec<String>> {
    let decl = ir.decls.iter().find(|d| d.name == model)?;
    let r = decl.rules().iter().find(|r| r.name == rule)?;
    Some(r.reads.clone())
}

#[test]
fn vector_fields_match_the_reads_of_each_rule() {
    let mut files = Vec::new();
    veld_files(&root(), &mut files);
    let mut checked = 0;
    for f in files {
        let Ok(json) = fs::read_to_string(f.with_extension("json")) else {
            continue;
        };
        let v: Value = serde_json::from_str(&json).unwrap();
        let ir = compile_one(&f).ir.expect("compiles");
        for case in v["cases"].as_array().into_iter().flatten() {
            let model = case["type"].as_str().unwrap_or_default();
            for exp in case["expect"].as_array().into_iter().flatten() {
                if exp["path"].as_str() != Some("$") {
                    continue;
                }
                let rule = exp["rule"].as_str().unwrap_or_default();
                let want: Vec<String> = exp["fields"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect();
                let got = reads_of(&ir, model, rule);
                assert_eq!(
                    got.as_ref(),
                    Some(&want),
                    "{}: {model}.{rule} (case `{}`)",
                    f.display(),
                    case["name"]
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 20, "only {checked} expectations compared");
}
