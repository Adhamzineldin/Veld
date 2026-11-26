//! Sagas in the OpenAPI export (`spec/platform.md` sections 17 and 18): `x-veld-saga`, the
//! implicit errors, the required idempotency header, and an export that is unchanged for a
//! contract without a saga.

use serde_json::Value;
use veld_ir::ServiceId;

use super::openapi;
use crate::testutil::ir;

const BASE: &str = "model Req { id: uuid }\nmodel Pay { id: uuid }\n\
service Pays at \"/pays\" {\n  @idempotent\n  post \"/charge\" charge(body: Req) -> Pay\n}\n";

const SAGA: &str = "service Shop at \"/shop\" {\n  saga post \"/\" buy(body: Req) -> Pay {\n    step pay = Pays.charge(body: body)\n    timeout minutes(1)\n  }\n}\n";

fn doc(src: &str) -> Value {
    let ir = ir(src);
    let ids: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
    openapi(&ir, &ids)
}

#[test]
fn a_saga_operation_is_marked_and_lists_its_implicit_errors() {
    let d = doc(&format!("{BASE}{SAGA}"));
    let op = &d["paths"]["/shop"]["post"];
    assert_eq!(op["x-veld-saga"], true);
    for status in ["502", "409", "422", "500"] {
        assert!(op["responses"].get(status).is_some(), "{status} in {op}");
    }
    assert!(
        op["responses"]["502"]["headers"]
            .get("Veld-Idempotent-Replay")
            .is_some(),
        "a saga keeps SagaFailed too: {op}"
    );
}

#[test]
fn the_idempotency_header_of_a_saga_is_required() {
    let d = doc(&format!("{BASE}{SAGA}"));
    let params = d["paths"]["/shop"]["post"]["parameters"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let key = params
        .iter()
        .find(|p| p["in"] == "header" && p["name"] == "Idempotency-Key");
    assert_eq!(key.map(|k| k["required"].clone()), Some(Value::Bool(true)));
    assert_eq!(
        d["paths"]["/shop"]["post"]["x-veld-idempotent"]["required"],
        true
    );
}

#[test]
fn a_contract_without_a_saga_exports_the_same_bytes() {
    let text = doc(BASE).to_string();
    assert!(!text.contains("x-veld-saga"));
    assert!(!text.contains("SagaFailed"));
    // The operation the saga calls is not marked.
    let with = doc(&format!("{BASE}{SAGA}"));
    assert!(
        with["paths"]["/pays/charge"]["post"]
            .get("x-veld-saga")
            .is_none()
    );
}
