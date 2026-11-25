//! Deeply nested input must produce a diagnostic, never a stack overflow.
//!
//! Each case runs in a thread with a deliberately small stack: an unguarded recursion over
//! 10,000 levels overflows it and aborts the whole test process.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::PackageKind;

const DEPTH: usize = 10_000;
const CHAIN: usize = 2_000;
const SMALL_STACK: usize = 256 * 1024;

fn compile_codes(src: String) -> Vec<String> {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "app".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "app.veld".into(),
                text: src,
            }],
            base_url: None,
        }],
        consumes: Default::default(),
    });
    assert!(out.ir.is_none(), "deep nesting must not compile");
    out.diagnostics.iter().map(|d| d.code.to_string()).collect()
}

fn on_small_stack(src: String) -> Vec<String> {
    std::thread::Builder::new()
        .stack_size(SMALL_STACK)
        .spawn(move || compile_codes(src))
        .unwrap()
        .join()
        .expect("compiling deeply nested input must not overflow the stack")
}

#[test]
fn nested_generic_types_are_a_diagnostic() {
    let ty = format!("{}int{}", "Map<string, ".repeat(DEPTH), ">".repeat(DEPTH));
    let codes = on_small_stack(format!("model A {{ a: {ty} }}"));
    assert!(codes.iter().any(|c| c == "E0005"), "{codes:?}");
}

#[test]
fn nested_list_types_are_a_diagnostic() {
    let ty = format!("int{}", "[]".repeat(DEPTH));
    let codes = on_small_stack(format!("model A {{ a: {ty} }}"));
    assert!(codes.iter().any(|c| c == "E0005"), "{codes:?}");
}

#[test]
fn nested_annotation_calls_are_a_diagnostic() {
    let arg = format!("{}x{}", "f(".repeat(DEPTH), ")".repeat(DEPTH));
    let codes = on_small_stack(format!("model A {{ a: int @thing({arg}) }}"));
    assert!(codes.iter().any(|c| c == "E0005"), "{codes:?}");
}

#[test]
fn nesting_within_the_limit_still_compiles() {
    let ty = format!("{}int{}", "Map<string, ".repeat(60), ">".repeat(60));
    let src = format!("model A {{ a: {ty} }}");
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "app".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "app.veld".into(),
                text: src,
            }],
            base_url: None,
        }],
        consumes: Default::default(),
    });
    assert!(out.ir.is_some(), "{}", out.render());
}

fn compile_ok_or_diag(src: String) -> Vec<String> {
    std::thread::Builder::new()
        .stack_size(SMALL_STACK)
        .spawn(move || {
            let out = compile(&CompileInput {
                packages: vec![PackageInput {
                    name: "app".into(),
                    kind: PackageKind::Service,
                    files: vec![SourceFile {
                        path: "app.veld".into(),
                        text: src,
                    }],
                    base_url: None,
                }],
                consumes: Default::default(),
            });
            // Either a valid IR or diagnostics; the point is that it returns.
            assert!(out.ir.is_some() || !out.diagnostics.is_empty());
            out.diagnostics.iter().map(|d| d.code.to_string()).collect()
        })
        .unwrap()
        .join()
        .expect("a long declaration chain must not overflow the stack")
}

#[test]
fn long_inheritance_chain_does_not_overflow() {
    let mut src = String::from("model M0 { a: int }\n");
    for i in 1..CHAIN {
        src.push_str(&format!("model M{i} extends M{} {{ f{i}: int }}\n", i - 1));
    }
    let codes = compile_ok_or_diag(src);
    assert!(codes.iter().any(|c| c == "E0006"), "{codes:?}");
    assert_eq!(codes.iter().filter(|c| *c == "E0006").count(), 1);
}

#[test]
fn long_reference_chain_does_not_overflow() {
    let mut src = String::new();
    for i in 0..CHAIN {
        src.push_str(&format!("model M{i} {{ next: M{} }}\n", i + 1));
    }
    src.push_str(&format!("model M{DEPTH} {{ a: int }}\n"));
    compile_ok_or_diag(src);
}

#[test]
fn long_optional_reference_chain_does_not_overflow() {
    let mut src = String::new();
    for i in 0..CHAIN {
        src.push_str(&format!("model M{i} {{ next?: M{} }}\n", i + 1));
    }
    src.push_str(&format!("model M{DEPTH} {{ a: int }}\n"));
    compile_ok_or_diag(src);
}

/// A legal 64-level rule needs more than the 256 KiB above in a debug build; 1 MiB still
/// overflows long before 10,000 unguarded levels would finish.
const RULE_STACK: usize = 1024 * 1024;

fn rule_outcome(expr: String) -> (Vec<String>, bool) {
    let src = format!("model A {{ a: bool\n n: int\n rule r: {expr} }}");
    std::thread::Builder::new()
        .stack_size(RULE_STACK)
        .spawn(move || {
            let out = compile(&CompileInput {
                packages: vec![PackageInput {
                    name: "app".into(),
                    kind: PackageKind::Service,
                    files: vec![SourceFile {
                        path: "app.veld".into(),
                        text: src,
                    }],
                    base_url: None,
                }],
                consumes: Default::default(),
            });
            let codes = out.diagnostics.iter().map(|d| d.code.to_string()).collect();
            (codes, out.ir.is_some())
        })
        .unwrap()
        .join()
        .expect("a deeply nested rule must not overflow the stack")
}

fn rule_codes(expr: String) -> Vec<String> {
    rule_outcome(expr).0
}

#[test]
fn deeply_nested_rule_expressions_are_a_diagnostic() {
    let has_e0005 = |codes: Vec<String>| codes.iter().any(|c| c == "E0005");
    assert!(has_e0005(rule_codes(format!(
        "{}a{}",
        "(".repeat(DEPTH),
        ")".repeat(DEPTH)
    ))));
    assert!(has_e0005(rule_codes(format!("{}a", "!".repeat(DEPTH)))));
    assert!(has_e0005(rule_codes(format!("{}a", "not ".repeat(DEPTH)))));
    assert!(has_e0005(rule_codes(format!("{}n > 0", "-".repeat(DEPTH)))));
    assert!(has_e0005(rule_codes(format!(
        "{}a{}",
        "f(".repeat(DEPTH),
        ")".repeat(DEPTH)
    ))));
    assert!(has_e0005(rule_codes(format!(
        "n in {}1{}",
        "[".repeat(DEPTH),
        "]".repeat(DEPTH)
    ))));
}

#[test]
fn long_operator_chains_in_rules_are_a_diagnostic() {
    let has_e0005 = |codes: Vec<String>| codes.iter().any(|c| c == "E0005");
    for op in [" && ", " || ", " implies "] {
        assert!(has_e0005(rule_codes(vec!["a"; DEPTH].join(op))), "{op}");
    }
    for op in [" + ", " - ", " * ", " / ", " % "] {
        let sum = vec!["n"; DEPTH].join(op);
        assert!(has_e0005(rule_codes(format!("{sum} > 0"))), "{op}");
    }
    assert!(has_e0005(rule_codes(format!("a{}", ".x".repeat(DEPTH)))));
}

#[test]
fn expressions_within_the_limit_compile_on_a_modest_stack() {
    let shapes = [
        format!("{}a", "!".repeat(60)),
        format!("{}a{}", "(".repeat(60), ")".repeat(60)),
        format!("{} > 0", vec!["n"; 60].join(" + ")),
        vec!["a"; 60].join(" && "),
        vec!["a"; 60].join(" implies "),
    ];
    for s in shapes {
        let (codes, compiled) = rule_outcome(s.clone());
        assert!(compiled && codes.is_empty(), "{codes:?} for {}", &s[..30]);
    }
}

#[test]
fn a_rule_of_moderate_depth_survives_a_json_round_trip() {
    // Every level of an expression is two or three JSON levels, and serde_json reads at most
    // 128: 25 levels of negation stay well inside it.
    let (_, compiled) = rule_outcome(format!("{}a", "!".repeat(25)));
    assert!(compiled);
    let src = format!("model A {{ a: bool\n rule r: {}a }}", "!".repeat(25));
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "app".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "app.veld".into(),
                text: src,
            }],
            base_url: None,
        }],
        consumes: Default::default(),
    });
    let ir = out.ir.expect("compiles");
    let json = serde_json::to_string(&ir).unwrap();
    let back: veld_ir::Ir = serde_json::from_str(&json).expect("reads back");
    assert_eq!(back, ir);
}
