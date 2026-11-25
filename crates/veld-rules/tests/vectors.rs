//! Compiles every `tests/rules/core/**/*.veld` and runs every case of its vector file through the
//! reference evaluator (`spec/rules.md` section 15). The generated targets run the same cases.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::PackageKind;
use veld_rules::vectors::{load_cases, run_case};

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

#[test]
fn the_reference_evaluator_passes_every_vector() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/rules/core");
    let mut files = Vec::new();
    veld_files(&root, &mut files);
    assert!(!files.is_empty(), "no vectors under tests/rules/core");
    let (mut cases_run, mut failures) = (0usize, Vec::new());
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
        let cases = std::fs::read_to_string(veld.with_extension("json")).unwrap();
        for case in load_cases(&cases).unwrap() {
            cases_run += 1;
            if let Err(m) = run_case(&ir, &case) {
                failures.push(format!("{name}: {m}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {cases_run} failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(cases_run >= 100, "only {cases_run} cases ran");
}
