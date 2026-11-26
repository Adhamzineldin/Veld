//! `@timeout` and `@slo` in the OpenAPI export (`spec/platform.md` sections 12 and 13): the
//! `504` answer, `x-veld-timeout-ms` and `x-veld-slo`, and an export that is unchanged for a
//! contract without them.

use serde_json::{Value, json};
use veld_ir::ServiceId;

use super::openapi;
use crate::testutil::ir;

fn doc(src: &str) -> Value {
    let ir = ir(src);
    let ids: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
    openapi(&ir, &ids)
}

fn contract(annotations: &str) -> String {
    format!(
        "model Room {{ id: uuid, name: string }}\n\
         service S at \"/s\" {{\n  {annotations}\n  get \"/{{id}}\" one(id: uuid) -> Room | NotFound\n  get \"/all\" all() -> Room[]\n}}\n"
    )
}

#[test]
fn a_timeout_adds_the_extension_and_the_504() {
    let d = doc(&contract("@timeout(seconds(2))"));
    let op = &d["paths"]["/s/{id}"]["get"];
    assert_eq!(op["x-veld-timeout-ms"], json!(2000));
    let answer = &op["responses"]["504"]["content"]["application/problem+json"]["schema"];
    assert_eq!(answer["properties"]["title"]["const"], "DeadlineExceeded");
    assert!(answer["properties"]["timeoutMs"].is_object(), "{answer}");
}

#[test]
fn a_service_level_adds_the_extension_with_numbers() {
    let d = doc(&contract("@slo(p99: ms(300), p50: ms(80), errors: 0.1)"));
    let op = &d["paths"]["/s/{id}"]["get"];
    assert_eq!(
        op["x-veld-slo"],
        json!({ "p50_ms": 80, "p99_ms": 300, "errors": 0.1 })
    );
    assert!(op.get("x-veld-timeout-ms").is_none());
    assert!(op["responses"].get("504").is_none());
}

#[test]
fn a_contract_without_them_exports_the_same_bytes() {
    let plain = doc(&contract(""));
    let text = plain.to_string();
    assert!(!text.contains("x-veld-timeout-ms") && !text.contains("x-veld-slo"));
    assert!(!text.contains("DeadlineExceeded"));
    // And an annotated contract differs only in those members.
    let timed = doc(&contract("@timeout(ms(500))"));
    let mut stripped = timed.clone();
    for op in ["/s/{id}", "/s/all"] {
        if let Some(o) = stripped["paths"][op]["get"].as_object_mut() {
            o.remove("x-veld-timeout-ms");
            o["responses"].as_object_mut().map(|r| r.remove("504"));
        }
    }
    let mut baseline = plain.clone();
    if let Some(s) = baseline["components"]["schemas"].as_object_mut() {
        s.retain(|k, _| !k.contains("DeadlineExceeded"));
    }
    if let Some(s) = stripped["components"]["schemas"].as_object_mut() {
        s.retain(|k, _| !k.contains("DeadlineExceeded"));
    }
    assert_eq!(stripped["paths"], baseline["paths"]);
}
