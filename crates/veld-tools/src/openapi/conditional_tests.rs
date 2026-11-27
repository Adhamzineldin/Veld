//! `@version` and `@idempotent` in the OpenAPI export (`spec/rules.md` sections 11.7 to 11.9): the
//! headers, the `ETag`, the `412` and `409` answers, and an export that is unchanged for a
//! contract without the annotations.

use serde_json::{Value, json};
use veld_ir::ServiceId;

use super::openapi;
use crate::testutil::ir;

fn doc(src: &str) -> Value {
    let ir = ir(src);
    let ids: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
    openapi(&ir, &ids)
}

fn contract(version: &str, idempotent: &str) -> String {
    format!(
        "model Room {{\n  id: uuid\n  {version}\n  revision: long\n}}\n\
         model Note {{ text: string }}\n\
         service S at \"/s\" {{\n\
         \x20 get \"/{{id}}\" one(id: uuid) -> Room | NotFound\n\
         \x20 {idempotent}\n\
         \x20 put \"/{{id}}\" save(id: uuid, body: Note) -> Room | NotFound {{\n    fact room: Room = id\n  }}\n}}\n"
    )
}

fn header<'a>(op: &'a Value, name: &str) -> Option<&'a Value> {
    op["parameters"]
        .as_array()?
        .iter()
        .find(|p| p["in"] == "header" && p["name"] == name)
}

#[test]
fn a_guard_adds_the_if_match_header_and_the_412() {
    let d = doc(&contract("@version", ""));
    let put = &d["paths"]["/s/{id}"]["put"];
    let h = header(put, "If-Match").unwrap();
    assert_eq!(h["required"], false);
    assert_eq!(h["schema"]["type"], "string");
    assert!(
        put["responses"].get("412").is_some(),
        "{}",
        put["responses"]
    );
    let precondition = &put["responses"]["412"]["content"]["application/problem+json"]["schema"];
    assert_eq!(
        precondition["properties"]["title"]["const"],
        "PreconditionFailed"
    );
    assert_eq!(put["x-veld-version"]["fact"], "room");
    assert_eq!(put["x-veld-version"]["field"], "revision");
    assert!(
        put["x-veld-version"]["id"]
            .as_str()
            .unwrap()
            .contains(".version@")
    );
}

#[test]
fn a_response_that_is_a_versioned_model_carries_an_etag() {
    let d = doc(&contract("@version", ""));
    for (path, method) in [("/s/{id}", "get"), ("/s/{id}", "put")] {
        let ok = &d["paths"][path][method]["responses"]["200"];
        assert_eq!(
            ok["headers"]["ETag"]["schema"]["type"], "string",
            "{method}"
        );
    }
    // Only the endpoint with a guard has the request header: the plain `get` has none.
    assert!(header(&d["paths"]["/s/{id}"]["get"], "If-Match").is_none());
    assert_eq!(
        d["components"]["schemas"]["app.Room"]["x-veld-version"],
        "revision"
    );
}

#[test]
fn idempotent_adds_its_header_the_replay_header_and_the_409() {
    let d = doc(&contract(
        "",
        "@idempotent(required: true, header: \"X-Request-Id\")",
    ));
    let put = &d["paths"]["/s/{id}"]["put"];
    let h = header(put, "X-Request-Id").unwrap();
    assert_eq!(h["required"], true);
    assert_eq!(h["schema"]["maxLength"], 255);
    assert!(
        put["responses"].get("409").is_some(),
        "{}",
        put["responses"]
    );
    assert_eq!(
        put["responses"]["200"]["headers"]["Veld-Idempotent-Replay"]["schema"]["type"],
        "string"
    );
    assert!(
        put["responses"]["404"]["headers"]
            .get("Veld-Idempotent-Replay")
            .is_some()
    );
    // A 5xx answer is never stored, so it is never a replay.
    assert!(put["responses"]["500"].get("headers").is_none());
    assert_eq!(
        put["x-veld-idempotent"],
        json!({ "header": "X-Request-Id", "required": true, "caller": false })
    );
}

#[test]
fn the_default_header_is_optional_and_a_plain_endpoint_has_neither() {
    let d = doc(&contract("", "@idempotent"));
    let put = &d["paths"]["/s/{id}"]["put"];
    assert_eq!(header(put, "Idempotency-Key").unwrap()["required"], false);
    let get = &d["paths"]["/s/{id}"]["get"];
    assert!(header(get, "Idempotency-Key").is_none());
    assert!(get.get("x-veld-idempotent").is_none());
}

#[test]
fn a_contract_without_the_annotations_exports_without_a_trace_of_them() {
    let text = doc(&contract("", "")).to_string();
    for word in [
        "If-Match",
        "ETag",
        "Idempotency",
        "Replay",
        "x-veld-version",
        "x-veld-idempotent",
        "412",
    ] {
        assert!(!text.contains(word), "{word} in {text}");
    }
}

#[test]
fn removing_what_the_annotations_add_gives_the_export_without_them() {
    let mut with = doc(&contract("@version", "@idempotent"));
    let without = doc(&contract("", ""));
    strip(&mut with);
    assert_eq!(first_difference(&with, &without, "$"), None);
}

/// Removes every key and entry that the two annotations add.
fn strip(v: &mut Value) {
    match v {
        Value::Object(m) => {
            for key in [
                "x-veld-version",
                "x-veld-idempotent",
                "headers",
                "412",
                "409",
                "veld.Conflict",
                "veld.PreconditionFailed",
            ] {
                m.remove(key);
            }
            if let Some(Value::Array(params)) = m.get_mut("parameters") {
                params.retain(|p| p["name"] != "If-Match" && p["name"] != "Idempotency-Key");
            }
            m.values_mut().for_each(strip);
        }
        Value::Array(items) => items.iter_mut().for_each(strip),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

/// Where two documents first differ, as a path, so a failure says where and not the whole document.
fn first_difference(a: &Value, b: &Value, at: &str) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let keys: std::collections::BTreeSet<&String> = x.keys().chain(y.keys()).collect();
            keys.into_iter().find_map(|k| match (x.get(k), y.get(k)) {
                (Some(l), Some(r)) => first_difference(l, r, &format!("{at}.{k}")),
                (l, r) => Some(format!("{at}.{k}: {} against {}", l.is_some(), r.is_some())),
            })
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => x
            .iter()
            .zip(y)
            .enumerate()
            .find_map(|(i, (l, r))| first_difference(l, r, &format!("{at}[{i}]"))),
        _ if a == b => None,
        _ => Some(at.to_owned()),
    }
}
