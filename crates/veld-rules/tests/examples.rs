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
    assert!(examples_run >= 20, "only {examples_run} examples ran");
}

/// The state machine vectors (`spec/rules.md` section 10) are held back as
/// `transitions.veld.pending`: the generators read every `.veld` file of that folder and refuse
/// transitions with `E0801` until they learn them. The reference evaluator runs them from here.
/// When the file is renamed to `transitions.veld`, the test above covers it and this one goes.
#[test]
fn the_reference_evaluator_passes_the_pending_state_machine_vectors() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/rules/endpoints/transitions.veld.pending");
    let text = std::fs::read_to_string(&path).unwrap();
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "transitions.veld".into(),
                text,
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    let Some(ir) = out.ir else {
        panic!("does not compile:\n{}", out.render());
    };
    assert_eq!(ir.examples.len(), 14);
    let failures: Vec<String> = ir
        .examples
        .iter()
        .filter_map(|e| match run_example(&ir, e) {
            Ok(r) if r.passed() => None,
            Ok(r) => Some(r.to_string()),
            Err(err) => Some(format!("example `{}`: {err}", e.name)),
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
