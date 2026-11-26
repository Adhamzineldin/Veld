//! Derived fields in the OpenAPI export (`spec/rules.md` section 17.2): `readOnly` and
//! `x-veld-derived` on the property, and the same bytes for a contract without derived fields.

use serde_json::{Value, json};
use veld_ir::ServiceId;

use super::openapi;
use crate::testutil::ir;

fn doc(src: &str) -> Value {
    let ir = ir(src);
    let ids: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
    openapi(&ir, &ids)
}

const SOURCE: &str = "model Line { price: decimal, qty: int, total: decimal = price * qty }\n\
model Order {\n  items: Line[]\n  discount: decimal = 0\n  subtotal: decimal = sum(items, i => i.total)\n  total: decimal = subtotal - discount\n}\n\
service S at \"/s\" { post \"/\" make(body: Order) -> 201 Order }\n";

#[test]
fn a_derived_field_is_read_only_and_carries_its_expression() {
    let d = doc(SOURCE);
    let order = &d["components"]["schemas"]["app.Order"]["properties"];
    assert_eq!(order["total"]["readOnly"], json!(true));
    assert_eq!(
        order["total"]["x-veld-derived"],
        json!("subtotal - discount")
    );
    assert_eq!(
        order["subtotal"]["x-veld-derived"],
        json!("sum(items, i => i.total)")
    );
    let line = &d["components"]["schemas"]["app.Line"]["properties"];
    assert_eq!(line["total"]["x-veld-derived"], json!("price * qty"));
}

#[test]
fn a_field_that_is_not_derived_has_neither_key() {
    let d = doc(SOURCE);
    let order = &d["components"]["schemas"]["app.Order"]["properties"];
    assert!(order["discount"].get("readOnly").is_none(), "{order}");
    assert!(order["discount"].get("x-veld-derived").is_none(), "{order}");
    assert_eq!(order["discount"]["default"], json!("0"));
}

#[test]
fn a_derived_field_stays_in_required_for_responses_like_a_readonly_one() {
    let d = doc(SOURCE);
    let required = &d["components"]["schemas"]["app.Order"]["required"];
    assert!(
        required.as_array().unwrap().contains(&json!("total")),
        "{required}"
    );
}

#[test]
fn a_derived_field_is_not_a_query_parameter() {
    let d = doc("model Q { limit: int, double: int = limit * 2 }\n\
         service S at \"/s\" { get \"/\" f(@query q: Q) -> 200 }\n");
    let params = d["paths"]["/s"]["get"]["parameters"].as_array().unwrap();
    let names: Vec<&str> = params.iter().filter_map(|p| p["name"].as_str()).collect();
    assert_eq!(names, ["limit"]);
}

#[test]
fn a_contract_without_derived_fields_exports_the_same_bytes() {
    let text =
        doc("model M { a: int = 1, b?: string }\nservice S at \"/s\" { get \"/\" f() -> M }")
            .to_string();
    assert!(!text.contains("x-veld-derived"), "{text}");
    assert!(!text.contains("readOnly"), "{text}");
}
