//! Field visibility in the reference evaluator (`spec/platform.md` section 2): the byte goldens of
//! `tests/wire/visibility`, the pending vector contract, and the places the goldens cannot state.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value as Json, json};
use veld_codegen::canonical::canonical;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{DeclId, Ir, PackageKind};
use veld_rules::visibility::unrooted;
use veld_rules::{
    Caller, Outcome, OutputMode, Pipeline, Request, apply_visibility_decl, apply_visibility_type,
    run_example,
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn compile_text(package: &str, text: &str) -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: package.into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "t.veld".into(),
                text: text.replace("\r\n", "\n"),
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

fn contract() -> Ir {
    let text = std::fs::read_to_string(root().join("tests/wire/visibility/contract.veld")).unwrap();
    compile_text("guests", &text)
}

fn cases() -> Vec<Json> {
    let text = std::fs::read_to_string(root().join("tests/wire/visibility/cases.json")).unwrap();
    let file: Json = serde_json::from_str(&text).unwrap();
    file["cases"].as_array().unwrap().clone()
}

fn user_model(ir: &Ir) -> DeclId {
    ir.decls.iter().find(|d| d.name == "User").unwrap().id
}

fn clock(text: &str) -> i64 {
    veld_rules::helpers::instant::parse_datetime(text).unwrap()
}

#[test]
fn there_are_twenty_three_cases() {
    assert_eq!(cases().len(), 23);
}

#[test]
fn every_case_gives_the_golden_bytes_and_paths() {
    let ir = contract();
    let user = user_model(&ir);
    for case in cases() {
        let name = case["name"].as_str().unwrap();
        let decl = ir
            .decls
            .iter()
            .find(|d| d.name == case["type"].as_str().unwrap())
            .unwrap_or_else(|| panic!("{name}: no such declaration"));
        let caller = if case["caller"].is_null() {
            Caller::none()
        } else {
            Caller::of(user, &case["caller"])
        };
        let now = clock(case["now"].as_str().unwrap());
        let got = apply_visibility_decl(&ir, decl, &case["value"], caller, now).unwrap();
        assert_eq!(
            canonical(&got.value),
            case["shown"].as_str().unwrap(),
            "{name}"
        );
        let want: Vec<&str> = case["hidden"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap())
            .collect();
        assert_eq!(got.hidden, want, "{name}");
    }
}

#[test]
fn the_paths_are_sorted_and_unique() {
    for case in cases() {
        let paths: Vec<String> = case["hidden"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap().to_owned())
            .collect();
        let mut sorted = paths.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(paths, sorted, "{}", case["name"]);
    }
}

#[test]
fn applying_it_twice_changes_nothing_more() {
    let ir = contract();
    let user = user_model(&ir);
    for case in cases() {
        let decl = ir
            .decls
            .iter()
            .find(|d| d.name == case["type"].as_str().unwrap())
            .unwrap();
        let caller = if case["caller"].is_null() {
            Caller::none()
        } else {
            Caller::of(user, &case["caller"])
        };
        let now = clock(case["now"].as_str().unwrap());
        let once = apply_visibility_decl(&ir, decl, &case["value"], caller, now).unwrap();
        let twice = apply_visibility_decl(&ir, decl, &once.value, caller, now).unwrap();
        assert_eq!(once.value, twice.value, "{}", case["name"]);
    }
}

#[test]
fn the_input_value_is_not_touched() {
    let ir = contract();
    let case = &cases()[1];
    let decl = ir.decls.iter().find(|d| d.name == "Guest").unwrap();
    let before = case["value"].clone();
    let user = user_model(&ir);
    let _ = apply_visibility_decl(
        &ir,
        decl,
        &case["value"],
        Caller::of(user, &case["caller"]),
        0,
    )
    .unwrap();
    assert_eq!(case["value"], before);
}

#[test]
fn a_caller_of_another_principal_model_sees_what_no_caller_sees() {
    let ir = contract();
    let guest = ir.decls.iter().find(|d| d.name == "Guest").unwrap();
    let case = &cases()[0];
    let someone = json!({ "id": "x", "roles": ["staff"] });
    let other = DeclId(guest.id.0);
    let foreign = apply_visibility_decl(
        &ir,
        guest,
        &case["value"],
        Caller::of(other, &someone),
        clock("2026-06-10T12:00:00Z"),
    )
    .unwrap();
    let nobody = apply_visibility_decl(
        &ir,
        guest,
        &case["value"],
        Caller::none(),
        clock("2026-06-10T12:00:00Z"),
    )
    .unwrap();
    assert_eq!(foreign, nobody);
}

#[test]
fn a_value_of_the_wrong_shape_is_an_error_not_a_pass() {
    let ir = contract();
    let guest = ir.decls.iter().find(|d| d.name == "Guest").unwrap();
    let mut bad = cases()[0]["value"].clone();
    bad["past"] = json!({ "not": "a list" });
    let someone = json!({ "id": "x", "roles": [] });
    let user = user_model(&ir);
    assert!(apply_visibility_decl(&ir, guest, &bad, Caller::of(user, &someone), 0).is_err());
    let ty = veld_ir::Type::List(Box::new(veld_ir::Type::Decl(guest.id)));
    assert!(apply_visibility_type(&ir, &ty, &json!({}), Caller::none(), 0).is_err());
}

#[test]
fn a_list_at_the_root_names_its_items_with_an_index() {
    let ir = contract();
    let address = ir.decls.iter().find(|d| d.name == "Address").unwrap();
    let ty = veld_ir::Type::List(Box::new(veld_ir::Type::Decl(address.id)));
    let value = json!([{ "street": "a", "city": "b", "door": "1" }]);
    let got = apply_visibility_type(&ir, &ty, &value, Caller::none(), 0).unwrap();
    assert_eq!(got.hidden, ["$[0].door"]);
    assert_eq!(unrooted(&got.hidden[0]), "[0].door");
    assert_eq!(unrooted("$.a.b"), "a.b");
    assert_eq!(unrooted("$"), "");
}

// --- the pending vector contract and the pipeline --------------------------------------------

fn vectors() -> Ir {
    let text =
        std::fs::read_to_string(root().join("tests/rules/endpoints/visibility.veld.pending"))
            .unwrap();
    compile_text("t", &text)
}

#[test]
fn the_pending_vector_contract_holds_its_fourteen_examples() {
    let ir = vectors();
    assert_eq!(ir.examples.len(), 14);
    for example in &ir.examples {
        let report = run_example(&ir, example).unwrap();
        assert!(report.passed(), "{report}");
    }
}

const BOOKING: &str = "00000000-0000-4000-8000-000000000001";
const GUEST: &str = "00000000-0000-4000-8000-0000000000aa";

fn pipeline<'a>(ir: &'a Ir, service: &str, endpoint: &str) -> Pipeline<'a> {
    let s = ir.services.iter().find(|s| s.name == service).unwrap();
    Pipeline::new(ir, s, endpoint).unwrap()
}

fn get(ir: &Ir, service: &str, endpoint: &str, caller: Option<Json>) -> veld_rules::Reported {
    let request = Request {
        params: BTreeMap::from([("id".to_owned(), json!(BOOKING))]),
        principal: caller,
        now: clock("2026-06-10T12:00:00Z"),
        result: Some(json!({
            "id": BOOKING, "guestId": GUEST, "divisor": 1,
            "opensAt": "2026-06-09T12:00:00Z",
            "guestPhone": "+201000000001", "ratio": "r", "early": "e"
        })),
        ..Request::default()
    };
    pipeline(ir, service, endpoint)
        .run_reported(&request, OutputMode::Strict)
        .unwrap()
}

#[test]
fn the_pipeline_reports_what_it_hid_for_the_audit_event() {
    let ir = vectors();
    let guest = Some(json!({ "id": GUEST, "roles": [] }));
    let got = get(&ir, "Bookings", "getBooking", guest);
    assert!(matches!(got.outcome, Outcome::Passes));
    assert!(got.hidden.is_empty(), "{:?}", got.hidden);
    let other = Some(json!({ "id": BOOKING, "roles": [] }));
    let got = get(&ir, "Bookings", "getBooking", other);
    assert_eq!(got.hidden, ["guestPhone"]);
    assert!(got.body.unwrap().get("guestPhone").is_none());
}

#[test]
fn an_endpoint_without_auth_has_no_caller_even_when_the_request_names_one() {
    let ir = vectors();
    let guest = Some(json!({ "id": GUEST, "roles": ["support"] }));
    let got = get(&ir, "Notices", "publicBooking", guest);
    assert_eq!(got.hidden, ["early", "guestPhone", "ratio"]);
}

#[test]
fn the_etag_is_taken_from_the_whole_value() {
    // A model with a version field and a visible field: the tag does not depend on the caller.
    let ir = compile_text(
        "t",
        "model User { id: uuid }\nprincipal User\nmodel Doc { id: uuid\n v: int @version\n note?: string @visible(when: user.id == id) }\n\
         @auth(bearer)\nservice Docs at \"/d\" { get \"/{id}\" one(id: uuid) -> Doc }\n",
    );
    let request = |caller: &str| Request {
        params: BTreeMap::from([("id".to_owned(), json!(BOOKING))]),
        principal: Some(json!({ "id": caller })),
        result: Some(json!({ "id": BOOKING, "v": 3, "note": "n" })),
        ..Request::default()
    };
    let p = pipeline(&ir, "Docs", "one");
    let a = p
        .run_reported(&request(BOOKING), OutputMode::Strict)
        .unwrap();
    let b = p.run_reported(&request(GUEST), OutputMode::Strict).unwrap();
    assert_eq!(a.etag, b.etag);
    assert!(a.hidden.is_empty() && b.hidden == ["note"]);
}

#[test]
fn a_contract_without_visibility_leaves_a_result_alone_when_output_rules_are_off() {
    let ir = compile_text(
        "t",
        "model A { n: int }\nservice S at \"/s\" { get \"/\" one() -> A }\n",
    );
    let request = Request {
        result: Some(json!({ "n": "not a number" })),
        ..Request::default()
    };
    let got = pipeline(&ir, "S", "one")
        .run_reported(&request, OutputMode::Off)
        .unwrap();
    assert!(got.hidden.is_empty());
    assert_eq!(got.body, Some(json!({ "n": "not a number" })));
}
