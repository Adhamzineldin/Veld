//! Compiles every contract under `tests/events/` and runs its event examples through the
//! reference evaluator (`spec/events.md` section 12). The outcomes were written by hand from the
//! spec; later every server target must give the same ones.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_rules::{Verdict, run_example};

fn veld_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            veld_files(&path, out);
        } else if path.extension().is_some_and(|x| x == "veld") {
            out.push(path);
        }
    }
}

fn compile_file(path: &Path) -> Ir {
    let text = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
    let out = compile(&CompileInput {
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
    });
    let errors: Vec<String> = out
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| format!("{}: {}", d.code, d.message))
        .collect();
    assert!(errors.is_empty(), "{}: {errors:?}", path.display());
    out.ir.unwrap()
}

fn compile_text(text: &str) -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "t.veld".into(),
                text: text.into(),
            }],
            base_url: None,
        }],
        consumes: Default::default(),
    });
    let errors: Vec<String> = out
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| format!("{}: {}", d.code, d.message))
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    out.ir.unwrap()
}

const BASE: &str = r#"
event Paid {
  id: uuid
  amount: decimal
  rule positive: amount > 0
}
event Other { id: uuid }
channel Pay { carries Paid, Other }
service P { publishes Pay }
service A { subscribes Pay as "a" { on Paid -> one } }
service B {
  subscribes Pay as "b" {
    on Paid -> two
    on Other -> three
  }
}
"#;

fn verdict(example: &str) -> Verdict {
    let ir = compile_text(&format!("{BASE}\n{example}"));
    let ex = ir.examples.iter().find(|e| e.event.is_some()).unwrap();
    run_example(&ir, ex).unwrap().verdict
}

#[test]
fn a_delivery_calls_the_handlers_of_every_subscription_that_has_an_on_line() {
    let ok = verdict(
        "example \"x\" { given event Paid { amount: 1.00 }\n expect A.one called\n expect B.two called\n expect B.three not called }",
    );
    assert_eq!(ok, Verdict::Passed);
}

#[test]
fn a_wrong_expectation_fails_with_expected_and_actual() {
    let Verdict::Failed(m) = verdict(
        "example \"x\" { given event Paid { amount: 1.00 }\n expect A.one not called\n expect B.two called }",
    ) else {
        panic!("should fail")
    };
    assert_eq!(m.expected, "A.one not called; B.two called");
    assert_eq!(m.actual, "called A.one, B.two");
    assert_eq!(m.differences.len(), 1);
    assert_eq!(m.differences[0].path, "A.one");
}

#[test]
fn a_rejected_event_calls_nothing_and_a_handled_one_is_not_rejected() {
    let ok = verdict("example \"x\" { given event Paid { amount: 0.00 }\n expect rejected }");
    assert_eq!(ok, Verdict::Passed);
    let Verdict::Failed(m) =
        verdict("example \"x\" { given event Paid { amount: 0.00 }\n expect A.one called }")
    else {
        panic!("should fail")
    };
    assert_eq!(m.actual, "rejected (rule: rule positive)");
    let Verdict::Failed(m) =
        verdict("example \"x\" { given event Paid { amount: 1.00 }\n expect rejected }")
    else {
        panic!("should fail")
    };
    assert_eq!(m.actual, "called A.one, B.two");
}

#[test]
fn an_event_with_no_on_line_anywhere_is_ignored() {
    let ir = compile_text(
        "event Q { id: uuid }\nevent R { id: uuid }\nchannel C { carries Q, R }\nservice P { publishes C }\nservice S { subscribes C as \"g\" { on Q -> q } }\nexample \"x\" { given event R { }\n expect S.q not called }",
    );
    let ex = &ir.examples[0];
    let r = run_example(&ir, ex).unwrap();
    assert_eq!(r.verdict, Verdict::Passed);
    let filled = veld_rules::fill_event_example(&ir, ex).unwrap();
    let veld_rules::Delivery::Handled { called } =
        veld_rules::deliver(&ir, filled.channel, filled.event, &filled.value)
    else {
        panic!("should be handled")
    };
    assert!(called.is_empty());
}

#[test]
fn left_out_fields_are_filled_in_like_a_parameter() {
    let ir = compile_text(&format!(
        "{BASE}\nexample \"x\" {{ given event Paid {{ amount: 2.00 }}\n expect A.one called }}"
    ));
    let filled = veld_rules::fill_event_example(&ir, &ir.examples[0]).unwrap();
    assert_eq!(filled.value["id"], "00000000-0000-4000-8000-000000000001");
    assert_eq!(filled.value["amount"], "2.00");
}

#[test]
fn a_filled_in_value_that_breaks_a_rule_is_e0953() {
    let ir = compile_text(
        "event E {\n n: int\n rule big: n > 5\n}\nchannel C { carries E }\nservice P { publishes C }\nservice S { subscribes C as \"g\" { on E -> h } }\nexample \"x\" { given event E { }\n expect S.h called }",
    );
    let err = run_example(&ir, &ir.examples[0]).unwrap_err();
    assert_eq!(err.code(), "E0953", "{err}");
}

#[test]
fn a_written_value_may_break_a_rule_on_purpose() {
    let ir = compile_text(
        "event E {\n n: int\n rule big: n > 5\n}\nchannel C { carries E }\nservice P { publishes C }\nservice S { subscribes C as \"g\" { on E -> h } }\nexample \"x\" { given event E { n: 1 }\n expect rejected }",
    );
    assert_eq!(
        run_example(&ir, &ir.examples[0]).unwrap().verdict,
        Verdict::Passed
    );
}

#[test]
fn a_shape_that_does_not_fit_is_rejected_as_shape() {
    let ir = compile_text(&format!(
        "{BASE}\nexample \"x\" {{ given event Paid {{ amount: 1.00 }}\n expect A.one called }}"
    ));
    let ex = &ir.examples[0];
    let spec = ex.event.as_ref().unwrap();
    let bad = serde_json::json!({ "id": "00000000-0000-4000-8000-000000000001" });
    let veld_rules::Delivery::Rejected { reason, .. } =
        veld_rules::deliver(&ir, spec.channel, spec.decl, &bad)
    else {
        panic!("a missing field is a shape problem")
    };
    assert_eq!(reason, veld_rules::RejectReason::Shape);
}

#[test]
fn a_call_example_is_not_an_event_example_and_the_other_way_round() {
    let ir = compile_text(&format!(
        "{BASE}\nexample \"x\" {{ given event Paid {{ amount: 1.00 }}\n expect A.one called }}"
    ));
    let err = veld_rules::fill_example(&ir, &ir.examples[0]).unwrap_err();
    assert_eq!(err.code(), "E0952");
}

#[test]
fn every_event_vector_passes_in_the_reference_evaluator() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/events");
    let mut files = Vec::new();
    veld_files(&root, &mut files);
    assert!(!files.is_empty(), "no contracts under tests/events");
    let (mut run, mut failures) = (0usize, Vec::new());
    for path in files {
        let ir = compile_file(&path);
        for ex in ir.examples.iter().filter(|e| e.event.is_some()) {
            run += 1;
            match run_example(&ir, ex) {
                Ok(r) if r.verdict == Verdict::Passed => {}
                Ok(r) => failures.push(format!("{}: {r}", path.display())),
                Err(e) => failures.push(format!("{}: {}: {e}", path.display(), ex.name)),
            }
        }
    }
    assert!(run >= 11, "only {run} event examples ran");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
