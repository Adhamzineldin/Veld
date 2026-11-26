//! The message goldens of `tests/wire/messages` against the reference
//! (`spec/platform.md` section 22).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value as Json;
use veld_compiler::messages::{CatalogFile, Catalogs, MemberCatalogs, compile_with_catalogs};
use veld_compiler::{CompileInput, PackageInput, SourceFile};
use veld_ir::{Ir, PackageId, PackageKind};
use veld_rules::messages::{
    Localized, accepted_languages, choose_language, content_language, localize,
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture() -> PathBuf {
    root().join("tests/wire/messages/workspace")
}

fn read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap().replace("\r\n", "\n")
}

fn catalogs() -> Catalogs {
    let mut files: Vec<CatalogFile> = std::fs::read_dir(fixture().join("messages"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .map(|p| CatalogFile {
            name: p.file_name().unwrap().to_str().unwrap().to_owned(),
            text: read(&p),
        })
        .collect();
    files.sort_by(|a, b| a.name.cmp(&b.name));
    let member = MemberCatalogs {
        dir: "messages".into(),
        files,
    };
    [("bookings".to_owned(), member)].into()
}

fn ir() -> Ir {
    let out = compile_with_catalogs(
        &CompileInput {
            packages: vec![PackageInput {
                name: "bookings".into(),
                kind: PackageKind::Service,
                files: vec![SourceFile {
                    path: "bookings.veld".into(),
                    text: read(&fixture().join("contracts/bookings.veld")),
                }],
                base_url: None,
            }],
            consumes: BTreeMap::new(),
        },
        None,
        &BTreeMap::new(),
        &catalogs(),
    );
    assert!(out.diagnostics.iter().all(|d| !d.is_error()), "{}", out.render());
    out.ir.unwrap()
}

fn cases() -> Json {
    let text = std::fs::read_to_string(root().join("tests/wire/messages/cases.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn package(ir: &Ir) -> PackageId {
    ir.packages.iter().find(|p| p.name == "bookings").unwrap().id
}

fn opt(v: &Json) -> Option<&str> {
    v.as_str()
}

#[test]
fn the_fixture_has_two_catalogs_and_warns_only_about_what_they_lack() {
    let out = compile_with_catalogs(
        &CompileInput {
            packages: vec![PackageInput {
                name: "bookings".into(),
                kind: PackageKind::Service,
                files: vec![SourceFile {
                    path: "bookings.veld".into(),
                    text: read(&fixture().join("contracts/bookings.veld")),
                }],
                base_url: None,
            }],
            consumes: BTreeMap::new(),
        },
        None,
        &BTreeMap::new(),
        &catalogs(),
    );
    let codes: Vec<&str> = out.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(codes, ["W1302", "W1302"], "{}", out.render());
    let ir = out.ir.unwrap();
    let langs: Vec<&str> = ir.packages[0].messages.keys().map(String::as_str).collect();
    assert_eq!(langs, cases()["languages"].as_array().unwrap().iter().map(|l| l.as_str().unwrap()).collect::<Vec<_>>());
}

#[test]
fn every_header_picks_the_golden_language() {
    let available = ["ar", "pt-br"];
    let table = cases();
    let rows = table["headers"].as_array().unwrap();
    assert_eq!(rows.len(), 29);
    for row in rows {
        let header = opt(&row["header"]);
        let want = opt(&row["language"]).map(str::to_owned);
        assert_eq!(choose_language(header, &available), want, "{header:?}");
    }
}

#[test]
fn accepted_languages_are_lower_case_best_first_and_skip_what_is_unusable() {
    assert_eq!(
        accepted_languages("pt-BR;q=0.7, ar;q=0.9, *, fr;q=0, de;q=x"),
        ["ar", "pt-br"]
    );
    assert!(accepted_languages("").is_empty());
}

#[test]
fn every_message_has_the_golden_text_and_language() {
    let ir = ir();
    let pkg = package(&ir);
    let table = cases();
    let rows = table["messages"].as_array().unwrap();
    assert_eq!(rows.len(), 15);
    for row in rows {
        let got = localize(
            &ir,
            pkg,
            opt(&row["header"]),
            row["key"].as_str().unwrap(),
            row["default"].as_str().unwrap(),
        );
        let note = row["note"].as_str().unwrap();
        assert_eq!(got.text, row["text"].as_str().unwrap(), "{note}");
        assert_eq!(got.message_key, row["key"].as_str().unwrap(), "{note}");
        assert_eq!(got.language.as_deref(), opt(&row["language"]), "{note}");
        assert_eq!(got.catalog_used(), row["language"].is_string(), "{note}");
    }
}

#[test]
fn content_language_is_set_when_some_message_came_from_a_catalog() {
    let ir = ir();
    let pkg = package(&ir);
    let from_catalog = localize(&ir, pkg, Some("ar"), "veld/not-permitted", "not permitted");
    let from_contract = localize(&ir, pkg, Some("ar"), "veld/version", "x");
    assert_eq!(content_language(&[from_contract.clone()]), None);
    assert_eq!(
        content_language(&[from_contract, from_catalog]),
        Some("ar")
    );
    assert_eq!(content_language(&[] as &[Localized]), None);
}

fn rule_id(ir: &Ir, row: &Json) -> String {
    let kind = row["kind"].as_str().unwrap();
    let endpoint = || {
        ir.services[0]
            .endpoints
            .iter()
            .find(|e| Some(e.name.as_str()) == row["endpoint"].as_str())
            .unwrap()
    };
    let name = opt(&row["name"]);
    match kind {
        "invariant" => ir
            .decls
            .iter()
            .find(|d| Some(d.name.as_str()) == row["decl"].as_str())
            .unwrap()
            .rules()
            .iter()
            .find(|r| Some(r.name.as_str()) == name)
            .unwrap()
            .id
            .clone(),
        "require" => endpoint()
            .rules
            .requires
            .iter()
            .find(|r| r.name.as_deref() == name)
            .unwrap()
            .id
            .clone(),
        "ensure" => endpoint()
            .rules
            .ensures
            .iter()
            .find(|r| r.name.as_deref() == name)
            .unwrap()
            .id
            .clone(),
        "transition" => endpoint().rules.transition.as_ref().unwrap().id.clone(),
        "version" => endpoint().rules.version.as_ref().unwrap().id.clone(),
        "denial" => ir
            .policies
            .iter()
            .find(|p| Some(p.name.as_str()) == row["policy"].as_str())
            .unwrap()
            .id
            .clone(),
        other => panic!("unknown kind {other}"),
    }
}

#[test]
fn every_kind_of_rule_has_its_golden_key() {
    let ir = ir();
    let table = cases();
    let rows = table["keys"].as_array().unwrap();
    assert_eq!(rows.len(), 10);
    for row in rows {
        let id = rule_id(&ir, row);
        let forbid = row["forbid"].as_u64().map(|n| usize::try_from(n).unwrap());
        let key = ir.violation_key(&id, forbid);
        assert_eq!(key.as_deref(), row["key"].as_str(), "{}", row["what"]);
    }
}

#[test]
fn the_catalog_keys_of_the_member_are_the_golden_ones() {
    let ir = ir();
    let table = cases();
    let want: BTreeMap<String, Option<String>> = table["catalogKeys"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().map(str::to_owned)))
        .collect();
    assert_eq!(ir.catalog_keys(package(&ir)), want);
    let fixed: Vec<&str> = table["fixedKeys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k.as_str().unwrap())
        .collect();
    assert_eq!(fixed, veld_ir::message_keys::FIXED);
}
