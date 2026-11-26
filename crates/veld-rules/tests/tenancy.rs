//! Tenancy in the reference evaluator (`spec/platform.md` section 3): the pending vector
//! contract, the tenant id of a model, the check of a response, a declared error and a stream
//! item, a fact of another tenant, a caller without a tenant, the envelope attribute
//! `veldtenant` and the goldens of `tests/wire/events/tenant.json`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value as Json, json};
use sha2::{Digest, Sha256};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{ChannelId, DeclId, Ir, PackageKind, Type};
use veld_rules::envelope::{Inputs, decode, encode, source_of};
use veld_rules::tenant::{caller_tenant, refused};
use veld_rules::{Outcome, OutputMode, Pipeline, Request, run_example};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn compile_as(package: &str, text: &str) -> Ir {
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

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap()
}

fn vectors() -> Ir {
    compile_as("t", &read("tests/rules/endpoints/tenancy.veld.pending"))
}

const A: &str = "00000000-0000-4000-8000-0000000000aa";
const B: &str = "00000000-0000-4000-8000-0000000000bb";

#[test]
fn the_pending_vector_contract_holds_its_seventeen_examples() {
    let ir = vectors();
    assert_eq!(ir.examples.len(), 17);
    for example in &ir.examples {
        let report = run_example(&ir, example).unwrap();
        assert!(report.passed(), "{report}");
    }
}

// --- the tenant id ----------------------------------------------------------------------------

fn hex16(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn the_tenant_id_is_the_hash_of_the_model_and_the_field() {
    let ir = vectors();
    let booking = ir.decls.iter().find(|d| d.name == "Booking").unwrap();
    let field = booking.tenant_field().unwrap();
    let bytes = format!(
        r#"{{"d":"{}","k":"tenant","n":"tenantId"}}"#,
        booking.qualified
    );
    assert_eq!(
        field.tenant_id.as_deref(),
        Some(format!("veld:t/Booking.tenant@{}", hex16(bytes.as_bytes())).as_str())
    );
}

#[test]
fn an_inherited_tenant_has_the_id_of_the_model_that_wrote_it() {
    let ir = compile_as(
        "t",
        "model Base { tenantId: uuid @tenant }\nmodel Child extends Base { n: int }\n",
    );
    let id = |name: &str| {
        let d = ir.decls.iter().find(|d| d.name == name).unwrap();
        d.tenant_field().unwrap().tenant_id.clone().unwrap()
    };
    assert_eq!(id("Base"), id("Child"));
    assert!(id("Child").starts_with("veld:t/Base.tenant@"));
}

// --- the check of a value ---------------------------------------------------------------------

fn ty(ir: &Ir, name: &str) -> Type {
    Type::Decl(ir.decls.iter().find(|d| d.name == name).unwrap().id)
}

#[test]
fn a_violation_names_the_rule_the_path_and_the_field() {
    let ir = vectors();
    let list = Type::List(Box::new(ty(&ir, "Booking")));
    let value = json!([
        { "tenantId": A, "title": "a", "open": true, "id": A },
        { "tenantId": B, "title": "b", "open": true, "id": B }
    ]);
    let found = refused(&ir, &list, &value, Some(&json!(A)), "result").unwrap();
    assert_eq!(found.len(), 1);
    let v = &found[0];
    assert_eq!(v.path, "result[1]");
    assert_eq!(v.fields, ["tenantId"]);
    assert_eq!(v.message, "violates rule tenant");
    assert_eq!(v.rule_name, "tenant");
    assert!(
        v.rule_id.starts_with("veld:t/Booking.tenant@"),
        "{}",
        v.rule_id
    );
    assert!(v.error.is_none());
}

#[test]
fn every_value_of_another_tenant_is_reported_in_the_order_of_the_walk() {
    let ir = vectors();
    let list = Type::List(Box::new(ty(&ir, "Booking")));
    let value = json!([
        { "tenantId": B, "title": "b", "open": true, "id": A },
        { "tenantId": A, "title": "a", "open": true, "id": A },
        { "tenantId": B, "title": "c", "open": true, "id": A }
    ]);
    let found = refused(&ir, &list, &value, Some(&json!(A)), "result").unwrap();
    let paths: Vec<&str> = found.iter().map(|v| v.path.as_str()).collect();
    assert_eq!(paths, ["result[0]", "result[2]"]);
}

#[test]
fn a_caller_without_a_tenant_owns_nothing() {
    let ir = vectors();
    let b = ty(&ir, "Booking");
    let value = json!({ "tenantId": A, "title": "a", "open": true, "id": A });
    assert_eq!(refused(&ir, &b, &value, None, "result").unwrap().len(), 1);
    // A value with no tenant belongs to nobody either, not even to a caller with none.
    let user = ty(&ir, "User");
    let nobody = json!({ "id": A });
    assert_eq!(
        refused(&ir, &user, &nobody, None, "result").unwrap().len(),
        1
    );
}

#[test]
fn the_tenant_of_the_caller_is_the_tenant_field_of_the_principal_model() {
    let ir = vectors();
    let user = ir.decls.iter().find(|d| d.name == "User").unwrap().id;
    assert_eq!(
        caller_tenant(&ir, user, &json!({ "id": B, "tenantId": A })),
        Some(json!(A))
    );
    assert_eq!(caller_tenant(&ir, user, &json!({ "id": B })), None);
    assert_eq!(
        caller_tenant(&ir, user, &json!({ "id": B, "tenantId": null })),
        None
    );
    let booking = ir.decls.iter().find(|d| d.name == "Summary").unwrap().id;
    assert_eq!(caller_tenant(&ir, booking, &json!({ "title": "t" })), None);
}

#[test]
fn tenants_are_compared_as_values_of_their_type() {
    let ir = compile_as(
        "t",
        "model Shard { n: long @tenant }\nmodel Small { k: int @tenant }\nmodel Name { s: string @tenant }\n",
    );
    let same = |model: &str, a: &Json, b: &Json| {
        let t = ty(&ir, model);
        let value = a.clone();
        let wrapped = match model {
            "Shard" => json!({ "n": value }),
            "Small" => json!({ "k": value }),
            _ => json!({ "s": value }),
        };
        refused(&ir, &t, &wrapped, Some(b), "x").unwrap().is_empty()
    };
    assert!(same("Shard", &json!("12"), &json!("12")));
    assert!(!same("Shard", &json!("12"), &json!("13")));
    assert!(same("Small", &json!(7), &json!(7)));
    assert!(!same("Small", &json!(7), &json!(8)));
    assert!(same("Name", &json!("Acme"), &json!("Acme")));
    assert!(!same("Name", &json!("Acme"), &json!("acme")));
}

// --- the pipeline -----------------------------------------------------------------------------

fn pipeline<'a>(ir: &'a Ir, endpoint: &str) -> Pipeline<'a> {
    let s = ir.services.iter().find(|s| s.name == "Bookings").unwrap();
    Pipeline::new(ir, s, endpoint).unwrap()
}

fn request(principal: Json, result: Json) -> Request {
    Request {
        principal: Some(principal),
        result: Some(result),
        ..Request::default()
    }
}

#[test]
fn a_refused_response_is_a_500_with_the_violations_for_the_hook_and_no_body() {
    let ir = vectors();
    let r = request(
        json!({ "id": A, "tenantId": A }),
        json!([{ "tenantId": B, "title": "b", "open": true, "id": A }]),
    );
    for mode in [OutputMode::Strict, OutputMode::Audit, OutputMode::Off] {
        let got = pipeline(&ir, "listBookings")
            .run_reported(&r, mode)
            .unwrap();
        let Outcome::Error(answer) = &got.outcome else {
            panic!("{mode:?}: {:?}", got.outcome)
        };
        assert_eq!((answer.name.as_str(), answer.status), ("Internal", 500));
        assert!(answer.violations.is_empty(), "no detail for the caller");
        assert_eq!(answer.hook.len(), 1);
        assert_eq!(answer.hook[0].path, "result[0]");
        assert!(got.body.is_none() && got.etag.is_none() && got.hidden.is_empty());
    }
}

#[test]
fn a_stream_item_is_judged_by_the_same_function() {
    let ir = vectors();
    let r = request(json!({ "id": A, "tenantId": A }), Json::Null);
    let p = pipeline(&ir, "getBooking");
    let item = json!({ "tenantId": B, "title": "b", "open": true, "id": A });
    let found = p.refused(&r, &ty(&ir, "Booking"), &item, "item").unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].path, "item");
}

#[test]
fn a_type_without_a_tenant_is_not_walked() {
    let ir = vectors();
    let r = request(json!({ "id": A }), Json::Null);
    let p = pipeline(&ir, "getBooking");
    // Not even a value that has no shape is looked at: nothing here can be tenant-scoped.
    let found = p
        .refused(&r, &ty(&ir, "Summary"), &json!([1, 2]), "result")
        .unwrap();
    assert!(found.is_empty());
}

// --- the compiler's side ----------------------------------------------------------------------

#[test]
fn a_tenant_field_is_server_set_like_a_readonly_one() {
    let ir = vectors();
    let booking = ir.decls.iter().find(|d| d.name == "Booking").unwrap();
    let f = booking.tenant_field().unwrap();
    assert!(f.server_set() && !f.readonly);
    let title = booking.fields().iter().find(|f| f.name == "title").unwrap();
    assert!(!title.server_set());
}

#[test]
fn a_rule_that_reads_a_tenant_field_is_skipped_in_a_request() {
    let ir = compile_as(
        "t",
        "model Doc { tenantId: uuid @tenant\n n: int\n rule fits: n > 0 || tenantId == tenantId }\n",
    );
    let doc = ir.decls.iter().find(|d| d.name == "Doc").unwrap();
    assert!(doc.rules().iter().all(|r| r.reads_readonly));
}

// --- the envelope -----------------------------------------------------------------------------

fn cases() -> Vec<Json> {
    serde_json::from_str(&read("tests/wire/events/tenant.json")).unwrap()
}

fn channel_id(ir: &Ir, name: &str) -> ChannelId {
    ChannelId(u32::try_from(ir.channels.iter().position(|c| c.name == name).unwrap()).unwrap())
}

fn event_id(ir: &Ir, name: &str) -> DeclId {
    ir.decls
        .iter()
        .find(|d| d.is_event() && d.name == name)
        .unwrap()
        .id
}

fn contract() -> Ir {
    compile_as("bookings", &read("tests/wire/events/tenant.veld"))
}

#[test]
fn the_encoder_gives_every_golden_and_the_tenant_attribute() {
    let ir = contract();
    let all = cases();
    assert_eq!(all.len(), 6);
    for case in all {
        let service = ir
            .services
            .iter()
            .find(|s| s.name == case["service"].as_str().unwrap())
            .unwrap();
        let source = source_of(&ir, service.id);
        let bytes = encode(
            &ir,
            channel_id(&ir, case["channel"].as_str().unwrap()),
            event_id(&ir, case["event"].as_str().unwrap()),
            &case["value"],
            &Inputs {
                id: case["id"].as_str().unwrap(),
                source: &source,
                time_ms: veld_rules::helpers::instant::parse_datetime(
                    case["time"].as_str().unwrap(),
                )
                .unwrap(),
            },
        )
        .unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert_eq!(
            text,
            case["canonical"].as_str().unwrap(),
            "{}",
            case["name"]
        );
        assert_eq!(
            text.contains("\"veldtenant\""),
            case["event"] != "Plain",
            "{}",
            case["name"]
        );
    }
}

#[test]
fn a_consumer_hands_the_handler_the_tenant_and_does_not_check_it() {
    let ir = contract();
    for case in cases() {
        let event = event_id(&ir, case["event"].as_str().unwrap());
        let got = decode(
            &ir,
            &[event],
            case["canonical"].as_str().unwrap().as_bytes(),
        )
        .unwrap();
        let sent: Json = serde_json::from_str(case["canonical"].as_str().unwrap()).unwrap();
        assert_eq!(
            got.veldtenant.as_deref(),
            sent.get("veldtenant").and_then(Json::as_str),
            "{}",
            case["name"]
        );
    }
    // The attribute may say anything: the consumer has no caller to compare it with.
    let first = &cases()[0];
    let mut message: Json = serde_json::from_str(first["canonical"].as_str().unwrap()).unwrap();
    message["veldtenant"] = json!("another");
    let event = event_id(&ir, "BookingPaid");
    let got = decode(&ir, &[event], message.to_string().as_bytes()).unwrap();
    assert_eq!(got.veldtenant.as_deref(), Some("another"));
    message["veldtenant"] = json!(5);
    assert!(decode(&ir, &[event], message.to_string().as_bytes()).is_err());
}
