//! The formatter's line width (100 columns) over every contract of `tests/` and `examples/`:
//! the result is a fixed point, keeps the comments and the documents, and compiles to the same IR.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use veld_compiler::{CompileInput, FileId, PackageInput, SourceFile, compile, parse_file};
use veld_ir::PackageKind;
use veld_tools::format;

fn veld_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.filter_map(Result::ok).map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned());
        if p.is_dir() {
            if name.as_deref() != Some("node_modules") && name.as_deref() != Some("target") {
                veld_files(&p, out);
            }
        } else if p.extension().is_some_and(|e| e == "veld") {
            out.push(p);
        }
    }
}

fn all_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    veld_files(&root.join("tests"), &mut out);
    veld_files(&root.join("examples"), &mut out);
    out
}

fn strip_origins(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.remove("origin");
            m.values_mut().for_each(strip_origins);
        }
        Value::Array(a) => a.iter_mut().for_each(strip_origins),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

/// The IR of one file as a package of its own, or the sorted codes when it does not compile.
fn ir_of(text: &str) -> Result<Value, Vec<&'static str>> {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "t.veld".into(),
                text: text.to_owned(),
            }],
            base_url: None,
        }],
        consumes: Default::default(),
    });
    match out.ir {
        Some(ir) => {
            let mut v = serde_json::to_value(&ir).unwrap_or(Value::Null);
            strip_origins(&mut v);
            Ok(v)
        }
        None => {
            let mut codes: Vec<&'static str> = out.diagnostics.iter().map(|d| d.code).collect();
            codes.sort_unstable();
            Err(codes)
        }
    }
}

fn comments(src: &str) -> Vec<String> {
    let (_, cs, _) = parse_file(src, FileId(0));
    let mut v: Vec<String> = cs
        .iter()
        .map(|c| format!("{:?}:{}", c.kind, c.text))
        .collect();
    v.sort();
    v
}

#[test]
fn every_contract_of_the_repository_formats_to_a_fixed_point_with_the_same_meaning() {
    let (mut formatted, mut refused) = (0, 0);
    for path in all_files() {
        let name = path.display().to_string();
        let src = fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
        let Ok(once) = format(&src) else {
            refused += 1;
            continue;
        };
        formatted += 1;
        assert_eq!(
            format(&once).ok().as_deref(),
            Some(once.as_str()),
            "{name}: not idempotent"
        );
        assert_eq!(comments(&src), comments(&once), "{name}: comments changed");
        assert_eq!(ir_of(&src), ir_of(&once), "{name}: the meaning changed");
    }
    assert!(
        formatted >= 100,
        "only {formatted} files formatted ({refused} refused)"
    );
}
