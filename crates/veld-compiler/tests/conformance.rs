//! Conformance suite: every directory under `tests/conformance/` is one case.
//!
//! Layout of a case:
//! - `input/*.veld`: a single package named `app`; or
//! - `input/<pkg>/**.veld`: one package per directory (`@org+name` is the
//!   registry package `@org/name`);
//! - `case.toml` (optional): `[consumes]`, `[base_url]` and `[kinds]` tables.
//!
//! The snapshot (`snapshot.snap`, stored in the case directory) holds the
//! rendered diagnostics followed by the IR as pretty JSON.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use veld_compiler::{CompileInput, CompileOutput, PackageInput, SourceFile, compile};
use veld_ir::PackageKind;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance")
}

fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

fn case_dirs() -> Vec<PathBuf> {
    sorted_entries(&root())
        .into_iter()
        .filter(|p| p.is_dir())
        .collect()
}

fn read_veld_files(dir: &Path, base: &Path, recursive: bool, out: &mut Vec<SourceFile>) {
    for p in sorted_entries(dir) {
        if p.is_dir() && recursive {
            read_veld_files(&p, base, recursive, out);
        } else if p.extension().is_some_and(|e| e == "veld") {
            let rel = p
                .strip_prefix(base)
                .map(|r| r.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            let text = fs::read_to_string(&p)
                .unwrap_or_default()
                .replace("\r\n", "\n");
            out.push(SourceFile { path: rel, text });
        }
    }
}

#[derive(Default)]
struct CaseConfig {
    consumes: BTreeMap<String, Vec<String>>,
    base_url: BTreeMap<String, String>,
    kinds: BTreeMap<String, String>,
}

/// Minimal parser for the three `case.toml` tables used by the suite.
fn parse_case_toml(text: &str) -> CaseConfig {
    let mut cfg = CaseConfig::default();
    let mut section = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.trim().to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim().to_string(), value.trim());
        match section.as_str() {
            "consumes" => {
                let items = value
                    .trim_matches(['[', ']'])
                    .split(',')
                    .map(|s| s.trim().trim_matches('"').to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                cfg.consumes.insert(key, items);
            }
            "base_url" => {
                cfg.base_url
                    .insert(key, value.trim_matches('"').to_string());
            }
            "kinds" => {
                cfg.kinds.insert(key, value.trim_matches('"').to_string());
            }
            _ => {}
        }
    }
    cfg
}

fn package_input(name: &str, files: Vec<SourceFile>, cfg: &CaseConfig) -> PackageInput {
    let kind = if name.starts_with('@') {
        PackageKind::Registry
    } else if cfg.kinds.get(name).is_some_and(|k| k == "library") {
        PackageKind::Library
    } else {
        PackageKind::Service
    };
    PackageInput {
        name: name.to_string(),
        kind,
        files,
        base_url: cfg.base_url.get(name).cloned(),
    }
}

fn load_case(dir: &Path) -> CompileInput {
    let cfg = fs::read_to_string(dir.join("case.toml"))
        .map(|t| parse_case_toml(&t))
        .unwrap_or_default();
    let input = dir.join("input");
    let mut top = Vec::new();
    read_veld_files(&input, &input, false, &mut top);
    let packages = if top.is_empty() {
        sorted_entries(&input)
            .into_iter()
            .filter(|p| p.is_dir())
            .map(|p| {
                let mut files = Vec::new();
                read_veld_files(&p, &p, true, &mut files);
                let dirname = p
                    .file_name()
                    .map(|n| n.to_string_lossy().replace('+', "/"))
                    .unwrap_or_default();
                package_input(&dirname, files, &cfg)
            })
            .collect()
    } else {
        vec![package_input("app", top, &cfg)]
    };
    CompileInput {
        packages,
        consumes: cfg.consumes,
    }
}

fn render_case(out: &CompileOutput) -> String {
    let diags = if out.diagnostics.is_empty() {
        "(none)\n".to_string()
    } else {
        out.render()
    };
    let ir = match &out.ir {
        Some(ir) => serde_json::to_string_pretty(ir)
            .unwrap_or_else(|e| format!("serialization failed: {e}")),
        None => "(no IR: compilation failed)".to_string(),
    };
    format!("== diagnostics ==\n{diags}\n== ir ==\n{ir}\n")
}

fn case_name(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[test]
fn conformance_snapshots() {
    let cases = case_dirs();
    assert!(
        cases.len() >= 40,
        "expected at least 40 conformance cases, found {}",
        cases.len()
    );
    for dir in cases {
        let out = compile(&load_case(&dir));
        let content = render_case(&out);
        let mut settings = insta::Settings::clone_current();
        settings.set_snapshot_path(&dir);
        settings.set_prepend_module_to_snapshot(false);
        settings.set_omit_expression(true);
        settings.bind(|| insta::assert_snapshot!("snapshot", content));
    }
}

#[test]
fn case_names_state_what_they_expect() {
    for dir in case_dirs() {
        let name = case_name(&dir);
        let out = compile(&load_case(&dir));
        let has_errors = out
            .diagnostics
            .iter()
            .any(veld_compiler::Diagnostic::is_error);
        if name.starts_with("ok_") {
            assert!(
                !has_errors && out.ir.is_some(),
                "{name} must compile:\n{}",
                out.render()
            );
        } else if let Some(code) = name.split('_').next().filter(|c| c.len() == 5) {
            let code = code.to_uppercase();
            assert!(
                out.diagnostics.iter().any(|d| d.code == code),
                "{name} must report {code}:\n{}",
                out.render()
            );
            if code.starts_with('E') {
                assert!(
                    out.ir.is_none(),
                    "{name} has an error, so there must be no IR"
                );
            }
        } else {
            panic!("case `{name}` must start with `ok_` or a diagnostic code");
        }
    }
}

#[test]
fn every_diagnostic_code_has_an_invalid_case() {
    let mut codes: Vec<String> = Vec::new();
    codes.extend((1..=4).map(|n| format!("E{n:04}")));
    codes.push("E0101".to_string());
    codes.extend((201..=205).map(|n| format!("E0{n}")));
    codes.extend((301..=304).map(|n| format!("E0{n}")));
    codes.extend((401..=410).map(|n| format!("E0{n}")));
    codes.extend((501..=512).map(|n| format!("E0{n}")));
    codes.extend((601..=603).map(|n| format!("E0{n}")));
    codes.extend(["E0701", "E0702", "W0001", "W0201", "W0202", "W0701"].map(String::from));
    let names: Vec<String> = case_dirs()
        .iter()
        .map(|d| case_name(d).to_lowercase())
        .collect();
    for code in codes {
        let prefix = format!("{}_", code.to_lowercase());
        assert!(
            names.iter().any(|n| n.starts_with(&prefix)),
            "no conformance case for {code}"
        );
    }
}

#[test]
fn compilation_is_deterministic() {
    for dir in case_dirs() {
        let a = render_case(&compile(&load_case(&dir)));
        let b = render_case(&compile(&load_case(&dir)));
        assert_eq!(a, b, "{} is not deterministic", case_name(&dir));
    }
}

#[test]
fn valid_cases_have_unique_signatures_and_dense_ids() {
    for dir in case_dirs()
        .into_iter()
        .filter(|d| case_name(d).starts_with("ok_"))
    {
        let Some(ir) = compile(&load_case(&dir)).ir else {
            continue;
        };
        assert!(
            ir.decls.iter().enumerate().all(|(i, d)| d.id.index() == i),
            "{}",
            case_name(&dir)
        );
        assert!(
            ir.services
                .iter()
                .enumerate()
                .all(|(i, s)| s.id.index() == i),
            "{}",
            case_name(&dir)
        );
        let mut sigs: Vec<&String> = ir.decls.iter().map(|d| &d.sig).collect();
        sigs.sort();
        sigs.dedup();
        assert_eq!(
            sigs.len(),
            ir.decls.len(),
            "duplicate decl signatures in {}",
            case_name(&dir)
        );
        let sorted = ir.decls.windows(2).all(|w| w[0].qualified < w[1].qualified);
        assert!(
            sorted,
            "decls of {} are not sorted by qualified name",
            case_name(&dir)
        );
    }
}
