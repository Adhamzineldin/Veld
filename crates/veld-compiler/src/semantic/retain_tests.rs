//! `@retain` (`spec/platform.md` section 23): the checks (`E1310`), what lowers to the IR, and the
//! plan every generator embeds.

use crate::testing::{codes, ir as build, one};
use veld_ir::retention::RetainThen;
use veld_ir::{Feature, Ir, ServiceId};

const GUEST: &str = "@retain(days(30), since: checkedOutAt, then: anonymize)
model Guest {
  id: uuid @readonly
  name: string @pii
  phone?: string @pii
  password: string @secret
  checkedOutAt?: datetime
}
";

fn with_service(models: &str) -> String {
    format!(
        "{models}\n@auth(bearer)\nservice Guests at \"/guests\" {{\n  get \"/\" list() -> Guest\n}}\n"
    )
}

fn model(ir: &Ir, name: &str) -> veld_ir::Model {
    match ir.decls.iter().find(|d| d.name == name).map(|d| &d.kind) {
        Some(veld_ir::DeclKind::Model(m)) => m.clone(),
        _ => panic!("no model {name}"),
    }
}

#[test]
fn a_retained_model_keeps_its_policy_in_the_ir() {
    let ir = build(&with_service(GUEST));
    let r = model(&ir, "Guest").retain.unwrap();
    assert_eq!(r.after_ms, 30 * 86_400_000);
    assert_eq!(r.since, "checkedOutAt");
    assert_eq!(r.then, RetainThen::Anonymize);
    let json = serde_json::to_value(&ir).unwrap();
    let guest = json["decls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["name"] == "Guest")
        .unwrap();
    assert_eq!(
        guest["retain"],
        serde_json::json!({"after_ms": "2592000000", "since": "checkedOutAt", "then": "anonymize"})
    );
}

#[test]
fn since_is_the_wire_name_of_the_field() {
    let src = GUEST.replace(
        "checkedOutAt?: datetime",
        "checkedOutAt?: datetime @wire(\"checked_out_at\")",
    );
    let ir = build(&with_service(&src));
    assert_eq!(model(&ir, "Guest").retain.unwrap().since, "checked_out_at");
}

#[test]
fn a_contract_without_it_has_no_retain_key_and_the_same_signature() {
    let plain = GUEST.replace(
        "@retain(days(30), since: checkedOutAt, then: anonymize)\n",
        "",
    );
    let (a, b) = (build(&with_service(GUEST)), build(&with_service(&plain)));
    assert!(!serde_json::to_string(&b).unwrap().contains("retain"));
    let sig = |ir: &Ir| {
        ir.decls
            .iter()
            .find(|d| d.name == "Guest")
            .unwrap()
            .sig
            .clone()
    };
    assert_eq!(sig(&a), sig(&b));
}

#[test]
fn delete_needs_no_pii_field_and_a_date_will_do() {
    let src =
        "@retain(days(1), since: day, then: delete)\nmodel Log {\n  id: uuid\n  day: date\n}\n";
    assert_eq!(codes(src), Vec::<&str>::new());
}

#[test]
fn every_mistake_of_the_arguments_is_e1310() {
    let model = |args: &str| {
        format!("@retain({args})\nmodel M {{\n  id: uuid\n  at: datetime\n  who: string @pii\n}}\n")
    };
    for bad in [
        "",
        "days(30)",
        "days(30), since: at",
        "since: at, then: delete",
        "days(30), then: delete",
        "hours(24), since: at, then: delete",
        "weeks(1), since: at, then: delete",
        "days(0), since: at, then: delete",
        "days(-3), since: at, then: delete",
        "days(1.5), since: at, then: delete",
        "days(\"3\"), since: at, then: delete",
        "days(1, 2), since: at, then: delete",
        "days(), since: at, then: delete",
        "30, since: at, then: delete",
        "days(30), since: at, then: archive",
        "days(30), since: at, then: \"delete\"",
        "days(30), since: \"at\", then: delete",
        "days(30), since: at, then: delete, extra: 1",
        "days(30), since: at, since: at, then: delete",
        "days(30), days(40), since: at, then: delete",
        "days(9999999999999999), since: at, then: delete",
    ] {
        assert_eq!(codes(&model(bad)), ["E1310"], "@retain({bad})");
    }
    assert_eq!(
        codes(&model("days(30), since: at, then: delete")),
        Vec::<&str>::new()
    );
}

#[test]
fn the_since_field_must_exist_and_be_a_datetime_or_a_date() {
    let model = |field: &str| {
        format!(
            "@retain(days(7), since: {field}, then: delete)\nmodel M {{\n  id: uuid\n  at: datetime\n  n: int\n}}\n"
        )
    };
    assert_eq!(codes(&model("at")), Vec::<&str>::new());
    assert_eq!(codes(&model("missing")), ["E1310"]);
    assert_eq!(codes(&model("n")), ["E1310"]);
    assert_eq!(codes(&model("id")), ["E1310"]);
}

#[test]
fn an_inherited_field_can_be_the_since_field_and_an_inherited_pii_counts() {
    let src = "model Base {\n  at: datetime\n  who: string @pii\n}\n\
               @retain(days(7), since: at, then: anonymize)\nmodel Kid extends Base {\n  n: int\n}\n";
    assert_eq!(codes(src), Vec::<&str>::new());
    let ir = build(src);
    assert_eq!(model(&ir, "Kid").retain.unwrap().since, "at");
}

#[test]
fn anonymize_needs_a_pii_field_and_secret_is_not_enough() {
    let src = "@retain(days(7), since: at, then: anonymize)\nmodel M {\n  at: datetime\n  key: string @secret\n}\n";
    assert_eq!(codes(src), ["E1310"]);
}

#[test]
fn it_stands_on_a_model_only() {
    for src in [
        "@retain(days(1), since: at, then: delete)\nerror E 400 { at: datetime }\n",
        "@retain(days(1), since: at, then: delete)\nenum S { a, b }\n",
        "model M {\n  at: datetime @retain(days(1), since: at, then: delete)\n}\n",
    ] {
        assert_eq!(codes(src), ["E0602"], "{src}");
    }
}

#[test]
fn the_word_stays_an_ordinary_name() {
    assert!(
        one("model A { retain: string\n since: string\n then: string\n days: int }\n")
            .ir
            .is_some()
    );
}

// ---- the plan ----------------------------------------------------------------------------------

#[test]
fn the_plan_lists_the_models_by_name_with_the_pii_fields_for_anonymize() {
    let src = with_service(&format!(
        "{GUEST}@retain(days(2), since: closedAt, then: delete)\nmodel Booking {{\n  id: uuid\n  closedAt: datetime\n  note: string @pii\n}}\nmodel Plain {{\n  id: uuid\n}}\n"
    ));
    let ir = build(&src);
    let plan = ir.retention_plan(&[ServiceId(0)]);
    let got: Vec<_> = plan
        .iter()
        .map(|e| {
            (
                e.model.as_str(),
                e.after_ms,
                e.since.as_str(),
                e.then,
                e.fields.clone(),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            (
                "app:app.Booking",
                2 * 86_400_000,
                "closedAt",
                RetainThen::Delete,
                vec![]
            ),
            (
                "app:app.Guest",
                30 * 86_400_000,
                "checkedOutAt",
                RetainThen::Anonymize,
                vec!["name".to_string(), "phone".to_string()]
            ),
        ]
    );
}

#[test]
fn a_member_without_retained_models_has_an_empty_plan_and_does_not_use_the_feature() {
    let ir = build("model A { x: string @pii }\nservice S at \"/s\" { get \"/\" f() -> A }\n");
    assert!(ir.retention_plan(&[ServiceId(0)]).is_empty());
    assert!(
        !ir.features_used(&[ServiceId(0)])
            .contains(&Feature::Retention)
    );
}

#[test]
fn a_member_with_a_retained_model_uses_the_feature() {
    let ir = build(&with_service(GUEST));
    assert!(
        ir.features_used(&[ServiceId(0)])
            .contains(&Feature::Retention)
    );
}
