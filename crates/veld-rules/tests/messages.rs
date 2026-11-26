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
    Localized, accepted_languages, choose_language, content_language, localize, localize_denial,
    localize_violation,
};
use veld_rules::{EvalError, Options, Violation, check_value, eval_policy};

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
    assert!(
        out.diagnostics.iter().all(|d| !d.is_error()),
        "{}",
        out.render()
    );
    out.ir.unwrap()
}

fn cases() -> Json {
    let text = std::fs::read_to_string(root().join("tests/wire/messages/cases.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn package(ir: &Ir) -> PackageId {
    ir.packages
        .iter()
        .find(|p| p.name == "bookings")
        .unwrap()
        .id
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
    assert_eq!(
        langs,
        cases()["languages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap())
            .collect::<Vec<_>>()
    );
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
    assert_eq!(content_language(std::slice::from_ref(&from_contract)), None);
    assert_eq!(content_language(&[from_contract, from_catalog]), Some("ar"));
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

const AR_END_AFTER_START: &str = "يجب أن تنتهي الإقامة بعد بدايتها";
const AR_NOT_PERMITTED: &str = "غير مسموح";

fn booking(status: &str, end: &str) -> Json {
    serde_json::json!({
        "id": "0b5c9c3e-1111-4a5b-8c9d-000000000001",
        "status": status,
        "revision": "1",
        "start": "2026-06-10T12:00:00Z",
        "end": end,
        "total": "10.00"
    })
}

#[test]
fn a_failing_invariant_is_localized_by_its_key() {
    let ir = ir();
    let pkg = package(&ir);
    let decl = ir.decls.iter().find(|d| d.name == "Booking").unwrap();
    let found = check_value(
        &ir,
        decl,
        &booking("pending", "2026-06-09T12:00:00Z"),
        Options::default(),
    )
    .unwrap();
    assert_eq!(found.len(), 1);
    let v = &found[0];
    let ar = localize_violation(&ir, pkg, Some("ar, fr;q=0.5"), v).unwrap();
    assert_eq!(ar.message_key, "bookings/Booking.endAfterStart");
    assert_eq!(ar.text, AR_END_AFTER_START);
    assert_eq!(ar.language.as_deref(), Some("ar"));
    let pt = localize_violation(&ir, pkg, Some("pt-BR"), v).unwrap();
    assert_eq!(pt.text, "A reserva deve terminar depois de começar");
    assert_eq!(pt.language.as_deref(), Some("pt-br"));
    for header in [None, Some("fr"), Some("pt")] {
        let plain = localize_violation(&ir, pkg, header, v).unwrap();
        assert_eq!(plain.text, "A booking must end after it starts");
        assert_eq!(plain.message_key, "bookings/Booking.endAfterStart");
        assert!(!plain.catalog_used());
    }
}

#[test]
fn an_evaluation_error_has_its_key_and_is_not_translated() {
    let ir = ir();
    let pkg = package(&ir);
    let decl = ir.decls.iter().find(|d| d.name == "Booking").unwrap();
    let rule = decl
        .rules()
        .iter()
        .find(|r| r.name == "endAfterStart")
        .unwrap();
    let v = Violation {
        rule_name: rule.name.clone(),
        rule_id: rule.id.clone(),
        path: "$".into(),
        fields: vec!["end".into(), "start".into()],
        message: "rule endAfterStart could not be evaluated".into(),
        error: Some(EvalError::Overflow),
    };
    let got = localize_violation(&ir, pkg, Some("ar"), &v).unwrap();
    assert_eq!(got.message_key, "bookings/Booking.endAfterStart");
    assert_eq!(got.text, "rule endAfterStart could not be evaluated");
    assert_eq!(got.language, None);
}

#[test]
fn a_rule_id_that_is_not_one_has_no_key() {
    let ir = ir();
    let v = Violation {
        rule_name: "x".into(),
        rule_id: "not an id".into(),
        path: "$".into(),
        fields: vec![],
        message: "m".into(),
        error: None,
    };
    assert_eq!(localize_violation(&ir, package(&ir), Some("ar"), &v), None);
}

fn denial(
    user: Json,
    status: &str,
    header: Option<&str>,
) -> (bool, Option<usize>, Option<Localized>) {
    let ir = ir();
    let policy = ir.policies.iter().find(|p| p.name == "Manage").unwrap();
    let args = [user, booking(status, "2026-06-11T12:00:00Z")];
    let verdict = eval_policy(&ir, policy, &args, 0).unwrap();
    let localized = localize_denial(&ir, package(&ir), header, &policy.id, &verdict);
    (verdict.permitted, verdict.forbid, localized)
}

fn user(suspended: bool, banned: bool) -> Json {
    serde_json::json!({
        "id": "0b5c9c3e-3333-4a5b-8c9d-000000000001",
        "suspended": suspended,
        "banned": banned
    })
}

#[test]
fn a_denial_by_the_first_forbid_has_its_key_and_its_translation() {
    let (permitted, forbid, got) = denial(user(true, false), "pending", Some("ar"));
    assert!(!permitted);
    assert_eq!(forbid, Some(1));
    let got = got.unwrap();
    assert_eq!(got.message_key, "bookings/Manage.forbid1");
    assert_eq!(got.language.as_deref(), Some("ar"));
    assert_ne!(got.text, "A suspended account cannot manage bookings");
}

#[test]
fn a_denial_by_a_forbid_without_a_message_is_not_permitted() {
    let (permitted, forbid, got) = denial(user(false, true), "pending", Some("ar"));
    assert!(!permitted);
    assert_eq!(forbid, None);
    let got = got.unwrap();
    assert_eq!(got.message_key, "veld/not-permitted");
    assert_eq!(got.text, AR_NOT_PERMITTED);
    let (_, _, plain) = denial(user(false, true), "pending", None);
    assert_eq!(plain.unwrap().text, "not permitted");
}

#[test]
fn a_denial_by_the_third_forbid_counts_the_one_without_a_message() {
    let (permitted, forbid, got) = denial(user(false, false), "cancelled", Some("ar"));
    assert!(!permitted);
    assert_eq!(forbid, Some(3));
    let got = got.unwrap();
    assert_eq!(got.message_key, "bookings/Manage.forbid3");
    // The `ar` catalog has no text for it: the contract's text, and no catalog used.
    assert_eq!(got.text, "A cancelled booking cannot be managed");
    assert_eq!(got.language, None);
}

#[test]
fn a_policy_that_permits_has_no_forbid() {
    let (permitted, forbid, _) = denial(user(false, false), "pending", None);
    assert!(permitted);
    assert_eq!(forbid, None);
}
