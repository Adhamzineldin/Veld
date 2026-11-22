//! Shared helpers for the integration tests: loading conformance cases.
#![allow(dead_code, clippy::panic)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};

pub fn conformance_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance")
}

fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

pub fn case_dirs() -> Vec<PathBuf> {
    sorted_entries(&conformance_root())
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

pub fn load_case(dir: &Path) -> CompileInput {
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

pub fn case_name(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The IR of a case; `None` when it does not compile.
pub fn case_ir(name: &str) -> Option<Ir> {
    compile(&load_case(&conformance_root().join(name))).ir
}
