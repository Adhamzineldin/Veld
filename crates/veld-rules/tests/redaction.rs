//! The redaction goldens of `tests/wire/redaction` against the reference
//! (`spec/rules.md` section 19.3).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value as Json;
use veld_codegen::canonical::canonical;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_rules::{redact_decl, redact_decl_at, redact_type};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn contract() -> Ir {
    let text = std::fs::read_to_string(root().join("tests/wire/redaction/contract.veld"))
        .unwrap()
        .replace("\r\n", "\n");
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "guests".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "t.veld".into(),
                text,
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
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

fn cases() -> Vec<Json> {
    let text = std::fs::read_to_string(root().join("tests/wire/redaction/cases.json")).unwrap();
    let file: Json = serde_json::from_str(&text).unwrap();
    file["cases"].as_array().unwrap().clone()
}

fn bytes(v: &Json) -> String {
    canonical(v)
}

fn paths_of(case: &Json) -> Vec<String> {
    case["paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn there_are_eleven_cases() {
    assert_eq!(cases().len(), 11);
}

#[test]
fn every_case_gives_the_golden_bytes_and_paths() {
    let ir = contract();
    for case in cases() {
        let name = case["name"].as_str().unwrap();
        let decl = ir
            .decls
            .iter()
            .find(|d| d.name == case["type"].as_str().unwrap())
            .unwrap_or_else(|| panic!("{name}: no such declaration"));
        let got = redact_decl(&ir, decl, &case["value"]).unwrap();
        assert_eq!(bytes(&got.value), case["redacted"].as_str().unwrap(), "{name}");
        assert_eq!(got.paths, paths_of(&case), "{name}");
    }
}

#[test]
fn the_paths_are_sorted_and_unique() {
    for case in cases() {
        let paths = paths_of(&case);
        let mut sorted = paths.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(paths, sorted, "{}", case["name"]);
    }
}

#[test]
fn redacting_twice_changes_nothing_and_lists_nothing() {
    let ir = contract();
    for case in cases() {
        let decl = ir
            .decls
            .iter()
            .find(|d| d.name == case["type"].as_str().unwrap())
            .unwrap();
        let once = redact_decl(&ir, decl, &case["value"]).unwrap();
        let twice = redact_decl(&ir, decl, &once.value).unwrap();
        assert_eq!(once.value, twice.value, "{}", case["name"]);
        assert!(twice.paths.is_empty(), "{}", case["name"]);
    }
}

#[test]
fn the_input_value_is_not_touched() {
    let ir = contract();
    let case = &cases()[0];
    let decl = ir.decls.iter().find(|d| d.name == "Guest").unwrap();
    let before = case["value"].clone();
    let _ = redact_decl(&ir, decl, &case["value"]).unwrap();
    assert_eq!(before, case["value"]);
}

#[test]
fn a_root_prefixes_every_path() {
    let ir = contract();
    let case = &cases()[0];
    let decl = ir.decls.iter().find(|d| d.name == "Guest").unwrap();
    let got = redact_decl_at(&ir, decl, &case["value"], "params.body").unwrap();
    assert!(got.paths.iter().all(|p| p.starts_with("params.body.")));
    assert_eq!(got.paths.len(), paths_of(case).len());
}

#[test]
fn a_list_of_models_is_redacted_by_type() {
    let ir = contract();
    let guest = ir.decls.iter().find(|d| d.name == "Guest").unwrap();
    let ty = veld_ir::Type::List(Box::new(veld_ir::Type::Decl(guest.id)));
    let one = &cases()[1]["value"];
    let got = redact_type(&ir, &ty, &Json::Array(vec![one.clone(), one.clone()])).unwrap();
    assert!(got.paths.contains(&"$[0].name".to_owned()));
    assert!(got.paths.contains(&"$[1].name".to_owned()));
}

#[test]
fn a_value_of_the_wrong_shape_is_refused_not_passed_on() {
    let ir = contract();
    let guest = ir.decls.iter().find(|d| d.name == "Guest").unwrap();
    assert!(redact_decl(&ir, guest, &Json::String("x".into())).is_err());
    let mut v = cases()[1]["value"].clone();
    v["home"] = Json::Bool(true);
    assert!(redact_decl(&ir, guest, &v).is_err());
}

#[test]
fn a_union_value_with_an_unknown_arm_is_refused() {
    let ir = contract();
    let order = ir.decls.iter().find(|d| d.name == "Order").unwrap();
    let mut v = cases()[5]["value"].clone();
    v["pay"]["kind"] = Json::String("cash".into());
    assert!(redact_decl(&ir, order, &v).is_err());
}
