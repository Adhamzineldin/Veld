//! `@rollout(audit)` in the OpenAPI export (`spec/rules.md` section 17.1): `"rollout": "audit"`
//! on the entry of an audited rule, and nothing on an enforced one, so an export of a contract
//! without the annotation keeps its bytes.

use serde_json::{Value, json};
use veld_ir::ServiceId;

use super::openapi;
use crate::testutil::ir;

fn doc(src: &str) -> Value {
    let ir = ir(src);
    let ids: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
    openapi(&ir, &ids)
}

fn contract(rollout: &str) -> String {
    format!(
        "model M {{\n a: int\n {rollout}\n rule small: a <= 6\n rule positive: a > 0\n}}\n\
         service S at \"/s\" {{ post \"/\" make(body: M) -> 201 M {{\n {rollout}\n require body.a > 1\n require body.a > 2\n {rollout}\n ensure result.a > 3\n ensure result.a > 4\n}} }}\n"
    )
}

#[test]
fn an_audited_rule_carries_rollout_audit_and_an_enforced_one_does_not() {
    let d = doc(&contract("@rollout(audit)"));
    let rules = &d["components"]["schemas"]["app.M"]["x-veld-rules"];
    assert_eq!(rules[0]["name"], "small");
    assert_eq!(rules[0]["rollout"], json!("audit"));
    assert_eq!(rules[1]["name"], "positive");
    assert!(rules[1].get("rollout").is_none(), "{rules}");
    let op = &d["paths"]["/s"]["post"]["x-veld-rules"];
    assert_eq!(op["requires"][0]["rollout"], json!("audit"));
    assert!(op["requires"][1].get("rollout").is_none(), "{op}");
    assert_eq!(op["ensures"][0]["rollout"], json!("audit"));
    assert!(op["ensures"][1].get("rollout").is_none(), "{op}");
}

#[test]
fn an_export_without_the_annotation_has_no_rollout_key_at_all() {
    let text = doc(&contract("")).to_string();
    assert!(!text.contains("rollout"), "{text}");
}

#[test]
fn removing_the_rollout_keys_gives_the_export_without_the_annotation() {
    let mut audited = doc(&contract("@rollout(audit)"));
    let plain = doc(&contract(""));
    strip(&mut audited);
    assert_eq!(audited, plain);
}

fn strip(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.remove("rollout");
            m.values_mut().for_each(strip);
        }
        Value::Array(a) => a.iter_mut().for_each(strip),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}
