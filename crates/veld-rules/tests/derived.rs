//! Derived fields in the reference evaluator (`spec/rules.md` section 17.2): the pending vector
//! files, and where the derived values are computed and what a failing expression does, member by
//! member, written down from the spec. A generator is held to the same answers by the vectors.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value as Json, json};
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_rules::vectors::{load_cases, run_case, value_at};
use veld_rules::{
    Options, Outcome, OutputMode, Pipeline, Request, check_value, fill_example, run_example,
    with_defaults,
};

fn compile_text(text: &str) -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "t.veld".into(),
                text: text.to_owned(),
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    match out.ir {
        Some(ir) => ir,
        None => panic!("does not compile:\n{}", out.render()),
    }
}

fn vector_text(group: &str, file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/rules")
        .join(group)
        .join(file);
    std::fs::read_to_string(path).unwrap()
}

// --- the pending vector files --------------------------------------------------------------------------

#[test]
fn the_pending_core_vectors_hold_their_twenty_two_cases() {
    let ir = compile_text(&vector_text("core", "derived.veld.pending"));
    let cases = load_cases(&vector_text("core", "derived.json.pending")).unwrap();
    assert_eq!(cases.len(), 22);
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|c| run_case(&ir, c).err().map(|m| m.to_string()))
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // The vectors say something about the computed values: most cases name at least one.
    assert!(cases.iter().filter(|c| !c.computed.is_empty()).count() >= 18);
}

#[test]
fn the_pending_endpoint_contract_holds_its_nine_examples() {
    let ir = compile_text(&vector_text("endpoints", "derived.veld.pending"));
    assert_eq!(ir.examples.len(), 9);
    for example in &ir.examples {
        let report = run_example(&ir, example).unwrap();
        assert!(report.passed(), "{report}");
    }
}

#[test]
fn a_computed_entry_that_is_wrong_fails_the_case() {
    let ir = compile_text(&vector_text("core", "derived.veld.pending"));
    let text = vector_text("core", "derived.json.pending").replace("\"$.c\": 7", "\"$.c\": 8");
    let cases = load_cases(&text).unwrap();
    let failed = cases.iter().filter(|c| run_case(&ir, c).is_err()).count();
    assert_eq!(failed, 1);
}

#[test]
fn value_at_reads_the_path_syntax_of_a_violation() {
    let v = json!({ "a": [ { "b": 1 }, { "b": 2 } ], "m": { "k": { "z": true } } });
    assert_eq!(value_at(&v, "$"), Some(&v));
    assert_eq!(value_at(&v, "$.a[1].b"), Some(&json!(2)));
    assert_eq!(value_at(&v, "$.m.k.z"), Some(&json!(true)));
    assert_eq!(value_at(&v, "$.a[2]"), None);
    assert_eq!(value_at(&v, "a"), None);
}

// --- a contract of our own ----------------------------------------------------------------------------

const SOURCE: &str = r#"
model Money {
  amount: int
  cents: int = amount * 100
}

model Wallet {
  id: uuid
  amount: int
  per: int
  share: int = amount / per
}

model User {
  id: uuid
  rank: int
  level: int = rank * 10
}

policy Seniors(user: User) {
  permit when user.level >= 30
}

principal User

model Receipt {
  amount: int
  fee: int
  net: int = amount - fee
  ratio: int = amount / fee
  rule hasNet: net >= 0
}

service Shop at "/s" {
  @auth(bearer)
  post "/w/{id}" spend(id: uuid, body: Money) -> Receipt {
    fact wallet: Wallet = id
    require body.cents <= wallet.amount * 100
    require wallet.share >= 1
  }
  @auth(bearer)
  get "/vip" vip() -> Money {
    allow Seniors(user)
  }
  get "/r" receipt() -> Receipt
}
"#;

const ID: &str = "00000000-0000-4000-8000-000000000001";

fn pipeline<'a>(ir: &'a Ir, endpoint: &str) -> Pipeline<'a> {
    Pipeline::new(ir, &ir.services[0], endpoint).unwrap()
}

fn spend(amount: i64, wallet: Json) -> Request {
    let mut r = Request::default();
    r.params.insert("id".into(), json!(ID));
    r.params.insert("body".into(), json!({ "amount": amount }));
    r.facts.insert("wallet".into(), Some(wallet));
    r
}

fn error_name(o: &Outcome) -> Option<&str> {
    match o {
        Outcome::Error(a) => Some(a.name.as_str()),
        Outcome::Passes => None,
    }
}

#[test]
fn a_derived_value_decides_a_require() {
    let ir = compile_text(SOURCE);
    let p = pipeline(&ir, "spend");
    let wallet = json!({ "id": ID, "amount": 10, "per": 2 });
    assert_eq!(p.run(&spend(10, wallet.clone())).unwrap(), Outcome::Passes);
    // 11 * 100 > 10 * 100: the require reads `body.cents`, which the server computed.
    let out = p.run(&spend(11, wallet)).unwrap();
    assert_eq!(error_name(&out), Some("ValidationError"));
}

#[test]
fn a_fact_is_given_its_derived_fields_after_it_is_loaded() {
    let ir = compile_text(SOURCE);
    let p = pipeline(&ir, "spend");
    // `share` is not in the value the loader gave; the server computes it, and a `require` reads it.
    let enough = json!({ "id": ID, "amount": 10, "per": 5 });
    assert_eq!(p.run(&spend(1, enough)).unwrap(), Outcome::Passes);
    let short = json!({ "id": ID, "amount": 4, "per": 5 });
    assert_eq!(
        error_name(&p.run(&spend(1, short)).unwrap()),
        Some("ValidationError")
    );
    // What the loader put there is not believed.
    let lying = json!({ "id": ID, "amount": 4, "per": 5, "share": 99 });
    assert_eq!(
        error_name(&p.run(&spend(1, lying)).unwrap()),
        Some("ValidationError")
    );
}

#[test]
fn a_fact_whose_derived_field_fails_is_the_applications_mistake_500() {
    let ir = compile_text(SOURCE);
    let p = pipeline(&ir, "spend");
    let wallet = json!({ "id": ID, "amount": 10, "per": 0 });
    let r = p
        .run_reported(&spend(1, wallet), OutputMode::Strict)
        .unwrap();
    let Outcome::Error(a) = &r.outcome else {
        panic!("expected an error")
    };
    assert_eq!((a.name.as_str(), a.status), ("Internal", 500));
    assert_eq!(a.hook.len(), 1);
    assert_eq!(a.hook[0].rule_name, "share");
    assert!(a.hook[0].rule_id.starts_with("veld:t/Wallet.share@"));
    assert_eq!(a.hook[0].error, Some(veld_rules::EvalError::DivisionByZero));
    assert_eq!(a.hook[0].fields, ["amount", "per"]);
}

#[test]
fn the_principal_is_given_its_derived_fields_before_a_policy_reads_it() {
    let ir = compile_text(SOURCE);
    let p = pipeline(&ir, "vip");
    let caller = |rank: i64| Request {
        principal: Some(json!({ "id": ID, "rank": rank })),
        ..Request::default()
    };
    assert_eq!(p.run(&caller(3)).unwrap(), Outcome::Passes);
    assert_eq!(error_name(&p.run(&caller(2)).unwrap()), Some("Forbidden"));
}

fn receipt(amount: i64, fee: i64) -> Request {
    Request {
        result: Some(json!({ "amount": amount, "fee": fee, "net": 123456 })),
        ..Request::default()
    }
}

#[test]
fn the_result_is_given_its_derived_fields_and_the_handlers_value_is_overwritten() {
    let ir = compile_text(SOURCE);
    let p = pipeline(&ir, "receipt");
    let r = p
        .run_reported(&receipt(100, 30), OutputMode::Strict)
        .unwrap();
    assert_eq!(r.outcome, Outcome::Passes);
    let body = r.body.unwrap();
    assert_eq!(body["net"], json!(70));
    assert_eq!(body["ratio"], json!(3));
}

#[test]
fn an_output_rule_reads_the_derived_field_and_strict_refuses() {
    let ir = compile_text(SOURCE);
    let p = pipeline(&ir, "receipt");
    let r = p
        .run_reported(&receipt(10, 30), OutputMode::Strict)
        .unwrap();
    let Outcome::Error(a) = &r.outcome else {
        panic!("expected an error")
    };
    assert_eq!(a.name, "Internal");
    assert_eq!(a.hook.len(), 1);
    assert_eq!(a.hook[0].rule_name, "hasNet");
}

#[test]
fn a_failing_expression_in_a_result_is_a_violation_of_the_output_mode() {
    let ir = compile_text(SOURCE);
    let p = pipeline(&ir, "receipt");
    // fee 0: `ratio` divides by zero. Strict: 500 with one reported violation, named by the field.
    let strict = p.run_reported(&receipt(10, 0), OutputMode::Strict).unwrap();
    let Outcome::Error(a) = &strict.outcome else {
        panic!("expected an error")
    };
    assert_eq!(a.name, "Internal");
    assert_eq!(a.hook.len(), 1, "{:?}", a.hook);
    assert_eq!(a.hook[0].rule_name, "ratio");
    assert_eq!(a.hook[0].path, "result");
    assert_eq!(a.hook[0].fields, ["amount", "fee"]);
    // Audit: the response is sent, the field is absent, the violation is reported.
    let audit = p.run_reported(&receipt(10, 0), OutputMode::Audit).unwrap();
    assert_eq!(audit.outcome, Outcome::Passes);
    assert_eq!(audit.hook.len(), 1);
    let body = audit.body.unwrap();
    assert!(body.get("ratio").is_none());
    assert_eq!(body["net"], json!(10));
}

#[test]
fn the_output_mode_off_still_computes_the_derived_fields_and_reports_nothing() {
    let ir = compile_text(SOURCE);
    let p = pipeline(&ir, "receipt");
    let off = p.run_reported(&receipt(10, 0), OutputMode::Off).unwrap();
    assert_eq!(off.outcome, Outcome::Passes);
    assert!(off.hook.is_empty() && off.audited.is_empty());
    let body = off.body.unwrap();
    assert_eq!(body["net"], json!(10));
    assert!(body.get("ratio").is_none());
}

#[test]
fn a_value_the_caller_sent_for_a_derived_field_is_ignored_even_when_malformed() {
    let ir = compile_text(SOURCE);
    let money = ir.decls.iter().find(|d| d.name == "Money").unwrap();
    let sent = json!({ "amount": 2, "cents": "not a number" });
    let full = with_defaults(&ir, money, &sent, Options::default()).unwrap();
    assert_eq!(full, json!({ "amount": 2, "cents": 200 }));
    assert!(
        check_value(
            &ir,
            money,
            &sent,
            Options {
                request: true,
                partial: false
            }
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn a_value_that_is_not_derived_still_has_to_be_there() {
    let ir = compile_text(SOURCE);
    let money = ir.decls.iter().find(|d| d.name == "Money").unwrap();
    let err = with_defaults(&ir, money, &json!({}), Options::default()).unwrap_err();
    assert!(err.to_string().contains("amount"), "{err}");
}

// --- examples ---------------------------------------------------------------------------------------------

#[test]
fn an_example_never_fills_in_a_derived_field_and_the_fact_is_computed() {
    let ir = compile_text(&vector_text("endpoints", "derived.veld.pending"));
    let ex = ir
        .examples
        .iter()
        .find(|e| e.name == "a fact is given its derived field")
        .unwrap();
    let filled = fill_example(&ir, ex).unwrap();
    let wallet = filled.request.facts["wallet"].clone().unwrap();
    assert!(wallet.get("available").is_none(), "{wallet}");
    let body = &filled.request.params["body"];
    assert!(
        body.get("subtotal").is_none() && body.get("payable").is_none(),
        "{body}"
    );
    assert!(body["items"][0].get("total").is_none());
}
