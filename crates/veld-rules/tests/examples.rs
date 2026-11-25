//! Compiles every contract under `tests/rules/endpoints` and runs every example in it through
//! the reference evaluator (`spec/rules.md` sections 12.3 and 12.5). The examples are the shared
//! vectors for endpoint rules: every server target must give the same outcomes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::PackageKind;
use veld_rules::run_example;

fn contracts() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/rules/endpoints");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "veld"))
        .collect();
    files.sort();
    files
}

#[test]
fn the_reference_evaluator_passes_every_example() {
    let files = contracts();
    assert!(
        !files.is_empty(),
        "no contracts under tests/rules/endpoints"
    );
    let (mut examples_run, mut failures) = (0usize, Vec::new());
    for veld in files {
        let text = std::fs::read_to_string(&veld).unwrap();
        let out = compile(&CompileInput {
            packages: vec![PackageInput {
                name: "t".into(),
                kind: PackageKind::Service,
                files: vec![SourceFile {
                    path: "vectors.veld".into(),
                    text,
                }],
                base_url: None,
            }],
            consumes: BTreeMap::new(),
        });
        let name = veld.display().to_string();
        let Some(ir) = out.ir else {
            panic!("{name} does not compile:\n{}", out.render());
        };
        assert!(!ir.examples.is_empty(), "{name} has no examples");
        for example in &ir.examples {
            examples_run += 1;
            match run_example(&ir, example) {
                Ok(report) if report.passed() => {}
                Ok(report) => failures.push(format!("{name}: {report}")),
                Err(e) => failures.push(format!("{name}: example `{}`: {e}", example.name)),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {examples_run} examples failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(examples_run >= 44, "only {examples_run} examples ran");
}

/// The test above runs every example of the folder. This one pins how many each contract holds,
/// so a vector that goes missing is noticed.
#[test]
fn each_contract_of_the_folder_holds_its_examples() {
    let expected = [
        ("bookings.veld", 20),
        ("transitions.veld", 14),
        ("policies.veld", 10),
    ];
    for (file, count) in expected {
        assert_eq!(compile_vectors(file).examples.len(), count, "{file}");
    }
    assert_eq!(
        contracts().len(),
        expected.len(),
        "a contract without a count"
    );
}

// --- the whole error, written down from the spec -------------------------------------------
//
// An example states the error and some of its payload. These tests state the rest by hand, from
// `spec/rules.md` sections 10.2 and 11.3: the payload in full and the one violation, member by
// member. A live run holds every generated server to what the evaluator answers
// (`veld_testing::Live`), so this is what pins the evaluator itself.

fn compile_vectors(file: &str) -> veld_ir::Ir {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/rules/endpoints")
        .join(file);
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "vectors.veld".into(),
                text: std::fs::read_to_string(&path).unwrap(),
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    match out.ir {
        Some(ir) => ir,
        None => panic!("{file} does not compile:\n{}", out.render()),
    }
}

fn error_of(ir: &veld_ir::Ir, example: &str) -> veld_rules::ErrorAnswer {
    let ex = ir.examples.iter().find(|e| e.name == example).unwrap();
    match run_example(ir, ex).unwrap().outcome {
        Some(veld_rules::Outcome::Error(a)) => a,
        other => panic!("`{example}` is no error: {other:?}"),
    }
}

fn is_id(id: &str, prefix: &str) -> bool {
    id.strip_prefix(prefix)
        .is_some_and(|hex| hex.len() == 16 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

#[test]
fn a_denial_is_403_forbidden_with_the_message_twice_and_the_policy_as_the_rule() {
    let ir = compile_vectors("policies.veld");
    let policy = ir
        .policies
        .iter()
        .find(|p| p.name == "ManageBooking")
        .unwrap();
    assert!(is_id(&policy.id, "veld:t/ManageBooking@"), "{}", policy.id);
    let cases = [
        // The first `forbid` that is true has a message.
        (
            "forbid beats permit",
            "A suspended account cannot manage bookings",
        ),
        // No `permit` is true and no `forbid` is.
        ("another caller may not", "not permitted"),
    ];
    for (example, message) in cases {
        let a = error_of(&ir, example);
        assert_eq!((a.name.as_str(), a.status), ("Forbidden", 403), "{example}");
        assert_eq!(
            a.data,
            serde_json::json!({ "message": message }),
            "{example}"
        );
        assert_eq!(a.violations.len(), 1, "{example}");
        let v = &a.violations[0];
        assert_eq!(v.rule_id, policy.id, "{example}");
        assert_eq!(v.path, "request", "{example}");
        // `allow ManageBooking(user, booking)` reads the principal and a fact, no parameter.
        assert!(v.fields.is_empty(), "{example}: {:?}", v.fields);
        assert_eq!(v.message, message, "{example}");
        assert_eq!(v.error, None, "{example}");
        assert!(a.hook.is_empty(), "{example}");
    }
}

#[test]
fn an_illegal_move_is_409_invalid_transition_with_the_transition_as_the_rule() {
    let ir = compile_vectors("transitions.veld");
    let pay = ir
        .services
        .iter()
        .flat_map(|s| &s.endpoints)
        .find(|e| e.name == "payBooking")
        .unwrap();
    let transition = pay.rules.transition.as_ref().unwrap();
    assert!(
        is_id(&transition.id, "veld:t/Bookings.payBooking.transition@"),
        "{}",
        transition.id
    );
    let a = error_of(&ir, "a paid booking cannot be paid again");
    assert_eq!((a.name.as_str(), a.status), ("InvalidTransition", 409));
    assert_eq!(
        a.data,
        serde_json::json!({ "field": "status", "from": "paid", "to": "paid" })
    );
    assert_eq!(a.violations.len(), 1);
    let v = &a.violations[0];
    assert_eq!(v.rule_id, transition.id);
    assert_eq!(v.path, "request");
    assert!(v.fields.is_empty(), "{:?}", v.fields);
    assert_eq!(v.message, "violates rule transition");
    assert_eq!(v.error, None);
    assert!(a.hook.is_empty());
}
