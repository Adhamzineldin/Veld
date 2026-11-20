//! Formatter properties over every conformance source that parses:
//! idempotence, semantic preservation and comment preservation.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{case_dirs, case_name, load_case};
use serde_json::Value;
use veld_compiler::{CompileInput, FileId, compile, parse_file};
use veld_tools::format;

fn strip_origins(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.remove("origin");
            m.values_mut().for_each(strip_origins);
        }
        Value::Array(a) => a.iter_mut().for_each(strip_origins),
        _ => {}
    }
}

fn comment_multiset(src: &str) -> Vec<String> {
    let (_, comments, _) = parse_file(src, FileId(0));
    let mut v: Vec<String> = comments
        .iter()
        .map(|c| format!("{:?}:{}", c.kind, c.text))
        .collect();
    v.sort();
    v
}

/// Doc comments are part of the tree, so compare them as text too.
fn doc_lines(src: &str) -> Vec<String> {
    src.lines()
        .map(str::trim)
        .filter(|l| l.starts_with("///") && !l.starts_with("////"))
        .map(|l| l.trim_start_matches("///").trim().to_string())
        .collect()
}

fn formatted_input(input: &CompileInput) -> (CompileInput, usize, usize) {
    let mut out = input.clone();
    let (mut formatted, mut skipped) = (0, 0);
    for pkg in &mut out.packages {
        for file in &mut pkg.files {
            match format(&file.text) {
                Ok(t) => {
                    file.text = t;
                    formatted += 1;
                }
                Err(_) => skipped += 1,
            }
        }
    }
    (out, formatted, skipped)
}

fn ir_json(input: &CompileInput) -> Result<Value, Vec<&'static str>> {
    let out = compile(input);
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

#[test]
fn formatting_is_idempotent_and_preserves_everything() {
    let (mut files, mut refused) = (0, 0);
    for dir in case_dirs() {
        let name = case_name(&dir);
        let input = load_case(&dir);
        for pkg in &input.packages {
            for f in &pkg.files {
                let Ok(once) = format(&f.text) else {
                    refused += 1;
                    continue;
                };
                files += 1;
                let twice = format(&once);
                assert_eq!(
                    twice.as_deref().ok(),
                    Some(once.as_str()),
                    "{name}/{}: not idempotent",
                    f.path
                );
                assert_eq!(
                    comment_multiset(&f.text),
                    comment_multiset(&once),
                    "{name}/{}: comments changed",
                    f.path
                );
                assert_eq!(
                    doc_lines(&f.text),
                    doc_lines(&once),
                    "{name}/{}: docs changed",
                    f.path
                );
                assert!(
                    once.ends_with('\n') && !once.ends_with("\n\n"),
                    "{name}/{}: trailing newline",
                    f.path
                );
            }
        }
    }
    assert!(
        files >= 60,
        "only {files} files formatted ({refused} refused)"
    );
}

#[test]
fn formatting_preserves_the_ir() {
    for dir in case_dirs() {
        let name = case_name(&dir);
        let input = load_case(&dir);
        let (formatted, n, _) = formatted_input(&input);
        if n == 0 {
            continue;
        }
        assert_eq!(
            ir_json(&input),
            ir_json(&formatted),
            "{name}: semantics changed"
        );
    }
}

#[test]
fn formatting_refuses_syntax_errors() {
    let r = format("model A {");
    assert!(r.is_err());
    let r = format("import \"x.veld\"");
    assert!(r.is_err());
}
