//! Migrates every v1 contract under `examples/*/veld` and compiles the result.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::PackageKind;
use veld_tools::migrate_v1;

fn examples_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

/// Reads `.veld` files below `dir`, skipping copies in `packages/` and
/// `node_modules/`. Paths are relative to `base`.
fn read_v1(dir: &Path, base: &Path, out: &mut Vec<(String, String)>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    entries.sort();
    for p in entries {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if p.is_dir() {
            if name != "packages" && name != "node_modules" {
                read_v1(&p, base, out);
            }
        } else if p.extension().is_some_and(|e| e == "veld") {
            let rel = p
                .strip_prefix(base)
                .map(|r| r.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            let text = fs::read_to_string(&p)
                .unwrap_or_default()
                .replace("\r\n", "\n");
            out.push((rel, text));
        }
    }
}

fn example_dirs() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(examples_root())
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path().join("veld"))
                .collect()
        })
        .unwrap_or_default();
    v.retain(|p| p.is_dir());
    v.sort();
    v
}

fn compile_errors(files: &[(String, String)]) -> Vec<String> {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "migrated".into(),
            kind: PackageKind::Service,
            files: files
                .iter()
                .map(|(p, t)| SourceFile {
                    path: p.clone(),
                    text: t.clone(),
                })
                .collect(),
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    out.diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| format!("{} {}", d.code, d.message))
        .collect()
}

#[test]
fn every_example_migrates_to_a_package_that_compiles() {
    let (mut files_total, mut files_ok) = (0, 0);
    for dir in example_dirs() {
        let mut files = Vec::new();
        read_v1(&dir, &dir, &mut files);
        let result = migrate_v1(&files);
        let errors = compile_errors(&result.files);
        let name = dir
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        files_total += files.len();
        if errors.is_empty() {
            files_ok += files.len();
        }
        assert!(
            errors.is_empty(),
            "{name}: {errors:?}\nwarnings: {:#?}",
            result.warnings
        );
        assert!(
            result
                .warnings
                .iter()
                .all(|w| !w.contains("migrated output has an error")),
            "{name}: {:#?}",
            result.warnings
        );
    }
    assert!(
        files_total >= 30,
        "expected the example contracts, found {files_total} files"
    );
    assert_eq!(files_ok, files_total);
}

fn migrate_dir(rel: &str) -> veld_tools::MigrateResult {
    let dir = examples_root().join(rel);
    let mut files = Vec::new();
    read_v1(&dir, &dir, &mut files);
    migrate_v1(&files)
}

fn render(r: &veld_tools::MigrateResult) -> String {
    let mut s = String::new();
    for (path, text) in &r.files {
        s.push_str(&format!("=== {path} ===\n{text}\n"));
    }
    s.push_str("=== warnings ===\n");
    for w in &r.warnings {
        s.push_str(&format!("- {w}\n"));
    }
    s
}

#[test]
fn snapshot_microservices_accounts() {
    insta::assert_snapshot!(render(&migrate_dir("microservices/veld/services/accounts")));
}

#[test]
fn accounts_migration_versus_the_hand_translation() {
    let r = migrate_dir("microservices/veld/services/accounts");
    let migrated = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "accounts".into(),
            kind: PackageKind::Service,
            files: r
                .files
                .iter()
                .map(|(p, t)| SourceFile {
                    path: p.clone(),
                    text: t.clone(),
                })
                .collect(),
            base_url: Some("http://accounts:3001".into()),
        }],
        consumes: BTreeMap::new(),
    })
    .ir
    .unwrap_or_else(|| panic!("migrated accounts must compile"));
    let hand = common::case_ir("ok_v1_translation_accounts")
        .unwrap_or_else(|| panic!("hand translation must compile"));
    let changes = veld_tools::diff(&hand, &migrated);
    let mut lines: Vec<String> = changes
        .iter()
        .map(|c| {
            format!(
                "{:?} {:?} {} | {}",
                c.severity, c.kind, c.subject, c.message
            )
        })
        .collect();
    lines.sort();
    insta::assert_snapshot!(lines.join("\n"));
}

#[test]
fn migrated_files_are_already_canonical() {
    for dir in example_dirs() {
        let mut files = Vec::new();
        read_v1(&dir, &dir, &mut files);
        for (path, text) in migrate_v1(&files).files {
            let again = veld_tools::format(&text).ok();
            assert_eq!(
                again.as_deref(),
                Some(text.as_str()),
                "{}: {path}",
                dir.display()
            );
        }
    }
}
