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
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
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

fn rules_contracts() -> Vec<std::path::PathBuf> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
            .unwrap_or_default();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|e| e == "veld") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/rules"),
        &mut out,
    );
    out
}

/// The shared rule vectors (`tests/rules/`) are formatted like everything else: idempotent,
/// comments kept and the IR (rule ids included) unchanged.
#[test]
fn formatting_the_rules_contracts_is_idempotent_and_keeps_the_ir() {
    let files = rules_contracts();
    assert!(files.len() >= 8, "found {} rules contracts", files.len());
    for path in files {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        let once = format(&text).unwrap_or_else(|d| panic!("{}: {d:?}", path.display()));
        assert_eq!(
            format(&once).as_deref().ok(),
            Some(once.as_str()),
            "{}: not idempotent",
            path.display()
        );
        assert_eq!(
            comment_multiset(&text),
            comment_multiset(&once),
            "{}: comments changed",
            path.display()
        );
        let input = |t: &str| CompileInput {
            packages: vec![veld_compiler::PackageInput {
                name: "t".into(),
                kind: veld_ir::PackageKind::Service,
                files: vec![veld_compiler::SourceFile {
                    path: "t.veld".into(),
                    text: t.to_string(),
                }],
                base_url: None,
            }],
            consumes: Default::default(),
        };
        let before = ir_json(&input(&text));
        assert!(before.is_ok(), "{}: {before:?}", path.display());
        assert_eq!(
            before,
            ir_json(&input(&once)),
            "{}: IR changed",
            path.display()
        );
    }
}

/// Spellings that parse differently when printed carelessly: the minus rule, parentheses,
/// `not` against `!`, decimal text.
#[test]
fn tricky_expression_spellings_keep_their_ir() {
    let src = "const K: int = 3\nmodel M {\n  a: int\n  b: int\n  d: decimal\n  x: bool\n  y: bool\n  z: bool\n  e: int[]\n\
        rule w1: x implies (-1 < a)\n rule w2: x implies (-0.5 < d)\n rule w3: x implies (- 1 < a)\n rule w4: x implies (-1 < a && x)\n\
        rule neg: a > - 1\n rule lit: a > -1\n rule sub: a -1 > b - -1\n rule dub: a - - 1 > 0\n\
        rule dec: d == 1.50 && d != 0.250\n rule imp: (x implies y) implies z\n rule imp2: x implies (y implies z)\n\
        rule cmp: (a < b) == x\n rule not1: not x == y\n rule not2: !(x == y)\n rule mix: x || y && z || (x || y) && z\n\
        rule arith: (a + b) * 2 - (a - b) == 4 % K\n rule lst: a in [-1, - 2, K]\n rule neg2: - -a < - (a + b)\n}\n";
    let formatted = format(src).unwrap();
    assert_eq!(format(&formatted).unwrap(), formatted);
    let input = |t: &str| CompileInput {
        packages: vec![veld_compiler::PackageInput {
            name: "t".into(),
            kind: veld_ir::PackageKind::Service,
            files: vec![veld_compiler::SourceFile {
                path: "t.veld".into(),
                text: t.to_string(),
            }],
            base_url: None,
        }],
        consumes: Default::default(),
    };
    let before = ir_json(&input(src));
    assert!(before.is_ok(), "{before:?}");
    assert_eq!(before, ir_json(&input(&formatted)));
}
